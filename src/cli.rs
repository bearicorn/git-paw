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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// Helper: parse args as if running `git-paw <args>`.
    fn parse(args: &[&str]) -> Cli {
        let mut full = vec!["git-paw"];
        full.extend(args);
        Cli::try_parse_from(full).expect("failed to parse")
    }

    // -- Default subcommand --

    #[test]
    fn no_args_defaults_to_none_command() {
        let cli = parse(&[]);
        assert!(
            cli.command.is_none(),
            "no args should yield None (handled as Start in main)"
        );
    }

    // -- Start subcommand --

    #[test]
    fn start_with_no_flags() {
        let cli = parse(&["start"]);
        match cli.command.unwrap() {
            Command::Start {
                cli,
                branches,
                dry_run,
                preset,
            } => {
                assert!(cli.is_none());
                assert!(branches.is_none());
                assert!(!dry_run);
                assert!(preset.is_none());
            }
            other => panic!("expected Start, got {other:?}"),
        }
    }

    #[test]
    fn start_with_cli_flag() {
        let cli = parse(&["start", "--cli", "claude"]);
        match cli.command.unwrap() {
            Command::Start { cli, .. } => assert_eq!(cli.as_deref(), Some("claude")),
            other => panic!("expected Start, got {other:?}"),
        }
    }

    #[test]
    fn start_with_branches_flag_comma_separated() {
        let cli = parse(&["start", "--branches", "feat/a,feat/b,fix/c"]);
        match cli.command.unwrap() {
            Command::Start { branches, .. } => {
                let b = branches.expect("branches should be set");
                assert_eq!(b, vec!["feat/a", "feat/b", "fix/c"]);
            }
            other => panic!("expected Start, got {other:?}"),
        }
    }

    #[test]
    fn start_with_dry_run() {
        let cli = parse(&["start", "--dry-run"]);
        match cli.command.unwrap() {
            Command::Start { dry_run, .. } => assert!(dry_run),
            other => panic!("expected Start, got {other:?}"),
        }
    }

    #[test]
    fn start_with_preset() {
        let cli = parse(&["start", "--preset", "backend"]);
        match cli.command.unwrap() {
            Command::Start { preset, .. } => assert_eq!(preset.as_deref(), Some("backend")),
            other => panic!("expected Start, got {other:?}"),
        }
    }

    #[test]
    fn start_with_all_flags() {
        let cli = parse(&[
            "start",
            "--cli",
            "gemini",
            "--branches",
            "a,b",
            "--dry-run",
            "--preset",
            "dev",
        ]);
        match cli.command.unwrap() {
            Command::Start {
                cli,
                branches,
                dry_run,
                preset,
            } => {
                assert_eq!(cli.as_deref(), Some("gemini"));
                assert_eq!(branches.unwrap(), vec!["a", "b"]);
                assert!(dry_run);
                assert_eq!(preset.as_deref(), Some("dev"));
            }
            other => panic!("expected Start, got {other:?}"),
        }
    }

    // -- Stop subcommand --

    #[test]
    fn stop_parses() {
        let cli = parse(&["stop"]);
        assert!(matches!(cli.command.unwrap(), Command::Stop));
    }

    // -- Purge subcommand --

    #[test]
    fn purge_without_force() {
        let cli = parse(&["purge"]);
        match cli.command.unwrap() {
            Command::Purge { force } => assert!(!force),
            other => panic!("expected Purge, got {other:?}"),
        }
    }

    #[test]
    fn purge_with_force() {
        let cli = parse(&["purge", "--force"]);
        match cli.command.unwrap() {
            Command::Purge { force } => assert!(force),
            other => panic!("expected Purge, got {other:?}"),
        }
    }

    // -- Status subcommand --

    #[test]
    fn status_parses() {
        let cli = parse(&["status"]);
        assert!(matches!(cli.command.unwrap(), Command::Status));
    }

    // -- List-CLIs subcommand --

    #[test]
    fn list_clis_parses() {
        let cli = parse(&["list-clis"]);
        assert!(matches!(cli.command.unwrap(), Command::ListClis));
    }

    // -- Add-CLI subcommand --

    #[test]
    fn add_cli_with_required_args() {
        let cli = parse(&["add-cli", "my-agent", "/usr/local/bin/my-agent"]);
        match cli.command.unwrap() {
            Command::AddCli {
                name,
                command,
                display_name,
            } => {
                assert_eq!(name, "my-agent");
                assert_eq!(command, "/usr/local/bin/my-agent");
                assert!(display_name.is_none());
            }
            other => panic!("expected AddCli, got {other:?}"),
        }
    }

    #[test]
    fn add_cli_with_display_name() {
        let cli = parse(&[
            "add-cli",
            "my-agent",
            "my-agent",
            "--display-name",
            "My Agent",
        ]);
        match cli.command.unwrap() {
            Command::AddCli {
                name,
                command,
                display_name,
            } => {
                assert_eq!(name, "my-agent");
                assert_eq!(command, "my-agent");
                assert_eq!(display_name.as_deref(), Some("My Agent"));
            }
            other => panic!("expected AddCli, got {other:?}"),
        }
    }

    // -- Remove-CLI subcommand --

    #[test]
    fn remove_cli_parses() {
        let cli = parse(&["remove-cli", "my-agent"]);
        match cli.command.unwrap() {
            Command::RemoveCli { name } => assert_eq!(name, "my-agent"),
            other => panic!("expected RemoveCli, got {other:?}"),
        }
    }

    // -- Help text quality --

    #[test]
    fn version_flag_is_accepted() {
        let result = Cli::try_parse_from(["git-paw", "--version"]);
        // clap returns Err(DisplayVersion) for --version, which is expected
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn help_flag_is_accepted() {
        let result = Cli::try_parse_from(["git-paw", "--help"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn unknown_subcommand_is_rejected() {
        let result = Cli::try_parse_from(["git-paw", "unknown-command"]);
        assert!(result.is_err());
    }

    #[test]
    fn add_cli_missing_required_args_is_rejected() {
        let result = Cli::try_parse_from(["git-paw", "add-cli"]);
        assert!(result.is_err());
    }
}
