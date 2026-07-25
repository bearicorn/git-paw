## Why

The standards skills (`test-strategy`, `code-standards`) only pay off if they are actually
consulted. Wire both consumers to them: the **implementing agent** reads them during
implementation (write to the standard), and the **supervisor** reads them at the review gate
(verify against the standard). Because a project's gate process is defined by its own `AGENTS.md`,
this is agnostic by construction — git-paw ships the *mechanism* (consult the project's
`.agents/skills/` standards), never an opinion. In git-paw's own dogfood it resolves to git-paw's
`test-strategy` + `code-standards`; a consumer supplies their own.

## What Changes

- **Exported supervisor skill** (`assets/agent-skills/supervisor.md`): at the review/verification
  gate, consult the project's `.agents/skills/` standards skills (e.g. `test-strategy`,
  `code-standards`) when present and verify the change conforms.
- **Exported worker-facing surface** (the coordination skill / boot block): during implementation,
  consult the project's `.agents/skills/` standards skills.
- **Project-agnostic wording** — reference "the project's standards skills" generically; embed no
  git-paw-specific (Rust/cargo/`PawError`) standards in the exported assets. Extends the
  `lang-agnostic-skills` no-leak audit to cover the new consult wording.

Additive and backward-compatible: when a project has no `.agents/skills/` standards, behavior is
unchanged.

## Capabilities

### New Capabilities
- `standards-skill-integration`: the agnostic mechanism by which the worker (author-time) and the
  supervisor (gate-time) consult the project's `.agents/skills/` standards skills.

### Modified Capabilities
_None yet._ Authored as a new capability for clarity; the spec-consolidation workstream MAY later
fold it under `supervisor-skill-discipline` (gate side) and `agent-skills` (worker side).

## Impact

- **Exported assets:** `assets/agent-skills/supervisor.md` + `assets/agent-skills/coordination.md`
  (agnostic consult steps).
- **Skill-content ripple (scope up front):** editing the exported skills ripples into the
  `skills.rs` unit tests and the `*_skill_content.rs` integration tests
  (`coordination_region_skill_content.rs`, `supervisor_routing_skill_content.rs`, …) that pin skill
  prose — update in lockstep. The **tracked `.git-paw/scripts`/skill copies** that dogfood executes
  must be synced from `assets/` (per the known drift hazard).
- **Agnosticism guard:** extend the `lang-agnostic-skills` audit so the new consult wording carries
  no hard-coded stack/language standard.
- **Consumes:** git-paw's own `test-strategy` + `code-standards` for the dogfood; consumers supply
  their own. No product code path changes; no CLI/config/wire change.
