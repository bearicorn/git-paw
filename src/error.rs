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
    /// The command wrote nothing because a condition for proceeding was not
    /// met. Shares the cancellation code: nothing was attempted, so this is
    /// not a failure the caller should treat as an error.
    pub const NOT_ATTEMPTED: i32 = USER_CANCELLED;
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
    #[error("Worktree: {0}")]
    WorktreeError(String),

    /// Session state read/write failed.
    #[error("Session: {0}")]
    SessionError(String),

    /// Config file parsing failed.
    #[error("Config: {0}")]
    ConfigError(String),

    /// Branch operation failed.
    #[error("Branch: {0}")]
    BranchError(String),

    /// User cancelled via Ctrl+C or empty selection.
    #[error("Cancelled.")]
    UserCancelled,

    /// The command wrote nothing because a condition for proceeding was not
    /// met — a declined confirmation, a confirmation that could not be shown
    /// because standard input is not a terminal, or a requested item that is
    /// not supported or not enabled for its target.
    ///
    /// Unlike [`PawError::UserCancelled`], whose message is the bare
    /// `Cancelled.`, the payload states the condition and, where one exists,
    /// names the flag or command that proceeds — so the user can tell a
    /// deliberate no-op apart from a silent cancellation.
    #[error("{0}")]
    NotAttempted(String),

    /// tmux operation failed.
    #[error("Tmux: {0}")]
    TmuxError(String),

    /// Custom CLI not found in config.
    #[error("CLI '{0}' not found in config")]
    CliNotFound(String),

    /// Init operation failed.
    #[error("Init: {0}")]
    InitError(String),

    /// AGENTS.md operation failed.
    #[error("AGENTS.md: {0}")]
    AgentsMdError(String),

    /// Spec scanning failed.
    #[error("Spec: {0}")]
    SpecError(String),

    /// Replay operation failed.
    #[error("Replay: {0}")]
    ReplayError(String),

    /// Broker operation failed.
    #[error("Broker: {0}")]
    BrokerError(#[from] crate::broker::BrokerError),

    /// Skill template loading failed.
    #[error(transparent)]
    SkillError(#[from] crate::skills::SkillError),

    /// Dashboard TUI operation failed.
    #[error("Dashboard: {0}")]
    DashboardError(String),

    /// MCP server startup / repository-resolution failure.
    #[error("MCP: {0}")]
    McpError(String),

    /// One or more `git paw doctor` checks reported a hard failure.
    ///
    /// The report itself is already on stdout; this variant only carries the
    /// non-zero exit status and a one-line pointer back at the remedies.
    #[error("doctor found {0} failing check(s) — fix the ✗ remedies above and re-run")]
    DoctorFailed(usize),

    /// I/O operation failed.
    #[error("I/O: {0}")]
    IoError(#[from] std::io::Error),
}

impl PawError {
    /// Returns the process exit code for this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::UserCancelled => exit_code::USER_CANCELLED,
            Self::NotAttempted(_) => exit_code::NOT_ATTEMPTED,
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

    // -----------------------------------------------------------------------
    // Actionable / hint-mapping messages (behavioral contract)
    //
    // These guard that a variant surfaces an actionable, user-facing message —
    // the install hint, the recovery command, the problem statement. The
    // `{0}`-detail-interpolation Display tests were dropped as tautologies
    // (they only re-checked that thiserror interpolates the format string); the
    // static-message variants below are kept as the sole guard that each still
    // produces a non-empty / actionable message.
    // -----------------------------------------------------------------------

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
    fn test_user_cancelled_is_not_empty() {
        let msg = PawError::UserCancelled.to_string();
        assert!(!msg.is_empty(), "should have a message");
    }

    // -----------------------------------------------------------------------
    // Exit-code mapping (behavioral contract)
    //
    // Two variants report "nothing was done" rather than a failure —
    // `UserCancelled` and `NotAttempted` — and both map to the dedicated
    // code 2; every other variant maps to the general error code. The general
    // table samples a representative variant of each shape.
    // -----------------------------------------------------------------------

    #[test]
    fn test_user_cancelled_exit_code() {
        assert_eq!(
            PawError::UserCancelled.exit_code(),
            exit_code::USER_CANCELLED
        );
    }

    #[test]
    fn test_not_attempted_exit_code_and_message() {
        // A confirmation that could not be shown because stdin is not a
        // terminal: exit 2, and a message that names the flag which proceeds
        // instead of the bare `Cancelled.` the user cannot act on.
        let err = PawError::NotAttempted(
            "nothing was written: the confirmation could not be shown because stdin is not a \
             terminal — re-run with --yes to apply, or --dry-run to preview"
                .into(),
        );
        assert_eq!(err.exit_code(), exit_code::NOT_ATTEMPTED);
        assert_eq!(err.exit_code(), 2, "the contract pins the code itself");

        let msg = err.to_string();
        assert!(msg.contains("--yes"), "should name the flag that proceeds");
        assert_ne!(
            msg,
            PawError::UserCancelled.to_string(),
            "must not degrade to the bare cancellation message"
        );
        assert!(!msg.contains("Cancelled."), "should state the condition");
    }

    #[test]
    fn test_general_errors_exit_code() {
        // Every variant other than `UserCancelled` and `NotAttempted`, which
        // have their own cases above.
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
            PawError::SpecError("test".into()),
            PawError::AgentsMdError("test".into()),
            PawError::DashboardError("test".into()),
            PawError::SkillError(crate::skills::SkillError::UnknownSkill {
                name: "test".into(),
            }),
        ];
        for err in errors {
            assert_eq!(err.exit_code(), exit_code::ERROR, "failed for {err:?}");
        }
    }

    #[test]
    fn wrapping_variant_display_does_not_embed_error() {
        // `exit()` prepends "error: ", so a wrapping variant's own Display must
        // not also say "error" or the user sees a doubled "error: Worktree error: ...".
        let msg = PawError::WorktreeError("disk full".into()).to_string();
        assert_eq!(msg, "Worktree: disk full");
        assert!(
            !msg.to_ascii_lowercase().contains("error"),
            "wrapping variant Display must not embed 'error': {msg}"
        );
        // A representative sample across the string-wrapping variants (their
        // Display is fully controlled, unlike the #[from] source variants).
        for err in [
            PawError::SessionError("x".into()),
            PawError::ConfigError("x".into()),
            PawError::TmuxError("x".into()),
            PawError::SpecError("x".into()),
        ] {
            assert!(
                !err.to_string().to_ascii_lowercase().contains("error"),
                "wrapping variant Display must not embed 'error': {err}"
            );
        }
    }
}
