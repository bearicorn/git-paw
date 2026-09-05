//! Skill-content assertions for the `worker-guidance-skill-export` change,
//! supervisor side.
//!
//! These are WHEN-the-skill-is-rendered / THEN-the-prose-SHALL-state
//! requirements pinning the tiered permission model into
//! `assets/agent-skills/supervisor.md` so a future edit can't silently
//! regress it: the CLI-native → `git paw __classify` → escalate ladder, and
//! that the skill defers entirely to `git paw __classify` rather than
//! maintaining its own parallel authoritative allowlist.

use std::fs;

fn supervisor_skill() -> String {
    fs::read_to_string("assets/agent-skills/supervisor.md")
        .expect("supervisor.md is at the expected path")
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

fn permission_model_section() -> String {
    section_after(&supervisor_skill(), "### The tiered permission model")
}

// Requirement: Supervisor skill — tiered permission model and safe-command
// policy / Scenario: The supervisor skill documents the tiered permission
// ladder.
#[test]
fn permission_model_describes_the_ladder_in_order() {
    let section = permission_model_section();
    let cli_native_idx = section
        .find("cli-native")
        .expect("section must mention the CLI-native check");
    let classify_idx = section
        .find("git paw __classify")
        .expect("section must mention `git paw __classify`");
    let escalate_idx = section
        .find("escalate")
        .expect("section must mention escalation to the human");

    assert!(
        cli_native_idx < classify_idx,
        "the CLI-native check must be described before the classify tier"
    );
    assert!(
        classify_idx < escalate_idx,
        "the classify tier must be described before escalation"
    );
    assert!(
        section.contains("human"),
        "the ladder must name the human as the escalation target"
    );
}

// Requirement: Supervisor skill — tiered permission model and safe-command
// policy / Scenario: The safe-command policy is single-sourced and
// stack-agnostic.
#[test]
fn permission_model_directs_to_classify_not_a_hand_maintained_allowlist() {
    let section = permission_model_section();
    assert!(
        section.contains("git paw __classify"),
        "section must direct the supervisor to `git paw __classify`"
    );
    assert!(
        section.contains("authoritative"),
        "section must state `git paw __classify` is the authoritative check"
    );
    assert!(
        section.contains("never maintains a parallel authoritative list")
            || section.contains("never maintain a parallel"),
        "section must state the skill never maintains a parallel authoritative list"
    );
}

#[test]
fn permission_model_summarises_safe_and_danger_classes() {
    let section = permission_model_section();
    assert!(
        section.contains("managed helper scripts"),
        "section must summarise the safe class as git-paw's managed helper scripts"
    );
    assert!(
        section.contains("worktree-confined dev/test commands"),
        "section must summarise worktree-confined dev/test commands as safe"
    );
    assert!(
        section.contains("read-mostly verbs"),
        "section must summarise read-mostly verbs as safe"
    );
    assert!(
        section.contains("protected paths"),
        "section must summarise writes under protected paths as danger"
    );
    assert!(
        section.contains("curated danger-list"),
        "section must reference the curated danger-list"
    );
}

#[test]
fn permission_model_is_stack_agnostic() {
    let section = permission_model_section();
    assert!(
        section.contains("does not name a consumer's toolchain verbs as universally safe"),
        "section must explicitly disclaim naming a consumer's toolchain verbs as universally safe"
    );
    for forbidden in [
        "cargo",
        "rustdoc",
        ".rs:",
        "cargo.toml",
        "rustc",
        "npm test",
        "pytest",
    ] {
        assert!(
            !section.contains(forbidden),
            "tiered permission model must not hard-code a stack toolchain verb; found `{forbidden}`"
        );
    }
}
