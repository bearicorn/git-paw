## Why

git-paw's own standard is "all tests behavioral," but the suite has drifted (source-grep
introspection tests, per-field batteries, brittle prose pins). Consolidation needs a
*repeatable decision procedure* — "what test type does this behavior need, and how do I write
it so a refactor can't falsely break it?" — not a one-off cut. Codifying that procedure as a
portable, discoverable, enforceable **agent skill** makes it durable: new tests conform by
default, and the skill is the reference the consolidation applies. It is the linchpin the rest
of the testing workstream keys off.

## What Changes

- Ship a **`test-strategy` agent skill** at `.agents/skills/test-strategy/SKILL.md` in the
  **agentskills.io** standard format (directory + `SKILL.md`; `name` matching the folder;
  `name`/`description` frontmatter). It encodes: the layer-decision procedure (unit-table /
  integration / e2e-PTY / asset-parity), robustness rules, the anti-pattern catalog, and the
  consolidation routing (delete / table-ify / collapse-to-integration / replace-with-e2e /
  keep / protect-sole-guard).
- Add a **CI validation guard** that checks every skill under `.agents/skills/` conforms to
  the agentskills.io spec (name↔folder match, required frontmatter), failing the build on a
  non-conformant skill. Dogfood git-paw's own `skill-validation` loader for this (no external
  dependency); `skills-ref validate` is the equivalent external option.
- Add an **AGENTS.md / CONTRIBUTING pointer** naming the skill as the canonical procedure for
  authoring and consolidating tests.

Repo-local dev tooling only — **not** an exported/bundled asset. It is git-paw-specific
(cargo, the PTY harness, OpenSpec scenarios), so it lives in `.agents/skills/` (git-paw's own
resolver's standard location), never in `assets/agent-skills/` (which stays project-agnostic).

## Capabilities

### New Capabilities
- `test-strategy`: the repo's test-authoring standard — the skill artifact, its agentskills.io
  conformance, the CI validation guard, and the contributor pointer.

### Modified Capabilities
_None._ No product behavior, CLI, config, or wire surface changes.

## Impact

- **New:** `.agents/skills/test-strategy/SKILL.md` (authored); a CI job / integration test
  validating `.agents/skills/*`; an AGENTS.md/CONTRIBUTING pointer.
- **Sequencing:** precedes `test-suite-consolidation` (which *applies* the skill across the
  suite) and pairs with the `cli-interaction-e2e` PTY net (the e2e layer the skill routes to).
- **No code/behavior change** to the shipped binary; no exported-asset change (agnosticism
  intact — this is a dev skill).
- **Not enum-variant ripple.**
