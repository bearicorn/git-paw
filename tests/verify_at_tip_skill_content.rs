//! Skill-content assertions for the `verify-at-tip` change.
//!
//! These are WHEN-the-skill-is-read / THEN-the-prose-SHALL-state requirements
//! that pin the tip-resolution and merge-base verification guidance into the
//! bundled `assets/agent-skills/supervisor.md`, so a future edit cannot
//! silently re-introduce the stale-`$SHA` snapshot bug that produced the
//! v0.9.0 doc-audit false negative.
//!
//! The behaviour is skill-content only (no `src/` code path changes), so these
//! grep-style assertions are the sole enforcement for the modified
//! `agent-skills` and `supervisor-skill-discipline` requirements.

use std::fs;

/// Raw contents of the bundled supervisor skill — used for the exact recipe
/// line checks where shell quoting (`"$TIP"` vs `"$SHA"`) matters.
fn supervisor_skill() -> String {
    fs::read_to_string("assets/agent-skills/supervisor.md")
        .expect("supervisor.md is at the expected path")
}

/// Whitespace-collapsed, lowercased, markdown-stripped view of the skill for
/// robust prose substring scans (backticks and emphasis asterisks removed so
/// checks are insensitive to code-span / bold formatting and line wrapping).
fn normalized() -> String {
    supervisor_skill()
        .replace(['`', '*'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

// Task 3.1 — maps scenario "Gates run against the re-resolved branch tip".
#[test]
fn gates_run_against_reresolved_tip_not_triggering_sha() {
    let s = normalized();
    assert!(
        s.contains("git rev-parse"),
        "five-gate prose must instruct re-resolving the tip with git rev-parse"
    );
    assert!(
        s.contains("re-resolve"),
        "five-gate prose must use the 're-resolve' framing"
    );
    assert!(
        s.contains("run all five gates against that tip"),
        "prose must state all five gates run against the re-resolved tip"
    );
    // Explicitly NOT the triggering committed-event / verify-now SHA.
    assert!(
        s.contains("supervisor.verify-now"),
        "prose must name the supervisor.verify-now nudge as the triggering event"
    );
    assert!(
        s.contains("first commit") && s.contains("stale"),
        "prose must explain the triggering SHA is the stale first commit"
    );
}

// Task 3.2 — maps scenario "Verify worktree checks out the re-resolved branch
// tip". Uses the RAW skill text so shell quoting is asserted exactly.
#[test]
fn recipe_passes_reresolved_tip_not_captured_sha() {
    let raw = supervisor_skill();
    assert!(
        raw.contains(r#"TIP="$(git rev-parse "$BRANCH")""#),
        "recipe must resolve TIP from git rev-parse of the branch"
    );
    assert!(
        raw.contains(r#"git worktree add --detach "$VERIFY" "$TIP""#),
        "recipe must pass the re-resolved $TIP to git worktree add --detach"
    );
    assert!(
        !raw.contains(r#"git worktree add --detach "$VERIFY" "$SHA""#),
        "recipe must NOT check out a captured $SHA as the worktree tip"
    );
    // --detach is preserved so the agent worktree stays the ref holder.
    assert!(
        raw.contains("--detach"),
        "recipe must keep --detach so the agent worktree holds the branch ref"
    );
}

// Task 3.3 — maps scenario "Change contribution is diffed against the
// merge-base".
#[test]
fn change_contribution_diffed_against_merge_base() {
    let s = normalized();
    assert!(
        s.contains("git merge-base"),
        "spec-audit / security-audit prose must diff against git merge-base"
    );
    assert!(
        s.contains("integration target"),
        "merge-base prose must reference the integration target"
    );
    // Rationale: a stale integration tip yields spurious mass deletions.
    assert!(
        s.contains("spurious") && (s.contains("deletions") || s.contains("additions")),
        "prose must give the stale-integration-tip spurious-deletions rationale"
    );
}

// Task 3.4 — maps scenario "Doc/test surfaces present at the tip are not
// reported missing".
#[test]
fn tip_present_surfaces_not_reported_missing_cites_v0_9_0() {
    let s = normalized();
    assert!(
        s.contains("must not be reported as missing"),
        "prose must forbid reporting tip-present surfaces as MISSING"
    );
    assert!(
        s.contains("v0.9.0"),
        "prose must cite the v0.9.0 false-negative as the motivating example"
    );
    // The citation must tie to the doc audit running on a stale snapshot.
    assert!(
        s.contains("doc audit"),
        "the v0.9.0 example must be tied to the doc audit"
    );
}

// Task 3.5 — maps scenarios "Re-verification re-resolves the tip before
// re-running gates" and "Recipe re-resolves the tip on re-run".
#[test]
fn reverification_reresolves_tip_before_rerun() {
    let s = normalized();
    assert!(
        s.contains("re-run"),
        "prose must address re-running the gates"
    );
    // A later committed event, a verify-now nudge, or a re-verify after
    // feedback all re-resolve before re-running.
    assert!(
        s.contains("agent.feedback"),
        "re-verification rule must cover re-verifying after agent.feedback"
    );
    // The recipe section states each (re-)run re-creates the worktree at the
    // freshly resolved tip.
    assert!(
        s.contains("re-create the worktree"),
        "recipe must state each (re-)run re-creates the worktree at the tip"
    );
}
