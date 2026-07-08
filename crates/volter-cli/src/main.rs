//! `volter` developer CLI.
//!
//! # Subcommands
//!
//! - [`new`](crate::Command::New) — scaffold a new volter project.
//! - `run` — planned: run the current project with reload-on-change.

#![deny(missing_docs)]

mod new;

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
    /// Run the current project with reload-on-change (not implemented yet).
    Run,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::New { name } => new::run_new(&name),
        Command::Run => {
            println!("TODO: run project with reload (not implemented yet)");
            Ok(())
        }
    }
}
