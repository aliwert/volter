//! `volter` developer CLI.
//!
//! Planned subcommands:
//!
//! - `volter new <name>` — scaffold a new volter project from a template.
//! - `volter run` — run the current project with a file-watcher / reload.
//!
//! This is the one crate in the workspace where `anyhow`, `unwrap`, and
//! `panic!` are acceptable (it's a binary, not a published library API —
//! see `RULES.md` #1 and #4) but they're still avoided by default for
//! consistency; prefer a clear `anyhow::Context` message over an `unwrap`.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "volter", about = "Developer CLI for the volter web framework")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scaffold a new volter project.
    New {
        /// Name of the new project.
        name: String,
    },
    /// Run the current project with reload-on-change.
    Run,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::New { name } => {
            println!("TODO: scaffold new project '{name}' (not implemented yet)");
        }
        Command::Run => {
            println!("TODO: run project with reload (not implemented yet)");
        }
    }

    Ok(())
}
