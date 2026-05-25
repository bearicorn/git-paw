## Why

`supervisor-as-pane-followups` codified the five-gate verification workflow (drift 66: testing → regression → spec audit → doc audit → security) and the continuous-sweep absorption doctrine (drift 65) in `assets/agent-skills/supervisor.md`. But the gate prose uses repo-specific command names: `just check`, `cargo test`, `mdbook build`, `cargo fmt --check`, `openspec validate <change> --strict`, `cargo audit`.

`{{TEST_COMMAND}}` substitution already exists in the skill template for gate 1 (Testing) — that piece is fine. The remaining four gates (regression analysis, spec audit, doc audit, security audit) reference git-paw-specific tooling that does NOT generalize. A user dogfooding paw in a Python or Node or Go repo gets a supervisor skill that recommends `mdbook build` and `cargo audit` — neither exists.

The user's correction (2026-05-16): *"just and cargo here are specific to this repo. So the actual supervisor skill needs to be configurable based on repo."*

This is MILESTONE drift 67. Specced into MILESTONE.md only; never folded into an active OpenSpec change. v0.5.0 ships an incomplete supervisor skill if drift 67 doesn't land.

## What Changes

### 1. Add new `[supervisor]` config keys for gate commands

`SupervisorConfig` in `src/config.rs` SHALL gain six new `Option<String>` fields:

- `test_command` — already exists, retained.
- `lint_command` — pre-stage lint (`cargo clippy` / `npm run lint` / `ruff` / etc.). Drives gate 1's lint sub-step where present.
- `build_command` — for repos where build is separate from test (`cargo build` / `npm run build` / `mvn package`). Used by gate 1's compile sub-step when set.
- `doc_build_command` — gate 4 doc audit input (`mdbook build` / `sphinx-build` / `mkdocs build`).
- `spec_validate_command` — gate 3 spec audit input (`openspec validate <change> --strict` for OpenSpec repos; empty for markdown-only). Substitution `{{CHANGE_ID}}` is expanded by the renderer.
- `fmt_check_command` — `cargo fmt --check` / `prettier --check` / `gofmt -l`.
- `security_audit_command` — gate 5 security tooling hook (`cargo audit` / `npm audit` / `bandit`). Empty string means "skip the tooling-aided phase; manual diff review still applies".

All six fields default to `None`. Configs without them load unchanged.

### 2. Template substitution in the supervisor skill

`src/skills.rs::render` SHALL gain placeholder substitution for each of the new keys, plus the existing `{{TEST_COMMAND}}`. New placeholders:

- `{{LINT_COMMAND}}`
- `{{BUILD_COMMAND}}`
- `{{DOC_BUILD_COMMAND}}`
- `{{SPEC_VALIDATE_COMMAND}}`
- `{{FMT_CHECK_COMMAND}}`
- `{{SECURITY_AUDIT_COMMAND}}`

When a placeholder's source value is `None`, the renderer SHALL substitute the literal text `(not configured)` so the rendered skill remains readable AND the supervisor agent recognizes the gate as having no tooling-aided phase.

`{{CHANGE_ID}}` is a per-invocation placeholder (the current change being verified). It is substituted by the supervisor agent at call time using the change name, not by the renderer.

### 3. Rewrite `assets/agent-skills/supervisor.md` gate prose to use placeholders

The five-gate verification section SHALL replace every hardcoded command with the corresponding placeholder. Examples:

- Gate 1 (Testing): `Run {{TEST_COMMAND}}` (unchanged).
- Gate 1 (Lint sub-step, NEW): `If {{LINT_COMMAND}} is configured (not "(not configured)"), run it.`
- Gate 3 (Spec audit): `Run {{SPEC_VALIDATE_COMMAND}} where {{CHANGE_ID}} is substituted with the change's name.`
- Gate 4 (Doc audit): `Run {{DOC_BUILD_COMMAND}} to confirm doc builds; skip if "(not configured)".`
- Gate 5 (Security audit): `Run {{SECURITY_AUDIT_COMMAND}} for tooling-aided checks; manual OWASP-category review applies in any case.`

The gate prose SHALL state explicitly: "When a command renders as `(not configured)`, skip the tooling invocation. The gate's manual review (e.g. spec scenario coverage check, OWASP-category diff scan) still applies."

### 4. Default `[supervisor]` config in `git paw init`

`src/init.rs::run_init` SHALL write a `.git-paw/config.toml` (or extend an existing one) with a commented-out `[supervisor]` block enumerating the new keys with their typical values for common stacks. Example commented block:

```toml
# [supervisor]
# enabled = false
# cli = "claude"
# # Gate command templates (substituted into the supervisor skill).
# test_command = "just check"                 # or: "cargo test", "npm test", "pytest"
# lint_command = "just lint"                  # or: "cargo clippy -- -D warnings", "npm run lint", "ruff check ."
# build_command = "cargo build"               # or: "npm run build", "mvn package"
# doc_build_command = "mdbook build docs/"    # or: "sphinx-build", "mkdocs build"
# spec_validate_command = "openspec validate {{CHANGE_ID}} --strict"  # OpenSpec only
# fmt_check_command = "cargo fmt --check"     # or: "prettier --check .", "gofmt -l ."
# security_audit_command = "cargo audit"      # or: "npm audit", "bandit -r ."
```

The block is commented-out so existing configs are unaffected.

## Capabilities

### New Capabilities

*(none — extends existing capabilities)*

### Modified Capabilities

- `supervisor-config` — `SupervisorConfig` gains six new optional gate-command fields with safe defaults.
- `agent-skills` — supervisor skill template gains six new `{{...}}` placeholders; the rendered prose uses them in place of hardcoded git-paw-specific commands.

## Impact

**Code:**

- `src/config.rs::SupervisorConfig` — add six `Option<String>` fields with serde defaults and skip-serializing-if-none.
- `src/skills.rs::render` — add substitution arms for the six new placeholders; treat `None` as `(not configured)`.
- `src/init.rs::run_init` — append the commented-out `[supervisor]` block to written `config.toml`.
- `assets/agent-skills/supervisor.md` — rewrite gate prose to use placeholders.

**Tests:**

- `src/config.rs::tests` — round-trip the new fields; pre-v0.5 configs (no `[supervisor]` block) load with all six fields = `None`; populated config exposes them correctly.
- `src/skills.rs::tests` — placeholder substitution: `None` → `(not configured)`; `Some("just check")` → `just check`; `Some("openspec validate {{CHANGE_ID}} --strict")` passes through (the `{{CHANGE_ID}}` is substituted at call time, not by `render`).
- Skill-content test: the rendered supervisor skill contains the six placeholder names (proving the template uses them, not hardcoded values).
- Integration test: `git paw init` writes a config with the commented-out block; subsequent `git paw start` parses it without error.

**Docs:**

- `docs/src/configuration/README.md` — document the six new keys with example values for common stacks.
- `docs/src/user-guide/supervisor.md` — note that supervisor skill commands are repo-configurable; link to configuration reference.
- `--help` on `git paw init`: no change (the wizard adds commented-out config).

**Backward compatibility:**

- All six fields are `Option<String>` with serde defaults; pre-v0.5.x configs load unchanged.
- The rendered skill text changes (placeholders replace hardcoded commands), so behaviour-snapshot tests that asserted on the old hardcoded forms will need updating. Audit `coordination-skill-followups` and `supervisor-as-pane-followups` skill-content tests for hardcoded `just check` / `cargo audit` etc. assertions and update them.

**Mismatches resolved:**

- MILESTONE drift item 67 — closed. Supervisor skill is now repo-agnostic for the five-gate workflow.
