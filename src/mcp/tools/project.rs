//! Project-knowledge tools: `get_specs`, `get_spec`, `get_tasks`, `get_task`,
//! `get_dependency_graph`, `get_skill`.
//!
//! Spec tools handle all three backends via the shared discovery used by
//! `git paw start --from-all-specs`. `get_skill` renders a named agent skill
//! through the existing resolution + `{{...}}` substitution pipeline
//! (read-only: no disk write, no watcher, no version endpoint).

use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{schemars, tool, tool_router};
use serde::{Deserialize, Serialize};

use crate::config;
use crate::git;
use crate::mcp::query;
use crate::mcp::query::specs::DependencyGraph;
use crate::mcp::server::GitPawMcpServer;
use crate::skills::{self, GateCommands, Source};
use crate::specs::{self, SpecBackendKind};

/// Parameters for [`GitPawMcpServer::get_spec`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSpecParams {
    /// Spec id (directory or file stem).
    pub id: String,
}

/// Parameters for [`GitPawMcpServer::get_tasks`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetTasksParams {
    /// Spec id whose tasks to return.
    pub spec: String,
}

/// Parameters for [`GitPawMcpServer::get_task`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetTaskParams {
    /// Spec id the task belongs to.
    pub spec: String,
    /// Task id (e.g. "T009" for Spec Kit, or the sequence number for `OpenSpec`).
    pub id: String,
}

/// Parameters for [`GitPawMcpServer::get_skill`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSkillParams {
    /// Skill name (e.g. "coordination", "supervisor").
    pub name: String,
}

/// Response for `get_specs`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SpecsResponse {
    /// Discovered specs.
    pub specs: Vec<query::specs::SpecInfo>,
}

/// Response for `get_spec`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SpecResponse {
    /// Spec detail, or null when not found.
    pub spec: Option<query::specs::SpecDetail>,
}

/// Response for `get_tasks`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct TasksResponse {
    /// Tasks for the spec.
    pub tasks: Vec<query::specs::TaskInfo>,
}

/// Response for `get_task`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct TaskResponse {
    /// Matching task, or null.
    pub task: Option<query::specs::TaskInfo>,
}

/// A rendered skill view.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SkillView {
    /// Skill name.
    pub name: String,
    /// Rendered content (post `{{...}}` substitution).
    pub content: String,
    /// Source: "standard" | "`user_override`" | "embedded".
    pub source: String,
}

/// Response for `get_skill`.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SkillResponse {
    /// Rendered skill, or null when unknown/unrenderable.
    pub skill: Option<SkillView>,
    /// Human-readable note when `skill` is null.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[tool_router(router = project_router, vis = "pub(crate)")]
impl GitPawMcpServer {
    /// `get_specs` — discovered specs across all backends.
    #[tool(
        description = "List discovered specs across OpenSpec, Markdown, and Spec Kit backends. \
                       Each carries id, backend, title, status, and path. Empty when none exist."
    )]
    pub(crate) fn get_specs(&self) -> Json<SpecsResponse> {
        Json(SpecsResponse {
            specs: query::specs::list_specs(&self.ctx),
        })
    }

    /// `get_spec` — full content of a named spec.
    #[tool(
        description = "Return the discovered artifacts (proposal/design/tasks/specs for OpenSpec; \
                       spec/plan/tasks/checklists for Spec Kit; body for Markdown) of a named spec \
                       with their content, or { \"spec\": null } when not found."
    )]
    pub(crate) fn get_spec(&self, Parameters(p): Parameters<GetSpecParams>) -> Json<SpecResponse> {
        Json(SpecResponse {
            spec: query::specs::get_spec(&self.ctx, &p.id),
        })
    }

    /// `get_tasks` — tasks for a named spec.
    #[tool(
        description = "List the tasks for a named spec: id, phase, parallel marker, description, \
                       and completion state. Empty when the spec has no tasks or is not found."
    )]
    pub(crate) fn get_tasks(
        &self,
        Parameters(p): Parameters<GetTasksParams>,
    ) -> Json<TasksResponse> {
        Json(TasksResponse {
            tasks: query::specs::get_tasks(&self.ctx, &p.spec),
        })
    }

    /// `get_task` — a single task within a spec.
    #[tool(
        description = "Return a single task by id within a spec, or { \"task\": null } when the \
                       spec or task id is not found."
    )]
    pub(crate) fn get_task(&self, Parameters(p): Parameters<GetTaskParams>) -> Json<TaskResponse> {
        let task = query::specs::get_tasks(&self.ctx, &p.spec)
            .into_iter()
            .find(|t| t.id == p.id);
        Json(TaskResponse { task })
    }

    /// `get_dependency_graph` — inter-spec `[[ref]]` dependency graph.
    #[tool(
        description = "Return the spec dependency graph derived from [[other-spec]] cross-references \
                       in proposals, as { nodes, edges }."
    )]
    pub(crate) fn get_dependency_graph(&self) -> Json<DependencyGraph> {
        Json(query::specs::dependency_graph(&self.ctx))
    }

    /// `get_skill` — rendered content of a named agent skill.
    #[tool(
        description = "Return the rendered content of a named agent skill (post {{...}} \
                       substitution) plus its source (standard | user_override | embedded). \
                       Read-only — no disk write. Unknown skills return { \"skill\": null } with a \
                       message, not a transport error."
    )]
    pub(crate) fn get_skill(
        &self,
        Parameters(p): Parameters<GetSkillParams>,
    ) -> Json<SkillResponse> {
        let root = &self.ctx.root;
        match skills::resolve(&p.name) {
            Ok(template) => {
                let cfg = config::load_config(root, None).unwrap_or_default();
                let project = git::project_name(root);
                let branch = git::current_branch(root).unwrap_or_else(|_| "main".to_string());
                let broker_url = self
                    .ctx
                    .broker_url
                    .clone()
                    .unwrap_or_else(|| "http://127.0.0.1:9119".to_string());
                let backends = match specs::resolved_spec_type(&cfg, root).as_deref() {
                    Some("speckit") => vec![SpecBackendKind::SpecKit],
                    Some("markdown") => vec![SpecBackendKind::Markdown],
                    Some("openspec") => vec![SpecBackendKind::OpenSpec],
                    _ => Vec::new(),
                };
                // Read-only render: gate commands are not wired here (the MCP
                // view is a static skill preview, not a launch), so they render
                // as "(not configured)".
                let gates = GateCommands {
                    test_command: None,
                    lint_command: None,
                    build_command: None,
                    doc_build_command: None,
                    spec_validate_command: None,
                    fmt_check_command: None,
                    security_audit_command: None,
                    doc_tool_command: None,
                };
                let content =
                    skills::render(&template, &branch, &broker_url, &project, &gates, &backends);
                let source = match template.source {
                    Source::Embedded => "embedded",
                    Source::AgentsStandard => "standard",
                    Source::User => "user_override",
                };
                Json(SkillResponse {
                    skill: Some(SkillView {
                        name: template.name,
                        content,
                        source: source.to_string(),
                    }),
                    message: None,
                })
            }
            Err(skills::SkillError::UnknownSkill { name }) => Json(SkillResponse {
                skill: None,
                message: Some(format!("unknown skill: {name}")),
            }),
            Err(e) => Json(SkillResponse {
                skill: None,
                message: Some(e.to_string()),
            }),
        }
    }
}
