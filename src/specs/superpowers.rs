//! Superpowers-format backend for spec scanning.
//!
//! Scans [obra/superpowers](https://github.com/obra/superpowers) `writing-plans`
//! documents (`docs/superpowers/plans/*.md`) into [`SpecEntry`] values — one per
//! incomplete plan. Unlike Spec Kit (feature subdirectories with `[P]` parallel
//! tasks), a superpowers plan is a flat file holding a sequential TDD chain for a
//! single worktree, so it never fans out into per-task entries.

use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use super::{SpecBackend, SpecBackendKind, SpecEntry};
use crate::broker::messages::slugify_branch;
use crate::error::PawError;

/// Default directory holding superpowers plan files.
pub(crate) const PLANS_DIR: &str = "docs/superpowers/plans";

/// Backend for the flat-file superpowers plan format.
#[derive(Debug)]
pub struct SuperpowersBackend;

impl SpecBackend for SuperpowersBackend {
    fn scan(&self, dir: &Path) -> Result<Vec<SpecEntry>, PawError> {
        let read = fs::read_dir(dir).map_err(|e| {
            PawError::SpecError(format!("cannot read directory {}: {e}", dir.display()))
        })?;

        // Collect immediate `*.md` files only — subdirectories and other file
        // types are ignored (design docs live under `specs/`, not `plans/`).
        let mut plan_paths: Vec<std::path::PathBuf> = Vec::new();
        for raw in read {
            let raw = raw
                .map_err(|e| PawError::SpecError(format!("error reading directory entry: {e}")))?;
            let path = raw.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
                plan_paths.push(path);
            }
        }
        // Stable ordering by filename.
        plan_paths.sort();

        let mut entries: Vec<SpecEntry> = Vec::new();
        for path in &plan_paths {
            let content = fs::read_to_string(path)
                .map_err(|e| PawError::SpecError(format!("cannot read {}: {e}", path.display())))?;
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let plan = parse_plan(&stem, &content);
            match plan.status() {
                PlanStatus::InScope => entries.push(plan.into_entry()),
                PlanStatus::AllComplete => {
                    eprintln!(
                        "warning: skipping superpowers plan {}: all steps complete",
                        path.display()
                    );
                }
                PlanStatus::NotAPlan => { /* silent — not a task-bearing plan */ }
            }
        }
        Ok(entries)
    }
}

/// A parsed superpowers plan.
struct Plan {
    stem: String,
    goal: Option<String>,
    architecture: Option<String>,
    tech_stack: Option<String>,
    /// Verbatim body from the first `### Task N` heading to the end.
    tasks_body: String,
    incomplete_steps: usize,
    has_tasks: bool,
}

/// In-scope classification for a parsed plan.
enum PlanStatus {
    /// At least one incomplete step remains — produce a `SpecEntry`.
    InScope,
    /// Has tasks but every step is complete — skip with a warning.
    AllComplete,
    /// No recognised `### Task`/step lines — skip silently (e.g. a design doc).
    NotAPlan,
}

impl Plan {
    fn status(&self) -> PlanStatus {
        if !self.has_tasks {
            PlanStatus::NotAPlan
        } else if self.incomplete_steps > 0 {
            PlanStatus::InScope
        } else {
            PlanStatus::AllComplete
        }
    }

    fn into_entry(self) -> SpecEntry {
        let branch = format!("plan/{}", slugify_branch(&self.stem));
        let prompt = self.build_prompt();
        SpecEntry {
            id: self.stem,
            backend: SpecBackendKind::Superpowers,
            branch,
            cli: None,
            prompt,
            owned_files: None,
        }
    }

