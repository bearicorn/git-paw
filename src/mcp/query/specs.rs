//! Spec & task reads across the `OpenSpec`, Markdown, Spec Kit, and
//! Superpowers backends.
//!
//! Uses the same discovery (`specs::scan_specs_with_override`) that
//! `git paw start --from-all-specs` uses, then reads per-backend artifacts
//! from where each format's backend locates them. Degrades to empty lists
//! when no specs are configured or discoverable. A scanned entry its backend
//! cannot locate (its files vanished after the scan) is not served.

use std::path::PathBuf;

use rmcp::schemars;
use serde::Serialize;

use crate::config;
use crate::mcp::RepoContext;
use crate::specs::{self, SpecArtifact, SpecEntry, SpecLocation, SpecTask};

/// A scanned spec and where its backend located it.
struct Located {
    entry: SpecEntry,
    location: SpecLocation,
}

/// Resolved spec discovery context.
struct Discovery {
    repo_root: PathBuf,
    specs: Vec<Located>,
}

impl Discovery {
    fn find(&self, id: &str) -> Option<&Located> {
        self.specs.iter().find(|s| s.entry.id == id)
    }

    /// `path` relative to the repository root, with `/` separators.
    fn rel_path(&self, location: &SpecLocation) -> String {
        let rel = location
            .path
            .strip_prefix(&self.repo_root)
            .unwrap_or(&location.path);
        rel.components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    }
}

/// Discovers specs, degrading to an empty set on any error so the MCP tools
/// still answer in a repo with no specs. Entries are located under the
/// directory the scan resolved.
fn discover(ctx: &RepoContext) -> Discovery {
    let repo_root = ctx.root.clone();
    let config = config::load_config(&repo_root, None).unwrap_or_default();
    let dir = specs::resolve_specs_config(&config, None)
        .ok()
        .flatten()
        .map_or_else(|| "specs".to_string(), |r| r.dir);
    let specs_dir = repo_root.join(&dir);
    let entries = specs::scan_specs(&config, &repo_root).unwrap_or_default();

    let locations = specs::locate_specs(&specs_dir, &entries);
    let specs = entries
        .into_iter()
        .filter_map(|entry| {
            let location = locations.get(&entry.id)?.clone();
            Some(Located { entry, location })
        })
        .collect();
    Discovery { repo_root, specs }
}

fn first_heading(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("# ").map(str::trim))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Derives a human title for a spec: the first `# ` heading of its title
/// file, else of its entry prompt, else the id.
fn derive_title(location: &SpecLocation, entry: &SpecEntry) -> String {
    std::fs::read_to_string(&location.title_file)
        .ok()
        .and_then(|content| first_heading(&content))
        .or_else(|| first_heading(&entry.prompt))
        .unwrap_or_else(|| entry.id.clone())
}

/// One discovered spec summary.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SpecInfo {
    /// Spec id (directory or file stem).
    pub id: String,
    /// Backend: "openspec" | "markdown" | "speckit" | "superpowers".
    pub backend: String,
    /// Human title.
    pub title: String,
    /// Status (discovery yields pending specs).
    pub status: String,
    /// Path relative to the repository root.
    pub path: String,
}

/// Lists every discovered spec across all backends.
#[must_use]
pub fn list_specs(ctx: &RepoContext) -> Vec<SpecInfo> {
    let d = discover(ctx);
    d.specs
        .iter()
        .map(|s| SpecInfo {
            id: s.entry.id.clone(),
            backend: s.entry.backend.as_str().to_string(),
            title: derive_title(&s.location, &s.entry),
            status: "pending".to_string(),
            path: d.rel_path(&s.location),
        })
        .collect()
}

/// One artifact (file) belonging to a spec.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct Artifact {
    /// Artifact name (e.g. "proposal", "design", "tasks", or a filename).
    pub name: String,
    /// Full content.
    pub content: String,
}

/// Full content of a single spec.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SpecDetail {
    /// Spec id.
    pub id: String,
    /// Backend string.
    pub backend: String,
    /// Path relative to the repository root.
    pub path: String,
    /// Discovered artifacts with their content.
    pub artifacts: Vec<Artifact>,
}

