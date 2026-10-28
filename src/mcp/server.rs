//! stdio transport setup, tool-registry wiring, and lifecycle for the MCP
//! server. This module only *wires* things together (design D2): it owns no
//! tool logic (that lives in [`crate::mcp::tools`]) and no data reads (those
//! live in [`crate::mcp::query`]).

use std::path::Path;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolRequestParams, CallToolResult, ServerCapabilities, ServerInfo};
use rmcp::service::RequestContext;
use rmcp::transport::stdio;
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt, tool_handler};

use crate::error::PawError;
use crate::mcp::{RepoContext, logging, repo_write};
use crate::specs::SpecBackendKind;

/// The tools offered to a root session only — the session lifecycle and the
/// verdict publish tools. A server resolved to a linked worktree registers
/// none of them, and answers a by-name call to one with an error naming the
/// restriction.
pub(crate) const ROOT_ONLY_TOOL_NAMES: &[&str] = &[
    "start_session",
    "pause_session",
    "stop_session",
    "purge_session",
    "publish_verified",
    "publish_feedback",
];

/// The tools that write file content into the repository working tree. They
/// exist only when [`repo_write::repo_write_access`] passes — a root session
/// of a repository whose own config sets `[mcp] allow_repo_writes = true` —
/// and a by-name call otherwise gets the refusal's message.
pub(crate) const REPO_WRITE_TOOL_NAMES: &[&str] = &["create_spec", "write_spec_artifact"];

/// The MCP server handler. Holds the resolved [`RepoContext`] (shared
/// read-only by every tool) and the merged tool router.
#[derive(Clone)]
pub struct GitPawMcpServer {
    /// Resolved repository context.
    pub(crate) ctx: RepoContext,
    /// Combined router across all tool categories. `pub(crate)` so registry-
    /// inspection tests (the `mcp-agent-publish` security boundary: no
    /// generic publish tool, no `agent_id` parameter, no authority-verb
    /// tools in a worktree session) can enumerate it from outside this
    /// module.
    pub(crate) tool_router: ToolRouter<Self>,
}

impl GitPawMcpServer {
    /// Builds the server, merging the per-category tool routers (each defined
    /// in its own file under `tools/`). The lifecycle and verdict routers are
    /// merged only for a root session ([`RepoContext::is_main_worktree`]), so
    /// a worktree server neither advertises nor dispatches them. The
    /// spec-authoring write router is merged only when the repository-write
    /// gate passes.
    #[must_use]
    pub fn new(ctx: RepoContext) -> Self {
        let mut tool_router = Self::coordination_router()
            + Self::governance_router()
            + Self::project_router()
            + Self::session_router()
            + Self::git_router()
            + Self::docs_router()
            + Self::source_router()
            + Self::publish_router()
            + Self::spec_check_router();
        if ctx.is_main_worktree {
            tool_router = tool_router + Self::lifecycle_router() + Self::verdict_router();
        }
        if repo_write::repo_write_access(ctx.is_main_worktree, ctx.allow_repo_writes).is_ok() {
            tool_router += Self::spec_write_router();
        }
        Self { ctx, tool_router }
    }
}

/// The error a worktree session gets for a by-name call to a root-only tool.
fn root_only_error(name: &str) -> ErrorData {
    ErrorData::invalid_request(
        format!(
            "`{name}` is available only from the repository's main working tree (a root \
             session); this server runs in a linked worktree"
        ),
        None,
    )
}

