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

use git_paw::agent_cli::{CliFamily, CliProfile};
use git_paw::config::CustomCli;
use git_paw::skills::{
    BundledFile, FragmentSlot, GateCommands, SkillFormat, SkillTemplate, Source, build_boot_block,
    bundled_package, bundled_packages, render, select_fragment,
};
use git_paw::specs::SpecBackendKind;
use std::collections::HashMap;

/// Resolves `name` from the embedded defaults only (no override root), so a
/// developer's own overrides never change what this test sees.
fn resolve(name: &str) -> Result<git_paw::skills::SkillTemplate, git_paw::skills::SkillError> {
    git_paw::skills::resolve_from(name, &git_paw::skills::SkillRoots::default())
}

/// Forbidden tokens that SHALL NOT appear in the rendered supervisor
/// skill outside `<!-- allowlist-prose -->` sentinel spans, per the
/// `Requirement: No language-leak audit` spec.
///
/// The list is the spec-mandated minimum; the audit can be tightened
/// over time without changing this file's contract. `Claude Code` and
/// `.claude/` are vendor product names/paths (D5): a bundled asset may name
/// a specific CLI only inside an `<!-- allowlist-prose -->` span, so naming
/// a vendor is a deliberate, reviewable act rather than an accident.
const FORBIDDEN_TOKENS: &[&str] = &[
    "cargo",
    "rustdoc",
    ".rs:",
    "Cargo.toml",
    "rustc",
    "Claude Code",
    ".claude/",
];

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
        CliFamily::Generic,
    )
}

/// Renders the embedded `cli-setup` skill against `backends`. Used by the
/// `cli-setup-consent` no-leak audit: the skill ships to every consumer, so
/// it SHALL name no vendor product, vendor settings path or stack token.
fn render_cli_setup_for(backends: &[SpecBackendKind]) -> String {
    let tmpl = resolve("cli-setup").expect("cli-setup skill resolves");
    render(
        &tmpl,
        "cli-setup",
        "http://127.0.0.1:9119",
        "git-paw",
        &GateCommands::default(),
        backends,
        CliFamily::Generic,
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
        CliFamily::Generic,
    )
}

/// Strips every `<!-- allowlist-prose -->...<!-- /allowlist-prose -->`
/// span from `content` so the no-leak audit can scan the remaining
/// prose without false-positives from a deliberate, reviewable
/// enumeration of known CLIs (the paste-buffer indicator list).
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
    let coordination = git_paw::skills::bundled_package("coordination")
        .map(|p| p.skill_md.content.to_string())
        .expect("coordination.md is at the expected path");
    assert_no_forbidden_tokens(&coordination, "coordination");
}

