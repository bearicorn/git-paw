//! Pins each format's outputs so moving behaviour into `SpecBackend` keeps
//! them byte-identical. Outputs changed on purpose: the Markdown task prompt
//! (`spec-backend-strategy`), and the Spec Kit, Markdown and Superpowers MCP
//! `list:`, `get_spec:` and `task:` lines, which `mcp-spec-artifacts-per-format`
//! corrected to each format's own files.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::agent_cli::CliFamily;
use crate::config::{self, PawConfig};
use crate::mcp::RepoContext;
use crate::mcp::query::specs as mcp_specs;
use crate::specs::{self, SpecBackendKind};

fn fixture(kind: SpecBackendKind, root: &Path) {
    let write = |rel: &str, body: &str| {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    };
    let (ty, dir) = match kind {
        SpecBackendKind::OpenSpec => {
            write(
                "openspec/changes/add-auth/proposal.md",
                "# Add auth\n\nWhy.\n",
            );
            write("openspec/changes/add-auth/design.md", "# Design\n");
            write(
                "openspec/changes/add-auth/tasks.md",
                "## 1. Setup\n- [x] 1.1 scaffold\n- [ ] 1.2 wire it\n## 2. Docs\n- [ ] 2.1 write\n",
            );
            write(
                "openspec/changes/add-auth/specs/auth/spec.md",
                "## ADDED Requirements\n",
            );
            ("openspec", "openspec/changes")
        }
        SpecBackendKind::Markdown => {
            write(
                "specs/fix-bug.md",
                "---\npaw_status: pending\n---\n# Fix the bug\n\n- [ ] reproduce\n",
            );
            ("markdown", "specs")
        }
        SpecBackendKind::SpecKit => {
            write("memory/constitution.md", "# Constitution\n");
            write("specs/001-login/spec.md", "# Login feature\n");
            write("specs/001-login/plan.md", "# Plan\n");
            write("specs/001-login/checklist.md", "# Checklist\n");
            write(
                "specs/001-login/tasks.md",
                "## Phase 1: Setup\n- [x] T001 init\n- [ ] T002 [P] add form\n## Phase 2: Core\n- [ ] T003 wire\n",
            );
            ("speckit", "specs")
        }
        SpecBackendKind::Superpowers => {
            write(
                "plans/2026-07-20-add-auth.md",
                "# Add auth Implementation Plan\n\n**Goal:** Add auth\n\n### Task 1: Validation\n\n- [ ] **Step 1: test**\n- [x] **Step 2: scaffold**\n",
            );
            ("superpowers", "plans")
        }
    };
    write(
        ".git-paw/config.toml",
        &format!("[specs]\ntype = \"{ty}\"\ndir = \"{dir}\"\n"),
    );
}

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

/// A tasks file exercising phases, a `[P]` marker and both checkbox states.
const TASKS_SAMPLE: &str = "## Phase 1: Setup\n- [x] T001 init\n- [ ] T002 [P] add form\n## Phase 2: Core\n- [ ] T003 wire\n";

/// `kind`'s task parsing over `content`, one rendered line per task.
fn parse_tasks(kind: SpecBackendKind, content: &str) -> Vec<String> {
    kind.backend()
        .parse_tasks(content)
        .into_iter()
        .map(|t| {
            format!(
                "{} {} {} {:?} {}",
                t.id, t.phase, t.parallel, t.description, t.complete
            )
        })
        .collect()
}

fn snapshot(kind: SpecBackendKind) -> String {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fixture(kind, root);
    let config: PawConfig = config::load_config(root, Some(&root.join("no-user.toml"))).unwrap();

    let entries = specs::scan_specs(&config, root).unwrap();
    let entry = entries.first().expect("fixture yields an entry");
    let prompt = crate::commands::launch::build_task_prompt(Some(entry));
    let doctrine = crate::skills::render_spec_path_doctrine(&[kind]);
    let coordination = crate::skills::render(
        &crate::skills::resolve_from("coordination", &crate::skills::SkillRoots::default())
            .unwrap(),
        "feat/x",
        "http://127.0.0.1:9119",
        "git-paw",
        &crate::skills::GateCommands::default(),
        &[kind],
        CliFamily::Generic,
    );

    let ctx = ctx_for(root);
    let mut out = String::new();
    writeln!(out, "prose: {}", prompt.prose).unwrap();
    writeln!(out, "entry_point: {:?}", prompt.entry_point).unwrap();
    writeln!(out, "doctrine: {}", doctrine.trim_end()).unwrap();
    for info in mcp_specs::list_specs(&ctx) {
        assert!(
            root.join(&info.path).exists(),
            "{kind:?}: reported path {} does not exist",
            info.path
        );
        writeln!(
            out,
            "list: {} {} {:?} {}",
            info.id, info.backend, info.title, info.path
        )
        .unwrap();
    }
    let detail = mcp_specs::get_spec(&ctx, &entry.id).expect("spec found");
    assert!(
        root.join(&detail.path).exists(),
        "{kind:?}: get_spec path {} does not exist",
        detail.path
    );
    let names: Vec<&str> = detail.artifacts.iter().map(|a| a.name.as_str()).collect();
    writeln!(
        out,
        "get_spec: {} {} {names:?}",
        detail.backend, detail.path
    )
    .unwrap();
    for t in mcp_specs::get_tasks(&ctx, &entry.id) {
        writeln!(
            out,
            "task: {} {} {} {:?} {}",
            t.id, t.phase, t.parallel, t.description, t.complete
        )
        .unwrap();
    }
    // Pin every parser over one shared sample too, independent of the
    // fixture's own task list.
    for t in parse_tasks(kind, TASKS_SAMPLE) {
        writeln!(out, "parse: {t}").unwrap();
    }
    writeln!(
        out,
        "opsx_regions: {}",
        coordination.contains("/opsx:verify")
    )
    .unwrap();
    writeln!(
        out,
        "opsx_guard: {}",
        specs::opsx_role_gating_applies(&config)
    )
    .unwrap();
    writeln!(
        out,
        "constitution: {}",
        config.governance.constitution.is_some()
    )
    .unwrap();
    out
}

