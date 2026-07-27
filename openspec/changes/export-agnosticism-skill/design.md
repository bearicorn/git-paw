# Design — export-agnosticism-skill

## Context

Export-agnosticism is an `AGENTS.md` design principle enforced by scattered audits; a HIGH leak once
survived a full cycle. This change codifies it as a focused, repo-local dev skill in the
standards-as-skills suite (`test-strategy`, `code-standards`, and now this).

## Decisions

### D1 — Its own skill, not a section of code-standards

It has a **distinct trigger** ("am I editing an export?"), its own enforcement machinery, and a
recurring-bug history. `code-standards` cross-links to it rather than absorbing it, keeping each
skill focused.

### D2 — Covers EVERY export surface, in every format

Exports are not just skills. The inventory spans code (bundled scripts + the classifier logic
mirrored into `sweep.sh`), the init default config, allowlist/classifier presets, injected artifacts
(AGENTS.md section, boot block, git hooks, settings seeding), and `{{VAR}}` templates. The
"exported vs internal" line: an export crosses into the consumer's environment; git-paw's own `src/`
does not (that is `code-standards`).

### D3 — Enforcement already exists; the skill points at it + adds a standing rule

The lang-agnostic audit, the `auto_approve` classifier stack-neutrality guard, and the
generated-default-config test already enforce agnosticism. The skill references them and states the
standing rule: **a new export surface must ship with its own guard.** Conformance of the skill file
itself is covered by `tests/agent_skills_conform.rs` (from `test-strategy`).

### D4 — Consumed via standards-skill-integration

The worker (author-time) and supervisor (review gate) already consult the project's `.agents/skills/`
standards; this skill joins that set, so no new wiring is needed.

## Non-goals

- Not itself an export; not shipped by `git paw init`.
- No product/CLI/config/wire change; no new enforcement code (the audits already exist).

## Risks

- **Very low** — a dev-tooling skill + a cross-link. The only failure mode is the skill drifting from
  the actual export surfaces as new ones are added; the standing rule (D3) mitigates by requiring a
  guard (and an inventory update) whenever a surface is added.
