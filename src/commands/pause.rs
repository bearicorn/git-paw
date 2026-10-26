//! `git paw pause` — detach and pause the session while leaving CLI panes
//! running.

use crate::error::PawError;
use crate::session::{self, SessionStatus};
use crate::surface::SessionLayout;
use crate::terminal::TerminalBackend;
use crate::terminal::tmux::TmuxBackend;
use crate::tmux;

use super::helpers::BROKER_SERVICE;
use super::{CommandContext, Output};

/// Options for [`pause`]. `git paw pause` takes none.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PauseOptions {}

/// What [`pause`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PauseOutcome {
    /// The repository has no session.
    NoSession,
    /// The session was already paused; nothing changed.
    AlreadyPaused {
        /// The session.
        session_name: String,
    },
    /// The session is stopped, so there is nothing to pause.
    AlreadyStopped {
        /// The session.
        session_name: String,
    },
    /// The session was paused.
    Paused {
        /// The session.
        session_name: String,
        /// How many coding-agent panes are still running.
        cli_panes: usize,
    },
}

/// Pauses the session: detaches the user's tmux client, stops the broker
/// (by closing its host's service surface), and updates session status to
/// `Paused`. The dashboard and the CLI panes keep running; the CLIs retain
/// their in-memory state.
///
/// Idempotent: pausing an already-paused or already-stopped session is
/// a no-op with a friendly message.
///
/// # Errors
///
/// Returns an error when the session state cannot be read or saved, or a
/// tmux operation fails.
pub fn pause(
    ctx: &CommandContext,
    _opts: PauseOptions,
    out: &mut dyn Output,
) -> Result<PauseOutcome, PawError> {
    let Some(existing) = session::find_session_for_repo(ctx.repo_root())? else {
        out.line("No active session for this repo.");
        return Ok(PauseOutcome::NoSession);
    };

    // Idempotency: already paused.
    if existing.status == SessionStatus::Paused {
        out.line(&format!(
            "Session '{}' is already paused.",
            existing.session_name
        ));
        return Ok(PauseOutcome::AlreadyPaused {
            session_name: existing.session_name,
        });
    }

    // Effective status check: stopped sessions can't be paused.
    let effective = existing.effective_status(|name| {
        matches!(
            tmux::session_liveness_for(name, existing.created_at, existing.agent_pane_offset()),
            tmux::SessionLiveness::Alive
        )
    });
    if effective == SessionStatus::Stopped {
        out.line(&format!(
            "Session '{}' is already stopped; pause has no effect.",
            existing.session_name
        ));
        return Ok(PauseOutcome::AlreadyStopped {
            session_name: existing.session_name,
        });
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

    out.line(&format!(
        "Session '{session_name}' paused. {cli_pane_count} CLI pane(s) still running. \
         Run 'git paw start' to resume."
    ));
    Ok(PauseOutcome::Paused {
        session_name,
        cli_panes: cli_pane_count,
    })
}
