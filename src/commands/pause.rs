//! `git paw pause` — detach and pause the session while leaving CLI panes
//! running. Extracted verbatim from `main.rs` (code-analysis-refactor R2b).

use git_paw::error::PawError;
use git_paw::git;
use git_paw::session::{self, SessionStatus};
use git_paw::tmux;

/// Pauses the session: detaches the user's tmux client, stops the broker
/// (by killing the dashboard pane only), and updates session status to
/// `Paused`. CLI panes keep running and retain their in-memory state.
///
/// Idempotent: pausing an already-paused or already-stopped session is
/// a no-op with a friendly message.
pub(crate) fn cmd_pause() -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    let Some(existing) = session::find_session_for_repo(&repo_root)? else {
        println!("No active session for this repo.");
        return Ok(());
    };

    // Idempotency: already paused.
    if existing.status == SessionStatus::Paused {
        println!("Session '{}' is already paused.", existing.session_name);
        return Ok(());
    }

    // Effective status check: stopped sessions can't be paused.
    let effective = existing.effective_status(|name| tmux::is_session_alive(name).unwrap_or(false));
    if effective == SessionStatus::Stopped {
        println!(
            "Session '{}' is already stopped; pause has no effect.",
            existing.session_name
        );
        return Ok(());
    }

    // Detach the user's tmux client. Idempotent in detach_client.
    tmux::detach_client(&existing.session_name)?;

    // Kill the dashboard pane only (which hosts the broker subprocess);
    // the BrokerHandle drop runs and broker shuts down gracefully. Only
    // applies when broker was enabled — without a broker there's no
    // dashboard pane to kill.
    if existing.broker_port.is_some() {
        let pane_idx = existing.dashboard_pane.unwrap_or(0);
        tmux::kill_pane(&existing.session_name, pane_idx)?;
    }

    let cli_pane_count = existing.worktrees.len();
    let session_name = existing.session_name.clone();

    let mut updated = existing;
    updated.status = SessionStatus::Paused;
    session::save_session(&updated)?;

    println!(
        "Session '{session_name}' paused. {cli_pane_count} CLI pane(s) still running. \
         Run 'git paw start' to resume."
    );
    Ok(())
}
