//! `cargo run -- task tinker`: an interactive SQL console (the nearest thing to `php artisan tinker`
//! in a compiled language). Type SQL ending with `;`, or a dot command (`.help`). Changes can be
//! wrapped in `.begin` ... `.commit` / `.rollback`. In production it is read-only unless started
//! with `write:true`.

use loco_rs::{environment::Environment, hash, prelude::*};
use rustyline::{error::ReadlineError, DefaultEditor};
use sea_orm::{
    ConnectionTrait, DatabaseTransaction, FromQueryResult, JsonValue, QueryResult, Statement,
    TransactionTrait,
};

const MAX_ROWS_SHOWN: usize = 200;
const MAX_CELL_WIDTH: usize = 60;

const HELP: &str = "\
SQL ends with ;  (it may span several lines)
  select * from users limit 5;
  update users set name = 'Ada' where id = 1;
Dot commands:
  .tables            list tables
  .desc <table>      show a table's columns
  .count <table>     number of rows
  .hash <password>   print a password hash you can store in users.password
  .begin             start a transaction (changes stay private until .commit)
  .commit            save the changes of the transaction
  .rollback          throw them away
  .help              this text
  .exit              leave (an open transaction is rolled back)
UPDATE or DELETE without WHERE asks for confirmation.";

/// What the console shows after one input.
#[derive(Debug, PartialEq, Eq)]
pub enum Reply {
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        total: usize,
    },
    Message(String),
    /// Ask "run it anyway?" before doing something that touches every row.
    Confirm(String),
    Exit,
}

/// One console session: the connection to use and the open transaction, if any.
pub struct Session {
    transaction: Option<DatabaseTransaction>,
    read_only: bool,
}

fn first_word(sql: &str) -> String {
    sql.split_whitespace()
        .next()
        .unwrap_or_default()
        .to_lowercase()
}

pub fn is_read(sql: &str) -> bool {
    matches!(
        first_word(sql).as_str(),
        "select" | "show" | "describe" | "desc" | "explain" | "with"
    )
}

/// `UPDATE`/`DELETE` with no `WHERE` anywhere would change every row.
pub fn touches_every_row(sql: &str) -> bool {
    let lowered = sql.to_lowercase();
    matches!(first_word(&lowered).as_str(), "update" | "delete")
        && !lowered.split_whitespace().any(|word| word == "where")
}

fn valid_table_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn show(value: &JsonValue) -> String {
    let text = match value {
        JsonValue::Null => "NULL".to_string(),
        JsonValue::String(text) => text.clone(),
        other => other.to_string(),
    };
    let flat = text.replace(['\n', '\r'], " ");
    if flat.chars().count() > MAX_CELL_WIDTH {
        let cut: String = flat.chars().take(MAX_CELL_WIDTH - 1).collect();
        format!("{cut}…")
    } else {
        flat
    }
}

fn table(rows: &[QueryResult]) -> Result<Reply> {
    let columns = rows
        .first()
        .map(QueryResult::column_names)
        .unwrap_or_default();
    let mut shown = Vec::new();
    for row in rows.iter().take(MAX_ROWS_SHOWN) {
        let json = JsonValue::from_query_result(row, "")
            .map_err(|error| Error::string(&error.to_string()))?;
        shown.push(
            columns
                .iter()
                .map(|column| json.get(column).map_or_else(String::new, show))
                .collect(),
        );
    }
    Ok(Reply::Table {
        columns,
        rows: shown,
        total: rows.len(),
    })
}

/// Aligned text for a [`Reply::Table`].
pub fn render_table(columns: &[String], rows: &[Vec<String>], total: usize) -> String {
    if columns.is_empty() {
        return "(no rows)".to_string();
    }
    let widths: Vec<usize> = columns
        .iter()
        .enumerate()
        .map(|(index, name)| {
            rows.iter()
                .map(|row| row[index].chars().count())
                .chain([name.chars().count()])
                .max()
                .unwrap_or(0)
        })
        .collect();
    let line = |cells: &[String]| {
        cells
            .iter()
            .zip(&widths)
            .map(|(cell, width)| format!("{cell:<width$}"))
            .collect::<Vec<_>>()
            .join(" | ")
    };
    let mut out = vec![
        line(columns),
        widths
            .iter()
            .map(|width| "-".repeat(*width))
            .collect::<Vec<_>>()
            .join("-+-"),
    ];
    out.extend(rows.iter().map(|row| line(row)));
    out.push(if total > rows.len() {
        format!("({total} rows, first {} shown)", rows.len())
    } else {
        format!("({total} row{})", if total == 1 { "" } else { "s" })
    });
    out.join("\n")
}

