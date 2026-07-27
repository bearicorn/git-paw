## Why

"Exported assets must be project-agnostic" is a core git-paw design principle (`AGENTS.md`), but
it lives as scattered prose plus a few audits — and a HIGH leak (the auto-approve classifier
hard-coding cargo/just/openspec) survived an entire release cycle. Codify the principle as a
repo-local dev skill the **worker** consults when editing an export and the **supervisor** checks
at the review gate — closing the same loop as `test-strategy` / `code-standards`. Crucially it must
span **every** export surface, not just skills: code (bundled scripts + mirrored classifier logic),
config (the init default), presets/allowlists, injected artifacts (AGENTS.md sections, boot blocks,
git hooks, settings seeding), and templates.

## What Changes

- New **`export-agnosticism` agent skill** at `.agents/skills/export-agnosticism/SKILL.md`
  (agentskills.io). It enumerates the full export-surface inventory, states the agnosticism rule,
  gives per-surface how-to, lists the enforcing audits, states the standing "new surface needs a
  guard" rule, and provides a review-gate checklist.
- **Cross-link** from `code-standards` (an agent editing an export is routed to it).

Repo-local dev skill — it is itself **not** an export. Conformance is auto-covered by the
`test-strategy` change's `tests/agent_skills_conform.rs`; the agnosticism enforcement itself already
exists (lang-agnostic audit, classifier guard, config-default test).

## Capabilities

### New Capabilities
- `export-agnosticism-skill`: the skill artifact, its agentskills.io conformance, and the
  code-standards cross-link.

### Modified Capabilities
_None._ No product behavior, CLI, config, or wire change.

## Impact

- **New:** `.agents/skills/export-agnosticism/SKILL.md`; a cross-link edit in
  `.agents/skills/code-standards/SKILL.md`.
- Consumed by the worker + supervisor via `standards-skill-integration`.
- Not itself exported (`assets/agent-skills/` untouched); no code/behavior change.
