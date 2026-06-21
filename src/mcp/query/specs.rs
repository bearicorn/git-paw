//! Spec & task reads across the `OpenSpec`, Markdown, and Spec Kit backends.
//!
//! Uses the same discovery (`specs::scan_specs_with_override`) that
//! `git paw start --from-all-specs` uses, then reads per-backend artifacts
//! from their conventional locations. Degrades to empty lists when no specs
//! are configured or discoverable.

use std::path::{Path, PathBuf};

use rmcp::schemars;
use serde::Serialize;

use crate::config::{self, PawConfig};
use crate::mcp::RepoContext;
use crate::specs::{self, SpecBackendKind, SpecEntry};

/// Maps a backend kind to its tool-facing string.
fn backend_str(kind: SpecBackendKind) -> &'static str {
    match kind {
        SpecBackendKind::OpenSpec => "openspec",
        SpecBackendKind::Markdown => "markdown",
        SpecBackendKind::SpecKit => "speckit",
    }
}

/// Resolved spec discovery context.
struct Discovery {
    repo_root: PathBuf,
    /// Directory specs are scanned from (relative to repo root).
    dir: String,
    entries: Vec<SpecEntry>,
}

/// Replicates `resolve_specs_config`'s directory choice (which is private):
/// explicit `[specs].dir`, else Spec Kit auto-detection.
fn resolve_dir(config: &PawConfig, repo_root: &Path) -> Option<String> {
    if let Some(specs) = config.specs.as_ref() {
        return Some(specs.dir.clone().unwrap_or_else(|| "specs".to_string()));
    }
    let specify = repo_root.join(".specify");
    if specify.is_dir() && specify.join("specs").is_dir() {
        return Some(".specify/specs".to_string());
    }
    None
}

/// Discovers specs for the repository, degrading to an empty set on any
/// discovery error (no config, missing directory, etc.).
fn discover(ctx: &RepoContext) -> Discovery {
    let repo_root = ctx.root.clone();
    let config = config::load_config(&repo_root, None).unwrap_or_default();
    let dir = resolve_dir(&config, &repo_root).unwrap_or_else(|| "specs".to_string());
    let entries = specs::scan_specs(&config, &repo_root).unwrap_or_default();
    Discovery {
        repo_root,
        dir,
        entries,
    }
}

fn first_heading(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("# ").map(str::trim))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Derives a human title for a spec: the first `# ` heading in its primary
/// artifact (proposal.md for `OpenSpec`, spec.md for Spec Kit), else the entry
/// prompt's heading, else the id.
fn derive_title(spec_dir: &Path, entry: &SpecEntry) -> String {
    let primary = match entry.backend {
        SpecBackendKind::OpenSpec => "proposal.md",
        SpecBackendKind::SpecKit => "spec.md",
        SpecBackendKind::Markdown => "",
    };
    if !primary.is_empty()
        && let Ok(content) = std::fs::read_to_string(spec_dir.join(primary))
        && let Some(h) = first_heading(&content)
    {
        return h;
    }
    first_heading(&entry.prompt).unwrap_or_else(|| entry.id.clone())
}