const OPENSPEC: &str = r#"prose: Read .git-paw/AGENTS.local.md first — it carries your assignment and coordination rules. Your workflow command follows as the next message; run it once you have read the sidecar.
entry_point: Some("/opsx:apply add-auth")
doctrine: OpenSpec specs live under `openspec/changes/<change-name>/{proposal,specs,tasks}.md` with archived deltas merged into `openspec/specs/`; run `openspec validate <change-name> --strict` to verify a change.
list: add-auth openspec "Add auth" openspec/changes/add-auth
get_spec: openspec openspec/changes/add-auth ["proposal", "design", "tasks", "specs/auth/spec.md"]
task: 1 1 false "1.1 scaffold" true
task: 2 1 false "1.2 wire it" false
task: 3 2 false "2.1 write" false
parse: 1 1 false "T001 init" true
parse: 2 1 false "T002 [P] add form" false
parse: 3 2 false "T003 wire" false
opsx_regions: true
opsx_guard: true
constitution: false
"#;

const MARKDOWN: &str = r#"prose: Begin your assigned task. Read .git-paw/AGENTS.local.md first — it carries the project rules and your full spec, embedded inline. The spec file is the whole contract; there is no other artifact directory to read.
entry_point: None
doctrine: Markdown specs are flat `.md` files with `paw_status: pending` frontmatter; the format has no per-artifact workflow — the file itself is the contract.
list: fix-bug markdown "Fix the bug" specs/fix-bug.md
get_spec: markdown specs/fix-bug.md ["fix-bug"]
task: 1 0 false "reproduce" false
parse: 1 1 false "T001 init" true
parse: 2 1 false "T002 [P] add form" false
parse: 3 2 false "T003 wire" false
opsx_regions: false
opsx_guard: false
constitution: false
"#;

const SPECKIT: &str = r#"prose: Begin your assigned task. Read .git-paw/AGENTS.local.md first — it already carries the project rules and your full Spec Kit assignment (feature spec, implementation plan, and task phase) embedded inline; no other artifact directory is needed to start.
entry_point: None
doctrine: Spec Kit specs live under `.specify/specs/<feature>/{spec,plan,tasks}.md` and use the Spec Kit checklist convention; mark `- [ ]` tasks complete as each one lands.
list: 001-login-T002 speckit "Login feature" specs/001-login
get_spec: speckit specs/001-login ["spec", "plan", "tasks", "checklist.md"]
task: T001 1 false "init" true
task: T002 1 true "add form" false
task: T003 2 false "wire" false
parse: T001 1 false "init" true
parse: T002 1 true "add form" false
parse: T003 2 false "wire" false
opsx_regions: false
opsx_guard: false
constitution: true
"#;

const SUPERPOWERS: &str = r#"prose: Begin your assigned task. Read .git-paw/AGENTS.local.md first — it carries the project rules and your full superpowers plan (goal, tasks, exact file paths, and per-step verification commands). Work the steps in order and flip `- [ ]` to `- [x]` in the plan as each one lands.
entry_point: None
doctrine: Superpowers plans are flat files under `docs/superpowers/plans/*.md` (obra/superpowers writing-plans documents); work each plan's `### Task N` steps in order and flip `- [ ]` to `- [x]` in the plan file as each step lands.
list: 2026-07-20-add-auth superpowers "Add auth Implementation Plan" plans/2026-07-20-add-auth.md
get_spec: superpowers plans/2026-07-20-add-auth.md ["2026-07-20-add-auth"]
task: 1 0 false "**Step 1: test**" false
task: 2 0 false "**Step 2: scaffold**" true
parse: 1 1 false "T001 init" true
parse: 2 1 false "T002 [P] add form" false
parse: 3 2 false "T003 wire" false
opsx_regions: false
opsx_guard: false
constitution: false
"#;

/// `spec-providers` scenarios "Per-format outputs are unchanged by the
/// move", "opsx role gating follows the backend" (`opsx_regions`,
/// `opsx_guard`), "Spec Kit governance discovery follows the backend"
/// (`constitution`) and "MCP spec paths use the scan's directory" (`list`,
/// `get_spec` paths under each fixture's configured `dir`, each naming an
/// existing file or directory, as `mcp-spec-artifacts-per-format` tightened).
#[test]
fn per_format_outputs_are_pinned() {
    for (kind, expected) in [
        (SpecBackendKind::OpenSpec, OPENSPEC),
        (SpecBackendKind::Markdown, MARKDOWN),
        (SpecBackendKind::SpecKit, SPECKIT),
        (SpecBackendKind::Superpowers, SUPERPOWERS),
    ] {
        assert_eq!(snapshot(kind), expected, "{kind:?} output drifted");
    }
}
