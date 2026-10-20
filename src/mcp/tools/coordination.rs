//! Coordination tools: `get_intents`, `get_intent`, `get_conflicts`,
//! `wait_for_events`.
//!
//! These read live agent-coordination state from the broker. Every tool here
//! degrades to empty arrays / null (never an error) when no broker is running.

use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::mcp::query;
use crate::mcp::server::GitPawMcpServer;

/// Parameters for [`GitPawMcpServer::get_intent`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetIntentParams {
    /// Branch id (agent id) whose active intent to look up.
    pub branch_id: String,
}

/// Response for `get_intents`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct IntentsResponse {
    /// Active intents.
    pub intents: Vec<query::intents::Intent>,
}

/// Response for `get_intent`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct IntentResponse {
    /// Matching active intent, or null.
    pub intent: Option<query::intents::Intent>,
}

/// Response for `get_conflicts`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct ConflictsResponse {
    /// Detected conflicts.
    pub conflicts: Vec<query::conflicts::Conflict>,
}

/// Parameters for [`GitPawMcpServer::wait_for_events`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WaitForEventsParams {
    /// Broker log sequence number to read after. Pass the `last_seq` from
    /// the previous call; `0` (the default) reads from the beginning.
    #[serde(default)]
    pub since: u64,
    /// How long to wait for a new entry, in seconds. Defaults to 30 and is
    /// capped at 60.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

#[tool_router(router = coordination_router, vis = "pub(crate)")]
impl GitPawMcpServer {
    /// `get_intents` — every active intent for this repo's session.
    #[tool(
        description = "List all active agent coordination intents for this repository's session. \
                       Each intent carries branch_id, files, summary, published_at, and \
                       valid_for_seconds. Returns an empty list when no broker/session is active."
    )]
    pub(crate) fn get_intents(&self) -> Json<IntentsResponse> {
        Json(IntentsResponse {
            intents: query::intents::active_intents(&self.ctx),
        })
    }

    /// `get_intent` — a single agent's active intent by branch id.
    #[tool(description = "Look up a single agent's active intent by branch_id. \
                       Returns { \"intent\": null } when no matching active intent exists.")]
    pub(crate) fn get_intent(
        &self,
        Parameters(p): Parameters<GetIntentParams>,
    ) -> Json<IntentResponse> {
        Json(IntentResponse {
            intent: query::intents::intent_for(&self.ctx, &p.branch_id),
        })
    }

    /// `get_conflicts` — all currently-detected coordination conflicts.
    #[tool(
        description = "List all currently-detected coordination conflicts between agents (forward \
                       overlaps on declared files/regions). Each carries shape, branches, files, \
                       and detected_at. Returns an empty list when no broker/session is active."
    )]
    pub(crate) fn get_conflicts(&self) -> Json<ConflictsResponse> {
        Json(ConflictsResponse {
            conflicts: query::conflicts::conflicts(&self.ctx),
        })
    }

    /// `wait_for_events` — long-poll the broker's log cursor.
    ///
    /// Read-only: it reads `GET /log` and `GET /status` and changes nothing
    /// in the broker or the repository.
    #[tool(
        description = "Wait for new agent-coordination events. Long-polls the session broker's \
                       message log from the `since` cursor and returns { entries, last_seq, wave \
                       }: the entries published after `since`, the new cursor to pass next time, \
                       and the wave's progress — so a wait that times out still reports whether \
                       the wave has stalled. `timeout_seconds` defaults to 30 and is capped at \
                       60. Returns at once with no_session=true and an unchanged cursor when no \
                       broker/session is active. Read-only."
    )]
    pub(crate) async fn wait_for_events(
        &self,
        Parameters(p): Parameters<WaitForEventsParams>,
    ) -> Json<query::events::EventsSnapshot> {
        let timeout = query::events::resolve_timeout_seconds(p.timeout_seconds);
        // The read underneath is a BLOCKING socket read held for up to the
        // full timeout. Every other tool here returns in milliseconds, so
        // running this one on a worker thread would park that thread for a
        // minute — two concurrent waits can wedge a small runtime. Hand it
        // to the blocking pool, which exists for exactly this.
        let ctx = self.ctx.clone();
        let snapshot = tokio::task::spawn_blocking(move || {
            query::events::wait_for_events(&ctx, p.since, timeout)
        })
        .await
        .unwrap_or_else(|_| query::events::EventsSnapshot::unavailable(p.since));
        Json(snapshot)
    }
}
