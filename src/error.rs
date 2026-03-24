//! Error types for git-paw.
//!
//! Defines [`PawError`], the central error enum used across all modules.
//! Each variant carries an actionable, user-facing message.

use std::process;

/// Exit codes for git-paw.
pub mod exit_code {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_a_git_repo_is_actionable() {
        let msg = PawError::NotAGitRepo.to_string();
        assert!(msg.contains("git repository"), "should explain the problem");
        assert!(msg.contains("git-paw"), "should name the tool");
    }

    #[test]
    fn test_tmux_not_installed_includes_install_instructions() {
        let msg = PawError::TmuxNotInstalled.to_string();
        assert!(msg.contains("tmux"), "should name the missing dependency");
        assert!(
            msg.contains("brew install"),
            "should include macOS install hint"
        );
        assert!(
            msg.contains("apt install"),
            "should include Linux install hint"
        );
    }

    #[test]
    fn test_no_clis_found_suggests_add_cli() {
        let msg = PawError::NoCLIsFound.to_string();
        assert!(
            msg.contains("add-cli"),
            "should suggest the add-cli command"
        );
    }

    #[test]
    fn test_worktree_error_includes_detail() {
        let msg = PawError::WorktreeError("failed to create".into()).to_string();
        assert!(
            msg.contains("failed to create"),
            "should include the inner detail"
        );
    }

    #[test]
    fn test_session_error_includes_detail() {
        let msg = PawError::SessionError("file corrupt".into()).to_string();
        assert!(
            msg.contains("file corrupt"),
            "should include the inner detail"
        );
    }

    #[test]
    fn test_config_error_includes_detail() {
        let msg = PawError::ConfigError("invalid toml".into()).to_string();
        assert!(
            msg.contains("invalid toml"),
            "should include the inner detail"
        );
    }

    #[test]
    fn test_branch_error_includes_detail() {
        let msg = PawError::BranchError("not found".into()).to_string();
        assert!(msg.contains("not found"), "should include the inner detail");
    }

    #[test]
    fn test_user_cancelled_is_not_empty() {
        let msg = PawError::UserCancelled.to_string();
        assert!(!msg.is_empty(), "should have a message");
    }

    #[test]
    fn test_tmux_error_includes_detail() {
        let msg = PawError::TmuxError("session failed".into()).to_string();
        assert!(
            msg.contains("session failed"),
            "should include the inner detail"
        );
    }

    #[test]
    fn test_cli_not_found_includes_cli_name() {
        let msg = PawError::CliNotFound("my-agent".into()).to_string();
        assert!(
            msg.contains("my-agent"),
            "should include the missing CLI name"
        );
    }

    #[test]
    fn test_user_cancelled_exit_code() {
        assert_eq!(
            PawError::UserCancelled.exit_code(),
            exit_code::USER_CANCELLED
        );
    }

    #[test]
    fn test_general_errors_exit_code() {
        let errors: Vec<PawError> = vec![
            PawError::NotAGitRepo,
            PawError::TmuxNotInstalled,
            PawError::NoCLIsFound,
            PawError::WorktreeError("test".into()),
            PawError::SessionError("test".into()),
            PawError::ConfigError("test".into()),
            PawError::BranchError("test".into()),
            PawError::TmuxError("test".into()),
            PawError::CliNotFound("test".into()),
        ];
        for err in errors {
            assert_eq!(err.exit_code(), exit_code::ERROR, "failed for {err:?}");
        }
    }

    #[test]
    fn test_debug_derived() {
        let err = PawError::NotAGitRepo;
        let debug = format!("{err:?}");
        assert!(debug.contains("NotAGitRepo"));
    }
}
