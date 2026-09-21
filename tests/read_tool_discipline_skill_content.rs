//! Skill-content assertions for the `worker-guidance-read-tools` change.
//!
//! These are WHEN-the-skill-is-read / THEN-the-prose-SHALL-state requirements
//! pinning the `skill-agent` capability's file-reading and scratch-file
//! discipline into the bundled coordination skill
//! (`### Prefer native read tools over shell pipelines` in
//! `assets/agent-skills/coordination.md`), following the pattern established
//! in `tests/commit_discipline_skill_content.rs`.

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

fn read_tool_section() -> String {
    section_after(
        &coordination_skill(),
        "### Prefer native read tools over shell pipelines",
    )
}

// Requirement: Coordination skill teaches file-reading and scratch-file discipline /
// Scenario: The skill teaches preferring native read tools.
#[test]
fn teaches_preferring_native_read_tools_over_shell_pipelines() {
    let section = read_tool_section();
    assert!(
        section.contains("native file-read and search"),
        "section must tell the agent to prefer its CLI's native file-read and search facilities"
    );
    assert!(
        section.contains("shell text-extraction pipeline"),
        "section must name the thing being avoided: a shell text-extraction pipeline"
    );
    assert!(
        section.contains("approval stall"),
        "section must state that such pipelines stall on approval"
    );
}

// Requirement: Coordination skill teaches file-reading and scratch-file discipline /
// Scenario: The skill explains the dead-grant cost.
#[test]
fn explains_the_dead_grant_cost() {
    let section = read_tool_section();
    assert!(
        section.contains("keyed on the exact command"),
        "section must explain a durable approval is keyed on the exact command"
    );
    assert!(
        section.contains("never matches again"),
        "section must state the keyed grant will not match a subsequent, differing command"
    );
}

// Requirement: Coordination skill teaches file-reading and scratch-file discipline /
// Scenario: The skill teaches scratch-path reuse.
#[test]
fn teaches_scratch_path_reuse() {
    let section = read_tool_section();
    assert!(
        section.contains("reuse one scratch path"),
        "section must instruct reusing a single scratch path across iterations"
    );
    assert!(
        section.contains("fresh numbered file"),
        "section must contrast scratch-path reuse against writing numbered files"
    );
    assert!(
        section.contains("costs a fresh approval"),
        "section must state that a new filename costs a fresh approval"
    );
}

// Requirement: Coordination skill teaches file-reading and scratch-file discipline /
// Scenario: The guidance names no specific CLI or tool.
//
// This is the guard against a future edit reintroducing a vendor name for
// readability. `lang_agnostic_skill_audit.rs` already scans the whole
// rendered skill for a short forbidden-token list; this test is narrower and
// specific to this section, so it survives even if that list is scoped away
// from this change.
#[test]
fn names_no_specific_cli_or_tool() {
    let section = read_tool_section();
    let forbidden = [
        "claude",
        "codex",
        "cursor",
        "antigravity",
        "copilot",
        "gemini",
        "qwen",
        "aider",
        "grep",
        "awk",
        "sed",
        "ripgrep",
        "rg ",
    ];
    for token in forbidden {
        assert!(
            !section.contains(token),
            "the file-reading section must not name a specific CLI or tool (`{token}` found)"
        );
    }
}