/// Scenario from `Requirement: Tone and example discipline in bundled
/// skills`: no meta-commentary names a language. v0.5.0 had a phrase
/// "v0.5.0 removed the Rust auto-merge loop" — this scenario rules
/// out that and any similarly stack-naming meta-commentary.
#[test]
fn no_meta_commentary_names_a_stack() {
    let supervisor = git_paw::skills::bundled_package("orchestrator")
        .map(|p| p.skill_md.content.to_string())
        .expect("supervisor.md is at the expected path");
    let coordination = git_paw::skills::bundled_package("coordination")
        .map(|p| p.skill_md.content.to_string())
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
    for (name, label) in [
        ("orchestrator", "supervisor"),
        ("coordination", "coordination"),
    ] {
        let body = bundled_package(name)
            .expect("package is registered")
            .skill_md
            .content;

        // The guidance: run dev commands bare; reject the exit-probe wrapper.
        assert!(
            body.contains("Run dev commands bare"),
            "{label} skill must carry the run-dev-commands-bare guidance heading",
        );
        assert!(
            body.contains("EXIT $?"),
            "{label} skill must name the exit-code-probe wrapper shape it forbids",
        );

        // The rationale: per-run variation defeats the agent CLI's own
        // command-string permission matching, so every run re-prompts.
        assert!(
            body.contains("command-string permission\nmatching")
                || body.contains("command-string permission matching"),
            "{label} skill must explain the command-string-matching rationale",
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

/// `standards-skill-integration` / `Requirement: ... project-agnostic`: the review-gate
/// consult step (supervisor) and the author-time consult step (coordination) SHALL pass the
/// no-leak audit on the rendered skills across every spec backend. The consult wording names
/// the *project's own* `.agents/skills/` standards, never a git-paw stack token — so this is a
/// repo-internal guard that the exported skills stay stack-agnostic for polyglot consumers.
#[test]
fn standards_consult_step_passes_no_leak_audit_across_backends() {
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
        assert!(
            supervisor.contains("Consult the project's standards at the review gate"),
            "rendered supervisor skill should include the standards review-gate consult step"
        );
        assert_no_forbidden_tokens(&supervisor, "supervisor-standards-consult");

        let coordination = render_coordination_for(&backends);
        assert!(
            coordination.contains("consult the project's standards skills under"),
            "rendered coordination skill should include the standards consult step"
        );
        assert_no_forbidden_tokens(&coordination, "coordination-standards-consult");
    }
}

/// `cli-setup-consent` / `skill-agent` scenario "The skill is project- and
/// vendor-agnostic": the rendered `cli-setup` skill SHALL carry no vendor
/// product name, vendor settings path, stack token or commit-convention
/// prefix, across every spec backend. The skill carries no
/// `<!-- allowlist-prose -->` span, so nothing is exempted from the scan.
#[test]
fn cli_setup_skill_passes_no_leak_audit_across_backends() {
    for backends in [
        vec![SpecBackendKind::OpenSpec],
        vec![SpecBackendKind::SpecKit],
        vec![SpecBackendKind::Markdown],
        vec![SpecBackendKind::Superpowers],
        vec![
            SpecBackendKind::OpenSpec,
            SpecBackendKind::SpecKit,
            SpecBackendKind::Markdown,
        ],
        vec![],
    ] {
        let rendered = render_cli_setup_for(&backends);
        // Sanity: the audited render is actually the cli-setup skill.
        assert!(
            rendered.contains("git paw setup-cli"),
            "rendered cli-setup skill should describe the setup-cli command"
        );
        assert!(
            !rendered.contains("<!-- allowlist-prose -->"),
            "the cli-setup skill must not exempt any prose from the no-leak audit"
        );
        assert_no_forbidden_tokens(&rendered, "cli-setup");
        assert_no_commit_convention(&rendered, "cli-setup");
    }
}

/// Companion to the above for the supervisor skill's new per-CLI setup
/// section: it is rendered as part of the supervisor skill, so it SHALL pass
/// the same scan. Asserting the section is present keeps this from silently
/// passing if the section is ever dropped.
#[test]
fn supervisor_per_cli_setup_section_passes_no_leak_audit_across_backends() {
    for backends in [
        vec![SpecBackendKind::OpenSpec],
        vec![SpecBackendKind::SpecKit],
        vec![SpecBackendKind::Markdown],
        vec![],
    ] {
        let rendered = render_supervisor_for(&backends);
        assert!(
            rendered.contains("## Per-CLI setup"),
            "rendered supervisor skill should include the per-CLI setup section"
        );
        let section = rendered
            .split("## Per-CLI setup")
            .nth(1)
            .expect("per-CLI setup section body");
        // Scan the section on its own so a leak there cannot hide behind an
        // allowlist-prose span that belongs to an unrelated section.
        assert!(
            !section.contains("<!-- allowlist-prose -->"),
            "the per-CLI setup section must not exempt its prose from the audit"
        );
        assert_no_forbidden_tokens(section, "supervisor-per-cli-setup");
        assert_no_commit_convention(section, "supervisor-per-cli-setup");
    }
}

#[test]
fn audit_excludes_allowlist_prose_via_sentinel() {
    // After the classifier's retirement the only surviving sentinel span is
    // the paste-buffer indicator list, which deliberately names a specific
    // CLI. The rendered baseline therefore carries no forbidden token
    // outside a span. Verify the sentinel pairing still exists AND that a
    // forbidden token placed *inside* a sentinel span is scoped out by the
    // strip (the mechanism the audit relies on).
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
        "{rendered}\nknown CLIs: <!-- allowlist-prose -->cargo (build, test)<!-- /allowlist-prose -->\n"
    );
    assert_no_forbidden_tokens(&with_sentinel_token, "sentinel-scoped-token");
}

/// `core-lang-agnostic` / Scenario: No dev-allowlist placeholder remains.
///
/// The classifier's retirement removed `{{DEV_ALLOWLIST_PRESET}}` and the
/// constant behind it. Neither bundled template may carry the placeholder,
/// and no rendered skill may present an enumeration of commands as
/// approvable — that framing is exactly what the retirement removed.
#[test]
fn no_dev_allowlist_placeholder_remains() {
    for name in ["orchestrator", "coordination"] {
        let raw = bundled_package(name)
            .expect("package is registered")
            .skill_md
            .content;
        assert!(
            !raw.contains("{{DEV_ALLOWLIST_PRESET}}"),
            "{name} must not carry the retired dev-allowlist placeholder"
        );
    }

    // The rendered supervisor skill carries no allowlist enumeration and no
    // reference to the retired classifier that produced one.
    let rendered = render_supervisor_for(&[SpecBackendKind::OpenSpec]).to_lowercase();
    for forbidden in [
        "{{dev_allowlist_preset}}",
        "dev_allowlist_preset",
        "__classify",
        "safe-command whitelist",
        "auto-approve whitelist",
        "safe_commands",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "rendered supervisor skill must not enumerate approvable commands; found `{forbidden}`"
        );
    }
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
    let coordination = git_paw::skills::bundled_package("coordination")
        .map(|p| p.skill_md.content.to_string())
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

// ---------------------------------------------------------------------------
// compiled-prose-to-assets: every bundled asset carrying agent-facing prose
// is in the audited set (task 5.1), not only the supervisor skill, and the
// forbidden list now includes vendor product names/paths (task 5.2, D5).
// ---------------------------------------------------------------------------

/// Every bundled asset outside the skill packages carrying agent-facing prose
/// that the no-leak audit covers. The skill packages' own text files are
/// enumerated from the registry instead (see
/// [`registered_package_text_files`]). Includes the prose relocated out of compiled Rust by
/// `compiled-prose-to-assets` and the hook bodies (`git-hook-injection`
/// scenario "Hook assets are covered by the export audit").
const AUDITED_PROSE_ASSETS: &[&str] = &[
    "assets/inter-agent-rules.md",
    "assets/governance-section-header.md",
    "assets/spec-path-doctrine/openspec.md",
    "assets/spec-path-doctrine/speckit.md",
    "assets/spec-path-doctrine/markdown.md",
    "assets/spec-path-doctrine/superpowers.md",
    "assets/boot-task-prompts/no-spec.md",
    "assets/boot-task-prompts/openspec.md",
    "assets/boot-task-prompts/openspec.entry.md",
    "assets/boot-task-prompts/markdown.md",
    "assets/boot-task-prompts/speckit.md",
    "assets/boot-task-prompts/superpowers.md",
    "assets/worktree-root-rule.md",
    "assets/task-prompts/speckit/feature-context-heading.md",
    "assets/task-prompts/speckit/implementation-plan-heading.md",
    "assets/task-prompts/speckit/validation-criteria.md",
    "assets/task-prompts/speckit/your-task-heading.md",
    "assets/task-prompts/speckit/consolidated-intro.md",
    "assets/task-prompts/speckit/consolidated-footer.md",
    "assets/task-prompts/superpowers/plan-context-heading.md",
    "assets/task-prompts/superpowers/your-tasks-heading.md",
    "assets/task-prompts/superpowers/execution.md",
    "assets/hooks/post-commit.sh",
    "assets/hooks/pre-push.sh",
    "assets/hooks/pre-commit.sh",
    "assets/escalation-framing.md",
    "assets/worker-role-line.md",
];

#[test]
fn audit_covers_every_relocated_prose_asset() {
    use std::fs;
    for path in AUDITED_PROSE_ASSETS {
        let content =
            fs::read_to_string(path).unwrap_or_else(|e| panic!("{path} must be readable: {e}"));
        assert_no_forbidden_tokens(&content, path);
    }
}

/// Scenario "Audit catches a vendor product-name leak": a bundled asset
/// edited to add a vendor product name outside an allowed span fails the
/// audit and identifies the offending name plus its location (task 5.4).
#[test]
fn audit_catches_a_vendor_product_name_leak() {
    let with_leak = "Read your assignment, then open Claude Code and begin.".to_string();
    let result = std::panic::catch_unwind(|| {
        assert_no_forbidden_tokens(&with_leak, "regression-test");
    });
    assert!(
        result.is_err(),
        "no-leak audit must fail when a vendor product name appears outside an allowed span",
    );

    let with_path_leak = "State lives under the repo's .claude/ directory.".to_string();
    let result = std::panic::catch_unwind(|| {
        assert_no_forbidden_tokens(&with_path_leak, "regression-test");
    });
    assert!(
        result.is_err(),
        "no-leak audit must fail when a vendor-specific path appears outside an allowed span",
    );
}

/// Scenario "Allowed span permits a deliberate CLI enumeration": the
/// paste-buffer indicator list in the supervisor skill names `Claude Code`
/// inside an `<!-- allowlist-prose -->` span, so it must not fail the audit.
#[test]
fn allowed_span_permits_the_paste_buffer_cli_enumeration() {
    let rendered = render_supervisor_for(&[SpecBackendKind::OpenSpec]);
    assert!(
        rendered.contains("Claude Code"),
        "supervisor skill should still name Claude Code inside its allowed CLI enumeration"
    );
    assert_no_forbidden_tokens(&rendered, "supervisor-paste-buffer-enumeration");
}

// ---------------------------------------------------------------------------
// orchestrator-skill-export: the exported SKILL.md files and the AGENTS.md
// pointer asset pass the same no-leak audit (core-lang-agnostic, *Exported
// skills and the AGENTS.md pointer pass the no-leak audit*).
// ---------------------------------------------------------------------------

/// The orchestrator skill rendered exactly as `git paw init` exports it —
/// frontmatter included — for `backends`.
fn exported_orchestrator_for(backends: &[SpecBackendKind]) -> String {
    let tmpl = resolve("orchestrator").expect("orchestrator skill resolves");
    git_paw::skills::render_standardized(&tmpl, &GateCommands::default(), backends)
}

/// The `cli-setup` skill as `git paw init` exports it.
fn exported_cli_setup() -> String {
    git_paw::skills::to_skill_md(&resolve("cli-setup").expect("cli-setup resolves"))
}

const POINTER_ASSET: &str = "assets/agents-md-orchestrator-section.md";

/// Every backend case the export can render: each spec format and none.
fn every_export_backend() -> Vec<(&'static str, Vec<SpecBackendKind>)> {
    vec![
        ("openspec", vec![SpecBackendKind::OpenSpec]),
        ("speckit", vec![SpecBackendKind::SpecKit]),
        ("markdown", vec![SpecBackendKind::Markdown]),
        ("superpowers", vec![SpecBackendKind::Superpowers]),
        ("no-backend", vec![]),
    ]
}

// Scenario: Rendered SKILL.md passes the audit for every backend
#[test]
fn exported_orchestrator_skill_passes_the_audit_for_every_backend() {
    for (label, backends) in every_export_backend() {
        let md = exported_orchestrator_for(&backends);
        assert!(
            md.starts_with("---\nname: orchestrator\n"),
            "{label}: frontmatter"
        );
        assert_no_forbidden_tokens(&md, label);
        assert_no_commit_convention(&md, label);
    }
}

// Scenario: Exported cli-setup SKILL.md passes the audit
#[test]
fn exported_cli_setup_skill_passes_the_audit() {
    let md = exported_cli_setup();
    assert_no_forbidden_tokens(&md, "exported-cli-setup");
    assert_no_commit_convention(&md, "exported-cli-setup");
}

// Scenario: Pointer asset passes the audit (and the per-session context
// template, the sidecar's prose, alongside it)
#[test]
fn pointer_asset_passes_the_audit() {
    for path in [POINTER_ASSET, "assets/orchestrator-session-context.md"] {
        let content = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_no_forbidden_tokens(&content, path);
        assert_no_commit_convention(&content, path);
        assert!(!content.contains("http://"), "{path} carries no broker URL");
    }
}

// Scenario: A vendor path regression is caught
#[test]
fn audit_catches_a_vendor_path_in_the_pointer_asset() {
    let pointer = std::fs::read_to_string(POINTER_ASSET).expect("pointer asset");
    let with_leak = format!("{pointer}\nCopy the skill into .claude/skills/ to load it.\n");
    let result = std::panic::catch_unwind(|| {
        assert_no_forbidden_tokens(&with_leak, "pointer-regression");
    });
    let message = result
        .expect_err("the audit must fail on a vendor path in the pointer asset")
        .downcast::<String>()
        .map(|s| *s)
        .unwrap_or_default();
    assert!(
        message.contains(".claude/"),
        "the failure names the token: {message}"
    );
}

// skill-agent / Scenario: The rendered orchestrator skill contains no curl —
// and no session value — in its exported form, for every backend.
#[test]
fn exported_orchestrator_skill_carries_no_curl_or_session_value() {
    for (label, backends) in every_export_backend() {
        let md = exported_orchestrator_for(&backends);
        assert!(!md.to_lowercase().contains("curl"), "{label}: curl");
        assert!(!md.contains("http://"), "{label}: broker URL");
        assert!(
            !md.contains("{{GIT_PAW_BROKER_URL}}"),
            "{label}: URL placeholder"
        );
        assert!(
            !md.contains("paw-{{PROJECT_NAME}}"),
            "{label}: session name"
        );
    }
}

// ---------------------------------------------------------------------------
// orchestrator-escalation-handoff: the headless escalation context
// (core-lang-agnostic, *Escalation framing prose is a bundled agnostic
// asset*).
// ---------------------------------------------------------------------------

// Scenario: Rendered escalation context passes the audit
#[test]
fn rendered_escalation_context_passes_the_audit_for_every_kind() {
    use git_paw::supervisor::escalation::{ContextInputs, HandOff, HandOffKind, render_context};

    let kinds = [
        HandOffKind::Question {
            question: "RS256 or HS256?".to_string(),
        },
        HandOffKind::MergeDecision {
            status: "committed".to_string(),
        },
        HandOffKind::CorrectionExhausted {
            detail: "branch is unrecoverable after 5 correction cycle(s); needs a human"
                .to_string(),
        },
    ];
    for kind in kinds {
        let label = format!("escalation-context-{kind:?}");
        let rendered = render_context(
            &HandOff {
                agent_id: "feat-x".to_string(),
                kind,
            },
            ContextInputs {
                repo_root: std::path::Path::new("/repo"),
                broker_url: Some("http://127.0.0.1:9119"),
            },
        );
        assert_no_forbidden_tokens(&rendered, &label);
        assert_no_commit_convention(&rendered, &label);
    }
}

// Scenario: Framing is not a compiled literal
#[test]
fn escalation_framing_is_loaded_from_the_bundled_asset() {
    let source = std::fs::read_to_string("src/supervisor/escalation.rs")
        .expect("escalation module must be readable");
    assert!(
        source.contains("include_str!(\"../../assets/escalation-framing.md\")"),
        "the framing prose must come from the bundled asset"
    );
}

// ---------------------------------------------------------------------------
// skill-package-layout: the audited set is derived from the bundled package
// registry (core-lang-agnostic, *The no-leak audit covers every registered
// package file*).
// ---------------------------------------------------------------------------

/// The prose (non-script) files of a package: `SKILL.md`, references, assets.
fn prose_files(files: &[BundledFile]) -> impl Iterator<Item = &BundledFile> {
    files.iter().filter(|f| !f.path.starts_with("scripts/"))
}

/// Renders one package text file for every backend set the audit covers
/// (each backend alone, all together, none), returning `(label, text)` pairs.
/// Placeholder substitution is a no-op on files without placeholders.
fn rendered_for_every_backend(package: &str, file: &BundledFile) -> Vec<(String, String)> {
    let template = SkillTemplate {
        name: package.to_string(),
        content: file.content.to_string(),
        source: Source::Embedded,
        format: SkillFormat::Standardized,
        metadata: None,
        resource_paths: None,
        origin_path: None,
    };
    let mut sets: Vec<Vec<SpecBackendKind>> =
        SpecBackendKind::ALL.iter().map(|k| vec![*k]).collect();
    sets.push(SpecBackendKind::ALL.to_vec());
    sets.push(Vec::new());
    let mut out = vec![(format!("{package}/{}", file.path), file.content.to_string())];
    for backends in sets {
        out.push((
            format!("{package}/{} {backends:?}", file.path),
            render(
                &template,
                "feat/audit",
                "http://127.0.0.1:9119",
                "git-paw",
                &GateCommands::default(),
                &backends,
                CliFamily::Generic,
            ),
        ));
    }
    out
}

/// Runs the no-leak audit over `files` of package `package`. A file under
/// `assets/cli/<family>/` is read as that family's fragment: it may name its
/// own vendor and no other.
fn audit_package_files(package: &str, files: &[BundledFile]) {
    for file in prose_files(files) {
        let owner = fragment_owner(file.path);
        for (label, text) in rendered_for_every_backend(package, file) {
            if let Some(family) = owner {
                assert_fragment_names_only_its_vendor(&text, family, &label);
            } else {
                assert_no_forbidden_tokens(&text, &label);
                assert_no_vendor_token(&text, None, &label);
            }
        }
    }
}

/// Every registered prose file, as `(package, package-relative path)` — the
/// audit's input set.
fn registered_package_text_files() -> Vec<(&'static str, BundledFile)> {
    let mut set = Vec::new();
    for package in bundled_packages() {
        let files: Vec<BundledFile> = package.files().copied().collect();
        for file in prose_files(&files) {
            set.push((package.name, *file));
        }
    }
    set
}

#[test]
fn audit_covers_every_registered_package_text_file() {
    let audited = registered_package_text_files();
    assert!(!audited.is_empty(), "the audit's input set is empty");
    for (package, file) in &audited {
        audit_package_files(package, std::slice::from_ref(file));
    }
}

/// Scenario "Every registered prose file is in the audited set".
#[test]
fn every_registered_prose_file_is_in_the_audited_set() {
    let audited = registered_package_text_files();
    for package in bundled_packages() {
        let mut expected = vec![package.skill_md.path];
        expected.extend(package.references.iter().map(|f| f.path));
        expected.extend(package.assets.iter().map(|f| f.path));
        for path in expected {
            assert!(
                audited
                    .iter()
                    .any(|(name, file)| *name == package.name && file.path == path),
                "{}/{path} is registered but not audited",
                package.name
            );
        }
    }
}

/// Scenario "A newly registered reference is audited automatically".
#[test]
fn a_newly_registered_reference_is_audited_automatically() {
    let coordination = bundled_package("coordination").expect("registered");
    let mut files: Vec<BundledFile> = coordination.files().copied().collect();
    files.push(BundledFile {
        path: "references/leak.md",
        source: "assets/agent-skills/coordination/references/leak.md",
        content: "Run `cargo test` before every commit.\n",
    });
    let result = std::panic::catch_unwind(|| audit_package_files("coordination", &files));
    let message = result
        .expect_err("the audit must fail on a forbidden token in a new reference")
        .downcast::<String>()
        .map(|s| *s)
        .unwrap_or_default();
    assert!(
        message.contains("references/leak.md"),
        "names the file: {message}"
    );
    assert!(message.contains("cargo"), "names the token: {message}");
}

// ---------------------------------------------------------------------------
// per-cli-skill-templates: vendor naming is confined to the matching CLI
// fragment (core-lang-agnostic, *Vendor naming is confined to the matching
// CLI fragment*).
// ---------------------------------------------------------------------------

/// Vendor product names and vendor-specific paths, per CLI family. A token
/// may appear only in text from its own family's fragment; `generic` owns
/// none.
const VENDOR_TOKENS: &[(CliFamily, &[&str])] = &[
    (CliFamily::Claude, &["Claude", "Anthropic", ".claude/"]),
    (CliFamily::Codex, &["Codex", "OpenAI", ".codex/"]),
    (
        CliFamily::Antigravity,
        &["Antigravity", "Gemini", ".gemini/"],
    ),
    (CliFamily::Qwen, &["Qwen", ".qwen/"]),
    (CliFamily::Cursor, &["Cursor", ".cursor/"]),
    (CliFamily::Generic, &[]),
];

/// The family whose fragment the package file at `path` is, if it lies
/// under `assets/cli/<family>/`.
fn fragment_owner(path: &str) -> Option<CliFamily> {
    CliFamily::ALL
        .into_iter()
        .find(|f| path.starts_with(&format!("assets/cli/{}/", f.as_str())))
}

/// Asserts `text` (allowlist spans stripped) names no vendor token other
/// than `owner`'s, naming `label` and the token on failure.
fn assert_no_vendor_token(text: &str, owner: Option<CliFamily>, label: &str) {
    let scanned = strip_allowlist_prose(text);
    for (family, tokens) in VENDOR_TOKENS {
        if Some(*family) == owner {
            continue;
        }
        for token in *tokens {
            if let Some(idx) = scanned.find(token) {
                let start = idx.saturating_sub(80);
                let end = (idx + 80).min(scanned.len());
                panic!(
                    "vendor audit failed for `{label}`: vendor token `{token}` at byte {idx}\n\
                     context: ...{}...",
                    &scanned[start..end]
                );
            }
        }
    }
}

/// Asserts a fragment of `owner` carries no stack token and names no vendor
/// but its own.
fn assert_fragment_names_only_its_vendor(text: &str, owner: CliFamily, label: &str) {
    let own: &[&str] = VENDOR_TOKENS
        .iter()
        .find(|(f, _)| *f == owner)
        .map_or(&[], |(_, t)| t);
    let scanned = strip_allowlist_prose(text);
    for token in FORBIDDEN_TOKENS {
        if !own.iter().any(|o| token.contains(o)) {
            assert!(
                !scanned.contains(token),
                "vendor audit failed for `{label}`: forbidden token `{token}`"
            );
        }
    }
    assert_no_vendor_token(text, Some(owner), label);
}

/// Audits `rendered` — a template rendered for an agent of `family` — so a
/// vendor token survives only inside the fragment selected for `family`.
/// Each selected fragment must name only its own vendor; the rest of the
/// text (the base template) names none.
fn audit_cli_rendering(rendered: &str, family: CliFamily, label: &str) {
    let coordination = bundled_package("coordination").expect("registered");
    let mut base = rendered.to_string();
    for slot in FragmentSlot::ALL {
        let fragment = select_fragment(family, slot, coordination).trim_end();
        // The selected fragment is the family's own, or the generic fallback.
        let owner = coordination
            .assets
            .iter()
            .find(|f| f.content.trim_end() == fragment)
            .and_then(|f| fragment_owner(f.path))
            .expect("a selected fragment is a registered package file");
        assert_fragment_names_only_its_vendor(
            fragment,
            owner,
            &format!("{label} {}", slot.path(owner)),
        );
        base = base.replace(fragment, "");
    }
    assert_no_forbidden_tokens(&base, label);
    assert_no_vendor_token(&base, None, label);
}

/// The coordination skill rendered for an agent of `family`.
fn coordination_for_family(family: CliFamily) -> String {
    let tmpl = resolve("coordination").expect("coordination skill resolves");
    render(
        &tmpl,
        "feat/audit",
        "http://127.0.0.1:9119",
        "git-paw",
        &GateCommands::default(),
        &[SpecBackendKind::OpenSpec],
        family,
    )
}

/// Both boot-block forms rendered for an agent of `family`.
fn boot_blocks_for_family(family: CliFamily) -> Vec<(bool, String)> {
    [false, true]
        .into_iter()
        .map(|mcp| {
            let mut clis = HashMap::new();
            clis.insert(
                "probe".to_string(),
                CustomCli {
                    command: "probe".to_string(),
                    family: Some(family.as_str().to_string()),
                    mcp: Some(mcp),
                    ..CustomCli::default()
                },
            );
            let profile = CliProfile::resolve("probe", &clis);
            (
                mcp,
                build_boot_block("feat/audit", "http://127.0.0.1:9119", &profile),
            )
        })
        .collect()
}

/// Task 4.1: the coordination skill and both boot-block forms pass the audit
/// rendered for every registered family and for `generic`.
#[test]
fn cli_placeholder_templates_pass_the_audit_for_every_family() {
    for family in CliFamily::ALL {
        audit_cli_rendering(
            &coordination_for_family(family),
            family,
            &format!("coordination/SKILL.md [{family}]"),
        );
        for (mcp, block) in boot_blocks_for_family(family) {
            audit_cli_rendering(&block, family, &format!("boot block mcp={mcp} [{family}]"));
        }
    }
}

/// Scenario "Generic rendering is vendor-free".
#[test]
fn generic_rendering_is_vendor_free() {
    let mut rendered = vec![coordination_for_family(CliFamily::Generic)];
    rendered.extend(
        boot_blocks_for_family(CliFamily::Generic)
            .into_iter()
            .map(|(_, b)| b),
    );
    for text in rendered {
        assert_no_forbidden_tokens(&text, "generic");
        assert_no_vendor_token(&text, None, "generic");
    }
}

/// The claude fragment does name its vendor, and only through the fragment.
#[test]
fn the_claude_fragment_is_the_only_source_of_its_vendor_name() {
    let block = &boot_blocks_for_family(CliFamily::Claude)[0].1;
    assert!(block.contains("Claude"), "{block}");
    audit_cli_rendering(block, CliFamily::Claude, "claude boot block");
}

/// Scenario "A fragment naming another vendor fails".
#[test]
fn a_fragment_naming_another_vendor_fails() {
    let coordination = bundled_package("coordination").expect("registered");
    let path = FragmentSlot::Skill.path(CliFamily::Claude);
    let fragment = coordination
        .assets
        .iter()
        .find(|f| f.path == path)
        .expect("claude skill fragment registered");
    let edited = format!(
        "{}\nOr run Codex and use its own command.\n",
        fragment.content
    );
    let files: Vec<BundledFile> = vec![BundledFile {
        content: Box::leak(edited.into_boxed_str()),
        ..*fragment
    }];
    let message = std::panic::catch_unwind(|| audit_package_files("coordination", &files))
        .expect_err("the audit must fail on another vendor in the claude fragment")
        .downcast::<String>()
        .map(|s| *s)
        .unwrap_or_default();
    assert!(message.contains(&path), "names the fragment: {message}");
    assert!(message.contains("Codex"), "names the token: {message}");
}

/// Scenario "A vendor name in a base template fails".
#[test]
fn a_vendor_name_in_a_base_template_fails() {
    let coordination = bundled_package("coordination").expect("registered");
    let edited = format!(
        "{}\nOpen Claude Code before you begin.\n",
        coordination.skill_md.content
    );
    let template = SkillTemplate {
        name: "coordination".to_string(),
        content: edited,
        source: Source::Embedded,
        format: SkillFormat::Standardized,
        metadata: None,
        resource_paths: None,
        origin_path: None,
    };
    for family in [CliFamily::Claude, CliFamily::Generic] {
        let rendered = render(
            &template,
            "feat/audit",
            "http://127.0.0.1:9119",
            "git-paw",
            &GateCommands::default(),
            &[],
            family,
        );
        let message = std::panic::catch_unwind(|| audit_cli_rendering(&rendered, family, "edited"))
            .expect_err("the audit must fail on a vendor name in the base template")
            .downcast::<String>()
            .map(|s| *s)
            .unwrap_or_default();
        assert!(
            message.contains("Claude"),
            "{family}: names the token: {message}"
        );
    }
}

/// A package file under `assets/cli/<family>/` is read as that family's
/// fragment by the per-file audit.
#[test]
fn a_package_file_under_assets_cli_is_read_as_its_familys_fragment() {
    assert_eq!(
        fragment_owner("assets/cli/claude/boot.md"),
        Some(CliFamily::Claude)
    );
    assert_eq!(
        fragment_owner("assets/cli/generic/skill.md"),
        Some(CliFamily::Generic)
    );
    assert_eq!(fragment_owner("assets/boot-block.md"), None);
    // The claude boot fragment names its vendor and still passes the
    // per-file audit; the same text read as a base asset fails.
    let coordination = bundled_package("coordination").expect("registered");
    let claude_boot: Vec<BundledFile> = coordination
        .assets
        .iter()
        .filter(|f| f.path == FragmentSlot::Boot.path(CliFamily::Claude))
        .copied()
        .collect();
    assert_eq!(claude_boot.len(), 1);
    audit_package_files("coordination", &claude_boot);
    let as_base = vec![BundledFile {
        path: "assets/boot-notes.md",
        ..claude_boot[0]
    }];
    assert!(
        std::panic::catch_unwind(|| audit_package_files("coordination", &as_base)).is_err(),
        "a vendor name outside a fragment directory fails the per-file audit"
    );
}
