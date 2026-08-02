//! `git paw stop` — kill the tmux session while preserving worktrees and
//! state. Extracted verbatim from `main.rs` (code-analysis-refactor R2b).

use git_paw::agents;
use git_paw::error::PawError;
use git_paw::git;
use git_paw::session::{self, SessionStatus};
use git_paw::tmux;

/// Stops the session: kills tmux but preserves worktrees and state.
pub(crate) fn cmd_stop(_force: bool) -> Result<(), PawError> {
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

    println!("Session stopped. Worktrees and state preserved.");
    println!("Run `git paw start` to recover.");
    Ok(())
}
