//! `git paw pause` — detach and pause the session while leaving CLI panes
//! running. Extracted verbatim from `main.rs` (code-analysis-refactor R2b).

use git_paw::error::PawError;
use git_paw::git;
use git_paw::session::{self, SessionStatus};
use git_paw::surface::SessionLayout;
use git_paw::terminal::TerminalBackend;
use git_paw::terminal::tmux::TmuxBackend;
use git_paw::tmux;

use super::helpers::BROKER_SERVICE;

/// Pauses the session: detaches the user's tmux client, stops the broker
/// (by closing its host's service surface), and updates session status to
/// `Paused`. The dashboard and the CLI panes keep running; the CLIs retain
/// their in-memory state.
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
    let effective = existing.effective_status(|name| {
        matches!(
            tmux::session_liveness_for(name, existing.created_at, existing.agent_pane_offset()),
            tmux::SessionLiveness::Alive
        )
    });
    if effective == SessionStatus::Stopped {
        println!(
            "Session '{}' is already stopped; pause has no effect.",
            existing.session_name
        );
        return Ok(());
    }

    // Detach the user's tmux client. Idempotent in detach_client.
    tmux::detach_client(&existing.session_name)?;

    // Stop the broker by closing its host's service surface: the host gets
    // SIGHUP, drops its BrokerHandle, and the broker shuts down gracefully
    // with a final log flush. The dashboard stays, showing the broker as
    // unreachable until `git paw start` brings the host back. A session
    // started before the host split has no such surface — its dashboard pane
    // IS the broker — so it falls back to killing that pane as before.
    if existing.broker_port.is_some() {
        let backend = TmuxBackend::new();
        let host = backend.service(&existing.session_name, BROKER_SERVICE);
        if backend.service_alive(&host)? {
            backend.stop_service(&host)?;
        } else {
            let pane_idx = SessionLayout::of(&existing).dashboard_index_or_default();
            tmux::kill_pane(&existing.session_name, pane_idx)?;
        }
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