impl From<SpecArtifact> for Artifact {
    fn from(a: SpecArtifact) -> Self {
        Self {
            name: a.name,
            content: a.content,
        }
    }
}

/// Returns the full content of a named spec, or `None` when not found.
#[must_use]
pub fn get_spec(ctx: &RepoContext, id: &str) -> Option<SpecDetail> {
    let d = discover(ctx);
    let spec = d.find(id)?;

    let artifacts = spec
        .entry
        .backend
        .backend()
        .artifacts(&spec.location.path, &spec.entry)
        .into_iter()
        .map(Artifact::from)
        .collect();

    Some(SpecDetail {
        id: id.to_string(),
        backend: spec.entry.backend.as_str().to_string(),
        path: d.rel_path(&spec.location),
        artifacts,
    })
}

/// One task entry.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct TaskInfo {
    /// Task id (e.g. "T009", or a derived sequence for `OpenSpec`).
    pub id: String,
    /// Phase number (0 for the implicit/leading phase).
    pub phase: u32,
    /// Whether the task carries a `[P]` parallel marker (Spec Kit only).
    pub parallel: bool,
    /// Description text.
    pub description: String,
    /// Completion state.
    pub complete: bool,
}

/// Returns the tasks for a named spec. Empty when the spec has no tasks or is
/// not found.
#[must_use]
pub fn get_tasks(ctx: &RepoContext, spec: &str) -> Vec<TaskInfo> {
    let d = discover(ctx);
    let Some(found) = d.find(spec) else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(&found.location.tasks_file) else {
        return Vec::new();
    };

    found
        .entry
        .backend
        .backend()
        .parse_tasks(&content)
        .into_iter()
        .map(TaskInfo::from)
        .collect()
}

impl From<SpecTask> for TaskInfo {
    fn from(t: SpecTask) -> Self {
        Self {
            id: t.id,
            phase: t.phase,
            parallel: t.parallel,
            description: t.description,
            complete: t.complete,
        }
    }
}

/// Spec dependency graph node.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct GraphNode {
    /// Spec id.
    pub id: String,
    /// Backend string.
    pub backend: String,
}

/// Spec dependency graph edge (`from` depends on / references `to`).
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct GraphEdge {
    /// Source spec id.
    pub from: String,
    /// Referenced spec id (the `[[target]]`).
    pub to: String,
}

/// The dependency graph derived from `[[other-spec]]` cross-references.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct DependencyGraph {
    /// Specs as nodes.
    pub nodes: Vec<GraphNode>,
    /// Cross-reference edges.
    pub edges: Vec<GraphEdge>,
}

/// Extracts `[[name]]` reference tokens from text.
fn extract_refs(text: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'['
            && bytes[i + 1] == b'['
            && let Some(end) = text[i + 2..].find("]]")
        {
            let name = text[i + 2..i + 2 + end].trim().to_string();
            if !name.is_empty() {
                refs.push(name);
            }
            i = i + 2 + end + 2;
            continue;
        }
        i += 1;
    }
    refs
}

