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
// arbitrary-code execution, the blast radius, the (non-)guarantees of each
// control that actually exists, and the fact that git-paw approves no prompt.
#[test]
fn disclaimer_present_in_readme_and_mdbook_naming_every_control() {
    let posture = read("docs/src/user-guide/security-posture.md");
    let missing = contains_all(
        &posture,
        &[
            "arbitrary code",
            "blast radius",
            // The three controls that remain.
            "permission system",
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

// Scenario: Disclaimer is present and reachable — "it states that git-paw
// approves no permission prompt on its own".
#[test]
fn disclaimer_states_git_paw_approves_no_prompt() {
    let posture = read("docs/src/user-guide/security-posture.md").to_lowercase();
    assert!(
        posture.contains("approves no permission prompt"),
        "the disclaimer must state plainly that git-paw approves no permission prompt on its own"
    );
    assert!(
        posture.contains("no safety judgement") || posture.contains("makes no safety"),
        "the disclaimer must state that git-paw makes no safety judgement about an agent's command"
    );
}

// Scenario: Controls are not overstated — a CLI allow rule is only as narrow as
// it is written, a full-auto mode skips the prompts entirely, and the sandbox
// default is write-integrity, not confidentiality.
#[test]
fn disclaimer_states_control_limits_honestly() {
    let posture = read("docs/src/user-guide/security-posture.md").to_lowercase();
    assert!(
        posture.contains("only as narrow as the rules you wrote"),
        "the disclaimer must state that a CLI allow rule is only as narrow as it is written"
    );
    assert!(
        posture.contains("skips the prompts entirely"),
        "the disclaimer must state that a full-auto level skips the prompts entirely"
    );
    assert!(
        posture.contains("not the work area") || posture.contains("not the confidentiality"),
        "the disclaimer must state the sandbox protects the machine around the worktree, not its contents/confidentiality"
    );
    assert!(
        posture.contains("write-integrity"),
        "the disclaimer must state the sandbox default protects write-integrity only"
    );
    assert!(
        posture.contains("not cover network"),
        "the disclaimer must state the sandbox does not cover network/resource/socket vectors by default"
    );
}

// Scenario: No classifier is presented as a control — the control list must not
// offer a git-paw command classifier, a git-paw command allowlist, or a git-paw
// protected-paths rule as something that protects the reader. The words may
// still appear, but only to say the mechanism is retired.
#[test]
fn no_classifier_is_presented_as_a_control() {
    let posture = read("docs/src/user-guide/security-posture.md");

    // Only the table's own rows count as "the control list": the first cell of
    // each `|`-delimited row names a control the reader is offered. Prose
    // around the table may still mention a retired mechanism — to say it is
    // retired — without presenting it as a control.
    let section = posture
        .split_once("## What the controls do")
        .map(|(_, rest)| rest.split("\n## ").next().unwrap_or(rest).to_string())
        .expect("security-posture.md must keep its control-table section");
    let control_names: Vec<String> = section
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('|'))
        .filter_map(|l| l.trim_start_matches('|').split('|').next())
        .map(|cell| cell.trim().to_lowercase())
        .filter(|cell| !cell.is_empty() && !cell.starts_with("---") && cell != "control")
        .collect();
    assert!(
        !control_names.is_empty(),
        "the control table must still list controls; parsed none from:\n{section}"
    );
    for name in &control_names {
        for retired in ["classifier", "allowlist", "protected-path", "danger"] {
            assert!(
                !name.contains(retired),
                "the control table must not offer {name:?} as a control: {retired:?} is retired"
            );
        }
    }

    // And the chapter must say so explicitly rather than just going quiet.
    let lower = posture.to_lowercase();
    assert!(
        lower.contains("retired") && lower.contains("have no effect"),
        "the chapter must state that the classifier and its config tables are retired and have no effect"
    );

    // The durable-grant residue the retirement leaves behind must be called out.
    assert!(
        lower.contains("don't ask again") && lower.contains("removes none"),
        "the chapter must state that durable grants written by earlier versions persist and are removed by nothing"
    );
    assert!(
        lower.contains("never edits or deletes it"),
        "the chapter must state that git-paw never edits or deletes the CLI's settings file"
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
            "Keychain",     // macOS credential refresh
            "toolchain",    // stack-specific caches
            "~/.ssh",       // confidentiality: safe to deny
            "does not",     // honest non-guarantee framing
            "/private/tmp", // heredoc temp-dir grant
            "heredoc",      // why the grant is required
            "setuid",       // setuid exec refusal
            "/bin/ps",      // the concrete setuid binary an agent reaches for
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

// Scenario: Posture chapter covers the guard and its limits — the Host and
// Origin checks, the DNS-rebinding threat, the non-loopback-bind exception, and
// the statement that local processes are not authenticated by the guard.
#[test]
fn posture_chapter_covers_the_guard_and_its_limits() {
    let posture = read("docs/src/user-guide/security-posture.md");
    let missing = contains_all(
        &posture,
        &[
            // The two checks, and the accepted host set.
            "`Host`",
            "`Origin`",
            "127.0.0.0/8",
            "localhost",
            "[::1]",
            // The refusal shapes.
            "forbidden host",
            "forbidden origin",
            "403",
            // The threat they address.
            "DNS rebinding",
            "prompt-injection",
            // The non-loopback exception, and its warning.
            "0.0.0.0",
            "non-loopback exception",
            "reachable from your network",
            "warning",
            // Not authentication.
            "not authentication",
            "can still reach the broker",
            // The proxy migration.
            "reverse prox",
            "proxy_set_header Host",
        ],
    );
    assert!(
        missing.is_empty(),
        "security-posture.md must cover the broker's browser-exposure guard and its limits; missing: {missing:?}"
    );

    // The chapter must NOT claim a non-loopback bind still blocks browsers: a
    // DNS-rebinding page is same-origin with the broker, and its GETs carry no
    // Origin, so with the Host check off it can read the coordination log.
    let posture_flat = posture.split_whitespace().collect::<Vec<_>>().join(" ");
    for claim in [
        "Only a loopback bind stops a rebinding page from reading the coordination log",
        "a same-origin `GET` carries **no `Origin` header at all**",
        "any page served from a loopback origin is accepted",
    ] {
        let flat_claim = claim.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            posture_flat
                .to_lowercase()
                .contains(&flat_claim.to_lowercase()),
            "security-posture.md must state: {claim}"
        );
    }
    assert!(
        !posture_flat.contains("so browsers are still blocked"),
        "security-posture.md must not claim a non-loopback bind still blocks browsers"
    );
}

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