    /// Assembles the boot prompt: Plan Context, Your Tasks, Execution — joined
    /// by `\n\n---\n\n`.
    fn build_prompt(&self) -> String {
        let mut sections: Vec<String> = Vec::new();

        let mut ctx_parts: Vec<String> = vec!["## Plan Context".to_string()];
        if let Some(g) = &self.goal {
            ctx_parts.push(format!("**Goal:** {g}"));
        }
        if let Some(a) = &self.architecture {
            ctx_parts.push(format!("**Architecture:** {a}"));
        }
        if let Some(t) = &self.tech_stack {
            ctx_parts.push(format!("**Tech Stack:** {t}"));
        }
        sections.push(ctx_parts.join("\n\n"));

        sections.push(format!("## Your Tasks\n\n{}", self.tasks_body.trim_end()));

        sections.push(
            "## Execution\n\nWork the steps in order. As you complete each step, flip its \
             `- [ ]` to `- [x]` in the plan file (mid-flight writeback). Publish `agent.done` \
             only when every step in the plan shows `- [x]`."
                .to_string(),
        );

        sections.join("\n\n---\n\n")
    }
}

fn task_heading_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // `### Task <N>` with leniency on the trailing separator/name.
        Regex::new(r"(?m)^###\s+Task\s+\d+").expect("task heading regex must compile")
    })
}

