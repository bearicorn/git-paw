//! Docs-parity guard for the `core-security-posture` capability.
//!
//! The consolidated security disclaimer and the documented FS-scoped sandbox
//! workflow are the deliverables of this capability, so these tests assert the
//! authored docs actually exist and cover what the spec requires — the
//! Gate-4 doc audit expressed as a standing test. Content, not prose style, is
//! what is pinned: each assertion maps to a spec scenario.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// Case-insensitive "contains all of these substrings".
fn contains_all(haystack: &str, needles: &[&str]) -> Vec<String> {
    let lower = haystack.to_lowercase();
    needles
        .iter()
        .filter(|n| !lower.contains(&n.to_lowercase()))
        .map(|n| (*n).to_string())
        .collect()
}

// Scenario: Disclaimer is present and reachable (README + mdBook), describing
// arbitrary-code execution, the blast radius, and each control's (non-)guarantees.
#[test]
fn disclaimer_present_in_readme_and_mdbook_naming_every_control() {
    let posture = read("docs/src/user-guide/security-posture.md");
    let missing = contains_all(
        &posture,
        &[
            "arbitrary code",
            "blast radius",
            "classifier",
            "allowlist",
            "protected-path",
            "sandbox",
            "remote control",
        ],
    );
    assert!(
        missing.is_empty(),
        "security-posture.md must name every control and the blast radius; missing: {missing:?}"
    );

    let readme = read("README.md");
    let missing = contains_all(&readme, &["## Security", "arbitrary code", "sandbox"]);
    assert!(
        missing.is_empty(),
        "README needs a Security section covering arbitrary-code execution + the sandbox; missing: {missing:?}"
    );
}

// Scenario: Controls are not overstated — the disclaimer states the classifier is
// heuristic and the sandbox default is write-integrity, not confidentiality.
#[test]
fn disclaimer_states_control_limits_honestly() {
    let posture = read("docs/src/user-guide/security-posture.md").to_lowercase();
    assert!(
        posture.contains("heuristic") && posture.contains("not a security boundary"),
        "the disclaimer must state the classifier is heuristic, not a boundary"
    );
    assert!(
        posture.contains("not the work area") || posture.contains("not the confidentiality"),
        "the disclaimer must state the sandbox protects the machine around the worktree, not its contents/confidentiality"
    );
}

// Scenario: both OSes documented; PATH-resolvable wrapper; the writable allow-list;
// the git persistence gap; per-OS credential refresh; confidentiality tiers.
#[test]
fn sandbox_chapter_covers_the_required_setup() {
    let sb = read("docs/src/user-guide/sandbox.md");
    let missing = contains_all(
        &sb,
        &[
            "sandbox-exec", // macOS backend
            "bwrap",        // Linux/WSL backend
            "PATH",         // launch-by-name requirement
            ".git/hooks",   // persistence gap
            ".git/config",
            "Keychain",  // macOS credential refresh
            "toolchain", // stack-specific caches
            "~/.ssh",    // confidentiality: safe to deny
            "does not",  // honest non-guarantee framing
        ],
    );
    assert!(
        missing.is_empty(),
        "sandbox.md must document the proven setup requirements; missing: {missing:?}"
    );
    // Both read-hardening tiers must be present.
    let low = sb.to_lowercase();
    assert!(
        low.contains("tier 1") && low.contains("tier 2"),
        "sandbox.md must document both confidentiality tiers"
    );
    // git-paw does not invoke the sandbox — the docs must say so.
    assert!(
        low.contains("does not") && low.contains("invoke"),
        "sandbox.md must state git-paw does not invoke the sandbox"
    );
}

// Scenario "Doctor points at the posture" is covered behaviorally in
// `src/doctor_tests.rs` (`sandbox_backend_absent_warns_with_a_remedy_never_fails`),
// which asserts the check's remedy output references the posture — not by
// grepping the doctor source.

// The chapters must be wired into the mdBook table of contents, or mdbook build
// silently omits them.
#[test]
fn chapters_are_linked_in_summary() {
    let summary = read("docs/src/SUMMARY.md");
    assert!(
        summary.contains("user-guide/security-posture.md"),
        "SUMMARY.md must link the Security Posture chapter"
    );
    assert!(
        summary.contains("user-guide/sandbox.md"),
        "SUMMARY.md must link the FS-Scoped Sandbox chapter"
    );
}
