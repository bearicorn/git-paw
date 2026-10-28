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
    /// The broker's wave-progress object, when a broker is reachable.
    ///
    /// The per-agent rows answer "is anyone alive"; this answers "has
    /// anything moved" — the question a roster of repeated `working`
    /// heartbeats cannot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave: Option<crate::mcp::query::events::WaveSnapshot>,
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
    let dir = session::sessions_dir().ok()?;
    session_status_in(ctx, &dir)
}

/// [`session_status`] reading session receipts from `sessions_dir`.
///
/// Mirrors `session::find_session_for_repo_in`: a test can seed a receipt in
/// a temporary directory and read it back without mutating `HOME` out from
/// under every other test sharing the process.
#[must_use]
pub fn session_status_in(
    ctx: &RepoContext,
    sessions_dir: &std::path::Path,
) -> Option<SessionSnapshot> {
    // Owner-resolving lookup: `git paw mcp` is routinely started inside an
    // agent worktree with no `--repo`, and a worktree root never equals the
    // repository root. Read-only, so widening the match is safe here — see
    // `session::find_session_for_repo` for why mutating commands must not.
    let session = session::find_session_for_repo_or_owner_in(&ctx.root, sessions_dir)
        .ok()
        .flatten()?;

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
        wave: ctx
            .broker_url
            .as_deref()
            .and_then(|url| crate::broker::publish::fetch_wave_over_http(url).ok())
            .flatten()
            .map(Into::into),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `mcp-server`'s "and SHALL omit it otherwise": a session with no
    /// reachable broker reports no `wave` at all, rather than a zeroed
    /// object a client could mistake for "idle since the epoch".
    #[test]
    fn the_wave_is_omitted_when_no_broker_is_reachable() {
        let repo = tempfile::tempdir().unwrap();
        let sessions = tempfile::tempdir().unwrap();
        let canonical = repo.path().canonicalize().unwrap();
        std::fs::write(
            sessions.path().join("paw-nobroker.json"),
            serde_json::json!({
                "session_name": "paw-nobroker",
                "repo_path": canonical.to_string_lossy(),
                "project_name": "nobroker",
                "created_at": "2026-05-01T00:00:00Z",
                "status": "active",
                "worktrees": [],
            })
            .to_string(),
        )
        .unwrap();

        let ctx = RepoContext {
            root: canonical,
            git_paw_dir: None,
            broker_url: None,
            server_name: "git-paw".to_string(),
            is_main_worktree: false,
            allow_repo_writes: false,
            authority: None,
        };
        let snapshot = session_status_in(&ctx, sessions.path()).expect("the session resolves");

        assert!(snapshot.wave.is_none());
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(!json.contains("wave"), "got {json}");
    }

    #[test]
    fn no_session_yields_none() {
        // A bare temp dir resolves no session receipt.
        let tmp = tempfile::tempdir().unwrap();
        let ctx = RepoContext {
            root: tmp.path().to_path_buf(),
            git_paw_dir: None,
            broker_url: None,
            server_name: "git-paw".to_string(),
            is_main_worktree: false,
            allow_repo_writes: false,
            authority: None,
        };
        assert!(session_status(&ctx).is_none());
    }

    /// broker-runtime "Status surfaces carry no credential": the MCP
    /// `get_session_status` snapshot of a session that holds a credential
    /// does not carry it.
    #[test]
    fn session_status_snapshot_carries_no_authority_credential() {
        let repo = tempfile::tempdir().unwrap();
        let sessions = tempfile::tempdir().unwrap();
        let canonical = repo.path().canonicalize().unwrap();
        let credential = "5ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7";
        std::fs::write(
            sessions.path().join("paw-cred.json"),
            serde_json::json!({
                "session_name": "paw-cred",
                "repo_path": canonical.to_string_lossy(),
                "project_name": "cred",
                "created_at": "2026-05-01T00:00:00Z",
                "status": "active",
                "worktrees": [],
                "broker_authority": credential,
            })
            .to_string(),
        )
        .unwrap();

        let ctx = RepoContext {
            root: canonical,
            git_paw_dir: None,
            broker_url: None,
            server_name: "git-paw".to_string(),
            is_main_worktree: false,
            allow_repo_writes: false,
            authority: None,
        };
        let snapshot = session_status_in(&ctx, sessions.path()).expect("the session resolves");
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(!json.contains(credential), "{json}");
        assert!(!json.contains("broker_authority"), "{json}");
    }
}
