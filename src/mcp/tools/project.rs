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
        match skills::resolve(&p.name, root) {
            Ok(template) => {
                let cfg = config::load_config(root, None).unwrap_or_default();
                let project = git::project_name(root);
                let branch = git::current_branch(root).unwrap_or_else(|_| "main".to_string());
                let broker_url = self
                    .ctx
                    .broker_url
                    .clone()
                    .unwrap_or_else(|| "http://127.0.0.1:9119".to_string());
                let backends: Vec<SpecBackendKind> =
                    specs::resolved_spec_kind(&cfg).into_iter().collect();
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
                // The orchestrator skill (or its `supervisor` alias) is served
                // exactly as `git paw init` exports it: the standardized
                // `SKILL.md`, rendered with the configured gate commands and no
                // session value.
                let supervisor = cfg.supervisor.clone().unwrap_or_default();
                let orchestrator = template.name == skills::ORCHESTRATOR_SKILL;
                let gates = if orchestrator {
                    supervisor.gate_commands()
                } else {
                    gates
                };
                let content = if orchestrator {
                    skills::render_standardized(&template, &gates, &backends)
                } else {
                    // No agent CLI is known here, so the per-CLI fragments
                    // render their `generic` form.
                    skills::render(
                        &template,
                        &branch,
                        &broker_url,
                        &project,
                        &gates,
                        &backends,
                        crate::agent_cli::CliFamily::Generic,
                    )
                };
                // Then each of the package's references, rendered with the
                // same values, under a heading naming its path — so an MCP
                // client loses no content the skill moved into a reference.
                let content = match skills::bundled_package(&template.name) {
                    Some(package) => {
                        let inputs = skills::RenderInputs {
                            branch: &branch,
                            root: false,
                            broker_url: &broker_url,
                            project: &project,
                            gates,
                            backends: &backends,
                            family: crate::agent_cli::CliFamily::Generic,
                        };
                        let resolved = skills::ResolvedPackage {
                            package,
                            template: template.clone(),
                        };
                        let references = skills::render_references(&resolved, &inputs);
                        skills::join_package_document(
                            &content,
                            references
                                .iter()
                                .map(|f| (f.path.as_str(), f.content.as_str())),
                        )
                    }
                    None => content,
                };
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

#[cfg(test)]
mod tests {
    use super::GetSkillParams;
    use crate::mcp::RepoContext;
    use crate::mcp::server::GitPawMcpServer;
    use rmcp::handler::server::wrapper::Parameters;

    fn server_for(root: std::path::PathBuf) -> GitPawMcpServer {
        GitPawMcpServer::new(RepoContext {
            root,
            git_paw_dir: None,
            broker_url: None,
            server_name: "git-paw".to_string(),
            is_main_worktree: false,
            allow_repo_writes: false,
            authority: None,
        })
    }

    /// `skill-agent` / Requirement: Embedded cli-setup skill, scenario
    /// "The skill resolves and is served" — the MCP half: `get_skill` serves
    /// `cli-setup` with the embedded content and source `embedded`.
    #[test]
    fn get_skill_serves_the_orchestrator_skill_as_exported_under_both_names() {
        let tmp = tempfile::tempdir().unwrap();
        let server = server_for(tmp.path().to_path_buf());
        let config = crate::config::PawConfig::default();
        let supervisor = config.supervisor.clone().unwrap_or_default();
        let resolved = crate::skills::ResolvedPackage {
            package: crate::skills::bundled_package(crate::skills::ORCHESTRATOR_SKILL).unwrap(),
            template: crate::skills::resolve_from(
                crate::skills::ORCHESTRATOR_SKILL,
                &crate::skills::SkillRoots::default(),
            )
            .unwrap(),
        };
        // The package as `git paw init` installs it: the standardized
        // `SKILL.md`, then each reference under its heading.
        let exported = crate::skills::render_package_document(
            &resolved,
            &crate::skills::RenderInputs {
                branch: "main",
                root: true,
                broker_url: "",
                project: "",
                gates: supervisor.gate_commands(),
                backends: &[],
                family: crate::agent_cli::CliFamily::Generic,
            },
        );

        for name in ["orchestrator", "supervisor"] {
            let resp = server.get_skill(Parameters(GetSkillParams {
                name: name.to_string(),
            }));
            let skill = resp.0.skill.expect("the orchestrator skill is served");
            assert_eq!(skill.name, "orchestrator");
            assert_eq!(
                skill.content, exported,
                "{name} serves the exported package"
            );
            assert!(!skill.content.contains("http://"));
        }
    }

    /// `skill-agent` / Scenario "A render with no agent uses generic" — the
    /// MCP half: `get_skill` serves the coordination skill with the `generic`
    /// skill fragment, since no agent CLI is known.
    #[test]
    fn get_skill_renders_the_generic_cli_fragment() {
        use crate::agent_cli::CliFamily;
        use crate::skills::{FragmentSlot, bundled_package, select_fragment};
        let tmp = tempfile::tempdir().unwrap();
        let server = server_for(tmp.path().to_path_buf());
        let resp = server.get_skill(Parameters(GetSkillParams {
            name: "coordination".to_string(),
        }));
        let skill = resp.0.skill.expect("coordination is served");
        let coordination = bundled_package("coordination").unwrap();
        let generic = select_fragment(CliFamily::Generic, FragmentSlot::Skill, coordination);
        let claude = select_fragment(CliFamily::Claude, FragmentSlot::Skill, coordination);
        assert!(
            skill.content.contains(generic.trim_end()),
            "{}",
            skill.content
        );
        assert!(
            !skill.content.contains(claude.trim_end()),
            "{}",
            skill.content
        );
        assert!(!skill.content.contains("{{CLI_NOTES}}"));
    }

    // mcp-server / Scenario: get_skill includes the orchestrator's gate references
    #[test]
    fn get_skill_includes_the_orchestrators_gate_references() {
        let tmp = tempfile::tempdir().unwrap();
        let server = server_for(tmp.path().to_path_buf());
        let skill = server
            .get_skill(Parameters(GetSkillParams {
                name: "orchestrator".to_string(),
            }))
            .0
            .skill
            .expect("the orchestrator skill is served");

        let skill_md = crate::skills::render_standardized(
            &crate::skills::resolve_from(
                crate::skills::ORCHESTRATOR_SKILL,
                &crate::skills::SkillRoots::default(),
            )
            .unwrap(),
            &crate::config::SupervisorConfig::default().gate_commands(),
            &[],
        );
        assert!(
            skill.content.starts_with(&skill_md),
            "the content opens with the rendered orchestrator SKILL.md"
        );
        let heading = "## Reference: references/gates/spec-audit.md";
        let at = skill
            .content
            .find(heading)
            .expect("a heading names references/gates/spec-audit.md");
        assert!(
            skill.content[at..].contains("### Spec Audit Procedure"),
            "the spec-audit reference's rendered content follows its heading"
        );
        assert!(
            !skill.content[at..].contains("{{SPEC_VALIDATE_COMMAND}}"),
            "the reference is rendered"
        );
    }

    // mcp-server / Scenario: A skill without references is unchanged
    #[test]
    fn get_skill_serves_a_skill_without_references_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let server = server_for(tmp.path().to_path_buf());
        let before: Vec<_> = std::fs::read_dir(tmp.path()).unwrap().collect();
        let skill = server
            .get_skill(Parameters(GetSkillParams {
                name: "docs-fetch".to_string(),
            }))
            .0
            .skill
            .expect("docs-fetch is served");

        let rendered = crate::skills::render(
            &crate::skills::resolve_from("docs-fetch", &crate::skills::SkillRoots::default())
                .unwrap(),
            &crate::git::current_branch(tmp.path()).unwrap_or_else(|_| "main".to_string()),
            "http://127.0.0.1:9119",
            &crate::git::project_name(tmp.path()),
            &crate::skills::GateCommands::default(),
            &[],
            crate::agent_cli::CliFamily::Generic,
        );
        assert_eq!(skill.content, rendered, "the rendered docs-fetch SKILL.md");
        assert!(!skill.content.contains("## Reference: "));
        let after: Vec<_> = std::fs::read_dir(tmp.path()).unwrap().collect();
        assert_eq!(before.len(), after.len(), "no file is written to disk");
    }

    #[test]
    fn get_skill_serves_the_embedded_cli_setup_skill() {
        let tmp = tempfile::tempdir().unwrap();
        let server = server_for(tmp.path().to_path_buf());

        let resp = server.get_skill(Parameters(GetSkillParams {
            name: "cli-setup".to_string(),
        }));

        let skill = resp.0.skill.expect("cli-setup is served, not null");
        assert_eq!(skill.name, "cli-setup");
        assert_eq!(skill.source, "embedded");
        assert!(
            !skill.content.is_empty(),
            "served skill carries the embedded content"
        );
        // Served content is rendered, not the raw template.
        assert!(
            !skill.content.contains("{{"),
            "placeholders must be substituted in the served skill"
        );
        // It is the cli-setup skill and not some other embedded asset.
        assert!(
            skill.content.contains("git paw setup-cli"),
            "served content must be the cli-setup skill body"
        );
    }
}
