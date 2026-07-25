## ADDED Requirements

### Requirement: Supervisor review gate consults the project's standards skills

The exported supervisor skill SHALL instruct the supervisor, at its review/verification gate, to
consult the project's standards skills under `.agents/skills/` (e.g. `test-strategy`,
`code-standards`) when present and verify the change conforms. The instruction SHALL be
project-agnostic: it SHALL reference "the project's standards skills" generically and SHALL NOT
embed git-paw-specific (Rust / cargo / `PawError`) standards into the exported asset.

#### Scenario: Gate step present in the exported supervisor skill

- **WHEN** the exported supervisor skill is inspected
- **THEN** it SHALL contain a review-gate step directing the supervisor to consult the project's `.agents/skills/` standards skills and verify conformance

#### Scenario: No baked-in stack standard

- **WHEN** the exported supervisor skill's consult wording is inspected
- **THEN** it SHALL NOT contain a hard-coded language/stack/toolchain standard (no `cargo`/`PawError`/Rust-specific rule); the standard content comes from the consumer's own skills

### Requirement: Worker implementation guidance consults the project's standards skills

The exported worker-facing skill surface (the coordination skill / boot block) SHALL instruct the
implementing agent to consult the project's `.agents/skills/` standards skills during
implementation. The instruction SHALL be project-agnostic in the same way.

#### Scenario: Consult step present in the exported worker surface

- **WHEN** the exported coordination skill / boot block is inspected
- **THEN** it SHALL contain a step directing the implementing agent to consult the project's `.agents/skills/` standards skills before/while implementing

#### Scenario: Absent standards leave behavior unchanged

- **GIVEN** a project with no `.agents/skills/` standards skills
- **WHEN** a session runs
- **THEN** behavior SHALL be identical to before this change (the consult step is a no-op)
