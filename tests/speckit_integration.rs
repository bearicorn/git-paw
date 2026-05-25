//! Integration tests for the Spec Kit backend.
//!
//! Builds a fixture `.specify/` tree with three features whose `tasks.md`
//! files exercise the full decomposition matrix:
//!
//! - Feature 1 (`001-alpha`): phase 1 fully complete, phase 2 mixed `[P]`/non-`[P]`.
//! - Feature 2 (`002-beta`): phase 1 only `[P]`, phase 2 only non-`[P]` (deferred).
//! - Feature 3 (`003-gamma`): fully complete (skipped).
//!
//! Plus a `.specify/memory/constitution.md` for the constitution probe.

use std::fs;

use git_paw::config::{PawConfig, SpecsConfig};
use git_paw::specs::scan_specs_with_override;
use git_paw::specs::speckit::detect_constitution;

/// Builds the fixture tree rooted at `root` and returns the `.specify/specs/`
/// path.
fn build_fixture(root: &std::path::Path) -> std::path::PathBuf {
    let specify = root.join(".specify");
    let specs = specify.join("specs");
    let memory = specify.join("memory");
    fs::create_dir_all(&specs).unwrap();
    fs::create_dir_all(&memory).unwrap();
    fs::write(memory.join("constitution.md"), "Be excellent.").unwrap();

    // Feature 1: phase 1 done, phase 2 mixed [P] + non-[P].
    let f1 = specs.join("001-alpha");
    fs::create_dir(&f1).unwrap();
    fs::write(f1.join("spec.md"), "Alpha feature spec content.").unwrap();
    fs::write(f1.join("plan.md"), "Alpha plan content.").unwrap();
    fs::create_dir(f1.join("checklists")).unwrap();
    fs::write(
        f1.join("checklists/security.md"),
        "Alpha security checklist.",
    )
    .unwrap();
    fs::write(
        f1.join("tasks.md"),
        "## Phase 1: Setup\n\
         - [x] T001 Initial scaffold\n\
         - [x] T002 Wire deps\n\
         ## Phase 2: Foundational\n\
         - [ ] T003 [P] Build login form\n\
         - [ ] T004 [P] Build signup form\n\
         - [ ] T005 Setup database schema\n\
         - [ ] T006 Create auth tables\n\
         - [ ] T007 Seed test data\n",
    )
    .unwrap();

    // Feature 2: phase 1 only [P] (2 entries), phase 2 only non-[P] (deferred).
    let f2 = specs.join("002-beta");
    fs::create_dir(&f2).unwrap();
    fs::write(f2.join("spec.md"), "Beta feature spec content.").unwrap();
    fs::write(
        f2.join("tasks.md"),
        "## Phase 1: Bootstrap\n\
         - [ ] T010 [P] Contract test A\n\
         - [ ] T011 [P] Contract test B\n\
         ## Phase 2: Integration\n\
         - [ ] T020 Wire-up A and B\n\
         - [ ] T021 Polish\n",
    )
    .unwrap();

    // Feature 3: fully complete.
    let f3 = specs.join("003-gamma");
    fs::create_dir(&f3).unwrap();
    fs::write(
        f3.join("tasks.md"),
        "## Phase 1: Setup\n\
         - [x] T030 Done\n\
         - [x] T031 Also done\n",
    )
    .unwrap();

    specs
}

fn config_with_specify(_specs_dir_relative: &str) -> PawConfig {
    PawConfig {
        specs: Some(SpecsConfig {
            dir: Some(".specify/specs".to_string()),
            spec_type: Some("speckit".to_string()),
        }),
        ..Default::default()
    }
}

#[test]
fn scan_returns_expected_entries_per_feature() {
    let tmp = tempfile::tempdir().unwrap();
    let _specs_dir = build_fixture(tmp.path());
    let config = config_with_specify(".specify/specs");

    let entries = scan_specs_with_override(&config, tmp.path(), None).unwrap();

    // Feature 1: 2 [P] entries + 1 consolidated = 3.
    // Feature 2: 2 [P] entries.
    // Feature 3: 0 (fully complete).
    assert_eq!(entries.len(), 5, "got entries: {entries:?}");

    let ids: std::collections::HashSet<String> = entries.iter().map(|e| e.id.clone()).collect();
    assert!(ids.contains("001-alpha-T003"), "missing 001-alpha-T003");
    assert!(ids.contains("001-alpha-T004"), "missing 001-alpha-T004");
    assert!(
        ids.contains("001-alpha-phase-2"),
        "missing 001-alpha-phase-2"
    );
    assert!(ids.contains("002-beta-T010"), "missing 002-beta-T010");
    assert!(ids.contains("002-beta-T011"), "missing 002-beta-T011");
    assert!(
        !ids.iter().any(|id| id.starts_with("003-gamma")),
        "fully complete feature should be skipped"
    );
    // Phase 2 of beta is deferred — never produced.
    assert!(!ids.contains("002-beta-phase-2"));
    assert!(!ids.contains("002-beta-T020"));
}

