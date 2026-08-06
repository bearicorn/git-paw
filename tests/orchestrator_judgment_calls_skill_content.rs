//! Skill-content assertions for the `supervisor-autonomous-orchestrator`
//! capability's judgment-call prose.
//!
//! These cover the two WHEN-the-skill-is-inspected / THEN-the-prose-SHALL
//! scenarios of `Requirement: Orchestrator judgment-call responsibilities are
//! documented in the skill`. The drive loop's *mechanical* hand-off is unit
//! tested in `git_paw::supervisor::drive`; what the orchestrator is supposed to
//! DO once handed a judgment call exists only as exported prose, so it needs its
//! own guard — otherwise a future edit can silently drop a responsibility and
//! the smart model is handed work with no doctrine for deciding it.

use std::fs;

/// Tokens that would bake a specific consumer's stack into an exported asset,
/// violating the export-agnosticism design principle.
///
/// Superset of the `lang-agnostic-skills` forbidden list: that audit scans the
/// whole rendered skill for a language leak, while this one also rejects the
/// build/test *tooling* names a judgment-call example is most tempting to reach
/// for ("re-run `pytest` before merging"), which would read as normative to
/// every consumer whose stack differs.
const STACK_SPECIFIC_TOKENS: &[&str] = &[
    "cargo",
    "rustc",
    "rustdoc",
    "cargo.toml",
    "clippy",
    "rustfmt",
    "npm",
    "yarn",
    "pnpm",
    "pytest",
    "gradle",
    "maven",
    "mdbook",
    "just check",
    "package.json",
];

fn supervisor_skill() -> String {
    fs::read_to_string("assets/agent-skills/supervisor.md")
        .expect("supervisor.md is at the expected path")
}

/// The judgment-call section, whitespace-collapsed and lower-cased so substring
/// checks survive line rewrapping.
fn judgment_call_section() -> String {
    let skill = supervisor_skill();
    let start = skill
        .find("### Judgment calls handed to you")
        .expect("supervisor.md documents the judgment calls handed to the orchestrator");
    let rest = &skill[start..];
    // The section runs to the next `### ` heading.
    let end = rest[1..].find("\n### ").map_or(rest.len(), |idx| idx + 1);
    rest[..end]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Scenario: "The skill enumerates the four responsibilities" — dependency-aware
/// spawn order, answering ambiguous questions, merge sequencing, and declaring a
/// worker unrecoverable.
#[test]
fn enumerates_the_four_judgment_call_responsibilities() {
    let section = judgment_call_section();

    // (a) dependency-aware spawn order, derived from intents / conflict edges.
    assert!(
        section.contains("spawn order"),
        "must name dependency-aware spawn order as a responsibility"
    );
    assert!(
        section.contains("agent.intent") && section.contains("conflict edges"),
        "spawn order SHALL be derived from agent.intent and the conflict edges"
    );

    // (b) answering ambiguous questions from specs + cross-agent state.
    assert!(
        section.contains("agent.question"),
        "must name answering an ambiguous agent.question"
    );
    assert!(
        section.contains("agent.answer"),
        "answering means publishing agent.answer"
    );
    assert!(
        section.contains("cross-agent state"),
        "the answer is derived from the specs plus cross-agent state"
    );

    // (c) merge sequencing.
    assert!(
        section.contains("merge sequencing"),
        "must name merge sequencing as a responsibility"
    );

    // (d) declaring a worker unrecoverable, per the correction-loop law.
    assert!(
        section.contains("unrecoverable"),
        "must name declaring a worker unrecoverable"
    );
    assert!(
        section.contains("max_cycles") && section.contains("on_exhausted"),
        "declaring a worker unrecoverable SHALL defer to max_cycles / on_exhausted"
    );
}

/// The human is the exception handler, not the default: the prose SHALL tell the
/// orchestrator to escalate only what it genuinely cannot decide.
#[test]
fn escalates_to_the_human_only_when_genuinely_undecidable() {
    let section = judgment_call_section();
    assert!(
        section.contains("undecidable"),
        "the escalate-only-when-undecidable rule must be explicit"
    );
    assert!(
        section.contains("does not wait") || section.contains("never waits"),
        "the prose must state the loop does not block on the orchestrator's answer"
    );
}

/// Scenario: "The documented responsibilities are project-agnostic" — the
/// guidance SHALL NOT hard-code any consumer's test/build/stack toolchain.
#[test]
fn judgment_call_guidance_is_project_agnostic() {
    let section = judgment_call_section();
    for token in STACK_SPECIFIC_TOKENS {
        assert!(
            !section.contains(token),
            "judgment-call guidance leaks the stack-specific token {token:?}; \
             express the responsibility generically and let the consumer's own \
             injected instructions supply its toolchain"
        );
    }
}
