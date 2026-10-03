//! Skill-content assertions for the conflict-acknowledgement guidance in
//! the bundled coordination skill (`assets/agent-skills/coordination.md`).
//!
//! These are WHEN-the-skill-is-read / THEN-the-prose-SHALL-state
//! requirements, following the pattern in
//! `tests/read_tool_discipline_skill_content.rs`. They pin the
//! broker-agent-helper requirement "Conflict-acknowledgement wire form and
//! `sweep.sh conflict-ack`", scenario "The coordination guidance tells
//! agents the tag is informational".
//!
//! Why this needs a test at all: agents are taught that `agent.feedback`
//! `errors` are problems to fix. An acknowledgement rides on that same
//! message, so if this prose is ever dropped the agents are left reading an
//! adjudication as a defect report — a silent regression no compile or
//! behavioural test would catch.

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

fn messages_section() -> String {
    section_after(&coordination_skill(), "### Messages you may receive")
}

#[test]
fn names_the_conflict_ack_tag_as_a_feedback_exception() {
    let section = messages_section();
    assert!(
        section.contains("[conflict-ack]"),
        "the agent-facing message guidance must name the `[conflict-ack]` tag"
    );
    assert!(
        section.contains("not a problem to fix"),
        "the guidance must say a `[conflict-ack]` entry is not a problem to fix"
    );
}

#[test]
fn tells_the_agent_to_take_no_action() {
    let section = messages_section();
    assert!(
        section.contains("do nothing in response"),
        "the guidance must tell the agent to do nothing in response"
    );
    assert!(
        section.contains("no re-published `agent.artifact`"),
        "the guidance must say no re-published agent.artifact is needed"
    );
    assert!(
        section.contains("no pausing your edits"),
        "the guidance must say the agent need not pause its edits on the file"
    );
}

#[test]
fn explains_what_an_acknowledgement_records() {
    let section = messages_section();
    assert!(
        section.contains("has been reviewed and ruled acceptable"),
        "the guidance must say the conflict was reviewed and ruled acceptable"
    );
    assert!(
        section.contains("[conflict-detector]"),
        "the guidance must connect the acknowledgement to the in-flight warning the agent saw"
    );
}

/// The short pointer on the stand-by list must agree with the long form, so
/// an agent that only reads the terminal-action section is not misled.
#[test]
fn the_standby_feedback_bullet_flags_the_exception_too() {
    let skill = coordination_skill().to_lowercase();
    let collapsed = skill.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        collapsed.contains("an `errors` entry beginning `[conflict-ack]` is the exception"),
        "the stand-by feedback bullet must flag the acknowledgement exception as well"
    );
}

/// Guard against a future edit naming a specific CLI or vendor in this
/// guidance — the bundled skills are exported to every consumer project.
#[test]
fn names_no_specific_cli_or_tool() {
    let section = messages_section();
    for token in [
        "claude",
        "codex",
        "cursor",
        "antigravity",
        "copilot",
        "gemini",
        "qwen",
        "aider",
    ] {
        assert!(
            !section.contains(token),
            "the message guidance must not name a specific CLI (`{token}` found)"
        );
    }
}
