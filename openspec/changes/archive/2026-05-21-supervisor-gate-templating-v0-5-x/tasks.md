## 1. Extend SupervisorConfig with six gate-command fields

- [x] 1.1 In `src/config.rs::SupervisorConfig`, add six `Option<String>` fields: `lint_command`, `build_command`, `doc_build_command`, `spec_validate_command`, `fmt_check_command`, `security_audit_command`. Annotate each with `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- [x] 1.2 Order them after the existing `test_command` field for readability. Doc-comment each (one paragraph: purpose + example value for common stacks).
- [x] 1.3 Unit test `gate_command_fields_default_to_none`: deserialize `[supervisor]\nenabled = true\n` (no gate keys); assert all six new fields are `None`.
- [x] 1.4 Unit test `gate_command_fields_round_trip`: build a fully-populated `SupervisorConfig`, serialize to TOML, deserialize, assert equality.
- [x] 1.5 Unit test `gate_command_fields_omit_from_toml_when_none`: build a `SupervisorConfig` with all six gate fields `None`, serialize to TOML; assert the output does not contain any of the six key names.

## 2. Skill template substitution for the six new placeholders

- [x] 2.1 In `src/skills.rs::render`, add string-replace passes for each of the six new placeholders. Source-field mapping per the spec delta (see `specs/agent-skills/spec.md` table). When the source field is `None`, substitute the literal string `(not configured)`.
- [x] 2.2 Confirm `{{CHANGE_ID}}` is NOT substituted by `render` — it passes through verbatim. Add a comment in the render function explaining why (per-invocation substitution by the supervisor agent, not per-render).
- [x] 2.3 Unit test `render_test_command_placeholder_substitutes_from_config`: render a template `"Run {{TEST_COMMAND}}."` with `test_command = Some("just check")`; assert output contains `Run just check.`.
- [x] 2.4 Unit test `render_test_command_placeholder_none_renders_not_configured`: same template, `test_command = None`; assert output contains `Run (not configured).`.
- [x] 2.5 Unit tests for each of the other five new placeholders mirroring 2.3 and 2.4.
- [x] 2.6 Unit test `render_change_id_placeholder_passes_through`: render a template `"Run {{SPEC_VALIDATE_COMMAND}}."` with `spec_validate_command = Some("openspec validate {{CHANGE_ID}} --strict")`; assert output contains `Run openspec validate {{CHANGE_ID}} --strict.` (the inner `{{CHANGE_ID}}` is preserved).

## 3. Rewrite supervisor skill gate prose to use placeholders

- [x] 3.1 In `assets/agent-skills/supervisor.md`, locate the five-gate verification section (§4-§7 per `supervisor-as-pane-followups` archive). Replace every hardcoded `just check` / `cargo test` / `mdbook build` / `cargo audit` / `cargo clippy` / `openspec validate` / `cargo fmt --check` reference in the gate prose with the corresponding `{{...}}` placeholder.
- [x] 3.2 Add a new paragraph at the top of the verification section (or as a sub-bullet on each gate) explaining: "When a placeholder renders as `(not configured)`, skip the tooling invocation. The gate's manual review (e.g. spec scenario coverage check, OWASP-category diff scan) still applies."
- [x] 3.3 Verify by grep that no hardcoded `just `, `cargo `, `mdbook `, `openspec validate` references remain in the gate prose (matches inside fenced code blocks demonstrating example config values are allowed and SHOULD be preserved).
- [x] 3.4 Skill-content test in `src/skills.rs::tests`: with all six placeholders set to distinct `Some("CMD-N")` values, the rendered skill contains each `CMD-N` value. With all six set to `None`, the rendered skill contains the literal `(not configured)` at least once in each gate section.
- [x] 3.5 Audit existing `coordination-skill-followups` and `supervisor-as-pane-followups` skill-content tests for hardcoded `tmpl.content.contains("just check")` or similar; migrate to assertions on placeholders or render with explicit configured values.

## 4. `git paw init` writes commented-out [supervisor] block

- [x] 4.1 In `src/init.rs::run_init`, when generating the initial `.git-paw/config.toml` content (or appending to an existing one), include a commented-out `[supervisor]` block. Every line in the block SHALL be prefixed with `# `. The block content SHALL be a complete example covering `enabled`, `cli`, `test_command`, `lint_command`, `build_command`, `doc_build_command`, `spec_validate_command`, `fmt_check_command`, `security_audit_command`.
- [x] 4.2 Choose example values that represent common stacks (Rust, Node, Python). Documented as comments next to each key (e.g. `# test_command = "just check"   # or: "cargo test", "npm test", "pytest"`).
- [x] 4.3 The init flow SHALL be idempotent: re-running `git paw init` on an existing repo SHALL NOT duplicate the commented block. The simplest approach is to grep the existing config for `# [supervisor]` and skip the block addition if found.
- [x] 4.4 Integration test `tests/cli_init_writes_supervisor_block.rs`: run `git paw init` in a `TempDir`, read the written `config.toml`, assert it contains a commented `[supervisor]` block with all seven keys listed; uncomment the block and verify it parses as a valid `SupervisorConfig`.

## 5. Update git-paw's own .git-paw/config.toml with the new keys

- [x] 5.1 The repo's own `.git-paw/config.toml` is gitignored — no change there.
- [x] 5.2 The repo's CI / dogfood config (or a `dogfood-config.toml` template if it exists) SHALL be updated to populate the six new fields with git-paw's values (`just check`, `cargo clippy -- -D warnings`, `cargo build`, `mdbook build docs/`, `openspec validate {{CHANGE_ID}} --strict`, `cargo fmt --check`, `cargo audit`).
- [x] 5.3 Verify by running a supervisor session in the git-paw repo after this change: the rendered skill contains git-paw's command strings (proving the substitution works) AND the same strings the supervisor agent used before this change (proving behaviour is unchanged from the git-paw perspective).

## 6. Documentation

- [x] 6.1 `docs/src/configuration/README.md` — document the six new `[supervisor]` keys with example values for common stacks (Rust/Node/Python/Go).
- [x] 6.2 `docs/src/user-guide/supervisor.md` — add a paragraph naming the placeholders and explaining the `(not configured)` graceful-skip behaviour.
- [x] 6.3 Rustdoc on each new `SupervisorConfig` field: purpose + example value.
- [x] 6.4 `mdbook build docs/` clean.

## 7. Quality gates

- [x] 7.1 `cargo fmt` + `cargo clippy --all-targets -- -D warnings` clean.
- [x] 7.2 `just check` (or the equivalent `{{TEST_COMMAND}}` setting in this repo) green.
- [x] 7.3 `openspec validate supervisor-gate-templating-v0-5-x --strict` passes.
- [x] 7.4 `just deny` clean.

## 8. Release notes (in archive's release-notes.md, NOT CHANGELOG)

- [x] 8.1 Call out: supervisor skill gate commands are now repo-configurable via `[supervisor].{test,lint,build,doc_build,spec_validate,fmt_check,security_audit}_command`. Pre-v0.5.x configs continue to work; missing keys render as `(not configured)` and skip the tooling invocation.
- [x] 8.2 Call out: `git paw init` writes a commented-out `[supervisor]` block listing every gate key with example values.
- [x] 8.3 Call out: `{{CHANGE_ID}}` in `spec_validate_command` is substituted by the supervisor agent at verification time (per-invocation), not by `skills::render` (per-render).
