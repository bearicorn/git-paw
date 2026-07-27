## ADDED Requirements

### Requirement: Doc-completeness skill provided

The repository SHALL provide a `doc-completeness` agent skill at
`.agents/skills/doc-completeness/SKILL.md` (agentskills.io; `name` matching the folder) covering
git-paw's four doc layers (`--help`, README, mdBook, rustdoc / config-reference), the per-change-type
mapping of which layers to update, the "layers consistent and `mdbook build` passes" rule, and the
Gate-4 doc-audit checklist.

#### Scenario: Skill present and conformant

- **GIVEN** the repository checkout
- **WHEN** `.agents/skills/doc-completeness/SKILL.md` is loaded
- **THEN** it SHALL parse as a valid agentskills.io skill (`name = "doc-completeness"`, non-empty description) and enumerate the four doc layers

### Requirement: Security-and-safety-review skill provided

The repository SHALL provide a `security-and-safety-review` agent skill at
`.agents/skills/security-and-safety-review/SKILL.md` (agentskills.io; `name` matching the folder)
covering two lenses — **security** (external bad actors: least-privilege path-scoped allowlists,
injection-safe construction, no secrets in flags/env, unsafe / path handling, vetted dependencies) and
**safety** (the blast radius of a rogue or mistaken agent git-paw runs: out-of-worktree actions,
irreversible git, persistence / backdoors, exfiltration; the containment a change must not weaken; the
interim role until the planned FS-scoped sandbox).

#### Scenario: Skill present and covers both lenses

- **WHEN** the `security-and-safety-review` skill is loaded
- **THEN** it SHALL be a valid agentskills.io skill with a security section (external bad actors) and a safety section (rogue-agent blast radius)
- **AND** the safety section SHALL name the containment (worktree confinement, danger-list, send-gate) and the planned sandbox

### Requirement: Definition-of-done skill ties the dimensions together

The repository SHALL provide a `definition-of-done` agent skill at
`.agents/skills/definition-of-done/SKILL.md` that defines a change as done only when spec, code,
tests, docs, security, safety, and (when an export is touched) export-agnosticism are all satisfied,
and that references each dimension's skill.

#### Scenario: Meta-skill references every dimension

- **WHEN** the `definition-of-done` skill is loaded
- **THEN** it SHALL enumerate the dimensions (spec, code, tests, docs, security, safety, export-agnosticism, backward-compat)
- **AND** it SHALL reference the per-dimension standards skills
