//! Governance tools: `get_adrs`, `get_adr`, `get_test_strategy`,
//! `get_security_checklist`, `get_dod`, `check_dod`, `get_constitution`.
//!
//! These serve the documents configured under `[governance]`. Unset paths
//! degrade to null / empty; a configured path that exists but cannot be read
//! surfaces as a JSON-RPC error so the client can tell the user to fix their
//! config.

use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::config::GovernanceConfig;
use crate::error::PawError;
use crate::mcp::query;
use crate::mcp::server::GitPawMcpServer;

/// Maps an internal error to an MCP protocol error. Takes the error by value
/// so it composes directly with `Result::map_err`.
#[allow(clippy::needless_pass_by_value)]
fn to_err(e: PawError) -> ErrorData {
    ErrorData::internal_error(e.to_string(), None)
}

/// Parameters for [`GitPawMcpServer::get_adr`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetAdrParams {
    /// Case-insensitive query matched against ADR id, title, and body.
    pub query: String,
}

/// Parameters for [`GitPawMcpServer::check_dod`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CheckDodParams {
    /// Branch the Definition-of-Done is being evaluated for (recorded in the
    /// response; checkbox state is read from the configured `DoD` file).
    pub branch: String,
}

/// Response carrying a single optional document body.
#[derive(Serialize, schemars::JsonSchema)]
pub struct DocResponse {
    /// Document content, or null when unset.
    pub content: Option<String>,
}

/// Response for `get_adrs`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct AdrsResponse {
    /// Discovered ADRs.
    pub adrs: Vec<query::governance::Adr>,
}

/// Response for `get_adr`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct AdrResponse {
    /// Matching ADR, or null.
    pub adr: Option<query::governance::AdrDetail>,
}

/// Response for `check_dod`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct CheckDodResponse {
    /// Branch the check was requested for.
    pub branch: String,
    /// Parsed `DoD` items, or null when no `DoD` is configured.
    pub items: Option<Vec<query::governance::DodItem>>,
}

impl GitPawMcpServer {
    fn governance(&self) -> Result<GovernanceConfig, ErrorData> {
        query::governance::load(&self.ctx.root).map_err(to_err)
    }
}

#[tool_router(router = governance_router, vis = "pub(crate)")]
impl GitPawMcpServer {
    /// `get_adrs` — list ADRs under the configured directory.
    #[tool(
        description = "List Architecture Decision Records under the configured [governance].adr \
                       directory. Each carries id, title, path, and status. Empty when unset."
    )]
    pub(crate) fn get_adrs(&self) -> Result<Json<AdrsResponse>, ErrorData> {
        let gov = self.governance()?;
        Ok(Json(AdrsResponse {
            adrs: query::governance::adrs(&self.ctx.root, &gov),
        }))
    }

    /// `get_adr` — a single ADR matched by query, with full content.
    #[tool(
        description = "Return a single ADR matching the query (over id/title/body) with its full \
                       Markdown content, or { \"adr\": null } when none matches."
    )]
    pub(crate) fn get_adr(
        &self,
        Parameters(p): Parameters<GetAdrParams>,
    ) -> Result<Json<AdrResponse>, ErrorData> {
        let gov = self.governance()?;
        Ok(Json(AdrResponse {
            adr: query::governance::adr(&self.ctx.root, &gov, &p.query),
        }))
    }

    /// `get_test_strategy` — the configured test-strategy document.
    #[tool(
        description = "Return the configured [governance].test_strategy document content, or null \
                       when unset. Errors only if the configured file is unreadable."
    )]
    pub(crate) fn get_test_strategy(&self) -> Result<Json<DocResponse>, ErrorData> {
        let gov = self.governance()?;
        let content = query::governance::single_doc(&self.ctx.root, gov.test_strategy.as_deref())
            .map_err(to_err)?;
        Ok(Json(DocResponse { content }))
    }

    /// `get_security_checklist` — the configured security checklist document.
    #[tool(
        description = "Return the configured [governance].security checklist content, or null when \
                       unset. Errors only if the configured file is unreadable."
    )]
    pub(crate) fn get_security_checklist(&self) -> Result<Json<DocResponse>, ErrorData> {
        let gov = self.governance()?;
        let content = query::governance::single_doc(&self.ctx.root, gov.security.as_deref())
            .map_err(to_err)?;
        Ok(Json(DocResponse { content }))
    }

    /// `get_dod` — the configured Definition-of-Done document.
    #[tool(
        description = "Return the configured [governance].dod (Definition of Done) document \
                       content, or null when unset. Errors only if the file is unreadable."
    )]
    pub(crate) fn get_dod(&self) -> Result<Json<DocResponse>, ErrorData> {
        let gov = self.governance()?;
        let content =
            query::governance::single_doc(&self.ctx.root, gov.dod.as_deref()).map_err(to_err)?;
        Ok(Json(DocResponse { content }))
    }

    /// `check_dod` — per-item completion state of the configured `DoD` checklist.
    #[tool(
        description = "Parse the configured Definition-of-Done checklist and return each item with \
                       its completion state as written in the file (no LLM judgment). \
                       { \"items\": null } when no DoD is configured."
    )]
    pub(crate) fn check_dod(
        &self,
        Parameters(p): Parameters<CheckDodParams>,
    ) -> Result<Json<CheckDodResponse>, ErrorData> {
        let gov = self.governance()?;
        let items = query::governance::check_dod(&self.ctx.root, &gov).map_err(to_err)?;
        Ok(Json(CheckDodResponse {
            branch: p.branch,
            items,
        }))
    }

    /// `get_constitution` — the configured project constitution.
    #[tool(
        description = "Return the configured [governance].constitution document content (e.g. Spec \
                       Kit's constitution.md), or null when unset. Errors only if unreadable."
    )]
    pub(crate) fn get_constitution(&self) -> Result<Json<DocResponse>, ErrorData> {
        let gov = self.governance()?;
        let content = query::governance::single_doc(&self.ctx.root, gov.constitution.as_deref())
            .map_err(to_err)?;
        Ok(Json(DocResponse { content }))
    }
}
