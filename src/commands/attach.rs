//! `git paw attach` — reattach the current terminal to the running tmux
//! session for the current repository.

use std::path::Path;

use crate::error::PawError;
use crate::session;
use crate::tmux;

use super::{CommandContext, Output};

/// Options for [`attach`]. `git paw attach` takes none.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AttachOptions {}

/// The session [`attach`] resolved; the caller attaches the terminal to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachOutcome {
    /// The live tmux session to attach to.
    pub session_name: String,
}

/// Resolves the running session for the repository, the same way
/// `git paw status` does (by repository path). It does not attach: the
/// caller attaches the terminal to [`AttachOutcome::session_name`].
///
/// # Errors
///
/// Returns an actionable error when no session is running for the
/// repository.
pub fn attach(
    ctx: &CommandContext,
    _opts: AttachOptions,
    _out: &mut dyn Output,
) -> Result<AttachOutcome, PawError> {
    let session_name = resolve_attach_session_name(ctx.repo_root())?;
    Ok(AttachOutcome { session_name })
}

/// Resolves the tmux session name to attach to for `repo_root`.
///
/// A session must exist for the repository AND be alive in tmux; otherwise
/// this returns an actionable error naming the problem and how to start a
/// session.
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