impl Session {
    pub fn new(read_only: bool) -> Self {
        Self {
            transaction: None,
            read_only,
        }
    }

    pub fn in_transaction(&self) -> bool {
        self.transaction.is_some()
    }

    async fn query(&self, ctx: &AppContext, sql: &str) -> Result<Vec<QueryResult>> {
        let statement = Statement::from_string(ctx.db.get_database_backend(), sql.to_string());
        Ok(match &self.transaction {
            Some(transaction) => transaction.query_all_raw(statement).await?,
            None => ctx.db.query_all_raw(statement).await?,
        })
    }

    async fn execute(&self, ctx: &AppContext, sql: &str) -> Result<u64> {
        let statement = Statement::from_string(ctx.db.get_database_backend(), sql.to_string());
        let result = match &self.transaction {
            Some(transaction) => transaction.execute_raw(statement).await?,
            None => ctx.db.execute_raw(statement).await?,
        };
        Ok(result.rows_affected())
    }

    /// Rolls back an open transaction; used when leaving.
    pub async fn finish(&mut self) -> Option<String> {
        let transaction = self.transaction.take()?;
        let _ = transaction.rollback().await;
        Some("open transaction rolled back".to_string())
    }

    /// Runs one complete input (a dot command or one SQL statement). `confirmed` answers a
    /// previous [`Reply::Confirm`].
    pub async fn handle(
        &mut self,
        ctx: &AppContext,
        input: &str,
        confirmed: bool,
    ) -> Result<Reply> {
        let input = input.trim().trim_end_matches(';').trim();
        if input.is_empty() {
            return Ok(Reply::Message(String::new()));
        }
        if let Some(command) = input.strip_prefix('.') {
            return self.dot_command(ctx, command.trim()).await;
        }

        if is_read(input) {
            return table(&self.query(ctx, input).await?);
        }
        if self.read_only {
            return Err(Error::string(
                "read-only in production; start with `write:true` to change data",
            ));
        }
        if !confirmed && touches_every_row(input) {
            return Ok(Reply::Confirm(
                "This has no WHERE and changes every row.".to_string(),
            ));
        }
        let affected = self.execute(ctx, input).await?;
        let note = if self.in_transaction() {
            " (not saved yet: .commit or .rollback)"
        } else {
            ""
        };
        Ok(Reply::Message(format!("{affected} row(s) affected{note}")))
    }

    async fn dot_command(&mut self, ctx: &AppContext, command: &str) -> Result<Reply> {
        let (name, argument) = command
            .split_once(char::is_whitespace)
            .map_or((command, ""), |(name, rest)| (name, rest.trim()));
        match name {
            "help" | "h" | "?" => Ok(Reply::Message(HELP.to_string())),
            "exit" | "quit" | "q" => Ok(Reply::Exit),
            "tables" => table(&self.query(ctx, "SHOW TABLES").await?),
            "desc" | "describe" if valid_table_name(argument) => {
                table(&self.query(ctx, &format!("DESCRIBE `{argument}`")).await?)
            }
            "count" if valid_table_name(argument) => table(
                &self
                    .query(ctx, &format!("SELECT COUNT(*) AS total FROM `{argument}`"))
                    .await?,
            ),
            "desc" | "describe" | "count" => {
                Err(Error::string("give a table name, e.g. .desc users"))
            }
            "hash" if !argument.is_empty() => Ok(Reply::Message(hash::hash_password(argument)?)),
            "hash" => Err(Error::string("usage: .hash <password>")),
            "begin" => {
                if self.in_transaction() {
                    return Err(Error::string("a transaction is already open"));
                }
                self.transaction = Some(ctx.db.begin().await?);
                Ok(Reply::Message("transaction started".to_string()))
            }
            "commit" => match self.transaction.take() {
                Some(transaction) => {
                    transaction.commit().await?;
                    Ok(Reply::Message("committed".to_string()))
                }
                None => Err(Error::string("no open transaction")),
            },
            "rollback" => match self.transaction.take() {
                Some(transaction) => {
                    transaction.rollback().await?;
                    Ok(Reply::Message("rolled back".to_string()))
                }
                None => Err(Error::string("no open transaction")),
            },
            other => Err(Error::string(&format!(
                "unknown command .{other} (try .help)"
            ))),
        }
    }
}

