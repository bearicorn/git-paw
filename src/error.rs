//! Error types for git-paw.
//!
//! Defines [`PawError`], the central error enum used across all modules.
//! Each variant carries an actionable, user-facing message.

use std::process;

/// Exit codes for git-paw.
pub mod exit_code {
    /// Successful execution.
    pub const SUCCESS: i32 = 0;
    /// General error.
    pub const ERROR: i32 = 1;
    /// User cancelled (Ctrl+C or empty selection).
    pub const USER_CANCELLED: i32 = 2;
}

/// Central error type for git-paw operations.
#[derive(Debug, thiserror::Error)]
pub enum PawError {
    /// Not inside a git repository.
    #[error("Not a git repository. Run git-paw from inside a git project.")]
    NotAGitRepo,

    /// tmux is not installed.
    #[error(
        "tmux is required but not installed. Install with: brew install tmux (macOS) or apt install tmux (Linux)"
    )]
    TmuxNotInstalled,

    /// No AI CLIs found on PATH or in config.
    #[error(
        "No AI CLIs found on PATH. Install one or use `git paw add-cli` to register a custom CLI."
    )]
    NoCLIsFound,

    /// Git worktree operation failed.
    #[error("Worktree error: {0}")]
    WorktreeError(String),

    /// Session state read/write failed.
    #[error("Session error: {0}")]
    SessionError(String),

    /// Config file parsing failed.
    #[error("Config error: {0}")]
    ConfigError(String),

    /// Branch operation failed.
    #[error("Branch error: {0}")]
    BranchError(String),

    /// User cancelled via Ctrl+C or empty selection.
    #[error("Cancelled.")]
    UserCancelled,

    /// tmux operation failed.
    #[error("Tmux error: {0}")]
    TmuxError(String),

    /// Custom CLI not found in config.
    #[error("CLI '{0}' not found in config")]
    CliNotFound(String),
}

impl PawError {
    /// Returns the process exit code for this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::UserCancelled => exit_code::USER_CANCELLED,
            _ => exit_code::ERROR,
        }
    }

    /// Prints the error message to stderr and exits with the appropriate code.
    pub fn exit(&self) -> ! {
        eprintln!("error: {self}");
        process::exit(self.exit_code());
    }
}
