---
name: export-agnosticism
description: Keep everything git-paw EXPORTS to a consumer project-agnostic across every export surface — bundled skills, shell scripts and mirrored logic (code), the init default config, allowlist/classifier presets, injected AGENTS.md sections / boot blocks / git hooks / settings seeding, and templates. Use whenever a change adds or edits any exported asset. Codifies the export-surface inventory, the agnosticism rule, how to stay agnostic per surface, the enforcing audits, and the review-gate checklist.
license: MIT
compatibility: git-paw
---

# Export Agnosticism

Use this **whenever a change adds or edits anything git-paw ships or writes into a consumer's
repo or session — in ANY format** (markdown, shell, TOML, JSON, injected doc sections, git
hooks, templates). If the diff touches an export surface below, it must stay project-agnostic.

## Exported vs internal — the line

"Exported" = an artifact that crosses into the **consumer's** environment. git-paw's own `src/`
implementation is **not** an export (that's `code-standards`). Ask: *would a consumer of git-paw
receive, run, or have this written into their repo/session?* If yes, it's an export and this
skill applies.

## The rule

Exported assets MUST be project-agnostic. Per-project conventions — commit-message format,
stack/test/build commands, governance/spec tooling — belong to the **consumer** via their
injected `AGENTS.md` / config / presets. git-paw's OWN conventions (Conventional Commits, its
cargo/just/openspec toolchain) live ONLY in git-paw's `AGENTS.md` / `cliff.toml` — never in an
export.

## Export-surface inventory — check EVERY one

- **Skills** — `assets/agent-skills/{coordination,supervisor,docs-fetch}.md`.
- **Scripts & mirrored logic (code)** — `assets/scripts/{sweep,broker,docs-fetch}.sh`, including
  the auto-approve **classifier** logic mirrored into `sweep.sh`.
- **Init default config** — the `.git-paw/config.toml` that `git paw init` generates.
- **Allowlists / classifier presets** — the dev-command-allowlist preset, the `auto_approve`
  safe-command classifier, the curl / broker-helper allowlist seeding.
- **Injected artifacts** — the pane **boot block**, the marker-delimited git-paw section injected
  into the consumer's `AGENTS.md`, **git hooks** (post-commit / pre-push), CLI `settings.json`
  allowlist seeding.
- **Templates** — `{{VAR}}` placeholders (`{{TEST_COMMAND}}`, `{{BRANCH_ID}}`,
  `{{GIT_PAW_BROKER_URL}}`, …) that pull consumer specifics from config rather than inlining them.

## How to stay agnostic (per surface)

- **Toolchain verbs** — source from the resolved stack preset (`[supervisor.common_dev_allowlist]`)
  / consumer config; NEVER hard-code `cargo`/`just`/`openspec`/`npm`/`pytest`/`go` as universally
  safe or required. The classifier must not treat git-paw's toolchain as always-safe for every
  consumer (this is the exact leak that survived a whole cycle — the `auto_approve` classifier).
- **Config defaults** — all-commented examples with multi-stack `# or:` disclaimers; no active
  git-paw-specific default value.
- **Allowlists** — a universal tier + opt-in named stack presets; **path-scoped** grants (never
  `curl *` / `cd *`); least-privilege.
- **Skills / injected prose** — no stack tokens; region-gate opsx/OpenSpec prose
  (`<!-- opsx-role-gating:begin/end -->`); wrap legitimate preset enumerations in
  `<!-- allowlist-prose -->` sentinels; defer commit-message format to the consumer.
- **Injected sections / hooks** — marker-delimited and idempotent; carry no baked stack or
  convention.
- **Consumer specifics** — always via `{{VAR}}` template substitution or injected config, never
  inlined.

## How it's enforced — and the standing rule

- `tests/lang_agnostic_skill_audit.rs` — forbidden stack tokens + Conventional-Commits-prefix leak
  in the rendered skills (with `<!-- allowlist-prose -->` stripping).
- The `auto_approve` classifier stack-neutrality guard (`src/supervisor/auto_approve.rs`) + its
  `sweep.sh` parity tests.
- The generated-default-config "all commented / multi-stack" test (`src/config.rs`).

**Standing rule:** a NEW export surface MUST ship with an agnosticism guard. If you add an
exported asset and cannot point to the test that would catch a stack/convention leak in it, add
that test in the same change.

## Review-gate checklist (supervisor)

- [ ] Does this diff touch an export surface — skill / script / config / preset / injected
      section / hook / template?
- [ ] If so, does it bake git-paw's stack, toolchain, commit-format, or spec-tooling?
- [ ] Is every consumer-specific value sourced from config / preset / injected `AGENTS.md`, not
      inlined?
- [ ] Is the agnostic behavior covered by an enforcing audit — and if it's a NEW surface, was a
      guard added in the same change?

---

This skill governs git-paw's OWN exports; it is itself a **repo-local dev skill**, not an export.
A consumer of git-paw applies their own standards to their own artifacts.
