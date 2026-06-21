## Why

The binary exports bundled skills (`assets/agent-skills/*.md`, installed into a
consumer repo by `git paw init` and injected into agent context). Those
exported assets MUST be project-agnostic — but git-paw's OWN commit convention
(Conventional Commits, `feat(scope): …`) has leaked into them. This is one
instance of a broader separation that needs enforcing: **what the binary
exports** (generic; serves every consumer) vs **what is git-paw-repo-specific**
(git-paw's own `AGENTS.md` / `CLAUDE.md` / `cliff.toml`, which legitimately
require Conventional Commits for git-paw's changelog generation).

Two concrete leaks remain:
1. **Spec contradiction.** The `agent-skills` "Embedded coordination skill"
   requirement (item 13) already de-opinionates message format, but the sibling
   "Coordination skill SHALL teach per-group commit cadence" requirement still
   hardcodes it — item 3 says the message "SHALL follow the project's
   conventional-commit pattern", and a scenario *requires* the exported skill to
   show a conventional-commit prefix example. These contradict each other.
2. **Residual lean in the exported asset.** `coordination.md` defers to
   `AGENTS.md` but still *illustrates* with git-paw's own `feat(coverage):`
   example commits — nudging every consumer toward git-paw's convention.

Commit-message format is the *consumer's* call, supplied by their injected
`AGENTS.md` — exactly like the already-reverted AI-trailer rule and the v0.8.0
`DEV_ALLOWLIST_PRESET` de-opinionation. git-paw's repo keeps Conventional
Commits, but only via git-paw's own `AGENTS.md`, never via the exported skill.

## What Changes

- De-opinionate the "Coordination skill SHALL teach per-group commit cadence"
  requirement: commit-message format defers ENTIRELY to the host project's
  `AGENTS.md` — the bundled skill SHALL NOT mandate, default to, OR present as
  its recommendation a Conventional-Commits prefix. Any commit example the
  section needs (e.g. the `(part N of M)` split) SHALL use a format-NEUTRAL
  subject (no convention-specific prefix).
- Replace the "Coordination skill names conventional-commit types" scenario
  (which *required* a conventional-commit prefix example) with one asserting the
  section defers format to `AGENTS.md` and shows only format-neutral examples.
- **Generalise the principle into a testable contract** (`lang-agnostic-skills`):
  bundled skills are convention-agnostic, not only stack-agnostic. The existing
  bundled-skill leak audit SHALL also flag a hardcoded commit-convention
  mandate/default/recommendation — so the export/repo-specific separation is
  enforced by the audit, not just this one fix.
- Genericise the exported `coordination.md`: replace the `feat(coverage): …`
  example commits with format-neutral examples; remove the Conventional-Commits
  illustration; keep the deferral to `AGENTS.md`.
- The per-group *cadence* discipline (commit per task group, ~10-file soft cap,
  `(part N of M)` split) and the releasable-unit / amend-fixup discipline are
  unchanged — only the message-FORMAT lean is removed.
- git-paw's own `AGENTS.md` / `CLAUDE.md` Conventional-Commits convention is
  **not touched** (it is the repo-specific side of the separation).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `agent-skills`: de-opinionate the "Coordination skill SHALL teach per-group
  commit cadence" requirement — defer commit-message format entirely to the
  consumer's `AGENTS.md`, no Conventional-Commits mandate/default/example; the
  per-group cadence and releasable-unit discipline are unchanged.
- `lang-agnostic-skills`: add the convention-neutrality principle — bundled
  skills SHALL NOT hardcode a project-specific commit convention, and the
  bundled-skill leak audit SHALL flag one. Generalises the existing
  stack-neutrality audit to cover conventions.

## Impact

- **Specs:** `openspec/specs/agent-skills/spec.md` (one MODIFIED requirement) +
  `openspec/specs/lang-agnostic-skills/spec.md` (convention-neutrality audit).
- **Code:** `assets/agent-skills/coordination.md` — replace the `feat(coverage):`
  example commits and the Conventional-Commits illustration with format-neutral
  examples + deferral to `AGENTS.md`. Plus the bundled-skill leak-audit test
  gains a commit-convention check. (NOT git-paw's own `AGENTS.md`/`CLAUDE.md`.)
- **Tests:** reframe the skill-content test that asserted a conventional-commit
  prefix example is present → assert deferral to `AGENTS.md` + absence of a
  Conventional-Commits mandate/example; extend the leak audit
  (`src/skills.rs`) with the commit-convention check.
- **Docs:** verify `docs/src/user-guide/coordination.md` carries no
  Conventional-Commits mandate (the bundled skill is the source of truth).
