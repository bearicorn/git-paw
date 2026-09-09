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
    /// Combined router across all tool categories. `pub(crate)` so registry-
    /// inspection tests (the `mcp-agent-publish` security boundary: no
    /// generic publish tool, no `agent_id` parameter, no authority-verb
    /// tools) can enumerate it from outside this module.
    pub(crate) tool_router: ToolRouter<Self>,
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
            + Self::source_router()
            + Self::publish_router();
        Self { ctx, tool_router }
    }
}

// The `#[tool_handler]` macro expands to an async trait method with no `.await`;
// silence clippy's 1.98 lint for the generated code. `unknown_lints` keeps this
// harmless on the older toolchains (MSRV 1.96) that do not know the lint yet.
#[allow(unknown_lints, clippy::unused_async_trait_impl)]
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
            "git-paw repository state over MCP: coordination intents/conflicts, governance docs, \
             specs and tasks, session status and learnings, agent skills, git context, and source \
             browsing (list_files, read_file, search_code over the local working tree) are all \
             read-only and return empty/null results (not errors) when their data source is \
             unavailable. The one write surface is a bounded, agent-scoped publish category \
             covering exactly the four agent boot events (publish_status, publish_artifact, \
             publish_blocked, publish_question) — each publishes as the calling agent only. \
             Supervisor authority verbs (agent.verified, agent.feedback) are not exposed."
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
            "publish_status",
            "publish_artifact",
            "publish_blocked",
            "publish_question",
        ] {
            assert!(
                names.iter().any(|n| n == expected),
                "tool {expected} should be registered; have: {names:?}"
            );
        }
    }

    /// The complete, closed set of tool names this server may ever
    /// advertise. `mcp-agent-publish` D2 requires the publish surface stay a
    /// FIXED set of four narrow tools; the guarantee only holds if the
    /// *whole* registry is closed, so this list covers every tool, read-only
    /// or not. Adding, renaming, or removing any tool must touch this list —
    /// deliberately, since that touch is the review speed bump the bounded-
    /// surface argument depends on (design "Risks / Trade-offs").
    const ALL_ALLOWED_TOOL_NAMES: &[&str] = &[
        // coordination (read-only)
        "get_intents",
        "get_intent",
        "get_conflicts",
        // governance (read-only)
        "get_dod",
        "get_constitution",
        "get_adrs",
        "get_adr",
        "get_test_strategy",
        "get_security_checklist",
        "check_dod",
        // project: specs/tasks/skills (read-only)
        "get_specs",
        "get_spec",
        "get_tasks",
        "get_task",
        "get_skill",
        "get_dependency_graph",
        // session state (read-only)
        "get_session_status",
        "get_session_summary",
        "get_learnings",
        // git context (read-only)
        "get_branches",
        "get_recent_commits",
        "get_diff",
        // docs (read-only)
        "get_readme",
        "list_docs",
        "get_doc",
        // source browsing (read-only)
        "list_files",
        "read_file",
        "search_code",
        // mcp-agent-publish: the ONLY write category — bounded to exactly
        // the four agent boot events, agent-scoped, authority verbs absent.
        "publish_status",
        "publish_artifact",
        "publish_blocked",
        "publish_question",
    ];

    /// `mcp-server` scenario "No file or git mutation is exposed" + `mcp-
    /// agent-publish` scenario "No generic publish tool exists": the
    /// registry is exactly the closed allowlist above — nothing more,
    /// nothing less. A stray extra tool (mutating or not) fails this test
    /// immediately, forcing a conscious update rather than a silent drift.
    #[test]
    fn registry_is_exactly_the_closed_allowlist() {
        let server = GitPawMcpServer::new(ctx());
        let names: std::collections::BTreeSet<String> = server
            .tool_router
            .list_all()
            .into_iter()
            .map(|t| t.name.to_string())
            .collect();
        let allowed: std::collections::BTreeSet<String> = ALL_ALLOWED_TOOL_NAMES
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        assert_eq!(
            names, allowed,
            "the tool registry drifted from the closed allowlist — any addition or removal \
             must be a deliberate edit to ALL_ALLOWED_TOOL_NAMES, not an incidental one"
        );
    }

    /// `mcp-agent-publish` scenarios "No generic publish tool exists", "No
    /// verified publish tool", "No feedback publish tool", and "Agent id is
    /// derived, not supplied": adversarially inspect every advertised tool's
    /// name and parameter schema, not just the four publish tools by name.
    #[test]
    fn no_generic_publish_or_authority_verb_tool_is_advertised() {
        let server = GitPawMcpServer::new(ctx());
        for tool in server.tool_router.list_all() {
            assert_ne!(
                tool.name, "publish",
                "a generic publish(type, payload) tool must never exist (D2)"
            );
            assert!(
                !tool.name.to_lowercase().contains("verified"),
                "no tool may publish agent.verified (D1): {}",
                tool.name
            );
            assert!(
                !tool.name.to_lowercase().contains("feedback"),
                "no tool may publish agent.feedback (D1): {}",
                tool.name
            );
            // Inspect the schema's declared *properties* (parameter names),
            // not the raw JSON text — the schema's own `"type"` keyword
            // (`"type":"object"`/`"array"`/…) appears on every tool and must
            // not be confused with a parameter literally named `type`.
            let props = tool
                .input_schema
                .get("properties")
                .and_then(serde_json::Value::as_object);
            let has_property = |name: &str| props.is_some_and(|p| p.contains_key(name));
            assert!(
                !has_property("agent_id"),
                "{} schema must not accept an agent_id parameter (D3): {:?}",
                tool.name,
                tool.input_schema
            );
            if tool.name.starts_with("publish_") {
                assert!(
                    !has_property("type") && !has_property("message_type"),
                    "{} schema must not accept a generic message-type parameter (D2): {:?}",
                    tool.name,
                    tool.input_schema
                );
            }
        }
    }

    /// `mcp-agent-publish` scenario "Agent cannot self-verify via MCP": even
    /// granting the strongest available capability (calling every advertised
    /// tool), there is no tool call sequence that publishes an
    /// `agent.verified` message — the verb simply is not on the surface.
    #[test]
    fn agent_cannot_self_verify_because_no_tool_publishes_it() {
        let server = GitPawMcpServer::new(ctx());
        let names: Vec<String> = server
            .tool_router
            .list_all()
            .into_iter()
            .map(|t| t.name.to_string())
            .collect();
        let publish_tools: Vec<&String> =
            names.iter().filter(|n| n.starts_with("publish_")).collect();
        assert_eq!(
            publish_tools.len(),
            4,
            "exactly four publish tools should exist; have: {publish_tools:?}"
        );
        for allowed in [
            "publish_status",
            "publish_artifact",
            "publish_blocked",
            "publish_question",
        ] {
            assert!(
                publish_tools.iter().any(|n| n.as_str() == allowed),
                "expected publish tool {allowed} missing from the bounded set"
            );
        }
    }
}
