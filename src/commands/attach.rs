//! `git paw attach` — reattach the current terminal to the running tmux
//! session for the current repository.

use std::path::Path;

use git_paw::error::PawError;
use git_paw::git;
use git_paw::session;
use git_paw::tmux;

/// Reattaches the current terminal to the running session for the current
/// repository.
///
/// Resolves the session the same way `git paw status` does (by repository
/// path) and delegates to the existing [`tmux::attach`] path — this adds no
/// new tmux capability, only the command surface.
pub(crate) fn cmd_attach() -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    let session_name = resolve_attach_session_name(&repo_root)?;
    tmux::attach(&session_name)
}

/// Resolves the tmux session name to attach to for `repo_root`.
///
/// A session must exist for the repository AND be alive in tmux; otherwise
/// this returns an actionable error naming the problem and how to start a
/// session. Extracted from [`cmd_attach`] so the resolution logic is
/// testable without taking over the calling process's terminal.
pub(crate) fn resolve_attach_session_name(repo_root: &Path) -> Result<String, PawError> {
    let Some(existing) = session::find_session_for_repo(repo_root)? else {
        return Err(PawError::SessionError(
            "no session is running for this repo\n  \u{21b3} run 'git paw start' to launch one"
                .to_string(),
        ));
    };

    let liveness = tmux::session_liveness(&existing.session_name);
    if !matches!(liveness, tmux::SessionLiveness::Alive) {
        return Err(PawError::SessionError(format!(
            "no session is running for this repo (session '{}' is not alive)\n  \u{21b3} run 'git paw start' to launch one",
            existing.session_name
        )));
    }

    Ok(existing.session_name)
}
