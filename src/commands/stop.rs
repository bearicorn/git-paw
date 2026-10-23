//! `git paw stop` — kill the tmux session while preserving worktrees and
//! state. Extracted verbatim from `main.rs` (code-analysis-refactor R2b).

use git_paw::agents;
use git_paw::error::PawError;
use git_paw::git;
use git_paw::session::{self, SessionStatus};
use git_paw::supervisor::loop_binding;
use git_paw::tmux;

/// Stops the session: kills tmux but preserves worktrees and state.
pub(crate) fn cmd_stop() -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    let Some(existing) = session::find_session_for_repo(&repo_root)? else {
        println!("No active session for this repo.");
        return Ok(());
    };

    if tmux::is_session_alive(&existing.session_name)? {
        tmux::kill_session(&existing.session_name)?;
    }

    // Bug E (v0-5-0-audit-cleanup §9c) — strip the supervisor-pane boot
    // block from AGENTS.md so it does not accumulate across sessions.
    // Idempotent: missing block / missing AGENTS.md is a no-op.
    if let Err(e) = agents::remove_session_boot_block(&repo_root) {
        eprintln!("warning: failed to clean session boot block from AGENTS.md: {e}");
    }

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
    loop_binding::revoke(&repo_root);

    println!("Session stopped. Worktrees and state preserved.");
    println!("Run `git paw start` to recover.");
    Ok(())
}
