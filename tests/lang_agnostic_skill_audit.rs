//! No-leak audit for the bundled supervisor skill.
//!
//! Implements the `Requirement: No language-leak audit` from the
//! `lang-agnostic-skills` capability: renders the supervisor skill
//! against fixture configurations for every spec backend variant and
//! asserts that no token from a forbidden list appears in the rendered
//! output outside explicitly-allowed `<!-- allowlist-prose -->` spans.
//!
//! The audit is the contractual guarantee that future skill edits
//! cannot silently re-introduce Rust-specific (or any other
//! stack-specific) assumptions into a bundled asset that ships to
//! polyglot users.

use git_paw::skills::{GateCommands, render, resolve};
use git_paw::specs::SpecBackendKind;

/// Forbidden tokens that SHALL NOT appear in the rendered supervisor
/// skill outside `<!-- allowlist-prose -->` sentinel spans, per the
/// `Requirement: No language-leak audit` spec.
///
/// The list is the spec-mandated minimum; the audit can be tightened
/// over time without changing this file's contract.
const FORBIDDEN_TOKENS: &[&str] = &["cargo", "rustdoc", ".rs:", "Cargo.toml", "rustc"];

/// Renders the embedded supervisor skill against `backends` and a
/// representative gate-command config. All gate fields are `None` so
/// the audit's input does not depend on user config — only the
/// template content + the new placeholder substitutions.
fn render_supervisor_for(backends: &[SpecBackendKind]) -> String {
    let tmpl = resolve("supervisor").expect("supervisor skill resolves");
    render(
        &tmpl,
        "supervisor",
        "http://127.0.0.1:9119",
        "git-paw",
        &GateCommands::default(),
        backends,
    )
}

/// Renders the embedded coordination skill against `backends`. Used by the
/// `coordination-context-budget` no-leak audit: the new "Context budget"
/// section SHALL pass the same forbidden-token scan across every backend.
fn render_coordination_for(backends: &[SpecBackendKind]) -> String {
    let tmpl = resolve("coordination").expect("coordination skill resolves");
    render(
        &tmpl,
        "coordination",
        "http://127.0.0.1:9119",
        "git-paw",
        &GateCommands::default(),
        backends,
    )
}

/// Strips every `<!-- allowlist-prose -->...<!-- /allowlist-prose -->`
/// span from `content` so the no-leak audit can scan the remaining
/// prose without false-positives from the legitimate
/// `DEV_ALLOWLIST_PRESET` enumeration.
fn strip_allowlist_prose(content: &str) -> String {
    let open = "<!-- allowlist-prose -->";
    let close = "<!-- /allowlist-prose -->";
    let mut out = String::with_capacity(content.len());
    let mut rest = content;
    while let Some(o) = rest.find(open) {
        out.push_str(&rest[..o]);
        rest = &rest[o + open.len()..];
        if let Some(c) = rest.find(close) {
            rest = &rest[c + close.len()..];
        } else {
            // unclosed sentinel — preserve remainder so the audit can flag it
            out.push_str(rest);
            return out;
        }
    }
    out.push_str(rest);
    out
}

/// Asserts the rendered output (with allowlist-prose spans stripped)
/// contains no forbidden token. Returns the offending token + location
/// in the failure message so a regression is easy to fix.
fn assert_no_forbidden_tokens(rendered: &str, label: &str) {
    let scanned = strip_allowlist_prose(rendered);
    for token in FORBIDDEN_TOKENS {
        if let Some(idx) = scanned.find(token) {
            // surface a useful window of context
            let window_start = idx.saturating_sub(80);
            let window_end = (idx + 80).min(scanned.len());
            let window = &scanned[window_start..window_end];
            panic!(
                "no-leak audit failed for backend `{label}`: forbidden token `{token}` at byte {idx}\n\
                 context: ...{window}...",
            );
        }
    }
}

#[test]
fn audit_passes_for_openspec_backend() {
    let rendered = render_supervisor_for(&[SpecBackendKind::OpenSpec]);
    assert_no_forbidden_tokens(&rendered, "openspec");
}

#[test]
fn audit_passes_for_speckit_backend() {
    let rendered = render_supervisor_for(&[SpecBackendKind::SpecKit]);
    assert_no_forbidden_tokens(&rendered, "speckit");
}

#[test]
fn audit_passes_for_markdown_backend() {
    let rendered = render_supervisor_for(&[SpecBackendKind::Markdown]);
    assert_no_forbidden_tokens(&rendered, "markdown");
}

