//! The mdBook chapters credit the agent CLI's permission system, not a
//! git-paw classifier.
//!
//! Spec scenario "User docs credit the CLI's permission system"
//! (`approval-command-safety`, requirement *Prompt-free operation is
//! attributed to the agent CLI's permission system*).
//!
//! This is the user-facing half of a claim the change turns on. Before the
//! retirement, the coordination chapter promised "prompt-free boot calls" as
//! though git-paw guaranteed them; it never did, and now it plainly cannot.
//! Whether a helper call or dev command runs without prompting is decided by
//! the agent CLI's resolved permission mode and whatever allow rules the user
//! configured — so the chapters must say that, and must not credit anything
//! of git-paw's.
//!
//! The sibling scenario, *Supervisor guidance credits the CLI's permission
//! system*, covers the bundled skill and lives in
//! `tests/tiered_permission_model_skill_content.rs`.

use std::fs;
use std::path::{Path, PathBuf};

fn docs(rel: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs/src")
        .join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// Phrases that would credit a git-paw-side decision for prompt-free
/// operation. Each is checked only outside an explicitly retiring sentence —
/// see `credits_outside_retirement_prose`.
const FORBIDDEN_CREDITS: &[&str] = &[
    "git-paw auto-approves",
    "git-paw approves",
    "auto-approve whitelist",
    "classifier-safe",
    "safe-command classifier",
];

/// Lines that mention a retired mechanism *in order to say it is gone* are
/// legitimate and must not trip the negative checks. A sentence counts as
/// retirement prose when it carries one of these markers.
const RETIREMENT_MARKERS: &[&str] = &[
    "retired",
    "no longer",
    "deprecated",
    "has no effect",
    "have no effect",
    "removed",
    "approves no",
    "no classifier",
    "not a classifier",
    "earlier versions",
];

/// Returns the lines of `body` that contain `needle` but are NOT retirement
/// prose — i.e. the ones that would genuinely be crediting the mechanism.
fn crediting_lines<'a>(body: &'a str, needle: &str) -> Vec<&'a str> {
    body.lines()
        .filter(|l| l.to_lowercase().contains(&needle.to_lowercase()))
        .filter(|l| {
            let lower = l.to_lowercase();
            !RETIREMENT_MARKERS.iter().any(|m| lower.contains(m))
        })
        .collect()
}

#[test]
fn supervisor_chapter_credits_the_cli_permission_system() {
    let body = docs("user-guide/supervisor.md");
    let lower = body.to_lowercase();

    assert!(
        lower.contains("permission mode") || lower.contains("allow rule"),
        "the supervisor chapter must attribute prompt-free operation to the \
         CLI's resolved permission mode or its own allow rules"
    );
    assert!(
        lower.contains("full-auto") && lower.contains("sandbox"),
        "and must name the sandboxed full-auto route as the other way to run \
         prompt-free"
    );
    for needle in FORBIDDEN_CREDITS {
        let bad = crediting_lines(&body, needle);
        assert!(
            bad.is_empty(),
            "the supervisor chapter must not credit a git-paw decision \
             ({needle:?}) outside retirement prose; offending lines: {bad:?}"
        );
    }
}

#[test]
fn coordination_chapter_credits_the_cli_permission_system() {
    let body = docs("user-guide/coordination.md");
    let lower = body.to_lowercase();

    // The chapter covers the bundled helpers, whose boot call is the exact
    // case that used to be promised as prompt-free unconditionally.
    assert!(
        lower.contains("broker.sh"),
        "the coordination chapter documents the broker helper"
    );
    assert!(
        lower.contains("permission") && (lower.contains("allow rule") || lower.contains("cli")),
        "it must condition prompt-free helper calls on the agent CLI's own \
         permission system"
    );
    for needle in FORBIDDEN_CREDITS {
        let bad = crediting_lines(&body, needle);
        assert!(
            bad.is_empty(),
            "the coordination chapter must not credit a git-paw decision \
             ({needle:?}) outside retirement prose; offending lines: {bad:?}"
        );
    }
}

/// Guard against the negative checks above passing vacuously: if a chapter
/// were moved or emptied, `crediting_lines` would trivially find nothing.
#[test]
fn both_chapters_are_substantial() {
    for rel in ["user-guide/supervisor.md", "user-guide/coordination.md"] {
        let body = docs(rel);
        assert!(
            body.len() > 2000,
            "{rel} is unexpectedly short ({} bytes) — the negative assertions \
             in this file would pass vacuously",
            body.len()
        );
    }
}
