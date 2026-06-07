//! Skill-content assertions for the `conflict-detector-fn-granularity`
//! capability's coordination prose.
//!
//! These are WHEN-the-skill-is-read / THEN-the-prose-SHALL-state requirements
//! pinning the "Declaring regions" subsection into the bundled
//! `assets/agent-skills/coordination.md` so a future edit can't silently drop
//! the region-declaration guidance.

use std::fs;

fn coordination_skill() -> String {
    fs::read_to_string("assets/agent-skills/coordination.md")
        .expect("coordination.md is at the expected path")
}

/// The "Declaring regions" subsection, lower-cased, for case-insensitive
/// substring scans.
fn regions_section() -> String {
    let skill = coordination_skill();
    let start = skill
        .find("### Declaring regions")
        .expect("coordination.md has a Declaring regions section");
    // The section runs until the next top-level subsection heading.
    let rest = &skill[start..];
    let end = rest[1..].find("\n### ").map_or(rest.len(), |idx| idx + 1);
    rest[..end].to_lowercase()
}

// Requirement: Coordination skill teaches region declaration / Scenario:
// Skill prose covers when to declare and when to omit.
#[test]
fn covers_declare_when_with_two_examples() {
    let section = regions_section();
    assert!(
        section.contains("declare regions when"),
        "section must include explicit `declare when` guidance"
    );
    // Two example shapes: same-file collaboration and a nameable symbol.
    assert!(
        section.contains("different parts of it"),
        "declare-when must give the same-file-collaboration example"
    );
    assert!(
        section.contains("function name"),
        "declare-when must give the nameable-function example"
    );
}

#[test]
fn covers_skip_when_with_two_examples() {
    let section = regions_section();
    assert!(
        section.contains("skip regions"),
        "section must include explicit `skip when` guidance"
    );
    // Two example shapes: whole-file refactor and planning-in-flux.
    assert!(
        section.contains("refactor across the whole file"),
        "skip-when must give the file-wide-refactor example"
    );
    assert!(
        section.contains("still in flux"),
        "skip-when must give the planning-in-flux example"
    );
}

// Requirement: Coordination skill teaches region declaration / Scenario:
// Skill prose forbids dodging the detector.
#[test]
fn forbids_manufacturing_narrow_regions_with_rationale() {
    let section = regions_section();
    assert!(
        section.contains("do not manufacture narrow regions"),
        "section must explicitly forbid manufactured narrow regions"
    );
    // One-sentence rationale: the dodge hides a real collision.
    assert!(
        section.contains("merge conflict") || section.contains("hides a collision"),
        "the dodge warning must carry a rationale explaining the harm"
    );
}

#[test]
fn names_all_four_region_kinds() {
    let section = regions_section();
    for kind in ["function", "class", "block", "range"] {
        assert!(
            section.contains(kind),
            "Declaring regions section must name the `{kind}` kind"
        );
    }
}