#[test]
fn audit_passes_for_multi_backend_session() {
    let rendered = render_supervisor_for(&[
        SpecBackendKind::OpenSpec,
        SpecBackendKind::SpecKit,
        SpecBackendKind::Markdown,
    ]);
    assert_no_forbidden_tokens(&rendered, "multi-backend");
}

#[test]
fn audit_passes_for_no_backend_session() {
    let rendered = render_supervisor_for(&[]);
    assert_no_forbidden_tokens(&rendered, "no-backend");
}

/// Scenario from `coordination-context-budget` /
/// `Requirement: Stack-agnostic phrasing`: the new "Context budget" section
/// SHALL pass the no-leak audit on the rendered coordination skill across all
/// supported spec backends.
#[test]
fn coordination_context_budget_passes_no_leak_audit_across_backends() {
    for backends in [
        vec![SpecBackendKind::OpenSpec],
        vec![SpecBackendKind::SpecKit],
        vec![SpecBackendKind::Markdown],
        vec![
            SpecBackendKind::OpenSpec,
            SpecBackendKind::SpecKit,
            SpecBackendKind::Markdown,
        ],
        vec![],
    ] {
        let rendered = render_coordination_for(&backends);
        // Sanity: the section under audit is actually present in the render.
        assert!(
            rendered.contains("### Context budget"),
            "rendered coordination skill should include the Context budget section"
        );
        assert_no_forbidden_tokens(&rendered, "coordination");
    }
}

#[test]
fn audit_catches_a_rust_leak_regression() {
    // Deliberately introduce `cargo test` into the rendered output
    // OUTSIDE the allowlist-prose sentinel span, and confirm the audit
    // flags it. This is the regression check the spec mandates.
    let rendered = render_supervisor_for(&[SpecBackendKind::OpenSpec]);
    let with_leak = format!("{rendered}\nrun `cargo test` to verify\n");

    let scanned = strip_allowlist_prose(&with_leak);
    assert!(
        scanned.contains("cargo"),
        "deliberate regression must survive the sentinel-stripper",
    );

    // Use the audit machinery to confirm it actually fails on this input.
    let result = std::panic::catch_unwind(|| {
        assert_no_forbidden_tokens(&with_leak, "regression-test");
    });
    assert!(
        result.is_err(),
        "no-leak audit must fail when a forbidden token appears outside the allowlist-prose sentinel",
    );
}

/// `advanced-main-event` §5.5 / `Requirement: Stack-agnostic phrasing`:
/// the coordination skill (which gained the "When main advances"
/// subsection) SHALL pass the no-leak audit. The bundled coordination
/// skill carries no `<!-- allowlist-prose -->` spans, so the raw file
/// is scanned directly against the forbidden-token list.
#[test]
fn audit_passes_for_coordination_skill() {
    use std::fs;
    let coordination = fs::read_to_string("assets/agent-skills/coordination.md")
        .expect("coordination.md is at the expected path");
    assert_no_forbidden_tokens(&coordination, "coordination");
}

/// Scenario from `Requirement: Tone and example discipline in bundled
/// skills`: no meta-commentary names a language. v0.5.0 had a phrase
/// "v0.5.0 removed the Rust auto-merge loop" — this scenario rules
/// out that and any similarly stack-naming meta-commentary.
#[test]
fn no_meta_commentary_names_a_stack() {
    use std::fs;
    let supervisor = fs::read_to_string("assets/agent-skills/supervisor.md")
        .expect("supervisor.md is at the expected path");
    let coordination = fs::read_to_string("assets/agent-skills/coordination.md")
        .expect("coordination.md is at the expected path");

    // Phrases that name a stack in meta-commentary about git-paw's own
    // implementation history. Each is a literal substring; the scan
    // covers both bundled skill files at once.
    let combined = format!("{supervisor}\n{coordination}");
    for forbidden in [
        "Rust auto-merge loop",
        "the Rust supervisor",
        "Rust binary",
        "cargo-shipped",
    ] {
        assert!(
            !combined.contains(forbidden),
            "bundled skill meta-commentary must not name a stack; found `{forbidden}`",
        );
    }
}

