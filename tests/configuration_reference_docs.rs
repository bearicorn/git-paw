//! Docs-parity guard for the `core-configuration` capability.
//!
//! Requirement *Configuration reference documents the configuration tables*:
//! `docs/src/configuration/README.md` is the deliverable, so these tests
//! assert the authored reference actually documents each table — the Gate-4
//! doc audit expressed as a standing test.
//!
//! Substance, not prose style, is what is pinned: each test asserts that a
//! section exists and names the specific field identifiers a reader needs,
//! so rewording a sentence does not break the suite but dropping a field
//! does.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn reference() -> String {
    let p = repo_root().join("docs/src/configuration/README.md");
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// Returns the markdown section introduced by `heading`, up to the next
/// heading at the same or a higher level.
///
/// Fenced code blocks are skipped when looking for that boundary: a TOML
/// example's `# comment` lines look exactly like markdown headings, and
/// treating one as the end of the section would silently truncate it — and
/// so silently weaken every assertion made against it.
fn section(doc: &str, heading: &str) -> String {
    let lines: Vec<&str> = doc.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim() == heading)
        .unwrap_or_else(|| panic!("configuration reference must contain the heading {heading:?}"));
    let level = heading.len() - heading.trim_start_matches('#').len();

    let mut out = vec![lines[start]];
    let mut in_fence = false;
    for line in &lines[start + 1..] {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        if !in_fence {
            let hashes = line.len() - line.trim_start_matches('#').len();
            if hashes > 0 && hashes <= level && line[hashes..].starts_with(' ') {
                break;
            }
        }
        out.push(line);
    }
    out.join("\n")
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

/// Scenario: Configuration reference documents supervisor.conflict.
#[test]
fn reference_documents_supervisor_conflict() {
    let sec = section(&reference(), "### Conflict detector tuning");
    let missing = missing_from(
        &sec,
        &[
            "[supervisor.conflict]",
            "window_seconds",
            "warn_on_intent_overlap",
            "escalate_on_violation",
        ],
    );
    assert!(
        missing.is_empty(),
        "the [supervisor.conflict] subsection must list every field; missing: {missing:?}"
    );
}

/// Scenario: Configuration reference marks the auto-approval tables
/// deprecated.
///
/// The highest-value assertion in this file: the two tables still parse, so
/// nothing in the code will tell a user they have stopped working. The
/// reference is the only place that says so, and it must also say what to
/// use instead — otherwise a reader who deletes them has no replacement.
#[test]
fn reference_marks_the_auto_approval_tables_deprecated() {
    let sec = section(&reference(), "### Deprecated auto-approval tables");

    let missing = missing_from(
        &sec,
        &[
            "[supervisor.auto_approve]",
            "[supervisor.common_dev_allowlist]",
        ],
    );
    assert!(
        missing.is_empty(),
        "the deprecation subsection must name both tables; missing: {missing:?}"
    );

    let lower = sec.to_lowercase();
    assert!(
        lower.contains("deprecated"),
        "the subsection must call both tables deprecated"
    );
    assert!(
        lower.contains("no effect"),
        "the subsection must state that the tables have no effect"
    );

    // The replacement must be named, or a reader who removes the tables is
    // left with no way to stop a routine command prompting.
    assert!(
        lower.contains("allow rules"),
        "the subsection must point at the agent CLI's own allow rules as the replacement"
    );
    assert!(
        lower.contains("full-auto") && lower.contains("sandbox"),
        "the subsection must offer `full-auto` inside the sandbox as the prompt-free alternative"
    );
}

/// Scenario: Configuration reference documents
/// `supervisor.learnings_config`.
#[test]
fn reference_documents_supervisor_learnings_config() {
    let sec = section(&reference(), "### Learnings mode tuning");
    let missing = missing_from(
        &sec,
        &["[supervisor.learnings_config]", "flush_interval_seconds"],
    );
    assert!(
        missing.is_empty(),
        "the [supervisor.learnings_config] subsection must name its fields; missing: {missing:?}"
    );
}

/// Scenario: Configuration reference documents governance — all five fields.
#[test]
fn reference_documents_governance_with_all_five_fields() {
    let sec = section(&reference(), "## Governance");
    let missing = missing_from(
        &sec,
        &[
            "[governance]",
            "adr",
            "test_strategy",
            "security",
            "dod",
            "constitution",
        ],
    );
    assert!(
        missing.is_empty(),
        "the [governance] subsection must list all five fields; missing: {missing:?}"
    );
}
