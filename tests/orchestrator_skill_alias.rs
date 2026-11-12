//! `skill-agent` / *Skill resolution order*: the `supervisor` alias of the
//! `orchestrator` skill on the repository override location
//! (`<repo>/.agents/skills/<name>/SKILL.md`), resolved through explicit roots
//! with no user level, so no test touches the process's current directory.

use std::path::Path;

use git_paw::skills::{SkillRoots, SkillTemplate, Source};

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

/// Resolves `name` with `project_dir` as the repository level.
fn resolve_in(project_dir: &Path, name: &str) -> SkillTemplate {
    let roots = SkillRoots {
        repo_root: Some(project_dir.to_path_buf()),
        user_config_dir: None,
    };
    git_paw::skills::resolve_from(name, &roots).expect("should resolve")
}

// Scenario: supervisor is an alias of orchestrator
#[test]
fn supervisor_and_orchestrator_resolve_identical_embedded_content() {
    let dir = tempfile::tempdir().unwrap();
    let supervisor = resolve_in(dir.path(), "supervisor");
    let orchestrator = resolve_in(dir.path(), "orchestrator");
    assert_eq!(supervisor.source, Source::Embedded);
    assert_eq!(orchestrator.source, Source::Embedded);
    assert_eq!(supervisor.content, orchestrator.content);
}

// Scenario: A legacy supervisor override applies to the orchestrator name
#[test]
fn legacy_supervisor_override_applies_to_the_orchestrator_name() {
    let dir = tempfile::tempdir().unwrap();
    write_override(dir.path(), "supervisor", "legacy supervisor override");

    let tmpl = resolve_in(dir.path(), "orchestrator");
    assert_eq!(tmpl.source, Source::AgentsStandard);
    assert!(tmpl.content.contains("legacy supervisor override"));
}

// An orchestrator override applies to the legacy name too.
#[test]
fn orchestrator_override_applies_to_the_supervisor_name() {
    let dir = tempfile::tempdir().unwrap();
    write_override(dir.path(), "orchestrator", "orchestrator override");

    let tmpl = resolve_in(dir.path(), "supervisor");
    assert_eq!(tmpl.source, Source::AgentsStandard);
    assert!(tmpl.content.contains("orchestrator override"));
}

// Scenario: An orchestrator override wins over a supervisor override
#[test]
fn orchestrator_override_wins_over_supervisor_override() {
    let dir = tempfile::tempdir().unwrap();
    write_override(dir.path(), "supervisor", "legacy supervisor override");
    write_override(dir.path(), "orchestrator", "orchestrator override");

    for name in ["supervisor", "orchestrator"] {
        let tmpl = resolve_in(dir.path(), name);
        assert!(
            tmpl.content.contains("orchestrator override"),
            "{name} must resolve the orchestrator override"
        );
        assert!(!tmpl.content.contains("legacy supervisor override"));
    }
}
