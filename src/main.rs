//! git-paw — Parallel AI Worktrees.
//!
//! Orchestrates multiple AI coding CLI sessions across git worktrees
//! from a single terminal using tmux.

#![allow(dead_code)]

mod cli;
mod config;
mod detect;
mod error;
mod git;
mod interactive;
mod session;
mod tmux;

use clap::Parser;

use cli::{Cli, Command};

fn main() {
    let args = Cli::parse();

    match args.command.unwrap_or(Command::Start {
        cli: None,
        branches: None,
        dry_run: false,
        preset: None,
    }) {
        Command::Start {
            cli: _cli,
            branches: _branches,
            dry_run: _dry_run,
            preset: _preset,
        } => {
            println!("start: not yet implemented");
        }
        Command::Stop => {
            println!("stop: not yet implemented");
        }
        Command::Purge { force: _force } => {
            println!("purge: not yet implemented");
        }
        Command::Status => {
            println!("status: not yet implemented");
        }
        Command::ListClis => {
            println!("list-clis: not yet implemented");
        }
        Command::AddCli {
            name: _name,
            command: _command,
            display_name: _display_name,
        } => {
            println!("add-cli: not yet implemented");
        }
        Command::RemoveCli { name: _name } => {
            println!("remove-cli: not yet implemented");
        }
    }
}