/// Scenario from `Requirement: Tone and example discipline in bundled
/// skills`: example bodies cover at least three failure shapes
/// (test-runner, lint/format, type-check or compile). This is a
/// content-shape scan of the rendered supervisor skill against the
/// gate-name + failure-shape vocabulary it should be using.
#[test]
fn agent_feedback_examples_cover_three_failure_shapes() {
    let rendered = render_supervisor_for(&[SpecBackendKind::OpenSpec]);

    // The §7 example block uses bracketed gate names `testing`,
    // `regression`, `spec audit`, `doc audit`, `security audit` and
    // illustrative failure-shape language. We verify the example
    // collectively names at least three distinct shapes.
    let mut shape_hits = 0;
    for shape in [
        "test runner failed",
        "panic-bearing path",
        "user-guide page",
        "fails now",
        "no scenario for",
    ] {
        if rendered.contains(shape) {
            shape_hits += 1;
        }
    }
    assert!(
        shape_hits >= 3,
        "agent.feedback examples must collectively illustrate at least 3 failure shapes; only {shape_hits} matched. Adjust the §7 examples or this assertion if the example body changed shape vocabulary."
    );
}

/// Scenario from `lang-agnostic-skills` /
/// `Requirement: Bundled skills nudge against exit-code-probe wrappers`:
/// the bundled supervisor and coordination skills SHALL contain guidance
/// to run dev commands bare AND the command-string-whitelisting rationale.
#[test]
fn bundled_skills_contain_no_exit_probe_guidance_and_rationale() {
    use std::fs;
    for (path, label) in [
        ("assets/agent-skills/supervisor.md", "supervisor"),
        ("assets/agent-skills/coordination.md", "coordination"),
    ] {
        let body = fs::read_to_string(path).expect("skill file is at the expected path");

        // The guidance: run dev commands bare; reject the exit-probe wrapper.
        assert!(
            body.contains("Run dev commands bare"),
            "{label} skill must carry the run-dev-commands-bare guidance heading",
        );
        assert!(
            body.contains("EXIT $?"),
            "{label} skill must name the exit-code-probe wrapper shape it forbids",
        );

        // The rationale: per-run variation defeats command-string whitelisting.
        assert!(
            body.contains("command-string permission\nwhitelisting")
                || body.contains("command-string permission whitelisting"),
            "{label} skill must explain the command-string-whitelisting rationale",
        );
        assert!(
            body.contains("approval prompt"),
            "{label} skill rationale must mention the re-prompt consequence",
        );

        // It must NOT discourage reading the exit status itself.
        assert!(
            body.contains("read its exit status directly"),
            "{label} skill must still direct the agent to read the exit status",
        );
    }
}

/// The exit-probe nudge is stack-neutral: it passes the no-leak audit on
/// both rendered skills across every spec backend.
#[test]
fn exit_probe_nudge_passes_no_leak_audit_across_backends() {
    for backends in [
        vec![SpecBackendKind::OpenSpec],
        vec![SpecBackendKind::SpecKit],
        vec![SpecBackendKind::Markdown],
        vec![],
    ] {
        let supervisor = render_supervisor_for(&backends);
        assert!(
            supervisor.contains("Run dev commands bare"),
            "rendered supervisor skill should include the exit-probe nudge"
        );
        assert_no_forbidden_tokens(&supervisor, "supervisor-exit-probe");

        let coordination = render_coordination_for(&backends);
        assert!(
            coordination.contains("Run dev commands bare"),
            "rendered coordination skill should include the exit-probe nudge"
        );
        assert_no_forbidden_tokens(&coordination, "coordination-exit-probe");
    }
}

#[test]
fn audit_excludes_allowlist_prose_via_sentinel() {
    // The universal DEV_ALLOWLIST_PRESET is now stack-neutral, so the
    // rendered baseline carries no forbidden token (a repo opts into
    // `cargo` via the `rust` stack preset, not via the hardcoded
    // universal set). The sentinel pair still wraps the placeholder so
    // a stack-preset enumeration could legitimately surface a token
    // there in future. Verify the sentinel pairing exists AND that a
    // forbidden token placed *inside* a sentinel span is scoped out by
    // the strip (the mechanism the audit relies on).
    let rendered = render_supervisor_for(&[SpecBackendKind::OpenSpec]);
    assert!(
        rendered.contains("<!-- allowlist-prose -->"),
        "rendered supervisor skill must include the allowlist-prose sentinel pair",
    );
    assert!(
        rendered.contains("<!-- /allowlist-prose -->"),
        "rendered supervisor skill must close every allowlist-prose sentinel",
    );

    // Inject a forbidden token inside a sentinel span and confirm the
    // audit still passes — the strip removes the span before scanning.
    let with_sentinel_token = format!(
        "{rendered}\nsafe-by-pattern: <!-- allowlist-prose -->cargo (build, test)<!-- /allowlist-prose -->\n"
    );
    assert_no_forbidden_tokens(&with_sentinel_token, "sentinel-scoped-token");
}

