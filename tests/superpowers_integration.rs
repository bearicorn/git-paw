//! Integration tests for the Superpowers spec backend.
//!
//! Builds a fixture `docs/superpowers/plans/` tree of flat `*.md` plan files
//! (the obra/superpowers `writing-plans` layout) and drives the public
//! `scan_specs`/`scan_specs_with_override` dispatch to guard the backend's
//! observable contract end-to-end:
//!
//! - `2026-07-20-add-auth.md`: in-scope plan (Goal/Architecture/Tech Stack +
//!   one task with a `Files:` block, a `Run:` line, one `- [ ]` and one `- [x]`
//!   step).
//! - `2026-07-21-Export-CSV.md`: in-scope, mixed-case stem, two tasks / several
//!   steps — exercises the one-entry-per-plan (no fan-out) rule and branch-slug
//!   lowercasing.
//! - `done.md`: every step `- [x]` (fully complete → skipped with a warning).
//! - `design-notes.md`: prose only, no `### Task` heading (not a plan → skipped
//!   silently).
//! - `notes.txt` and a `drafts/` subdirectory (both ignored).

use std::fs;

use git_paw::config::{PawConfig, SpecsConfig};
use git_paw::specs::{SpecBackendKind, scan_specs, scan_specs_with_override};

/// Conventional Superpowers plans directory, relative to the repo root. Mirrors
/// the backend's `PLANS_DIR` default (which is `pub(crate)` and so unreachable
/// from an integration test — hard-coded here as the `speckit` tests do for
/// `.specify/specs`).
const PLANS_DIR: &str = "docs/superpowers/plans";

const ADD_AUTH_PLAN: &str = r#"# Add auth Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement.

**Goal:** Add token auth to the API

**Architecture:** Middleware validates a bearer token

**Tech Stack:** Rust, axum

---

### Task 1: Validation

**Files:**
- Create: `src/auth.rs`
- Test: `tests/auth.rs`

- [ ] **Step 1: Write the failing test**

```rust
assert!(false);
```

Run: `cargo test auth`

- [x] **Step 2: Scaffold module**
"#;

const EXPORT_CSV_PLAN: &str = r#"# Export CSV Implementation Plan

**Goal:** Export data as CSV

### Task 1: Writer

- [ ] step a1
- [ ] step a2

### Task 2: Formatter

- [x] step b1
- [ ] step b2
"#;

const DONE_PLAN: &str = "### Task 1: X\n- [x] a\n- [X] b\n";

const DESIGN_NOTES: &str = "# A design doc\n\nProse only, no tasks.\n";

/// Builds the fixture plans tree rooted at `root` and returns the
/// `docs/superpowers/plans/` path.
fn build_fixture(root: &std::path::Path) -> std::path::PathBuf {
    let plans = root.join(PLANS_DIR);
    fs::create_dir_all(&plans).unwrap();

    fs::write(plans.join("2026-07-20-add-auth.md"), ADD_AUTH_PLAN).unwrap();
    fs::write(plans.join("2026-07-21-Export-CSV.md"), EXPORT_CSV_PLAN).unwrap();
    fs::write(plans.join("done.md"), DONE_PLAN).unwrap();
    fs::write(plans.join("design-notes.md"), DESIGN_NOTES).unwrap();

    // Non-`.md` file and a subdirectory (with a nested plan) — both ignored.
    fs::write(plans.join("notes.txt"), "ignore me").unwrap();
    let drafts = plans.join("drafts");
    fs::create_dir(&drafts).unwrap();
    fs::write(drafts.join("nested.md"), ADD_AUTH_PLAN).unwrap();

    plans
}

fn config_with_superpowers() -> PawConfig {
    PawConfig {
        specs: Some(SpecsConfig {
            dir: Some(PLANS_DIR.to_string()),
            spec_type: Some("superpowers".to_string()),
        }),
        ..Default::default()
    }
}

#[test]
fn scan_yields_one_entry_per_in_scope_plan_no_fanout() {
    let tmp = tempfile::tempdir().unwrap();
    build_fixture(tmp.path());
    let config = config_with_superpowers();

    let entries = scan_specs(&config, tmp.path()).unwrap();

    // Two in-scope plans; `done.md`, `design-notes.md`, `notes.txt`, and the
    // `drafts/` subdir all produce nothing. Export-CSV has two tasks / four
    // steps yet contributes exactly ONE entry — proving no per-task fan-out.
    assert_eq!(entries.len(), 2, "got entries: {entries:?}");

    let ids: std::collections::HashSet<String> = entries.iter().map(|e| e.id.clone()).collect();
    assert!(
        ids.contains("2026-07-20-add-auth"),
        "missing add-auth entry"
    );
    assert!(
        ids.contains("2026-07-21-Export-CSV"),
        "id is the plan file stem verbatim (not slugified); got: {ids:?}"
    );

    // Every entry carries the Superpowers backend tag, no CLI override, and no
    // file ownership (the format does not support ownership).
    assert!(
        entries
            .iter()
            .all(|e| e.backend == SpecBackendKind::Superpowers),
        "all entries tagged Superpowers"
    );
    assert!(entries.iter().all(|e| e.cli.is_none()));
    assert!(entries.iter().all(|e| e.owned_files.is_none()));
}

