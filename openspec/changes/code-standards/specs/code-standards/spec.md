## ADDED Requirements

### Requirement: Code-standards skill provided in the agentskills.io format

The repository SHALL provide a `code-standards` agent skill at
`.agents/skills/code-standards/SKILL.md` following the agentskills.io standard (skill directory
containing `SKILL.md`; `name` matching the parent directory; non-empty `description`). The skill
SHALL encode git-paw's decoupling patterns (process-execution seam, domain newtypes,
hidden-subcommand IPC, command-handler modules), the module-domain layout, the non-negotiable
idiom rules (no `unwrap`/`expect` in non-test code, `PawError`, doc-comments), the frozen
do-not-touch zones, the refactor rules, and a supervisor review-gate checklist.

#### Scenario: Skill is present and standard-conformant

- **GIVEN** the repository checkout
- **WHEN** `.agents/skills/code-standards/SKILL.md` is loaded
- **THEN** it SHALL parse as a valid agentskills.io skill with `name = "code-standards"` (matching the folder) and a non-empty `description`

### Requirement: Best-practice and NFR references bundled via progressive disclosure

The skill SHALL bundle, under `references/`, condensed **Rust API Guidelines**, **CLI / dev-tool**
best practices, and the **non-functional-requirements** rationale (the NFR set, the conflict
resolutions, and the precedence spine the standards serve), so `SKILL.md` stays lean and the detail
loads on demand.

#### Scenario: Reference files are present and linked

- **GIVEN** the `code-standards` skill directory
- **WHEN** its contents are inspected
- **THEN** it SHALL contain `references/rust-api-guidelines.md`, `references/cli-and-devtool-design.md`, and `references/non-functional-requirements.md`
- **AND** `SKILL.md` SHALL link to each

### Requirement: Contributor guidance references the code standards

`AGENTS.md` (or the CONTRIBUTING guide it points to) SHALL direct contributors and agents to the
`code-standards` skill as the canonical reference for structuring, decoupling, and refactoring code.

#### Scenario: Guidance points to the skill

- **WHEN** `AGENTS.md`'s code-style guidance is read
- **THEN** it SHALL reference the `code-standards` skill
