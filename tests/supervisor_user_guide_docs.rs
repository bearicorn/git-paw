//! Docs-parity guard for the `supervisor-launch` capability.
//!
//! Requirement *`docs/src/user-guide/supervisor.md` SHALL consolidate the
//! supervisor surfaces*: the chapter is the single place a user reads to
//! understand supervisor mode, so each of its eight required subsections is
//! pinned here — the Gate-4 doc audit expressed as a standing test.
//!
//! Each assertion is scoped to the relevant section rather than the whole
//! chapter, so a term appearing somewhere else in a 1400-line file cannot
//! satisfy a scenario about a section that is actually missing.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn guide() -> String {
    let p = repo_root().join("docs/src/user-guide/supervisor.md");
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// Returns the markdown section introduced by `heading`, up to the next
/// heading at the same or a higher level.
///
/// Fenced code blocks are skipped when looking for that boundary: a shell or
/// TOML example's `# comment` lines look exactly like markdown headings, and
/// treating one as the end of the section would silently truncate it — and
/// so silently weaken every assertion made against it.
fn section(doc: &str, heading: &str) -> String {
    let lines: Vec<&str> = doc.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim() == heading)
        .unwrap_or_else(|| panic!("supervisor.md must contain the heading {heading:?}"));
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

/// Scenario: Supervisor user-guide names governance sub-step + cross-link.
#[test]
fn guide_names_the_governance_sub_step_and_links_the_chapter() {
    let sec = section(&guide(), "## Spec audit governance sub-step");
    assert!(
        sec.contains("governance.md"),
        "the governance sub-step must link the governance chapter"
    );
}

/// Scenario: Supervisor user-guide explains agent permissions.
///
/// The section this change introduces, and the one that matters most: it is
/// where a user learns that git-paw decides nothing about an agent's
/// command. Without it, the deprecated tables look merely undocumented
/// rather than retired.
#[test]
fn guide_explains_agent_permissions() {
    let sec = section(&guide(), "## Agent permissions");
    let lower = sec.to_lowercase();

    assert!(
        lower.contains("does not approve") || lower.contains("approves no"),
        "the section must state that git-paw does not approve permission prompts"
    );
    assert!(
        lower.contains("permission system") || lower.contains("permission mode"),
        "the section must state that the agent CLI's own permission system decides"
    );
    assert!(
        lower.contains("sweep.sh approve"),
        "the section must name the explicit approve action"
    );

    let missing = missing_from(
        &sec,
        &[
            "[supervisor.auto_approve]",
            "[supervisor.common_dev_allowlist]",
        ],
    );
    assert!(
        missing.is_empty(),
        "the section must name both deprecated tables; missing: {missing:?}"
    );
    assert!(
        lower.contains("deprecated") && lower.contains("no effect"),
        "the section must state that both tables are deprecated and have no effect"
    );

    // The two cross-links the requirement calls for.
    let missing = missing_from(&sec, &["configuration/README.md", "security-posture.md"]);
    assert!(
        missing.is_empty(),
        "the section must cross-link the configuration reference and the security posture; missing: {missing:?}"
    );
}

/// Scenario: Supervisor user-guide names the gate-command templating.
#[test]
fn guide_names_the_gate_command_templating() {
    let sec = section(&guide(), "## Repo-configurable gate commands");

    let keys = [
        "test_command",
        "lint_command",
        "build_command",
        "doc_build_command",
        "spec_validate_command",
        "fmt_check_command",
        "security_audit_command",
    ];
    let named = keys
        .iter()
        .filter(|k| sec.to_lowercase().contains(&k.to_lowercase()))
        .count();
    assert!(
        named >= 3,
        "the section must name at least three of the gate-command keys, found {named}"
    );
    assert!(
        sec.contains("(not configured)"),
        "the section must mention the `(not configured)` graceful-skip behaviour"
    );
    assert!(
        sec.contains("configuration/README.md"),
        "the section must cross-link the configuration reference"
    );
}

/// Scenario: Supervisor user-guide names the broker-side conflict detector.
#[test]
fn guide_names_the_broker_side_conflict_detector() {
    let sec = section(&guide(), "## Broker-side conflict detector");
    let missing = missing_from(
        &sec,
        &["forward", "in-flight", "ownership", "conflict-detection.md"],
    );
    assert!(
        missing.is_empty(),
        "the section must name the three failure shapes and link the chapter; missing: {missing:?}"
    );
}

/// Scenario: Supervisor user-guide cross-links the learnings aggregator
/// chapter.
#[test]
fn guide_cross_links_the_learnings_chapter() {
    let sec = section(&guide(), "## Learnings aggregator");
    assert!(
        sec.contains("learnings.md"),
        "the section must link the learnings chapter"
    );
}

/// Scenario: Supervisor user-guide mirrors "When the user types in your
/// session" — all three input categories, and the pane-bound heading is gone.
#[test]
fn guide_mirrors_when_the_user_types_in_your_session() {
    let doc = guide();
    let sec = section(&doc, "## When the user types in your session");
    let lower = sec.to_lowercase();
    for category in ["status question", "directive", "judgment-call"] {
        assert!(
            lower.contains(category),
            "the section must describe the {category:?} input category"
        );
    }
    assert!(
        !doc.contains("When the user types in your pane"),
        "the pane-bound heading must be gone: the orchestrator has no pane"
    );
}

/// Scenario: Supervisor user-guide mirrors "Merge orchestration" —
/// including the unattended rule (merge without asking first, escalate only
/// on a conflict, an exhausted correction cycle or a regression).
#[test]
fn guide_mirrors_merge_orchestration() {
    let sec = section(&guide(), "## Merge orchestration");
    let missing = missing_from(&sec, &["agent.blocked", "git merge --ff-only", "topolog"]);
    assert!(
        missing.is_empty(),
        "the section must cover the topological order and the per-branch merge command; missing: {missing:?}"
    );

    let lower = sec.to_lowercase();
    assert!(
        lower.contains("unattended") && lower.contains("without asking"),
        "the section must state that an unattended run merges without asking first"
    );
    let missing = missing_from(
        &sec,
        &["conflict", "exhausted correction cycle", "regression"],
    );
    assert!(
        missing.is_empty(),
        "the section must name the only escalations; missing: {missing:?}"
    );
    assert!(
        !lower.contains("curl"),
        "merge orchestration reads and publishes through helpers or MCP, never a raw curl"
    );
}

/// Scenario: Supervisor user-guide documents driving from an external agent.
#[test]
fn guide_documents_driving_from_your_own_agent() {
    let sec = section(&guide(), "## Driving the session from your own agent");
    let missing = missing_from(
        &sec,
        &[
            ".git-paw/skills/orchestrator/SKILL.md",
            ".git-paw/AGENTS.local.md",
            "git-paw-orchestrator:start",
            "sweep.sh verified",
            "--unattended",
        ],
    );
    assert!(
        missing.is_empty(),
        "the section must name the skill, the session context, the pointer, the verdict path and the unattended pairing; missing: {missing:?}"
    );
}
