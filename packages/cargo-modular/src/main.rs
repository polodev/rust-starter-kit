mod commands;
mod stubs;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Invoked as `cargo modular <command>`; cargo passes `modular` as the first argument.
#[derive(Parser)]
#[command(name = "cargo-modular", bin_name = "cargo modular", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Wrapper,
}

#[derive(Subcommand)]
#[allow(clippy::enum_variant_names)]
enum Wrapper {
    Modular {
        #[command(subcommand)]
        command: Command,
    },
}

#[derive(Subcommand)]
#[allow(clippy::enum_variant_names)]
enum Command {
    /// Create a new module crate under modules/ and register it
    #[command(name = "make:module")]
    MakeModule {
        name: String,
        /// Print what would change without writing anything
        #[arg(long)]
        dry_run: bool,
        /// Project root (defaults to the current workspace root)
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Create a command (Loco task); with --module it is registered in that module
    #[command(name = "make:command")]
    MakeCommand {
        name: String,
        #[arg(long)]
        module: Option<String>,
        /// Task signature, default `<module>:<name>`
        #[arg(long)]
        signature: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Create a mail class with HTML and text templates in a module (like Laravel's make:mail)
    #[command(name = "make:mail")]
    MakeMail {
        name: String,
        /// The module that owns the mail (it gets a dependency on module-mail)
        #[arg(long)]
        module: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// List every command (task) the app knows
    #[command(name = "command:list")]
    CommandList {
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Run a command, e.g. `command:run pages:check-translations strict:true`
    #[command(name = "command:run")]
    CommandRun {
        signature: String,
        /// `key:value` arguments passed to the command
        args: Vec<String>,
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Show every scheduled job with its cron expression
    #[command(name = "schedule:list")]
    ScheduleList {
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Run the scheduler in the foreground (use a process supervisor in production)
    #[command(name = "schedule:work")]
    ScheduleWork {
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// List registered modules
    #[command(name = "modules:list")]
    ModulesList {
        #[arg(long)]
        root: Option<PathBuf>,
    },
    /// Rebuild src/modules.rs from the modules/ directory
    #[command(name = "modules:sync")]
    ModulesSync {
        #[arg(long)]
        root: Option<PathBuf>,
    },
}

fn main() -> anyhow::Result<()> {
    let Wrapper::Modular { command } = Cli::parse().command;
    match command {
        Command::MakeModule {
            name,
            dry_run,
            root,
        } => commands::make_module(&commands::resolve_root(root)?, &name, dry_run),
        Command::MakeCommand {
            name,
            module,
            signature,
            dry_run,
            root,
        } => commands::make_command(
            &commands::resolve_root(root)?,
            &name,
            module.as_deref(),
            signature.as_deref(),
            dry_run,
        ),
        Command::MakeMail {
            name,
            module,
            dry_run,
            root,
        } => commands::make_mail(&commands::resolve_root(root)?, &name, &module, dry_run),
        Command::CommandList { root } => commands::command_list(&commands::resolve_root(root)?),
        Command::CommandRun {
            signature,
            args,
            root,
        } => commands::command_run(&commands::resolve_root(root)?, &signature, &args),
        Command::ScheduleList { root } => commands::schedule_list(&commands::resolve_root(root)?),
        Command::ScheduleWork { root } => commands::schedule_work(&commands::resolve_root(root)?),
        Command::ModulesList { root } => commands::modules_list(&commands::resolve_root(root)?),
        Command::ModulesSync { root } => commands::modules_sync(&commands::resolve_root(root)?),
    }
}