fn print(reply: &Reply) {
    match reply {
        Reply::Table {
            columns,
            rows,
            total,
        } => println!("{}", render_table(columns, rows, *total)),
        Reply::Message(text) if !text.is_empty() => println!("{text}"),
        Reply::Confirm(text) => println!("{text}"),
        _ => {}
    }
}

/// True when the SQL on the line so far is complete (ends with `;`) or is a dot command.
fn input_complete(buffer: &str) -> bool {
    let trimmed = buffer.trim();
    trimmed.starts_with('.') || trimmed.ends_with(';')
}

pub struct Tinker;

#[async_trait]
impl Task for Tinker {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "tinker".to_string(),
            detail: "Interactive SQL console. Type .help inside.\nUsage: cargo run -- task tinker\nIn production add write:true to allow changes.".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &task::Vars) -> Result<()> {
        let allow_write = vars.cli_arg("write").is_ok_and(|value| value == "true");
        let production = ctx.environment == Environment::Production;
        let mut session = Session::new(production && !allow_write);

        println!(
            "tinker ({}){}. Type .help for commands, .exit to leave.",
            ctx.environment,
            if session.read_only { ", read-only" } else { "" }
        );
        let mut editor = DefaultEditor::new().map_err(|error| Error::string(&error.to_string()))?;
        let mut buffer = String::new();

        loop {
            let prompt = match (buffer.is_empty(), session.in_transaction()) {
                (false, _) => "   ...> ",
                (true, true) => "tinker*> ",
                (true, false) => "tinker> ",
            };
            // `readline` blocks, so tell the runtime this thread is busy.
            let line = tokio::task::block_in_place(|| editor.readline(prompt));
            match line {
                Ok(line) => {
                    buffer.push_str(&line);
                    buffer.push('\n');
                    if !input_complete(&buffer) {
                        continue;
                    }
                    let _ = editor.add_history_entry(buffer.trim());
                    let input = std::mem::take(&mut buffer);
                    let mut confirmed = false;
                    loop {
                        match session.handle(ctx, &input, confirmed).await {
                            Ok(Reply::Exit) => {
                                if let Some(note) = session.finish().await {
                                    println!("{note}");
                                }
                                return Ok(());
                            }
                            Ok(Reply::Confirm(question)) => {
                                println!("{question}");
                                let answer = tokio::task::block_in_place(|| {
                                    editor.readline("run it anyway? [y/N] ")
                                })
                                .unwrap_or_default();
                                if answer.trim().eq_ignore_ascii_case("y") {
                                    confirmed = true;
                                    continue;
                                }
                                println!("cancelled");
                            }
                            Ok(reply) => print(&reply),
                            Err(error) => println!("error: {error}"),
                        }
                        break;
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    // Ctrl-C clears what was typed; it does not leave.
                    buffer.clear();
                    println!("(cleared)");
                }
                Err(_) => {
                    if let Some(note) = session.finish().await {
                        println!("{note}");
                    }
                    return Ok(());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_are_told_apart() {
        for sql in [
            "select 1",
            "  SHOW tables",
            "describe users",
            "with x as (select 1) select * from x",
        ] {
            assert!(is_read(sql), "{sql}");
        }
        for sql in [
            "update users set name='a'",
            "delete from users",
            "insert into t values (1)",
            "drop table t",
        ] {
            assert!(!is_read(sql), "{sql}");
        }
    }

    #[test]
    fn statements_without_where_need_confirming() {
        assert!(touches_every_row("UPDATE users SET name = 'x'"));
        assert!(touches_every_row("delete from users"));
        assert!(!touches_every_row("update users set name='x' where id = 1"));
        assert!(!touches_every_row("insert into t values (1)"));
        assert!(!touches_every_row("select * from users"));
    }

    #[test]
    fn table_names_are_checked_before_use() {
        assert!(valid_table_name("users"));
        for bad in ["", "a b", "a;b", "users`", "a-b"] {
            assert!(!valid_table_name(bad), "{bad}");
        }
    }

    #[test]
    fn a_statement_is_complete_at_the_semicolon_or_for_dot_commands() {
        assert!(!input_complete("select *\nfrom users"));
        assert!(input_complete("select * from users;\n"));
        assert!(input_complete(".tables\n"));
    }

    #[test]
    fn tables_print_aligned_with_a_row_count() {
        let text = render_table(
            &["id".to_string(), "name".to_string()],
            &[vec!["1".to_string(), "Ada".to_string()]],
            1,
        );
        assert_eq!(text, "id | name\n---+-----\n1  | Ada \n(1 row)");
        assert_eq!(render_table(&[], &[], 0), "(no rows)");
    }
}
