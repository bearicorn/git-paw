//! Skill-content assertions for the `worker-guidance-skill-export` change,
//! coordination side.
//!
//! These are WHEN-the-skill-is-rendered / THEN-the-prose-SHALL-state
//! requirements pinning the worktree-environment orientation section into
//! `assets/agent-skills/coordination.md` so a future edit can't silently
//! regress it: `Operation not permitted` framed as policy (not a fault) with
//! don't-probe guidance, gitignored install artifacts with an
//! install-if-genuinely-missing directive, and the setuid-`ps`-under-sandbox
//! note.

use std::fs;

fn coordination_skill() -> String {
    fs::read_to_string("assets/agent-skills/coordination.md")
        .expect("coordination.md is at the expected path")
}

/// Extract the section starting at `heading` and running until the next
/// top-level (`### `) heading, lower-cased with whitespace collapsed so
/// substring scans are robust to line wrapping.
fn section_after(skill: &str, heading: &str) -> String {
    let start = skill
        .find(heading)
        .unwrap_or_else(|| panic!("skill has a `{heading}` section"));
    let rest = &skill[start..];
    let end = rest[1..].find("\n### ").map_or(rest.len(), |idx| idx + 1);
    rest[..end]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn orientation_section() -> String {
    section_after(
        &coordination_skill(),
        "### Worktree-environment orientation",
    )
}

// Requirement: Coordination skill — worktree environment orientation /
// Scenario: The coordination skill teaches worktree-environment orientation.
#[test]
fn orientation_frames_operation_not_permitted_as_policy_not_a_fault() {
    let section = orientation_section();
    assert!(
        section.contains("operation not permitted"),
        "orientation section must name the `Operation not permitted` condition"
    );
    assert!(
        section.contains("policy"),
        "orientation section must frame FS-confinement as policy"
    );
    assert!(
        section.contains("not a fault") || section.contains("not a broken machine"),
        "orientation section must state this is not a fault/broken machine"
    );
    assert!(
        section.contains("adapt rather than probe") || section.contains("do not probe"),
        "orientation section must direct the worker to adapt rather than probe"
    );
    for probe_tool in ["xattr", "ls -lo@"] {
        assert!(
            section.contains(probe_tool),
            "orientation section must name `{probe_tool}` as a probe to avoid"
        );
    }
}

#[test]
fn orientation_covers_gitignored_install_artifacts() {
    let section = orientation_section();
    assert!(
        section.contains("gitignored"),
        "orientation section must state install artifacts are gitignored"
    );
    assert!(
        section.contains("on_create"),
        "orientation section must mention the operator `on_create` hook"
    );
    assert!(
        section.contains("genuinely missing"),
        "orientation section must direct the worker to install only if genuinely missing"
    );
    assert!(
        section.contains("repo-root copy") && section.contains("not"),
        "orientation section must warn against reaching for the repo-root copy"
    );
}

#[test]
fn orientation_covers_setuid_ps_under_sandbox() {
    let section = orientation_section();
    assert!(
        section.contains("`ps`"),
        "orientation section must name `ps` as the setuid example"
    );
    assert!(
        section.contains("setuid"),
        "orientation section must name the setuid condition"
    );
    assert!(
        section.contains("expected") && section.contains("not fixable"),
        "orientation section must state the setuid failure is expected and not fixable"
    );
}

// Requirement: Coordination skill — worktree environment orientation /
// Scenario: The orientation section is stack-agnostic.
#[test]
fn orientation_is_stack_agnostic() {
    let section = orientation_section();
    assert!(
        section.contains("your stack's install"),
        "orientation section must phrase the install step generically"
    );
    // No hard-coded package-manager invocation or stack toolchain token.
    for forbidden in [
        "npm install",
        "cargo build",
        "pip install",
        "go build",
        "cargo",
        "npm ",
    ] {
        assert!(
            !section.contains(forbidden),
            "orientation section must not hard-code a stack package-manager command; found `{forbidden}`"
        );
    }
}
