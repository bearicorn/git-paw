//! Coordination tools: `get_intents`, `get_intent`, `get_conflicts`.
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
}
