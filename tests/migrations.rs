//! The starter ships clean base migrations: every table is created by exactly one migration file
//! and is never altered by a later one, tables that belong together share one file (so there are
//! fewer migration files than tables), and no two files share a sequence number. People who clone
//! the starter add their own migrations on top.

use std::{collections::BTreeMap, fs, path::Path};

fn migration_files() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut dirs = vec![root.join("migration/src")];
    for module in fs::read_dir(root.join("modules")).unwrap().flatten() {
        dirs.push(module.path().join("src/migrations"));
    }
    let mut files = Vec::new();
    for dir in dirs {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('m')
                && name.ends_with(".rs")
                && name
                    .get(1..9)
                    .is_some_and(|date| date.bytes().all(|b| b.is_ascii_digit()))
            {
                files.push((name, fs::read_to_string(entry.path()).unwrap()));
            }
        }
    }
    files
}

/// Names of the tables a migration creates: `create_table(m, "name", ...)` or `CREATE TABLE ... `name``.
fn created_tables(source: &str) -> Vec<String> {
    let mut tables = Vec::new();
    // `create_table(m, "name", ...)` and `create_join_table(m, "name", ...)` (a link table).
    for marker in ["create_table(", "create_join_table("] {
        let mut rest = source;
        while let Some(at) = rest.find(marker) {
            rest = &rest[at + marker.len()..];
            if let Some(open) = rest.find('"') {
                if let Some(close) = rest[open + 1..].find('"') {
                    tables.push(rest[open + 1..open + 1 + close].to_string());
                }
            }
        }
    }
    let mut rest = source;
    while let Some(at) = rest.find("CREATE TABLE IF NOT EXISTS `") {
        rest = &rest[at + "CREATE TABLE IF NOT EXISTS `".len()..];
        if let Some(close) = rest.find('`') {
            tables.push(rest[..close].to_string());
        }
    }
    tables
}

#[test]
fn every_table_has_one_migration_file_and_is_never_altered() {
    let files = migration_files();
    assert!(
        files.len() >= 8,
        "found only {} migration files",
        files.len()
    );

    let mut owner_of: BTreeMap<String, String> = BTreeMap::new();
    for (name, source) in &files {
        for forbidden in ["ALTER TABLE", "alter_table(", "add_column", "drop_column"] {
            assert!(
                !source.contains(forbidden),
                "{name} changes an existing table (`{forbidden}`); put the change in the table's own base migration"
            );
        }
        for table in created_tables(source) {
            if let Some(first) = owner_of.insert(table.clone(), name.clone()) {
                panic!("table `{table}` is created by both {first} and {name}");
            }
        }
    }
}

#[test]
fn migration_sequence_numbers_are_unique() {
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for (name, _) in migration_files() {
        // m{date}_{sequence}_{what}.rs
        let mut parts = name.trim_end_matches(".rs").splitn(3, '_');
        let (_date, sequence) = (parts.next().unwrap(), parts.next().unwrap().to_string());
        if let Some(other) = seen.insert(sequence.clone(), name.clone()) {
            panic!("{name} and {other} share the sequence number {sequence}");
        }
    }
}

/// `table -> file` for every table the migrations create.
fn table_files() -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for (name, source) in migration_files() {
        for table in created_tables(&source) {
            map.insert(table, name.clone());
        }
    }
    map
}

#[test]
fn there_are_fewer_migration_files_than_tables() {
    let tables = table_files();
    let files = migration_files().len();
    assert!(
        files < tables.len(),
        "{files} migration files for {} tables: group the tables that belong together in one file",
        tables.len()
    );
}

#[test]
fn tables_that_belong_together_are_created_in_one_file() {
    let tables = table_files();
    for group in [
        // roles, permissions and the three links between them and users
        vec![
            "roles",
            "permissions",
            "permission_roles",
            "role_users",
            "permission_users",
        ],
        // a page and its translations
        vec!["pages", "page_translations"],
        // tracked job history and the Laravel-style waiting and failed job tables
        vec!["queue_jobs", "jobs", "failed_jobs"],
    ] {
        let files: std::collections::BTreeSet<_> = group
            .iter()
            .map(|table| {
                tables
                    .get(*table)
                    .unwrap_or_else(|| panic!("no migration creates `{table}`"))
            })
            .collect();
        assert_eq!(
            files.len(),
            1,
            "{group:?} are spread over {files:?}; keep them in one migration file"
        );
    }
}
