//! Docs-parity guard for the `core-project-conventions` capability.
//!
//! Requirement *README features section enumerates the current user-facing
//! surface*: the Features list is how a user discovers the feature set from
//! a single page, so a capability that ships without an entry there is
//! effectively undiscoverable.
//!
//! Every assertion is scoped to the Features section specifically, not the
//! whole README. That matters in both directions: a flag mentioned only in
//! the Usage table does not satisfy the requirement, and — for the negative
//! scenario — the README legitimately discusses the deprecated auto-approval
//! tables elsewhere (in `## Security`, to say they are retired), which a
//! whole-file check would wrongly flag.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// The README's `## Features` section, up to the next top-level heading.
fn features_section() -> String {
    let p = repo_root().join("README.md");
    let readme = fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));

    let lines: Vec<&str> = readme.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim() == "## Features")
        .expect("README.md must have a `## Features` section");

    let mut out = vec![lines[start]];
    let mut in_fence = false;
    for line in &lines[start + 1..] {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        if !in_fence && line.starts_with("## ") {
            break;
        }
        out.push(line);
    }
    let section = out.join("\n");
    assert!(
        section.len() > 500,
        "parsed Features section looks truncated ({} bytes) — the parser is wrong, \
         and every assertion below would pass or fail for the wrong reason",
        section.len()
    );
    section
}

/// Case-insensitive "contains all of these substrings"; returns the misses.
fn missing_from(haystack: &str, needles: &[&str]) -> Vec<String> {
    let lower = haystack.to_lowercase();
    needles
        .iter()
        .filter(|n| !lower.contains(&n.to_lowercase()))
        .map(|n| (*n).to_string())
        .collect()
}

/// Scenario: README Features section mentions Spec Kit.
#[test]
fn features_mention_spec_kit() {
    let missing = missing_from(&features_section(), &["Spec Kit", "speckit"]);
    assert!(
        missing.is_empty(),
        "the Features section must name the Spec Kit backend and its TOML value; missing: {missing:?}"
    );
}

/// Scenario: README Features section mentions forward coordination and
/// conflict detection.
#[test]
fn features_mention_forward_coordination_and_conflict_detection() {
    let missing = missing_from(&features_section(), &["agent.intent", "conflict detection"]);
    assert!(
        missing.is_empty(),
        "the Features section must cover forward coordination and conflict detection; missing: {missing:?}"
    );
}

/// Scenario: README Features section mentions learnings mode.
#[test]
fn features_mention_learnings_mode() {
    let missing = missing_from(
        &features_section(),
        &["learnings", ".git-paw/session-learnings.md"],
    );
    assert!(
        missing.is_empty(),
        "the Features section must cover learnings mode and name its output file; missing: {missing:?}"
    );
}

/// Scenario: README Features section mentions every CLI flag.
#[test]
fn features_mention_every_cli_flag() {
    let missing = missing_from(
        &features_section(),
        &["--specs-format", "--no-supervisor", "start --force"],
    );
    assert!(
        missing.is_empty(),
        "the Features section must name every user-facing flag; missing: {missing:?}"
    );
}

/// Scenario: README Features section does not advertise auto-approval.
///
/// git-paw approves no permission prompt. Offering automatic approval as a
/// feature would be the most consequential doc overclaim available: a reader
/// would leave an unattended run expecting prompts to clear themselves.
#[test]
fn features_do_not_advertise_auto_approval() {
    let section = features_section();
    let lower = section.to_lowercase();

    assert!(
        !lower.contains("[supervisor.auto_approve]"),
        "the Features section must not offer the retired auto-approval table as a feature"
    );
    for claim in [
        "auto-approv",
        "auto approv",
        "automatically approv",
        "approves safe",
        "approve safe",
    ] {
        assert!(
            !lower.contains(claim),
            "the Features section must not describe git-paw as approving prompts ({claim:?})"
        );
    }
}