/// Builds the spec dependency graph from cross-references in every artifact
/// of each spec, or its entry prompt when it has none.
#[must_use]
pub fn dependency_graph(ctx: &RepoContext) -> DependencyGraph {
    let d = discover(ctx);
    let ids: std::collections::HashSet<String> =
        d.specs.iter().map(|s| s.entry.id.clone()).collect();

    let nodes = d
        .specs
        .iter()
        .map(|s| GraphNode {
            id: s.entry.id.clone(),
            backend: s.entry.backend.as_str().to_string(),
        })
        .collect();

    let mut edges = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for Located { entry, location } in &d.specs {
        let artifacts = entry.backend.backend().artifacts(&location.path, entry);
        let text = if artifacts.is_empty() {
            entry.prompt.clone()
        } else {
            artifacts
                .iter()
                .map(|a| a.content.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        };
        for target in extract_refs(&text) {
            // Only record edges to other discovered specs.
            if ids.contains(&target) && target != entry.id {
                let key = (entry.id.clone(), target.clone());
                if seen.insert(key) {
                    edges.push(GraphEdge {
                        from: entry.id.clone(),
                        to: target,
                    });
                }
            }
        }
    }
    edges.sort_by_key(|a| (a.from.clone(), a.to.clone()));

    DependencyGraph { nodes, edges }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn ctx_for(root: &Path) -> RepoContext {
        RepoContext {
            root: root.to_path_buf(),
            git_paw_dir: None,
            broker_url: None,
            server_name: "git-paw".to_string(),
            is_main_worktree: false,
            allow_repo_writes: false,
            authority: None,
        }
    }

    /// Builds a repo with an `OpenSpec` change and the matching `[specs]` config.
    fn openspec_repo() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".git-paw")).unwrap();
        std::fs::write(
            root.join(".git-paw/config.toml"),
            "[specs]\ndir = \"openspec/changes\"\ntype = \"openspec\"\n",
        )
        .unwrap();
        let change = root.join("openspec/changes/add-auth");
        std::fs::create_dir_all(&change).unwrap();
        std::fs::write(
            change.join("tasks.md"),
            "## 1. Setup\n- [x] 1.1 scaffold\n- [ ] 1.2 wire it\n",
        )
        .unwrap();
        std::fs::write(
            change.join("proposal.md"),
            "# Add auth\n\nDepends on [[other-change]].\n",
        )
        .unwrap();
        // Second change so the [[other-change]] edge resolves.
        let other = root.join("openspec/changes/other-change");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("tasks.md"), "- [ ] do thing\n").unwrap();
        tmp
    }

    #[test]
    fn list_specs_discovers_openspec_changes() {
        let tmp = openspec_repo();
        let specs = list_specs(&ctx_for(tmp.path()));
        assert!(
            specs
                .iter()
                .any(|s| s.id == "add-auth" && s.backend == "openspec")
        );
        let auth = specs.iter().find(|s| s.id == "add-auth").unwrap();
        assert_eq!(auth.title, "Add auth");
        assert!(auth.path.contains("openspec/changes/add-auth"));
    }

    #[test]
    fn list_specs_empty_when_no_config() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(list_specs(&ctx_for(tmp.path())).is_empty());
    }

    #[test]
    fn get_spec_returns_artifacts() {
        let tmp = openspec_repo();
        let detail = get_spec(&ctx_for(tmp.path()), "add-auth").expect("found");
        assert_eq!(detail.backend, "openspec");
        assert!(detail.artifacts.iter().any(|a| a.name == "tasks"));
        assert!(detail.artifacts.iter().any(|a| a.name == "proposal"));
    }

    #[test]
    fn get_spec_unknown_is_none() {
        let tmp = openspec_repo();
        assert!(get_spec(&ctx_for(tmp.path()), "nope").is_none());
    }

    #[test]
    fn get_tasks_parses_openspec_checkboxes() {
        let tmp = openspec_repo();
        let tasks = get_tasks(&ctx_for(tmp.path()), "add-auth");
        assert_eq!(tasks.len(), 2);
        assert!(tasks[0].complete);
        assert!(!tasks[1].complete);
        assert_eq!(tasks[0].phase, 1);
    }

    #[test]
    fn dependency_graph_resolves_cross_refs() {
        let tmp = openspec_repo();
        let graph = dependency_graph(&ctx_for(tmp.path()));
        assert!(graph.nodes.iter().any(|n| n.id == "add-auth"));
        assert!(
            graph
                .edges
                .iter()
                .any(|e| e.from == "add-auth" && e.to == "other-change")
        );
    }

    fn repo_with(ty: &str, dir: &str, files: &[(&str, &str)]) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let mut all = vec![(
            ".git-paw/config.toml".to_string(),
            format!("[specs]\ntype = \"{ty}\"\ndir = \"{dir}\"\n"),
        )];
        all.extend(
            files
                .iter()
                .map(|(p, b)| ((*p).to_string(), (*b).to_string())),
        );
        for (rel, body) in all {
            let path = tmp.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        tmp
    }

    fn speckit_repo() -> tempfile::TempDir {
        repo_with(
            "speckit",
            "specs",
            &[
                ("specs/001-login/spec.md", "# Login feature\n"),
                ("specs/001-login/plan.md", "# Plan\n"),
                ("specs/001-login/checklist.md", "# Checklist\n"),
                (
                    "specs/001-login/tasks.md",
                    "## Phase 1: Setup\n- [x] T001 init\n- [ ] T002 [P] add form\n## Phase 2: Core\n- [ ] T003 wire\n",
                ),
            ],
        )
    }

    fn markdown_repo() -> tempfile::TempDir {
        repo_with(
            "markdown",
            "specs",
            &[(
                "specs/fix-bug.md",
                "---\npaw_status: pending\n---\n# Fix the bug\n\n- [ ] reproduce\n",
            )],
        )
    }

    fn superpowers_repo() -> tempfile::TempDir {
        repo_with(
            "superpowers",
            "plans",
            &[(
                "plans/2026-07-20-add-auth.md",
                "# Add auth Implementation Plan\n\n### Task 1: Validation\n\n- [ ] **Step 1: test**\n- [x] **Step 2: scaffold**\n",
            )],
        )
    }

    fn openspec_delta_repo() -> tempfile::TempDir {
        repo_with(
            "openspec",
            "openspec/changes",
            &[
                ("openspec/changes/add-auth/proposal.md", "# Add auth\n"),
                ("openspec/changes/add-auth/design.md", "# Design\n"),
                (
                    "openspec/changes/add-auth/tasks.md",
                    "## 1. Setup\n- [x] 1.1 scaffold\n- [ ] 1.2 wire it\n",
                ),
                (
                    "openspec/changes/add-auth/specs/auth/spec.md",
                    "## ADDED Requirements\n",
                ),
            ],
        )
    }

    fn task_rows(tasks: &[TaskInfo]) -> Vec<(&str, u32, bool, &str, bool)> {
        tasks
            .iter()
            .map(|t| {
                (
                    t.id.as_str(),
                    t.phase,
                    t.parallel,
                    t.description.as_str(),
                    t.complete,
                )
            })
            .collect()
    }

    /// `mcp-server`: "A Spec Kit task entry is served from its feature
    /// directory".
    #[test]
    fn speckit_task_entry_is_served_from_its_feature_directory() {
        let tmp = speckit_repo();
        let ctx = ctx_for(tmp.path());
        let info = list_specs(&ctx)
            .into_iter()
            .find(|s| s.id == "001-login-T002")
            .expect("entry listed");
        assert_eq!(info.path, "specs/001-login");

        let detail = get_spec(&ctx, "001-login-T002").expect("found");
        assert_eq!(detail.path, "specs/001-login");
        let names: Vec<&str> = detail.artifacts.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["spec", "plan", "tasks", "checklist.md"]);

        let tasks = get_tasks(&ctx, "001-login-T002");
        assert_eq!(
            task_rows(&tasks),
            [
                ("T001", 1, false, "init", true),
                ("T002", 1, true, "add form", false),
                ("T003", 2, false, "wire", false),
            ]
        );
    }

    /// `mcp-server`: "A Markdown spec is served from its file".
    #[test]
    fn markdown_spec_is_served_from_its_file() {
        let tmp = markdown_repo();
        let ctx = ctx_for(tmp.path());
        let specs = list_specs(&ctx);
        let info = specs.iter().find(|s| s.id == "fix-bug").expect("listed");
        assert_eq!(info.path, "specs/fix-bug.md");
        assert_eq!(info.title, "Fix the bug");

        let tasks = get_tasks(&ctx, "fix-bug");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].description, "reproduce");
        assert!(!tasks[0].complete);
    }

    /// `mcp-server`: "A Superpowers plan is served from its file with its own
    /// title".
    #[test]
    fn superpowers_plan_is_served_from_its_file_with_its_own_title() {
        let tmp = superpowers_repo();
        let ctx = ctx_for(tmp.path());
        let specs = list_specs(&ctx);
        let info = specs
            .iter()
            .find(|s| s.id == "2026-07-20-add-auth")
            .expect("listed");
        assert_eq!(info.path, "plans/2026-07-20-add-auth.md");
        assert_eq!(info.title, "Add auth Implementation Plan");

        let tasks = get_tasks(&ctx, "2026-07-20-add-auth");
        let states: Vec<(&str, bool)> = tasks
            .iter()
            .map(|t| (t.description.as_str(), t.complete))
            .collect();
        assert_eq!(
            states,
            [("**Step 1: test**", false), ("**Step 2: scaffold**", true)]
        );
    }

    /// `mcp-server`: "An `OpenSpec` change is served as before".
    #[test]
    fn openspec_change_is_served_as_before() {
        let tmp = openspec_delta_repo();
        let ctx = ctx_for(tmp.path());
        let specs = list_specs(&ctx);
        let info = specs.iter().find(|s| s.id == "add-auth").expect("listed");
        assert_eq!(info.path, "openspec/changes/add-auth");
        assert_eq!(info.title, "Add auth");

        let detail = get_spec(&ctx, "add-auth").expect("found");
        let names: Vec<&str> = detail.artifacts.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["proposal", "design", "tasks", "specs/auth/spec.md"]);

        let tasks = get_tasks(&ctx, "add-auth");
        assert_eq!(
            task_rows(&tasks),
            [
                ("1", 1, false, "1.1 scaffold", true),
                ("2", 1, false, "1.2 wire it", false),
            ]
        );
    }

    /// `mcp-server`: "Every reported path exists".
    #[test]
    fn every_reported_path_exists() {
        for tmp in [
            openspec_delta_repo(),
            markdown_repo(),
            speckit_repo(),
            superpowers_repo(),
        ] {
            let specs = list_specs(&ctx_for(tmp.path()));
            assert!(!specs.is_empty());
            for s in specs {
                assert!(
                    tmp.path().join(&s.path).exists(),
                    "{} ({}) reports missing path {}",
                    s.id,
                    s.backend,
                    s.path
                );
            }
        }
    }

    #[test]
    fn dependency_graph_reads_refs_from_every_artifact() {
        let tasks = "## Phase 1: Setup\n- [ ] T001 [P] build\n";
        let tmp = repo_with(
            "speckit",
            "specs",
            &[
                ("specs/001-login/spec.md", "# Login\n"),
                ("specs/001-login/plan.md", "Needs [[002-signup-T001]].\n"),
                ("specs/001-login/tasks.md", tasks),
                ("specs/002-signup/tasks.md", tasks),
            ],
        );
        let graph = dependency_graph(&ctx_for(tmp.path()));
        assert!(
            graph
                .edges
                .iter()
                .any(|e| e.from == "001-login-T001" && e.to == "002-signup-T001"),
            "{:?}",
            graph.edges
        );
    }

    #[test]
    fn title_falls_back_to_the_prompt_heading_then_the_id() {
        let tmp = repo_with(
            "openspec",
            "openspec/changes",
            &[
                (
                    "openspec/changes/add-auth/proposal.md",
                    "## Why\n\nBecause.\n",
                ),
                (
                    "openspec/changes/add-auth/tasks.md",
                    "# Add auth tasks\n- [ ] 1.1 do\n",
                ),
                ("openspec/changes/bare/proposal.md", "## Why\n"),
                ("openspec/changes/bare/tasks.md", "- [ ] 1.1 do\n"),
            ],
        );
        let specs = list_specs(&ctx_for(tmp.path()));
        let title = |id: &str| specs.iter().find(|s| s.id == id).unwrap().title.clone();
        assert_eq!(title("add-auth"), "Add auth tasks");
        assert_eq!(title("bare"), "bare");
    }

    #[test]
    fn extract_refs_finds_double_bracket_tokens() {
        let refs = extract_refs("see [[a]] and [[ b ]] but not [single]");
        assert_eq!(refs, vec!["a", "b"]);
    }
}
