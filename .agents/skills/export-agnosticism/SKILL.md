---
name: export-agnosticism
description: Keep everything git-paw EXPORTS to a consumer project-agnostic across every export surface — bundled skills, shell scripts and mirrored logic (code), the init default config, any agent-CLI allow rules git-paw seeds, injected AGENTS.md sections / boot blocks / git hooks / settings seeding, and templates. Use whenever a change adds or edits any exported asset. Codifies the export-surface inventory, the agnosticism rule, how to stay agnostic per surface, the enforcing audits, and the review-gate checklist.
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
- **Agent-facing prose assets** — instructions, rules, directives, framing text and task-prompt
  scaffolding injected into an agent's sidecar, boot prompt, or skill content, authored in bundled
  assets rather than assembled from compiled string literals: `assets/inter-agent-rules.md`,
  `assets/governance-section-header.md`, `assets/spec-path-doctrine/*.md`, `assets/boot-task-prompts/*.md`, `assets/task-prompts/**/*.md`.
  Compiled code keeps the assembly around them (placeholder substitution, conditional inclusion,
  per-backend selection, dedupe, region gating) — only the sentences live in the asset.
- **Scripts & mirrored logic (code)** — `assets/scripts/{sweep,broker,docs-fetch}.sh`, including
  the live-prompt gate mirrored into `sweep.sh` from `supervisor::permission_prompt`.
- **Init default config** — the `.git-paw/config.toml` that `git paw init` generates.
- **Agent-CLI allow rules written by `git paw setup-cli`** — the permission rules the
  `cli-setup` capability writes into a consumer's agent CLI settings. This surface is **live**:
  `setup-cli --allowlist` writes path-scoped rules naming git-paw's own bundled helper scripts,
  but only when the user runs the command — running it is the consent, and nothing else in
  git-paw applies it. Every entry is a compiled constant derived from the fact row and the
  repository root, never from a command argument, the consumer's config or repository content,
  so two differently configured repositories get byte-identical entries. A rule names a git-paw
  helper by its stable path — never a consumer toolchain verb, and never a wildcard. Any change
  to the grant table is a change to this export surface.
- **Injected artifacts** — the pane **boot block**, the marker-delimited git-paw section injected
  into the consumer's `AGENTS.md`, **git hooks** (post-commit / pre-push / pre-commit, script
  bodies bundled at `assets/hooks/*.sh` — the installation mechanism itself stays compiled), CLI
  `settings.json` allowlist seeding.
- **Templates** — `{{VAR}}` placeholders (`{{TEST_COMMAND}}`, `{{BRANCH_ID}}`,
  `{{GIT_PAW_BROKER_URL}}`, …) that pull consumer specifics from config rather than inlining them.

## How to stay agnostic (per surface)

- **Toolchain verbs** — source from consumer config (the `[supervisor]` gate-command keys);
  NEVER hard-code `cargo`/`just`/`openspec`/`npm`/`pytest`/`go` as universally safe or required.
  The retired `auto_approve` classifier is the cautionary case: it baked a verb set that was
  either git-paw's own toolchain or a guess at the consumer's, and the leak survived a whole
  cycle. Prefer exporting no verb list at all.
- **Config defaults** — all-commented examples with multi-stack `# or:` disclaimers; no active
  git-paw-specific default value.
- **`setup-cli` allow rules** — **path-scoped** grants naming a git-paw helper script (never
  `curl *` / `cd *`, never a consumer toolchain verb); least-privilege; written only on the
  consumer's explicit consent (they ran the command, after seeing its plan); and never derived
  from the consumer's configuration, stack or repository content.
- **Skills / injected prose** — no stack tokens; region-gate opsx/OpenSpec prose
  (`<!-- opsx-role-gating:begin/end -->`); wrap legitimate preset enumerations in
  `<!-- allowlist-prose -->` sentinels; defer commit-message format to the consumer.
- **Injected sections / hooks** — marker-delimited and idempotent; carry no baked stack or
  convention.
- **Consumer specifics** — always via `{{VAR}}` template substitution or injected config, never
  inlined.

## How it's enforced — and the standing rule

- `tests/lang_agnostic_skill_audit.rs` — forbidden stack tokens + Conventional-Commits-prefix leak
  in the rendered skills (with `<!-- allowlist-prose -->` stripping), extended to cover every
  bundled agent-facing prose asset (not only the supervisor/coordination skills) and to forbid
  vendor product names/paths (e.g. `Claude Code`, `.claude/`) outside an `<!-- allowlist-prose -->`
  span — the mechanism that keeps a deliberate CLI enumeration legal without making vendor naming
  an accident.
- The live-prompt gate's `sweep.sh`↔Rust parity tests (`tests/sweep_sh_live_gate_parity.rs`).
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
