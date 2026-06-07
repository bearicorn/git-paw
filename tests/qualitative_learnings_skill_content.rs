//! Skill-content assertions for the `qualitative-learnings` capability.
//!
//! These scenarios are WHEN-the-skill-is-read / THEN-the-prose-SHALL-state
//! requirements: they pin the heuristic gates, the per-category primary
//! identifiers, the documented body shapes, and the no-speculative-publishing
//! discipline into the bundled `assets/agent-skills/supervisor.md` so a future
//! edit can't silently drop them.

use std::fs;

fn supervisor_skill() -> String {
    fs::read_to_string("assets/agent-skills/supervisor.md")
        .expect("supervisor.md is at the expected path")
}

/// The qualitative-learnings section, lower-cased, for substring scans that
/// don't care about capitalisation.
fn qualitative_section() -> String {
    let skill = supervisor_skill();
    let start = skill
        .find("### Qualitative learnings")
        .expect("supervisor.md has a Qualitative learnings section");
    skill[start..].to_lowercase()
}

// Requirement: Four qualitative category values — each category named.
#[test]
fn names_all_four_categories() {
    let section = qualitative_section();
    for category in [
        "recurring_failure_shape",
        "doc_gap",
        "adr_drift",
        "scope_mistake",
    ] {
        assert!(
            section.contains(category),
            "qualitative-learnings section must name `{category}`"
        );
    }
}

// Requirement: Four qualitative category values / Scenario: Each of the four
// categories has a documented body shape.
#[test]
fn documents_a_body_shape_per_category() {
    let section = qualitative_section();
    // Representative body fields from each category's documented shape (D1).
    for field in [
        "shape",
        "instances", // recurring_failure_shape
        "convention",
        "evidence_paths",
        "suggestion", // doc_gap
        "decision_area",
        "observed_pattern",
        "candidate_adr_title", // adr_drift
        "branches",
        "shared_files", // scope_mistake
    ] {
        assert!(
            section.contains(field),
            "body-shape documentation must mention the `{field}` field"
        );
    }
}

// Requirement: Within-session dedup discipline / Scenario: Skill prose names
// the primary identifier per category.
#[test]
fn names_primary_identifier_per_category() {
    let section = qualitative_section();
    assert!(
        section.contains("dedup"),
        "must document a dedup discipline"
    );
    // The primary identifiers per category (D3).
    for ident in ["shape", "convention", "decision_area", "branches"] {
        assert!(
            section.contains(ident),
            "dedup discipline must name the `{ident}` primary identifier"
        );
    }
}

// Requirement: Supervisor-skill heuristics / Scenario: recurring_failure_shape
// requires multi-branch evidence.
#[test]
fn recurring_failure_shape_heuristic_requires_multi_branch_evidence() {
    let section = qualitative_section();
    assert!(
        section.contains("three"),
        "heuristic must require three cycles"
    );
    assert!(
        section.contains("two") && section.contains("branch"),
        "heuristic must require two distinct branches"
    );
    // Each heuristic carries an explicit do-not-publish gate.
    assert!(
        section.contains("do not publish unless"),
        "heuristics must include explicit `do not publish unless` gates"
    );
}

// Requirement: Supervisor-skill heuristics / Scenario: doc_gap requires
// evidence the convention is missing.
#[test]
fn doc_gap_heuristic_requires_missing_from_governance_docs() {
    let section = qualitative_section();
    assert!(section.contains("convention"));
    assert!(
        section.contains("governance"),
        "doc_gap heuristic must reference the configured governance doc paths"
    );
    assert!(
        section.contains("verifiable from") || section.contains("evident in the code"),
        "doc_gap heuristic must require the convention be evident from code"
    );
}

// Requirement: Supervisor-skill heuristics / Scenario: adr_drift requires a
// concrete code commit.
#[test]
fn adr_drift_heuristic_requires_a_commit() {
    let section = qualitative_section();
    assert!(section.contains("adr"));
    assert!(
        section.contains("commit"),
        "adr_drift heuristic must require at least one commit introducing the pattern"
    );
}

// Requirement: Supervisor-skill heuristics / Scenario: scope_mistake requires
// overlapping intents plus coordination.
#[test]
fn scope_mistake_heuristic_requires_overlap_and_coordination() {
    let section = qualitative_section();
    assert!(
        section.contains("agent.intent") || section.contains("overlapping"),
        "scope_mistake heuristic must require overlapping intents"
    );
    assert!(
        section.contains("agent.feedback") || section.contains("coordination"),
        "scope_mistake heuristic must require coordination feedback"
    );
    assert!(
        section.contains("commit"),
        "scope_mistake heuristic must require a commit on each branch"
    );
}

// Requirement: No confidence field in payload / Scenario: Skill prose forbids
// speculative publishing.
#[test]
fn forbids_speculative_publishing_and_omits_confidence_field() {
    let section = qualitative_section();
    assert!(
        section.contains("just in case"),
        "skill must forbid publishing speculative records `just in case`"
    );
    assert!(
        section.contains("when in doubt, do not publish"),
        "skill must instruct to stay silent when in doubt"
    );
    // No `confidence` key is introduced into any documented body shape.
    assert!(
        !section.contains("\"confidence\""),
        "skill must not introduce a confidence body field"
    );
    // The prose explicitly states the absence (confidence is signalled by
    // publishing-or-not, per design D6).
    assert!(
        section.contains("no confidence field"),
        "skill must explicitly state there is no confidence field"
    );
}