#[test]
fn branch_shapes_match_spec_kit_format() {
    let tmp = tempfile::tempdir().unwrap();
    build_fixture(tmp.path());
    let config = config_with_specify(".specify/specs");

    let entries = scan_specs_with_override(&config, tmp.path(), None).unwrap();
    let by_id: std::collections::HashMap<String, &git_paw::specs::SpecEntry> =
        entries.iter().map(|e| (e.id.clone(), e)).collect();

    let t003 = by_id.get("001-alpha-T003").unwrap();
    assert_eq!(t003.branch, "task/t003-build-login-form");

    let phase = by_id.get("001-alpha-phase-2").unwrap();
    assert_eq!(phase.branch, "phase/001-alpha-foundational");
}

#[test]
fn entry_prompts_contain_spec_and_plan_content() {
    let tmp = tempfile::tempdir().unwrap();
    build_fixture(tmp.path());
    let config = config_with_specify(".specify/specs");

    let entries = scan_specs_with_override(&config, tmp.path(), None).unwrap();

    // Alpha entries should carry Alpha's spec.md, plan.md, and checklists.
    let alpha = entries
        .iter()
        .find(|e| e.id == "001-alpha-phase-2")
        .unwrap();
    assert!(alpha.prompt.contains("Alpha feature spec content."));
    assert!(alpha.prompt.contains("Alpha plan content."));
    assert!(alpha.prompt.contains("Alpha security checklist."));
    assert!(alpha.prompt.contains("T005"));
    assert!(alpha.prompt.contains("T006"));
    assert!(alpha.prompt.contains("T007"));
    // Sequential instruction is present on consolidated entries only.
    assert!(alpha.prompt.contains("agent.done"));

    // Beta has spec.md but no plan.md — Implementation Plan section is absent.
    let beta = entries.iter().find(|e| e.id == "002-beta-T010").unwrap();
    assert!(beta.prompt.contains("Beta feature spec content."));
    assert!(!beta.prompt.contains("## Implementation Plan"));
    // Single-[P] entries do not carry the agent.done writeback instruction.
    assert!(!beta.prompt.contains("agent.done"));
}

#[test]
fn constitution_probe_returns_fixture_path() {
    let tmp = tempfile::tempdir().unwrap();
    let specs_dir = build_fixture(tmp.path());

    let detected = detect_constitution(&specs_dir).expect("constitution should be detected");
    assert!(detected.ends_with("memory/constitution.md"));
    assert!(detected.is_file());
}

#[test]
fn flipping_a_phase_two_task_to_x_advances_to_next_phase() {
    let tmp = tempfile::tempdir().unwrap();
    let specs_dir = build_fixture(tmp.path());
    let config = config_with_specify(".specify/specs");

    // Initial scan: feature 002-beta is in phase 1 ([P] tasks).
    let entries_before = scan_specs_with_override(&config, tmp.path(), None).unwrap();
    let beta_before: Vec<_> = entries_before
        .iter()
        .filter(|e| e.id.starts_with("002-beta"))
        .collect();
    assert_eq!(beta_before.len(), 2);
    assert!(beta_before.iter().all(|e| e.branch.starts_with("task/")));

    // Mark both T010 and T011 done — phase 2 should become current.
    let beta_tasks = specs_dir.join("002-beta").join("tasks.md");
    fs::write(
        &beta_tasks,
        "## Phase 1: Bootstrap\n\
         - [x] T010 [P] Contract test A\n\
         - [x] T011 [P] Contract test B\n\
         ## Phase 2: Integration\n\
         - [ ] T020 Wire-up A and B\n\
         - [ ] T021 Polish\n",
    )
    .unwrap();

    let entries_after = scan_specs_with_override(&config, tmp.path(), None).unwrap();
    let beta_after: Vec<_> = entries_after
        .iter()
        .filter(|e| e.id.starts_with("002-beta"))
        .collect();
    assert_eq!(
        beta_after.len(),
        1,
        "expected exactly one consolidated entry"
    );
    assert_eq!(beta_after[0].id, "002-beta-phase-2");
    assert!(beta_after[0].branch.starts_with("phase/"));
}

#[test]
fn auto_detect_via_cli_override_routes_to_speckit() {
    let tmp = tempfile::tempdir().unwrap();
    build_fixture(tmp.path());

    // Config has no [specs] — without auto-detect the scan would fail. We
    // pass --specs-format speckit to force the SpecKit backend, which carries
    // its own ".specify/specs" default dir.
    let config = PawConfig::default();
    let entries = scan_specs_with_override(&config, tmp.path(), Some("speckit")).unwrap();
    assert!(
        !entries.is_empty(),
        "speckit override should produce entries"
    );
}
