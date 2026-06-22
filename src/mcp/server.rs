//! stdio transport setup, tool-registry wiring, and lifecycle for the MCP
//! server. This module only *wires* things together (design D2): it owns no
//! tool logic (that lives in [`crate::mcp::tools`]) and no data reads (those
//! live in [`crate::mcp::query`]).

use std::path::Path;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::transport::stdio;
use rmcp::{ServerHandler, ServiceExt, tool_handler};

use crate::error::PawError;
use crate::mcp::{RepoContext, logging};

/// The MCP server handler. Holds the resolved [`RepoContext`] (shared
/// read-only by every tool) and the merged tool router.
#[derive(Clone)]
pub struct GitPawMcpServer {
    /// Resolved repository context.
    pub(crate) ctx: RepoContext,
    /// Combined router across all five tool categories.
    tool_router: ToolRouter<Self>,
}

impl GitPawMcpServer {
    /// Builds the server, merging the per-category tool routers (each defined
    /// in its own file under `tools/`).
    #[must_use]
    pub fn new(ctx: RepoContext) -> Self {
        let tool_router = Self::coordination_router()
            + Self::governance_router()
            + Self::project_router()
            + Self::session_router()
            + Self::git_router()
            + Self::docs_router()
            + Self::source_router();
        Self { ctx, tool_router }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for GitPawMcpServer {
    fn get_info(&self) -> ServerInfo {
        // ServerInfo (InitializeResult) is #[non_exhaustive]; build from Default
        // and override the fields we care about.
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        // server_info (an Implementation) defaults to the rmcp SDK's own
        // identity ("rmcp" / the rmcp crate version). Override it so the
        // handshake advertises git-paw and its real crate version, honouring
        // any configured [mcp].name resolved onto the RepoContext.
        info.server_info.name.clone_from(&self.ctx.server_name);
        info.server_info.version = env!("CARGO_PKG_VERSION").to_string();
        info.instructions = Some(
            "Read-only git-paw repository state over MCP: coordination intents/conflicts, \
             governance docs, specs and tasks, session status and learnings, agent skills, \
             git context, and source browsing (list_files, read_file, search_code over the \
             local working tree). Tools return empty/null results (not errors) when their data \
             source is unavailable."
                .to_string(),
        );
        info
    }
}

/// Validates configuration that must be correct for the server to operate.
///
/// A configured `[specs].type` outside the supported set is a hard error per
/// the spec — the server exits non-zero with a clear stderr message rather
/// than silently mis-serving.
fn validate_startup_config(ctx: &RepoContext) -> Result<(), PawError> {
    let config = crate::config::load_config(&ctx.root, None)?;
    if let Some(specs) = config.specs.as_ref()
        && let Some(spec_type) = specs.spec_type.as_deref()
    {
        const VALID: [&str; 3] = ["openspec", "markdown", "speckit"];
        if !VALID.contains(&spec_type) {
            return Err(PawError::McpError(format!(
                "invalid [specs].type = \"{spec_type}\" in .git-paw/config.toml. \
                 Valid values: openspec, markdown, speckit."
            )));
        }
    }
    Ok(())
}

/// Runs the stdio MCP server until the client closes stdin.
///
/// Initialises stderr logging, validates startup config, then drives the
/// rmcp service loop on a Tokio runtime. Returns `Ok(())` (exit 0) on a clean
/// stdin EOF.
pub fn run(ctx: RepoContext, log_file: Option<&Path>) -> Result<(), PawError> {
    logging::init(log_file)?;
    validate_startup_config(&ctx)?;

    logging::info(&format!("serving repository {}", ctx.root.display()));
    if ctx.broker_url.is_none() {
        logging::info("no active broker; coordination/session tools will return empty results");
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| PawError::McpError(format!("failed to build async runtime: {e}")))?;

    runtime.block_on(async move {
        let server = GitPawMcpServer::new(ctx);
        let service = server
            .serve(stdio())
            .await
            .map_err(|e| PawError::McpError(format!("failed to start MCP server: {e}")))?;
        let reason = service
            .waiting()
            .await
            .map_err(|e| PawError::McpError(format!("MCP server loop error: {e}")))?;
        logging::info(&format!("MCP server stopped: {reason:?}"));
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> RepoContext {
        ctx_named("git-paw")
    }

    fn ctx_named(name: &str) -> RepoContext {
        RepoContext {
            root: std::path::PathBuf::from("/tmp"),
            git_paw_dir: None,
            broker_url: None,
            server_name: name.to_string(),
        }
    }

    #[test]
    fn server_advertises_tool_capability_and_instructions() {
        let server = GitPawMcpServer::new(ctx());
        let info = server.get_info();
        assert!(
            info.capabilities.tools.is_some(),
            "tools capability advertised"
        );
        assert!(info.instructions.is_some());
    }

    // mcp-server "Server identity" — Scenario: Default identity is git-paw.
    #[test]
    fn server_identity_defaults_to_git_paw_with_crate_version() {
        let server = GitPawMcpServer::new(ctx());
        let info = server.get_info();
        assert_eq!(info.server_info.name, "git-paw");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
    }

    // mcp-server "Server identity" — Scenario: Configured name overrides the
    // advertised identity (version stays the crate version).
    #[test]
    fn server_identity_uses_configured_name_keeping_crate_version() {
        let server = GitPawMcpServer::new(ctx_named("my-project"));
        let info = server.get_info();
        assert_eq!(info.server_info.name, "my-project");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
    }

    // The SDK default identity ("rmcp") must never leak through.
    #[test]
    fn server_identity_is_not_the_sdk_default() {
        let server = GitPawMcpServer::new(ctx());
        let info = server.get_info();
        assert_ne!(info.server_info.name, "rmcp");
    }

    #[test]
    fn new_merges_all_category_routers() {
        let server = GitPawMcpServer::new(ctx());
        let names: Vec<String> = server
            .tool_router
            .list_all()
            .into_iter()
            .map(|t| t.name.to_string())
            .collect();
        // Spot-check one tool from each category is registered.
        for expected in [
            "get_intents",
            "get_conflicts",
            "get_dod",
            "get_constitution",
            "get_specs",
            "get_skill",
            "get_session_status",
            "get_learnings",
            "get_branches",
            "get_diff",
            "get_readme",
            "list_docs",
            "get_doc",
            "list_files",
            "read_file",
            "search_code",
        ] {
            assert!(
                names.iter().any(|n| n == expected),
                "tool {expected} should be registered; have: {names:?}"
            );
        }
    }
}
