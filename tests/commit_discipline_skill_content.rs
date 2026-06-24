//! Skill-content assertions for the `coding-agent-commit-discipline` change.
//!
//! These are WHEN-the-skill-is-read / THEN-the-prose-SHALL-state requirements
//! pinning three pieces of commit-discipline guidance into the bundled skills so
//! a future edit can't silently regress them:
//!
//! - the **stand-by-after-commit** protocol in `coordination.md`
//!   (`Coordination skill — stand-by after final commit`),
//! - the **releasable-unit + amend-fixups** commit-cadence guidance in
//!   `coordination.md` (`releasable-unit commit discipline with amend fixups`),
//! - the **commit-message-format deferral** to the project's `AGENTS.md`
//!   (`Coordination skill defers commit-message format to the project AGENTS.md`),
//! - the **supervisor cross-reference** in `supervisor.md`
//!   (`supervisor relies on agents standing by` post-commit).

use std::fs;

fn coordination_skill() -> String {
    fs::read_to_string("assets/agent-skills/coordination.md")
        .expect("coordination.md is at the expected path")
}

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

// ---------------------------------------------------------------------------
// 3.1 — Stand-by after the final commit
// ---------------------------------------------------------------------------

// Requirement: Coordination skill — stand-by after final commit /
// Scenario: Coordination skill instructs standing by after the final commit.
#[test]
fn coordination_instructs_standby_after_final_commit() {
    let section = section_after(
        &coordination_skill(),
        "### Terminal action: commit then publish, never archive",
    );
    assert!(
        section.contains("stand by"),
        "Terminal action section must tell the agent to stand by after the final commit"
    );
    // What the agent waits *for*: all three supervisor messages.
    for msg in ["agent.verified", "agent.feedback", "agent.intent"] {
        assert!(
            section.contains(msg),
            "stand-by guidance must name `{msg}` as something the agent waits for"
        );
    }
}

// Requirement: Coordination skill — stand-by after final commit /
// Scenario: Stand-by protocol forbids self-verify and self-archive.
#[test]
fn standby_forbids_self_verify_and_archive_and_cross_references_role_gating() {
    let section = section_after(
        &coordination_skill(),
        "### Terminal action: commit then publish, never archive",
    );
    assert!(
        section.contains("shall not run `/opsx:verify`")
            || (section.contains("shall not")
                && section.contains("/opsx:verify")
                && section.contains("/opsx:archive")),
        "stand-by guidance must state the agent SHALL NOT run /opsx:verify or /opsx:archive"
    );
    // Cross-references the supervisor-only / forbidden-commands guidance rather
    // than re-deriving its own enforcement. (Must NOT reproduce the literal
    // `Commands you must not run` heading, which lives inside the
    // `opsx-role-gating` sentinel block stripped for non-OpenSpec backends.)
    assert!(
        section.contains("supervisor-only") && section.contains("forbidden-commands rule"),
        "stand-by guidance must cross-reference the supervisor-only forbidden-commands rule"
    );
}

// ---------------------------------------------------------------------------
// 3.2 — Releasable-unit commit discipline with amend fixups
// ---------------------------------------------------------------------------

fn commit_cadence_section() -> String {
    section_after(&coordination_skill(), "### Commit cadence")
}

// Scenario: Commit cadence requires each commit to be a releasable unit.
#[test]
fn commit_cadence_requires_releasable_units() {
    let section = commit_cadence_section();
    assert!(
        section.contains("must build and pass its own gates"),
        "Commit cadence must state each commit builds and passes its own gates"
    );
    assert!(
        section.contains("releasable unit"),
        "Commit cadence must describe a commit as a releasable unit"
    );
}

// Scenario: Commit cadence prefers amend for just-made-commit fixups.
#[test]
fn commit_cadence_prefers_amend_for_just_made_fixups() {
    let section = commit_cadence_section();
    assert!(
        section.contains("git commit --amend"),
        "Commit cadence must mention `git commit --amend` for fixups"
    );
    assert!(
        section.contains("micro-commit"),
        "Commit cadence must contrast amend against a separate micro-commit"
    );
}

// Scenario: Commit cadence forbids amending verified or earlier commits.
#[test]
fn commit_cadence_forbids_amending_verified_or_earlier_commits() {
    let section = commit_cadence_section();
    assert!(
        section.contains("already been verified") || section.contains("already-verified"),
        "Commit cadence must forbid amending an already-verified commit"
    );
    assert!(
        section.contains("earlier commit from a previous group")
            || section.contains("earlier commit"),
        "Commit cadence must forbid amending an earlier commit from a previous group"
    );
}

// ---------------------------------------------------------------------------
// 3.3 — Commit-message-format deferral to the project AGENTS.md
// ---------------------------------------------------------------------------

// Scenario: Coordination skill defers commit-message format to the project AGENTS.md.
#[test]
fn commit_cadence_defers_message_format_to_project_agents_md() {
    let section = commit_cadence_section();
    assert!(
        section.contains("commit-message conventions"),
        "Commit cadence must defer to the project's commit-message conventions"
    );
    assert!(
        section.contains("agents.md"),
        "Commit cadence must reference the project's AGENTS.md for message format"
    );
    // The de-opinionated prose must NOT mandate a specific format. The removed
    // prescription read "Use the project's conventional-commit prefix per group";
    // assert it is gone and that the skill explicitly disclaims mandating a format.
    assert!(
        !section.contains("use the project's conventional-commit prefix"),
        "the mandatory Conventional-Commits prescription must be removed"
    );
    assert!(
        section.contains("does not mandate") && section.contains("illustrative example"),
        "Commit cadence must state it does not mandate a format and that the prefix is illustrative"
    );
}

// ---------------------------------------------------------------------------
// 3.4 — Supervisor cross-reference (supervisor relies on agents standing by)
// ---------------------------------------------------------------------------

// Requirement: Reliable commit-cadence nudge /
// Scenario: Skill states the supervisor relies on agents standing by post-commit.
#[test]
fn supervisor_states_it_verifies_and_archives_post_commit() {
    let skill = supervisor_skill().to_lowercase();
    assert!(
        skill.contains("standing by"),
        "supervisor skill must state the workflow depends on agents standing by"
    );
    // The supervisor — not the agent — runs verify/archive after the final commit.
    assert!(
        skill.contains("/opsx:verify") && skill.contains("/opsx:archive"),
        "supervisor skill must name /opsx:verify and /opsx:archive as the supervisor's job"
    );
    assert!(
        skill.contains("supervisor, not the agent") || skill.contains("not the agent"),
        "supervisor skill must state the supervisor (not the agent) verifies/archives"
    );
    // Cross-references the agent-side stand-by protocol in coordination.md.
    assert!(
        skill.contains("coordination.md"),
        "supervisor skill must cross-reference the coordination.md stand-by protocol"
    );
}
