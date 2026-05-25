## Context

The five-gate verification workflow shipped in `supervisor-as-pane-followups` (drifts 65/66, archived 2026-05-20) is correct in structure but hardcodes git-paw-specific tooling in the rendered skill prose. Drift 67 surfaced on 2026-05-16 — captured to MILESTONE.md, deferred from supervisor-as-pane-followups (which was already large enough). v0.5.0 cannot ship the supervisor skill with `just check` / `cargo audit` baked in if the goal is multi-language repo adoption.

## Decisions

### D1 — Substitute via the existing `{{...}}` placeholder mechanism

`src/skills.rs::render` already substitutes `{{BRANCH_ID}}`, `{{GIT_PAW_BROKER_URL}}`, `{{PROJECT_NAME}}`, `{{TEST_COMMAND}}`. Adding six more placeholders is the obvious extension. Reusing the existing mechanism keeps the surface coherent and the unknown-placeholder warning path stable.

Alternative considered: parse the skill text as templated markdown via a templating crate (handlebars/tera). Rejected — adds a dependency for a 6-key substitution. The existing `replace` chain is fine.

### D2 — `None` renders as `(not configured)`, not empty string

When a gate command is unset in config, the skill could substitute:
- An empty string (rendered as nothing).
- A literal `(not configured)` placeholder.
- The `{{...}}` placeholder itself (delegating to a runtime check by the supervisor agent).

`(not configured)` is the chosen middle path: visible in the rendered skill (so the supervisor agent reads a complete sentence), clearly machine-readable (so the agent can branch on it: `if cmd == "(not configured)" { skip; } else { run cmd; }`), and pre-bound at render time so the supervisor agent doesn't have to query the broker for config lookup.

Empty string is rejected because the rendered prose breaks (sentences with empty command-name slots are confusing). The raw `{{...}}` is rejected because the unknown-placeholder warning path fires and pollutes logs.

### D3 — `{{CHANGE_ID}}` is per-invocation, not per-render

The spec-validate command typically takes a change name as argument (`openspec validate <change> --strict`). Embedding the change name in the rendered skill is wrong — the skill is rendered once at session boot but the supervisor verifies different changes over time.

The substitution rule: `render` does NOT substitute `{{CHANGE_ID}}`. It passes through into the rendered skill verbatim. The supervisor agent substitutes it at verification time, using the change name it's currently auditing. This matches how the existing `{{BRANCH_ID}}` placeholder works on the coding-agent side.

### D4 — Commented-out config block in `git paw init`, not silent defaults

`git paw init` could write either:
- Sensible language-detected defaults: e.g. detect Cargo.toml → write `test_command = "cargo test"`.
- A commented-out block listing every key with example values.

The commented-out approach is chosen because:
- Language detection is brittle (a monorepo with multiple stacks confuses it).
- The commented block teaches the user what's available without committing them to defaults.
- Existing configs are unaffected (no field is added; only a commented block, which TOML parsing ignores).

A future change could add `git paw init --detect` that runs language detection and writes uncommented sensible defaults. Out of scope for this change.

### D5 — No CLI flag overrides at the `git paw start` level

A user could plausibly want to override gate commands per-launch without editing config. A `--test-command "pytest"` flag, etc. The change does NOT add CLI overrides. Reasons:

- The gate commands are session-stable. A user wanting different gates for a session edits config or uses a per-repo profile.
- Adding six new flags multiplies the `git paw start` surface area without proportional benefit.
- A `[supervisor.profiles.<name>]` config block is the natural extension if profiles are needed; defer to v0.6.0 if requested.

## Risks / Trade-offs

- **Skill-content test breakage.** The previously hardcoded `just check`, `cargo audit`, etc. references in the skill text become placeholders. Any test asserting `tmpl.content.contains("just check")` MUST migrate to assert on the placeholder substitution (e.g. `tmpl.content.contains("{{TEST_COMMAND}}")` for the unrendered template, or set `test_command = Some("just check")` and assert on the rendered output). Audit ~5-10 existing assertions during implementation.

- **Pre-v0.5.x config silence.** Configs without `[supervisor]` get all six placeholders rendered as `(not configured)`. The supervisor agent reads "skip if not configured" prose and proceeds without erroring. The user sees a less-rigorous verification cycle; the supervisor flags it in its first `agent.status` ("Five-gate verification with lint / build / doc / security gates skipped — `[supervisor].{lint,build,doc_build,security_audit}_command` unset"). Trade-off: silent skip vs. force the user to set values they may not have. Silent skip is acceptable for v0.5.x.

- **`(not configured)` as a magic string.** A user could plausibly set `lint_command = "(not configured)"` literally to force-skip. That's fine — the parsing is whitespace-exact. Documented in `docs/src/configuration/README.md`.

## Migration / Rollout

- Existing `.git-paw/config.toml` files: unchanged behaviour. The new fields default to `None` and the rendered skill substitutes `(not configured)`.

- Users wanting full v0.5.x verification: add the six new keys to `[supervisor]` in `.git-paw/config.toml`. The commented-out block written by `git paw init` (for new repos) shows the expected shape.

- For the git-paw repo itself: the `.git-paw/config.toml` SHALL be updated as part of this change to populate the six fields with the git-paw-specific values. This is the implementation's own dogfood validation — once the placeholders work, the git-paw repo's supervisor sessions produce the same rendered prose as before, but the mechanism is now repo-agnostic.

- No version bump beyond v0.5.0; this change is part of the v0.5.0 release prep.
