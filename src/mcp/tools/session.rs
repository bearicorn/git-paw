//! Session-state tools: `get_session_status`, `get_session_summary`,
//! `get_learnings`.
//!
//! These read the active/most-recent session receipt and the learnings file.
//! A null session and empty learning sections are returned (not errors) when
//! no session is active.

use std::collections::BTreeMap;

use rmcp::handler::server::wrapper::Json;
use rmcp::{schemars, tool, tool_router};
use serde::Serialize;

use crate::mcp::query;
use crate::mcp::server::GitPawMcpServer;

/// Response for `get_session_status`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SessionStatusResponse {
    /// Active session snapshot, or null.
    pub session: Option<query::session::SessionSnapshot>,
}

/// Compact session summary.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SessionSummary {
    /// Session name.
    pub name: String,
    /// Session status.
    pub status: String,
    /// Number of registered agents.
    pub agent_count: usize,
    /// Counts of agents per live status label.
    pub agents_by_status: BTreeMap<String, usize>,
}

/// Response for `get_session_summary`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SummaryResponse {
    /// Session summary, or null.
    pub summary: Option<SessionSummary>,
}

/// Response for `get_learnings`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct LearningsResponse {
    /// Parsed learning sections.
    pub sections: Vec<query::learnings::LearningSection>,
}

#[tool_router(router = session_router, vis = "pub(crate)")]
impl GitPawMcpServer {
    /// `get_session_status` — the active session snapshot.
    #[tool(
        description = "Return the active session snapshot: name, mode, status, agent count, broker \
                       URL, pause state, and per-agent status (live from the broker when reachable). \
                       { \"session\": null } when no session is active."
    )]
    pub(crate) fn get_session_status(&self) -> Json<SessionStatusResponse> {
        Json(SessionStatusResponse {
            session: query::session::session_status(&self.ctx),
        })
    }

    /// `get_session_summary` — a compact one-object session summary.
    #[tool(
        description = "Return a compact summary of the current session (name, status, agent count, \
                       and per-status agent counts), or { \"summary\": null } when none is active."
    )]
    pub(crate) fn get_session_summary(&self) -> Json<SummaryResponse> {
        let summary = query::session::session_status(&self.ctx).map(|s| {
            let mut by_status: BTreeMap<String, usize> = BTreeMap::new();
            for a in &s.agents {
                if !a.status.is_empty() {
                    *by_status.entry(a.status.clone()).or_default() += 1;
                }
            }
            SessionSummary {
                name: s.name,
                status: s.status,
                agent_count: s.agent_count,
                agents_by_status: by_status,
            }
        });
        Json(SummaryResponse { summary })
    }

    /// `get_learnings` — parsed session-learnings sections.
    #[tool(
        description = "Parse .git-paw/session-learnings.md into structured sections, each with a \
                       category and its entries. Returns the canonical sections as empty arrays \
                       when no learnings file exists."
    )]
    pub(crate) fn get_learnings(&self) -> Json<LearningsResponse> {
        Json(LearningsResponse {
            sections: query::learnings::learnings(&self.ctx),
        })
    }
}