/// Extracts a `**<name>:**` header field's same-line value, if present.
fn extract_field(content: &str, name: &str) -> Option<String> {
    let marker = format!("**{name}:**");
    for line in content.lines() {
        if let Some(rest) = line.trim().strip_prefix(&marker) {
            let value = rest.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Parses a superpowers plan file into a [`Plan`].
fn parse_plan(stem: &str, content: &str) -> Plan {
    let goal = extract_field(content, "Goal");
    let architecture = extract_field(content, "Architecture");
    let tech_stack = extract_field(content, "Tech Stack");

    let (tasks_body, has_tasks) = match task_heading_re().find(content) {
        Some(m) => (content[m.start()..].to_string(), true),
        None => (String::new(), false),
    };

    // Count incomplete checkbox steps within the tasks body only, so prose /
    // `Files:` lines above the first task never register as steps. A plan is
    // in-scope iff at least one step is still `- [ ]`; completed steps need no
    // separate count (a fully-`- [x]` plan simply has zero incomplete steps).
    let incomplete_steps = tasks_body
        .lines()
        .filter(|l| l.trim_start().starts_with("- [ ]"))
        .count();

    Plan {
        stem: stem.to_string(),
        goal,
        architecture,
        tech_stack,
        tasks_body,
        incomplete_steps,
        has_tasks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const PLAN: &str = "# Add auth Implementation Plan\n\n\
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement.\n\n\
**Goal:** Add token auth to the API\n\n\
**Architecture:** Middleware validates a bearer token\n\n\
**Tech Stack:** Rust, axum\n\n\
---\n\n\
### Task 1: Validation\n\n\
**Files:**\n- Create: `src/auth.rs`\n- Test: `tests/auth.rs`\n\n\
- [ ] **Step 1: Write the failing test**\n\n\
```rust\nassert!(false);\n```\n\n\
Run: `cargo test auth`\n\n\
- [x] **Step 2: Scaffold module**\n";

    fn write_plan(dir: &Path, name: &str, body: &str) {
        fs::write(dir.join(name), body).unwrap();
    }

    #[test]
    fn scan_reads_plan_files_not_subdirs() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(tmp.path(), "2026-07-20-add-auth.md", PLAN);
        write_plan(tmp.path(), "2026-07-21-export.md", PLAN);
        fs::write(tmp.path().join("notes.txt"), "ignore me").unwrap();
        fs::create_dir(tmp.path().join("drafts")).unwrap();

        let entries = SuperpowersBackend.scan(tmp.path()).unwrap();
        assert_eq!(
            entries.len(),
            2,
            "one entry per .md plan; txt + subdir ignored"
        );
        assert!(
            entries
                .iter()
                .all(|e| e.backend == SpecBackendKind::Superpowers)
        );
    }

    #[test]
    fn scan_empty_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(SuperpowersBackend.scan(tmp.path()).unwrap().is_empty());
    }

    #[test]
    fn each_in_scope_plan_yields_exactly_one_entry_no_fanout() {
        let tmp = tempfile::tempdir().unwrap();
        // Two tasks, several steps — must still be ONE entry per plan.
        let multi = "### Task 1: A\n- [ ] step a1\n- [ ] step a2\n\n### Task 2: B\n- [ ] step b1\n";
        write_plan(tmp.path(), "plan.md", multi);
        let entries = SuperpowersBackend.scan(tmp.path()).unwrap();
        assert_eq!(
            entries.len(),
            1,
            "one plan -> one entry, no per-task fan-out"
        );
    }

    #[test]
    fn entry_id_is_stem_and_owned_files_none() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(tmp.path(), "2026-07-20-add-auth.md", PLAN);
        let entries = SuperpowersBackend.scan(tmp.path()).unwrap();
        assert_eq!(entries[0].id, "2026-07-20-add-auth");
        assert!(entries[0].owned_files.is_none());
    }

    #[test]
    fn branch_is_plan_prefixed_slug_with_safe_chars() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(
            tmp.path(),
            "2026-07-20-Add-Auth.md",
            "### Task 1: X\n- [ ] do it\n",
        );
        let entries = SuperpowersBackend.scan(tmp.path()).unwrap();
        assert_eq!(entries[0].branch, "plan/2026-07-20-add-auth");
        assert!(
            entries[0].branch.chars().all(|c| c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || matches!(c, '/' | '_' | '-')),
            "branch has only safe slug chars: {}",
            entries[0].branch
        );
    }

    #[test]
    fn fully_complete_plan_is_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(tmp.path(), "done.md", "### Task 1: X\n- [x] a\n- [X] b\n");
        assert!(SuperpowersBackend.scan(tmp.path()).unwrap().is_empty());
    }

    #[test]
    fn plan_with_remaining_step_is_in_scope() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(
            tmp.path(),
            "p.md",
            "### Task 1: X\n- [x] done\n### Task 2: Y\n- [ ] todo\n",
        );
        assert_eq!(SuperpowersBackend.scan(tmp.path()).unwrap().len(), 1);
    }

    #[test]
    fn non_plan_file_is_skipped_others_still_scan() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(
            tmp.path(),
            "design.md",
            "# A design doc\n\nProse only, no tasks.\n",
        );
        write_plan(tmp.path(), "real.md", "### Task 1: X\n- [ ] do\n");
        let entries = SuperpowersBackend.scan(tmp.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "real");
    }

    #[test]
    fn files_block_code_and_run_lines_do_not_error_or_count_as_steps() {
        let tmp = tempfile::tempdir().unwrap();
        write_plan(tmp.path(), "p.md", PLAN);
        let entries = SuperpowersBackend.scan(tmp.path()).unwrap();
        // PLAN has exactly one incomplete step (Step 1) and one complete (Step 2).
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn boot_prompt_carries_context_tasks_and_writeback_instruction() {
        let plan = parse_plan("add-auth", PLAN);
        let prompt = plan.build_prompt();
        assert!(prompt.contains("Plan Context"));
        assert!(prompt.contains("Add token auth to the API"), "Goal present");
        assert!(prompt.contains("Your Tasks"));
        assert!(
            prompt.contains("### Task 1: Validation"),
            "task heading present"
        );
        assert!(prompt.contains("src/auth.rs"), "Files paths present");
        assert!(prompt.contains("cargo test auth"), "Run command present");
        assert!(
            prompt.contains("- [ ]") && prompt.contains("- [x]"),
            "flip writeback described"
        );
        assert!(prompt.contains("agent.done"), "completion signal described");
    }

    #[test]
    fn parse_counts_only_incomplete_steps_ignoring_completed() {
        // `- [x]` / `- [X]` are not incomplete; only `- [ ]` counts.
        let plan = parse_plan("p", "### Task 1: X\n- [x] a\n- [X] b\n- [ ] c\n");
        assert_eq!(plan.incomplete_steps, 1);
        assert!(plan.has_tasks);
    }
}