// ---------------------------------------------------------------------------
// Convention-leak audit: bundled skills are convention-agnostic, not only
// stack-agnostic. Implements `lang-agnostic-skills` /
// `Requirement: Bundled skills are convention-agnostic`.
// ---------------------------------------------------------------------------

/// Conventional-Commits prefixes that SHALL NOT appear in a bundled skill as a
/// mandate, default, or recommendation. Commit-message format is the
/// *consumer's* property, deferred to their injected `AGENTS.md` — git-paw's
/// OWN Conventional-Commits convention lives only in git-paw's `AGENTS.md`,
/// never in the asset the binary exports to every consumer.
const FORBIDDEN_COMMIT_CONVENTION_TOKENS: &[&str] = &[
    "feat(",
    "fix(",
    "docs(",
    "test(",
    "chore(",
    "refactor(",
    "perf(",
];

/// Asserts the rendered output (allowlist-prose spans stripped) contains no
/// hardcoded Conventional-Commits prefix. Surfaces the offending token +
/// context window so a regression is easy to fix, mirroring
/// `assert_no_forbidden_tokens`.
fn assert_no_commit_convention(rendered: &str, label: &str) {
    let scanned = strip_allowlist_prose(rendered);
    for token in FORBIDDEN_COMMIT_CONVENTION_TOKENS {
        if let Some(idx) = scanned.find(token) {
            let window_start = idx.saturating_sub(80);
            let window_end = (idx + 80).min(scanned.len());
            let window = &scanned[window_start..window_end];
            panic!(
                "commit-convention leak audit failed for `{label}`: bundled skill hardcodes \
                 Conventional-Commits prefix `{token}` at byte {idx}\n\
                 context: ...{window}...\n\
                 commit-message format must defer to the consumer's AGENTS.md; \
                 git-paw's own convention belongs in git-paw's AGENTS.md, not the export",
            );
        }
    }
}

/// Scenario "Leak audit flags a hardcoded commit-convention mandate": the
/// rendered supervisor and coordination skills (empty substitutions) SHALL NOT
/// present a Conventional-Commits prefix as the commit-message format, across
/// every spec backend.
#[test]
fn audit_skills_carry_no_hardcoded_commit_convention() {
    for backends in [
        vec![SpecBackendKind::OpenSpec],
        vec![SpecBackendKind::SpecKit],
        vec![SpecBackendKind::Markdown],
        vec![
            SpecBackendKind::OpenSpec,
            SpecBackendKind::SpecKit,
            SpecBackendKind::Markdown,
        ],
        vec![],
    ] {
        let supervisor = render_supervisor_for(&backends);
        assert_no_commit_convention(&supervisor, "supervisor");
        let coordination = render_coordination_for(&backends);
        assert_no_commit_convention(&coordination, "coordination");
    }
}

/// Scenario "git-paw's own Conventional-Commits convention is not in the
/// exported asset": the source `coordination.md` (not just the render) SHALL
/// NOT carry git-paw's Conventional-Commits convention as a rule, default, or
/// recommendation.
#[test]
fn exported_coordination_asset_carries_no_commit_convention() {
    use std::fs;
    let coordination = fs::read_to_string("assets/agent-skills/coordination.md")
        .expect("coordination.md is at the expected path");
    assert_no_commit_convention(&coordination, "coordination-source");
}

/// Companion regression (mirrors `audit_catches_a_rust_leak_regression`): a
/// skill that DID hardcode a Conventional-Commits prefix as its example must be
/// flagged by the convention-leak audit.
#[test]
fn commit_convention_audit_catches_a_hardcoded_prefix_regression() {
    let rendered = render_coordination_for(&[SpecBackendKind::OpenSpec]);
    let with_leak = format!("{rendered}\nUse a `feat(<scope>): ...` prefix for every commit\n");

    let scanned = strip_allowlist_prose(&with_leak);
    assert!(
        scanned.contains("feat("),
        "deliberate regression must survive the sentinel-stripper",
    );

    let result = std::panic::catch_unwind(|| {
        assert_no_commit_convention(&with_leak, "regression-test");
    });
    assert!(
        result.is_err(),
        "commit-convention audit must fail when a bundled skill hardcodes a \
         Conventional-Commits prefix as the commit-message format",
    );
}
