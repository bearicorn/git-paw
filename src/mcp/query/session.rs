//! Session-state reads.
//!
//! Reads the active/most-recent session receipt for the repository and, when a
//! broker is reachable, enriches per-agent rows with live status from the
//! broker `/status` endpoint. Returns `None` (null session) when no session is
//! active.

use rmcp::schemars;
use serde::Serialize;

use crate::coordination::inventory::fetch_status_agents_over_http;
use crate::mcp::RepoContext;
use crate::session::{self, SessionStatus};

/// Per-agent row in a session snapshot.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct AgentRow {
    /// Branch / agent id.
    pub branch: String,
    /// CLI running in the agent's pane.
    pub cli: String,
    /// Live status label from the broker (empty when broker unreachable).
    pub status: String,
    /// Seconds since the agent was last seen (None when broker unreachable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen_seconds: Option<u64>,
}

/// A session snapshot.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SessionSnapshot {
    /// tmux session name.
    pub name: String,
    /// Session mode ("bare" or "supervisor").
    pub mode: String,
    /// Session status ("active" / "paused" / "stopped").
    pub status: String,
    /// Whether the session is paused.
    pub paused: bool,
    /// Number of registered agent worktrees.
    pub agent_count: usize,
    /// Broker base URL, when a broker is configured for the session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub broker_url: Option<String>,
    /// Per-agent rows.
    pub agents: Vec<AgentRow>,
}

fn status_label(status: &SessionStatus) -> &'static str {
    match status {
        SessionStatus::Active => "active",
        SessionStatus::Paused => "paused",
        SessionStatus::Stopped => "stopped",
    }
}

/// Returns the session snapshot for the repository, or `None` when no session
/// exists.
#[must_use]
pub fn session_status(ctx: &RepoContext) -> Option<SessionSnapshot> {
    let session = session::find_session_for_repo(&ctx.root).ok().flatten()?;

    // Live per-agent status, if the broker is reachable.
    let live = ctx
        .broker_url
        .as_deref()
        .and_then(|url| fetch_status_agents_over_http(url).ok())
        .unwrap_or_default();

    let agents = session
        .worktrees
        .iter()
        .map(|w| {
            let row = live.iter().find(|a| a.agent_id == w.branch);
            AgentRow {
                branch: w.branch.clone(),
                cli: w.cli.clone(),
                status: row.map(|r| r.status.clone()).unwrap_or_default(),
                last_seen_seconds: row.map(|r| r.last_seen_seconds),
            }
        })
        .collect::<Vec<_>>();

    let mode = match session.mode {
        crate::session::SessionMode::Bare => "bare",
        crate::session::SessionMode::Supervisor => "supervisor",
    };

    let paused = session.status == SessionStatus::Paused;
    Some(SessionSnapshot {
        name: session.session_name.clone(),
        mode: mode.to_string(),
        status: status_label(&session.status).to_string(),
        paused,
        agent_count: session.worktrees.len(),
        broker_url: ctx.broker_url.clone(),
        agents,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_session_yields_none() {
        // A bare temp dir resolves no session receipt.
        let tmp = tempfile::tempdir().unwrap();
        let ctx = RepoContext {
            root: tmp.path().to_path_buf(),
            git_paw_dir: None,
            broker_url: None,
            server_name: "git-paw".to_string(),
        };
        assert!(session_status(&ctx).is_none());
    }
}
