//! `skill-agent` / *Skill resolution order*: the `supervisor` alias of the
//! `orchestrator` skill on the override location production `resolve` reads
//! (`.agents/skills/<name>/SKILL.md`, from the current directory upward).
//!
//! These tests change the process's current directory, so they live in their
//! own test binary — a planted `supervisor` override must never leak into
//! another test's `resolve("supervisor")` — and run serially.

use std::path::Path;

use git_paw::skills::{SkillTemplate, Source, resolve};
use serial_test::serial;

/// Writes `.agents/skills/<dir_name>/SKILL.md` under `project_dir` with
/// `body` after a valid frontmatter block.
fn write_override(project_dir: &Path, dir_name: &str, body: &str) {
    let skill_dir = project_dir.join(".agents").join("skills").join(dir_name);
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {dir_name}\ndescription: override\n---\n\n{body}"),
    )
    .unwrap();
}

/// Resolves `name` with the process's current directory at `project_dir`.
fn resolve_from(project_dir: &Path, name: &str) -> SkillTemplate {
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(project_dir).unwrap();
    let result = resolve(name);
    std::env::set_current_dir(original_dir).unwrap();
    result.expect("should resolve")
}

// Scenario: supervisor is an alias of orchestrator
#[test]
#[serial]
fn supervisor_and_orchestrator_resolve_identical_embedded_content() {
    let dir = tempfile::tempdir().unwrap();
    let supervisor = resolve_from(dir.path(), "supervisor");
    let orchestrator = resolve_from(dir.path(), "orchestrator");
    assert_eq!(supervisor.source, Source::Embedded);
    assert_eq!(orchestrator.source, Source::Embedded);
    assert_eq!(supervisor.content, orchestrator.content);
}

// Scenario: A legacy supervisor override applies to the orchestrator name
#[test]
#[serial]
fn legacy_supervisor_override_applies_to_the_orchestrator_name() {
    let dir = tempfile::tempdir().unwrap();
    write_override(dir.path(), "supervisor", "legacy supervisor override");

    let tmpl = resolve_from(dir.path(), "orchestrator");
    assert_eq!(tmpl.source, Source::AgentsStandard);
    assert!(tmpl.content.contains("legacy supervisor override"));
}

// An orchestrator override applies to the legacy name too.
#[test]
#[serial]
fn orchestrator_override_applies_to_the_supervisor_name() {
    let dir = tempfile::tempdir().unwrap();
    write_override(dir.path(), "orchestrator", "orchestrator override");

    let tmpl = resolve_from(dir.path(), "supervisor");
    assert_eq!(tmpl.source, Source::AgentsStandard);
    assert!(tmpl.content.contains("orchestrator override"));
}

// Scenario: An orchestrator override wins over a supervisor override
#[test]
#[serial]
fn orchestrator_override_wins_over_supervisor_override() {
    let dir = tempfile::tempdir().unwrap();
    write_override(dir.path(), "supervisor", "legacy supervisor override");
    write_override(dir.path(), "orchestrator", "orchestrator override");

    for name in ["supervisor", "orchestrator"] {
        let tmpl = resolve_from(dir.path(), name);
        assert!(
            tmpl.content.contains("orchestrator override"),
            "{name} must resolve the orchestrator override"
        );
        assert!(!tmpl.content.contains("legacy supervisor override"));
    }
}