/// One discovered spec summary.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SpecInfo {
    /// Spec id (directory or file stem).
    pub id: String,
    /// Backend: "openspec" | "markdown" | "speckit".
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
    d.entries
        .iter()
        .map(|e| {
            let spec_dir = d.repo_root.join(&d.dir).join(&e.id);
            SpecInfo {
                id: e.id.clone(),
                backend: backend_str(e.backend).to_string(),
                title: derive_title(&spec_dir, e),
                status: "pending".to_string(),
                path: format!("{}/{}", d.dir.trim_end_matches('/'), e.id),
            }
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

fn read_named(dir: &Path, file: &str) -> Option<Artifact> {
    let content = std::fs::read_to_string(dir.join(file)).ok()?;
    Some(Artifact {
        name: file.trim_end_matches(".md").to_string(),
        content,
    })
}

/// Returns the full content of a named spec, or `None` when not found.
#[must_use]
pub fn get_spec(ctx: &RepoContext, id: &str) -> Option<SpecDetail> {
    let d = discover(ctx);
    let entry = d.entries.iter().find(|e| e.id == id)?;
    let spec_dir = d.repo_root.join(&d.dir).join(id);
    let rel_path = format!("{}/{}", d.dir.trim_end_matches('/'), id);

    let mut artifacts = Vec::new();
    match entry.backend {
        SpecBackendKind::OpenSpec => {
            for f in ["proposal.md", "design.md", "tasks.md"] {
                if let Some(a) = read_named(&spec_dir, f) {
                    artifacts.push(a);
                }
            }
            // Capability spec files under specs/<cap>/spec.md.
            let specs_sub = spec_dir.join("specs");
            collect_spec_md(&specs_sub, &spec_dir, &mut artifacts);
        }
        SpecBackendKind::SpecKit => {
            for f in ["spec.md", "plan.md", "tasks.md"] {
                if let Some(a) = read_named(&spec_dir, f) {
                    artifacts.push(a);
                }
            }
            // Checklists: any other *.md in the feature dir.
            if let Ok(rd) = std::fs::read_dir(&spec_dir) {
                let mut extra: Vec<_> = rd
                    .flatten()
                    .filter_map(|e| {
                        let p = e.path();
                        let is_md = p.extension().is_some_and(|x| x.eq_ignore_ascii_case("md"));
                        let name = p.file_name()?.to_str()?.to_string();
                        let lower = name.to_ascii_lowercase();
                        if is_md && !["spec.md", "plan.md", "tasks.md"].contains(&lower.as_str()) {
                            Some((name, std::fs::read_to_string(&p).ok()?))
                        } else {
                            None
                        }
                    })
                    .collect();
                extra.sort_by(|a, b| a.0.cmp(&b.0));
                for (name, content) in extra {
                    artifacts.push(Artifact { name, content });
                }
            }
        }
        SpecBackendKind::Markdown => {
            // Markdown specs are single files; the entry prompt holds the body.
            artifacts.push(Artifact {
                name: id.to_string(),
                content: entry.prompt.clone(),
            });
        }
    }

    Some(SpecDetail {
        id: id.to_string(),
        backend: backend_str(entry.backend).to_string(),
        path: rel_path,
        artifacts,
    })
}

/// Recursively collects `spec.md` files under `dir` into artifacts, naming each
/// by its path relative to `base`.
fn collect_spec_md(dir: &Path, base: &Path, out: &mut Vec<Artifact>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_spec_md(&path, base, out);
        } else if path.file_name().and_then(|n| n.to_str()) == Some("spec.md")
            && let Ok(content) = std::fs::read_to_string(&path)
        {
            let name = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            out.push(Artifact { name, content });
        }
    }
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
    let Some(entry) = d.entries.iter().find(|e| e.id == spec) else {
        return Vec::new();
    };
    let spec_dir = d.repo_root.join(&d.dir).join(spec);
    let tasks_path = spec_dir.join("tasks.md");
    let Ok(content) = std::fs::read_to_string(&tasks_path) else {
        return Vec::new();
    };

    match entry.backend {
        SpecBackendKind::SpecKit => specs::speckit::parse_tasks_md(&content)
            .into_iter()
            .flat_map(|phase| {
                phase.tasks.into_iter().map(move |t| TaskInfo {
                    id: t.id,
                    phase: t.phase,
                    parallel: t.p_marker,
                    description: t.description,
                    complete: t.complete,
                })
            })
            .collect(),
        // OpenSpec / Markdown tasks.md: flat checkbox list, phases from `## `.
        _ => parse_checkbox_tasks(&content),
    }
}

/// Parses a flat Markdown checkbox task list, tracking `## ` headings as
/// phases. Used for `OpenSpec` `tasks.md`.
fn parse_checkbox_tasks(content: &str) -> Vec<TaskInfo> {
    let mut phase = 0u32;
    let mut seq = 0u32;
    let mut out = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("## ") {
            phase += 1;
            continue;
        }
        let Some(rest) = t.strip_prefix("- [").or_else(|| t.strip_prefix("* [")) else {
            continue;
        };
        let Some(mark) = rest.chars().next() else {
            continue;
        };
        let desc = rest.get(2..).unwrap_or("").trim().to_string();
        seq += 1;
        out.push(TaskInfo {
            id: format!("{seq}"),
            phase,
            parallel: false,
            description: desc,
            complete: mark == 'x' || mark == 'X',
        });
    }
    out
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

/// Builds the spec dependency graph from cross-references in each spec's text.
#[must_use]
pub fn dependency_graph(ctx: &RepoContext) -> DependencyGraph {
    let d = discover(ctx);
    let ids: std::collections::HashSet<String> = d.entries.iter().map(|e| e.id.clone()).collect();

    let nodes = d
        .entries
        .iter()
        .map(|e| GraphNode {
            id: e.id.clone(),
            backend: backend_str(e.backend).to_string(),
        })
        .collect();

    let mut edges = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for entry in &d.entries {
        // Prefer the proposal for OpenSpec; otherwise scan the prompt text.
        let spec_dir = d.repo_root.join(&d.dir).join(&entry.id);
        let text = std::fs::read_to_string(spec_dir.join("proposal.md"))
            .unwrap_or_else(|_| entry.prompt.clone());
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

    fn ctx_for(root: &Path) -> RepoContext {
        RepoContext {
            root: root.to_path_buf(),
            git_paw_dir: None,
            broker_url: None,
            server_name: "git-paw".to_string(),
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

    #[test]
    fn extract_refs_finds_double_bracket_tokens() {
        let refs = extract_refs("see [[a]] and [[ b ]] but not [single]");
        assert_eq!(refs, vec!["a", "b"]);
    }
}
