//! `git paw stop` — kill the tmux session while preserving worktrees and
//! state.

use crate::agents;
use crate::error::PawError;
use crate::session::{self, SessionStatus};
use crate::supervisor::loop_binding;
use crate::tmux;

use super::{CommandContext, Output};

/// Options for [`stop`]. Stopping never asks for confirmation, so there is
/// no force or confirmation option (`stop-without-confirmation`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StopOptions {}

/// What [`stop`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopOutcome {
    /// The repository has no session.
    NoSession,
    /// The session was stopped; its worktrees and state are preserved.
    Stopped {
        /// The session.
        session_name: String,
    },
}

/// Stops the session: kills tmux but preserves worktrees and state.
///
/// # Errors
///
/// Returns an error when the session state cannot be read or saved, or tmux
/// cannot kill the session.
pub fn stop(
    ctx: &CommandContext,
    _opts: StopOptions,
    out: &mut dyn Output,
) -> Result<StopOutcome, PawError> {
    let repo_root = ctx.repo_root();
    let Some(existing) = session::find_session_for_repo(repo_root)? else {
        out.line("No active session for this repo.");
        return Ok(StopOutcome::NoSession);
    };

    if tmux::is_session_alive(&existing.session_name)? {
        tmux::kill_session(&existing.session_name)?;
    }

    // Bug E (v0-5-0-audit-cleanup §9c) — strip the supervisor-pane boot
    // block from AGENTS.md so it does not accumulate across sessions.
    // Idempotent: missing block / missing AGENTS.md is a no-op.
    if let Err(e) = agents::remove_session_boot_block(repo_root) {
        out.warn(&format!(
            "warning: failed to clean session boot block from AGENTS.md: {e}"
        ));
    }

    let session_name = existing.session_name.clone();
    let mut updated = existing;
    updated.status = SessionStatus::Stopped;
    session::save_session(&updated)?;

    // Revoke the drive-loop binding AFTER `Stopped` is recorded, so a loop
    // that reads the receipt between the two sees the stop either way.
    // Revoking is what makes the stop observable at all to a loop that is
    // about to be re-attached: recovery keeps the receipt's `created_at`, so a
    // session stopped and recovered inside one poll interval looks unchanged
    // to the loop's record check, and the old loop would go on driving the
    // recovered session (GP-06).
    loop_binding::revoke(repo_root);

    out.line("Session stopped. Worktrees and state preserved.");
    out.line("Run `git paw start` to recover.");
    Ok(StopOutcome::Stopped { session_name })
}
