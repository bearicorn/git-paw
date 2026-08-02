//! `git paw replay` — replays captured session logs. Extracted verbatim from
//! `main.rs` (code-analysis-refactor R2b).

use git_paw::error::PawError;
use git_paw::git;

/// Replays captured session logs.
pub(crate) fn cmd_replay(
    branch: Option<String>,
    list: bool,
    color: bool,
    session: Option<&str>,
) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    if list {
        return git_paw::replay::display_list(&repo_root);
    }

    // clap ensures branch is present when --list is absent
    let branch = branch.expect("branch is required unless --list is passed");
    let session_name = git_paw::replay::resolve_session(&repo_root, session)?;
    let log_path = git_paw::replay::find_log(&repo_root, &session_name, &branch)?;

    if color {
        git_paw::replay::replay_colored(&log_path)
    } else {
        git_paw::replay::replay_stripped(&log_path)
    }
}
