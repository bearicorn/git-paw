//! CLI argument parsing.
//!
//! Defines the command-line interface using `clap` v4 with derive macros.
//! All subcommands, flags, and options are declared here.

use clap::{Parser, Subcommand};

/// Parallel AI Worktrees — orchestrate multiple AI coding CLI sessions
/// across git worktrees from a single terminal using tmux.
#[derive(Debug, Parser)]
#[command(
    name = "git-paw",
    version,
    about = "Parallel AI Worktrees — orchestrate multiple AI coding CLI sessions across git worktrees",
    long_about = "git-paw orchestrates multiple AI coding CLI sessions (Claude, Codex, Gemini, etc.) \
                  across git worktrees from a single terminal using tmux. Each branch gets its own \
                  worktree and AI session, running in parallel.",
    after_help = "\x1b[1mQuick Start:\x1b[0m\n\n  \
                  # Launch interactive session (picks CLI and branches)\n  \
                  git paw\n\n  \
                  # Use Claude on specific branches\n  \
                  git paw start --cli claude --branches feat/auth,feat/api\n\n  \
                  # Check session status\n  \
                  git paw status\n\n  \
                  # Stop session (preserves worktrees for later)\n  \
                  git paw stop\n\n  \
                  # Remove everything\n  \
                  git paw purge"
)]
pub struct Cli {
    /// Subcommand to run. Defaults to `start` if omitted.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Launch a new session or reattach to an existing one
    #[command(
        about = "Launch a new session or reattach to an existing one",
        long_about = "Smart start: reattaches if a session is active, recovers if stopped/crashed, \
                      or launches a new interactive session.\n\n\
                      Examples:\n  \
                      git paw start\n  \
                      git paw start --cli claude\n  \
                      git paw start --cli claude --branches feat/auth,feat/api\n  \
                      git paw start --dry-run\n  \
                      git paw start --preset backend"
    )]
    Start {
        /// AI CLI to use (e.g., claude, codex, gemini). Skips CLI picker if provided.
        #[arg(long, help = "AI CLI to use (skips CLI picker)")]
        cli: Option<String>,

        /// Comma-separated branch names. Skips branch picker if provided.
        #[arg(
            long,
            value_delimiter = ',',
            help = "Comma-separated branches (skips branch picker)"
        )]
        branches: Option<Vec<String>>,

        /// Preview the session plan without executing.
        #[arg(long, help = "Preview the session plan without executing")]
        dry_run: bool,

        /// Use a named preset from config.
        #[arg(long, help = "Use a named preset from config")]
        preset: Option<String>,
    },

    /// Stop the session (kills tmux, keeps worktrees and state)
    #[command(
        about = "Stop the session (kills tmux, keeps worktrees and state)",
        long_about = "Kills the tmux session but preserves worktrees and session state on disk. \
                      Run `git paw start` later to recover the session.\n\n\
                      Example:\n  git paw stop"
    )]
    Stop,

    /// Remove everything (tmux session, worktrees, and state)
    #[command(
        about = "Remove everything (tmux session, worktrees, and state)",
        long_about = "Nuclear option: kills the tmux session, removes all worktrees, and deletes \
                      session state. Requires confirmation unless --force is used.\n\n\
                      Examples:\n  git paw purge\n  git paw purge --force"
    )]
    Purge {
        /// Skip confirmation prompt.
        #[arg(long, help = "Skip confirmation prompt")]
        force: bool,
    },

    /// Show session state for the current repo
    #[command(
        about = "Show session state for the current repo",
        long_about = "Displays the current session status, branches, CLIs, and worktree paths \
                      for the repository in the current directory.\n\n\
                      Example:\n  git paw status"
    )]
    Status,

    /// List detected and custom AI CLIs
    #[command(
        about = "List detected and custom AI CLIs",
        long_about = "Shows all AI CLIs found on PATH (auto-detected) and any custom CLIs \
                      registered in your config.\n\n\
                      Example:\n  git paw list-clis"
    )]
    ListClis,

    /// Register a custom AI CLI
    #[command(
        about = "Register a custom AI CLI",
        long_about = "Adds a custom CLI to your global config (~/.config/git-paw/config.toml). \
                      The command can be an absolute path or a binary name on PATH.\n\n\
                      Examples:\n  \
                      git paw add-cli my-agent /usr/local/bin/my-agent\n  \
                      git paw add-cli my-agent my-agent --display-name \"My Agent\""
    )]
    AddCli {
        /// Name to register the CLI as.
        #[arg(help = "Name to register the CLI as")]
        name: String,

        /// Command or path to the CLI binary.
        #[arg(help = "Command or path to the CLI binary")]
        command: String,

        /// Optional display name for the CLI.
        #[arg(long, help = "Display name shown in prompts")]
        display_name: Option<String>,
    },

    /// Unregister a custom AI CLI
    #[command(
        about = "Unregister a custom AI CLI",
        long_about = "Removes a custom CLI from your global config. Only custom CLIs can be \
                      removed — auto-detected CLIs cannot.\n\n\
                      Example:\n  git paw remove-cli my-agent"
    )]
    RemoveCli {
        /// Name of the custom CLI to remove.
        #[arg(help = "Name of the custom CLI to remove")]
        name: String,
    },
}