// The `#[tool_handler]` macro expands to async trait methods with no `.await`;
// silence clippy's 1.98 lint for the generated code. `unknown_lints` keeps this
// harmless on the older toolchains (MSRV 1.96) that do not know the lint yet.
#[allow(unknown_lints, clippy::unused_async_trait_impl)]
#[tool_handler(router = self.tool_router)]
impl ServerHandler for GitPawMcpServer {
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        // A worktree server has no route for a root-only tool; say why
        // instead of the generic "tool not found".
        if !self.ctx.is_main_worktree && ROOT_ONLY_TOOL_NAMES.contains(&request.name.as_ref()) {
            return Err(root_only_error(&request.name));
        }
        // An unadvertised repository-write tool: say which condition failed.
        if REPO_WRITE_TOOL_NAMES.contains(&request.name.as_ref())
            && let Err(refusal) =
                repo_write::repo_write_access(self.ctx.is_main_worktree, self.ctx.allow_repo_writes)
        {
            return Err(ErrorData::invalid_request(
                format!("`{}`: {refusal}", request.name),
                None,
            ));
        }
        let tcc = ToolCallContext::new(self, request, context);
        self.tool_router.call(tcc).await
    }

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
             unavailable. The write surface is bounded: an identity-scoped publish category \
             covering the four agent boot events (publish_status, publish_artifact, \
             publish_blocked, publish_question) — each publishes as the calling session only (an \
             agent's worktree as its branch, the main working tree as the supervisor). The \
             session lifecycle tools (start_session, pause_session, stop_session, purge_session) \
             and the verdict tools (publish_verified, publish_feedback) exist for a root session \
             only — a server started in the repository's main working tree; a server in an \
             agent's worktree neither lists nor runs them. Repository writes (create_spec, \
             write_spec_artifact) exist only for a root session of a repository whose own \
             .git-paw/config.toml sets [mcp] allow_repo_writes = true, read when the server \
             starts; they write inside one spec change directory and never stage or commit. \
             validate_spec is read-only and always available."
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
        && SpecBackendKind::parse(spec_type).is_none()
    {
        let valid = SpecBackendKind::ALL
            .iter()
            .map(|k| k.type_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(PawError::McpError(format!(
            "invalid [specs].type = \"{spec_type}\" in .git-paw/config.toml. \
             Valid values: {valid}."
        )));
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
    // Enabled writes are logged at `warn` so the line is recorded at the
    // default verbosity: it is the operator's signal that consent is on.
    match repo_write::repo_write_access(ctx.is_main_worktree, ctx.allow_repo_writes) {
        Ok(()) => logging::warn(
            "repository writes are ENABLED ([mcp] allow_repo_writes = true, root session)",
        ),
        Err(refusal) => logging::info(&format!("repository writes are disabled: {refusal}")),
    }
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

    /// A worktree session's context (the default: no root-only tools).
    fn ctx() -> RepoContext {
        ctx_named("git-paw")
    }

    /// A root session's context: the main working tree, with a credential.
    fn root_ctx() -> RepoContext {
        RepoContext {
            is_main_worktree: true,
            authority: Some(crate::mcp::SessionAuthority::new("c".repeat(64))),
            ..ctx()
        }
    }

    /// A root session of a repository that consented to repository writes.
    fn consenting_root_ctx() -> RepoContext {
        RepoContext {
            allow_repo_writes: true,
            ..root_ctx()
        }
    }

    /// The advertised tool names of a server built from `ctx`.
    fn advertised(ctx: RepoContext) -> std::collections::BTreeSet<String> {
        GitPawMcpServer::new(ctx)
            .tool_router
            .list_all()
            .into_iter()
            .map(|t| t.name.to_string())
            .collect()
    }

    fn ctx_named(name: &str) -> RepoContext {
        RepoContext {
            root: std::path::PathBuf::from("/tmp"),
            git_paw_dir: None,
            broker_url: None,
            server_name: name.to_string(),
            is_main_worktree: false,
            allow_repo_writes: false,
            authority: None,
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

    // mcp-server "Hard errors only for malformed configuration" — Scenario:
    // Every supported spec backend type is accepted at startup.
    #[test]
    fn validate_startup_config_accepts_every_known_spec_backend_type() {
        for kind in SpecBackendKind::ALL {
            let tmp = tempfile::tempdir().unwrap();
            let git_paw_dir = tmp.path().join(".git-paw");
            std::fs::create_dir(&git_paw_dir).unwrap();
            std::fs::write(
                git_paw_dir.join("config.toml"),
                format!("[specs]\ntype = \"{}\"\n", kind.type_str()),
            )
            .unwrap();
            let ctx = RepoContext {
                root: tmp.path().to_path_buf(),
                git_paw_dir: None,
                broker_url: None,
                server_name: "git-paw".to_string(),
                is_main_worktree: false,
                allow_repo_writes: false,
                authority: None,
            };
            assert!(
                validate_startup_config(&ctx).is_ok(),
                "{kind:?} ({}) should be accepted by startup validation",
                kind.type_str()
            );
        }
    }

    #[test]
    fn validate_startup_config_rejects_unrecognised_type() {
        let tmp = tempfile::tempdir().unwrap();
        let git_paw_dir = tmp.path().join(".git-paw");
        std::fs::create_dir(&git_paw_dir).unwrap();
        std::fs::write(
            git_paw_dir.join("config.toml"),
            "[specs]\ntype = \"unrecognised\"\n",
        )
        .unwrap();
        let ctx = RepoContext {
            root: tmp.path().to_path_buf(),
            git_paw_dir: None,
            broker_url: None,
            server_name: "git-paw".to_string(),
            is_main_worktree: false,
            allow_repo_writes: false,
            authority: None,
        };
        let err = validate_startup_config(&ctx).unwrap_err().to_string();
        assert!(err.contains("unrecognised"), "got: {err}");
        for kind in SpecBackendKind::ALL {
            assert!(
                err.contains(kind.type_str()),
                "error should list {} as a valid value; got: {err}",
                kind.type_str()
            );
        }
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
        "wait_for_events",
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
        // mcp-agent-publish: the four agent boot events, for every session,
        // publishing as the calling session only.
        "publish_status",
        "publish_artifact",
        "publish_blocked",
        "publish_question",
        // Root session only (ROOT_ONLY_TOOL_NAMES): the session lifecycle and
        // the two verdict tools.
        "start_session",
        "pause_session",
        "stop_session",
        "purge_session",
        "publish_verified",
        "publish_feedback",
        // mcp-spec-authoring-tools: read-only, every session.
        "validate_spec",
        // Repository writes (REPO_WRITE_TOOL_NAMES): a consenting root
        // session only.
        "create_spec",
        "write_spec_artifact",
    ];

    /// `mcp-server` scenario "No arbitrary file, git or config mutation is
    /// exposed" + `mcp-agent-publish` scenario "No generic publish tool
    /// exists": a consenting root session's registry is exactly the closed
    /// allowlist above — nothing more, nothing less. A stray extra tool (mutating or
    /// not) fails this test immediately, forcing a conscious update rather
    /// than a silent drift.
    #[test]
    fn registry_is_exactly_the_closed_allowlist() {
        let allowed: std::collections::BTreeSet<String> = ALL_ALLOWED_TOOL_NAMES
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        assert_eq!(
            advertised(consenting_root_ctx()),
            allowed,
            "the tool registry drifted from the closed allowlist — any addition or removal \
             must be a deliberate edit to ALL_ALLOWED_TOOL_NAMES, not an incidental one"
        );
    }

    /// `mcp-server` "Worktree session does not see root-only tools": a
    /// worktree session's registry is the allowlist minus exactly the
    /// root-only names.
    #[test]
    fn worktree_registry_is_the_allowlist_without_the_root_only_tools() {
        let expected: std::collections::BTreeSet<String> = ALL_ALLOWED_TOOL_NAMES
            .iter()
            .filter(|n| !ROOT_ONLY_TOOL_NAMES.contains(n) && !REPO_WRITE_TOOL_NAMES.contains(n))
            .map(std::string::ToString::to_string)
            .collect();
        assert_eq!(advertised(ctx()), expected);
        // Even with the switch on, a worktree gets no repository writes.
        let worktree_on = RepoContext {
            allow_repo_writes: true,
            ..ctx()
        };
        assert_eq!(advertised(worktree_on), expected);
        for worker in [
            "publish_status",
            "publish_artifact",
            "publish_blocked",
            "publish_question",
        ] {
            assert!(
                expected.contains(worker),
                "a worktree keeps the worker publish tools: {worker}"
            );
        }
    }

    /// `mcp-server` "Root session sees root-only tools": every root-only tool
    /// is advertised to a root session, and each is on the allowlist.
    #[test]
    fn root_session_sees_every_root_only_tool() {
        let names = advertised(root_ctx());
        for name in ROOT_ONLY_TOOL_NAMES {
            assert!(names.contains(*name), "{name} missing from a root session");
            assert!(
                ALL_ALLOWED_TOOL_NAMES.contains(name),
                "{name} must be on the closed allowlist"
            );
        }
    }

    /// `mcp-server` "Repository write tools require repository consent and a
    /// root session": the write tools are listed exactly in the
    /// (root, switch on) case, `validate_spec` in all four, and every gated
    /// name is on the closed allowlist.
    #[test]
    fn repo_write_tools_are_listed_only_for_a_consenting_root_session() {
        for (root, switch) in [(true, true), (true, false), (false, true), (false, false)] {
            let names = advertised(RepoContext {
                is_main_worktree: root,
                allow_repo_writes: switch,
                ..ctx()
            });
            for name in REPO_WRITE_TOOL_NAMES {
                assert_eq!(
                    names.contains(*name),
                    root && switch,
                    "{name} ({root}, {switch})"
                );
                assert!(ALL_ALLOWED_TOOL_NAMES.contains(name));
            }
            assert!(names.contains("validate_spec"), "({root}, {switch})");
        }
    }

    /// `mcp-server` "No tool parameter grants consent": no advertised tool's
    /// schema, in any session, accepts a parameter that enables writes.
    #[test]
    fn no_tool_schema_carries_a_consent_parameter() {
        let server = GitPawMcpServer::new(consenting_root_ctx());
        for tool in server.tool_router.list_all() {
            let props = tool
                .input_schema
                .get("properties")
                .and_then(serde_json::Value::as_object)
                .map(|p| p.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            for p in &props {
                let lowered = p.to_ascii_lowercase();
                assert!(
                    !["allow", "consent", "grant", "enable", "force", "override"]
                        .iter()
                        .any(|t| lowered.contains(t)),
                    "{} accepts {p}",
                    tool.name
                );
            }
        }
    }

    /// `mcp-server` "No arbitrary file, git or config mutation is exposed",
    /// in every session and switch combination: the only file-content writes
    /// are the two spec-authoring tools, whose parameters name a change and a
    /// path inside it — no tool takes a command, a git argument list, or a
    /// config key, and no tool's name says it writes config.
    #[test]
    fn bounded_write_surface_holds_for_every_session_and_switch() {
        for (root, switch) in [(true, true), (true, false), (false, true), (false, false)] {
            let server = GitPawMcpServer::new(RepoContext {
                is_main_worktree: root,
                allow_repo_writes: switch,
                ..root_ctx()
            });
            for tool in server.tool_router.list_all() {
                let name = tool.name.to_ascii_lowercase();
                assert!(
                    !name.contains("config") && !name.contains("exec") && !name.contains("shell"),
                    "{name}"
                );
                let props: Vec<String> = tool
                    .input_schema
                    .get("properties")
                    .and_then(serde_json::Value::as_object)
                    .map(|p| p.keys().cloned().collect())
                    .unwrap_or_default();
                for p in &props {
                    assert!(
                        !["command", "cmd", "args", "argv", "git_args", "script"]
                            .contains(&p.as_str()),
                        "{name} takes {p}"
                    );
                }
                if props.iter().any(|p| p == "content") {
                    assert!(
                        REPO_WRITE_TOOL_NAMES.contains(&tool.name.as_ref()),
                        "{name} writes content but is not behind the repository-write gate"
                    );
                }
            }
        }
    }

    /// Export agnosticism: the three spec-authoring tool descriptions carry
    /// no stack token and no commit-format prose. The token lists mirror
    /// `FORBIDDEN_TOKENS` and `FORBIDDEN_COMMIT_CONVENTION_TOKENS` in
    /// `tests/lang_agnostic_skill_audit.rs`.
    #[test]
    fn spec_authoring_descriptions_are_project_agnostic() {
        const FORBIDDEN: &[&str] = &[
            "cargo",
            "rustdoc",
            ".rs:",
            "Cargo.toml",
            "rustc",
            "Claude Code",
            ".claude/",
            "feat(",
            "fix(",
            "docs(",
            "test(",
            "chore(",
            "refactor(",
            "perf(",
        ];
        let server = GitPawMcpServer::new(consenting_root_ctx());
        for name in ["create_spec", "write_spec_artifact", "validate_spec"] {
            let tool = server
                .tool_router
                .list_all()
                .into_iter()
                .find(|t| t.name == name)
                .unwrap_or_else(|| panic!("{name} registered"));
            let description = tool.description.unwrap_or_default();
            assert!(!description.is_empty(), "{name} has a description");
            for token in FORBIDDEN {
                assert!(!description.contains(token), "{name} mentions {token}");
            }
        }
    }

    /// The error a worktree session gets for a root-only tool names the
    /// restriction (the call path is covered end to end in
    /// `tests/mcp_lifecycle_e2e.rs`).
    #[test]
    fn the_root_only_error_names_the_worktree_restriction() {
        let err = root_only_error("pause_session");
        assert!(err.message.contains("pause_session"), "got: {err:?}");
        assert!(err.message.contains("worktree"), "got: {err:?}");
        assert!(err.message.contains("main working tree"), "got: {err:?}");
    }

    /// `mcp-server` "`start_session` never runs the drive loop" and
    /// "`stop_session` takes no confirmation or force parameter".
    #[test]
    fn lifecycle_schemas_offer_no_unattended_confirm_or_force() {
        let server = GitPawMcpServer::new(root_ctx());
        let props = |name: &str| -> Vec<String> {
            let tool = server
                .tool_router
                .list_all()
                .into_iter()
                .find(|t| t.name == name)
                .unwrap_or_else(|| panic!("{name} registered for a root session"));
            tool.input_schema
                .get("properties")
                .and_then(serde_json::Value::as_object)
                .map(|p| p.keys().cloned().collect())
                .unwrap_or_default()
        };
        let start = props("start_session");
        assert!(start.contains(&"branches".to_string()), "got {start:?}");
        assert!(!start.iter().any(|p| p == "unattended"), "got {start:?}");
        let stop = props("stop_session");
        assert!(
            !stop.iter().any(|p| p == "confirm" || p == "force"),
            "got {stop:?}"
        );
        assert!(props("purge_session").contains(&"confirm".to_string()));
    }

    /// `mcp-server` "Purge is annotated destructive".
    #[test]
    fn purge_session_is_annotated_destructive() {
        let server = GitPawMcpServer::new(root_ctx());
        let purge = server
            .tool_router
            .list_all()
            .into_iter()
            .find(|t| t.name == "purge_session")
            .expect("purge_session registered for a root session");
        assert_eq!(
            purge.annotations.and_then(|a| a.destructive_hint),
            Some(true)
        );
    }

    /// `approval-command-safety` scenario "No MCP tool applies setup": the
    /// MCP surface SHALL expose no tool that runs `git paw setup-cli`, writes
    /// an agent CLI's settings file, or writes the setup record.
    ///
    /// Asserted over the live registry and its frozen allowlist, not over
    /// `--help` text: a new tool shows up here, and `--help` enumerates no
    /// tools at all.
    #[test]
    fn no_tool_applies_cli_setup_or_writes_cli_settings() {
        let server = GitPawMcpServer::new(ctx());
        let advertised: Vec<String> = server
            .tool_router
            .list_all()
            .into_iter()
            .map(|t| t.name.to_string())
            .collect();
        assert!(
            !advertised.is_empty(),
            "the registry must actually advertise tools, or this guard is vacuous"
        );

        for name in advertised.iter().chain(
            ALL_ALLOWED_TOOL_NAMES
                .iter()
                .map(|n| (*n).to_string())
                .collect::<Vec<_>>()
                .iter(),
        ) {
            let lowered = name.to_ascii_lowercase();
            for forbidden in ["setup", "settings", "allowlist", "permission", "grant"] {
                assert!(
                    !lowered.contains(forbidden),
                    "tool `{name}` looks like it touches CLI setup — setup is consent-only \
                     and runs from the command line, never through MCP"
                );
            }
        }
    }

    /// `mcp-agent-publish` scenarios "No generic publish tool exists", "No
    /// verified tool in a worktree session", "No feedback tool in a worktree
    /// session", and "Publisher id is derived, not supplied": adversarially
    /// inspect every tool a worktree session advertises — name and parameter
    /// schema, not just the four publish tools by name.
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
    /// granting a coding agent's worktree session the strongest available
    /// capability (calling every advertised tool), there is no tool call
    /// sequence that publishes an `agent.verified` message — the verb is not
    /// on its surface (a by-name call is refused end to end in
    /// `tests/mcp_lifecycle_e2e.rs`).
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