#[test]
fn branch_is_plan_prefixed_slug_with_safe_chars() {
    let tmp = tempfile::tempdir().unwrap();
    build_fixture(tmp.path());
    let config = config_with_superpowers();

    let entries = scan_specs(&config, tmp.path()).unwrap();
    let by_id: std::collections::HashMap<String, &git_paw::specs::SpecEntry> =
        entries.iter().map(|e| (e.id.clone(), e)).collect();

    let add_auth = by_id.get("2026-07-20-add-auth").unwrap();
    assert_eq!(add_auth.branch, "plan/2026-07-20-add-auth");

    // Mixed-case stem is lowercased through `slugify_branch` for the branch,
    // while the id stays verbatim.
    let export = by_id.get("2026-07-21-Export-CSV").unwrap();
    assert_eq!(export.branch, "plan/2026-07-21-export-csv");

    for entry in &entries {
        assert!(
            entry.branch.starts_with("plan/"),
            "branch is plan/-prefixed: {}",
            entry.branch
        );
        assert!(
            entry.branch.chars().all(|c| c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || matches!(c, '/' | '_' | '-')),
            "branch has only safe slug chars: {}",
            entry.branch
        );
    }
}

#[test]
fn boot_prompt_carries_plan_context_tasks_and_writeback_instruction() {
    let tmp = tempfile::tempdir().unwrap();
    build_fixture(tmp.path());
    let config = config_with_superpowers();

    let entries = scan_specs(&config, tmp.path()).unwrap();
    let add_auth = entries
        .iter()
        .find(|e| e.id == "2026-07-20-add-auth")
        .unwrap();
    let prompt = &add_auth.prompt;

    // Section 1: Plan Context with the header fields verbatim.
    assert!(prompt.contains("## Plan Context"), "Plan Context section");
    assert!(prompt.contains("Add token auth to the API"), "Goal present");
    assert!(
        prompt.contains("Middleware validates a bearer token"),
        "Architecture present"
    );
    assert!(prompt.contains("Rust, axum"), "Tech Stack present");

    // Section 2: Your Tasks with the task heading, Files paths, and Run command.
    assert!(prompt.contains("## Your Tasks"), "Your Tasks section");
    assert!(prompt.contains("### Task 1: Validation"), "task heading");
    assert!(prompt.contains("src/auth.rs"), "Files path present");
    assert!(prompt.contains("cargo test auth"), "Run command present");

    // Section 3: Execution with the checkbox-writeback + completion-signal text.
    assert!(prompt.contains("## Execution"), "Execution section");
    assert!(
        prompt.contains("- [ ]") && prompt.contains("- [x]"),
        "writeback (flip - [ ] to - [x]) described"
    );
    assert!(prompt.contains("agent.done"), "completion signal described");

    // Sections are joined by the `\n\n---\n\n` separator.
    assert!(
        prompt.contains("\n\n---\n\n"),
        "sections joined by --- separator; got: {prompt}"
    );
}

#[test]
fn complete_and_non_plan_files_are_skipped_without_failing_the_scan() {
    let tmp = tempfile::tempdir().unwrap();
    build_fixture(tmp.path());
    let config = config_with_superpowers();

    // The scan succeeds even though one plan is fully complete and one file is
    // not a plan at all — skipping either never fails the overall scan.
    let entries = scan_specs(&config, tmp.path()).unwrap();
    let ids: std::collections::HashSet<String> = entries.iter().map(|e| e.id.clone()).collect();

    // Fully-complete plan (`done.md`, all `- [x]`) is not launched.
    assert!(
        !ids.contains("done"),
        "fully-complete plan must be skipped; got: {ids:?}"
    );
    // Prose-only file with no `### Task` heading (`design-notes.md`) is skipped.
    assert!(
        !ids.contains("design-notes"),
        "non-plan file must be skipped; got: {ids:?}"
    );
    // The other in-scope plans still produced their entries.
    assert!(ids.contains("2026-07-20-add-auth"));
    assert!(ids.contains("2026-07-21-Export-CSV"));
}

#[test]
fn cli_override_superpowers_routes_and_supplies_default_dir() {
    let tmp = tempfile::tempdir().unwrap();
    // Fixture lives at the conventional `docs/superpowers/plans/` path.
    build_fixture(tmp.path());

    // Config has no [specs] section. `--specs-format superpowers` selects the
    // Superpowers backend and supplies its `docs/superpowers/plans` default dir.
    let config = PawConfig::default();
    let entries = scan_specs_with_override(&config, tmp.path(), Some("superpowers")).unwrap();

    assert_eq!(
        entries.len(),
        2,
        "override should route to Superpowers and find the two in-scope plans"
    );
    assert!(
        entries
            .iter()
            .all(|e| e.backend == SpecBackendKind::Superpowers),
        "override-routed entries are tagged Superpowers"
    );
    assert!(
        entries.iter().all(|e| e.branch.starts_with("plan/")),
        "Superpowers-supplied plan/ branch is preserved (not overwritten with spec/)"
    );
}

#[test]
fn unknown_spec_type_error_lists_superpowers_among_known_types() {
    let tmp = tempfile::tempdir().unwrap();
    // The dir must exist so dispatch reaches backend selection (rather than
    // failing earlier on a missing directory).
    build_fixture(tmp.path());
    let config = PawConfig {
        specs: Some(SpecsConfig {
            dir: Some(PLANS_DIR.to_string()),
            spec_type: Some("unrecognised".to_string()),
        }),
        ..Default::default()
    };

    let err = scan_specs(&config, tmp.path()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unrecognised"),
        "error names the unknown type; got: {msg}"
    );
    assert!(
        msg.contains("superpowers"),
        "unknown-type error lists known types incl superpowers; got: {msg}"
    );
}
