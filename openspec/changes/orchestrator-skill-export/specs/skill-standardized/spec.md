## ADDED Requirements

### Requirement: Bundled orchestrator and cli-setup skills are exported in the standardized format

The exported `<repo>/.git-paw/skills/orchestrator/SKILL.md` and `<repo>/.git-paw/skills/cli-setup/SKILL.md` SHALL each conform to the agentskills.io standardized format: a directory whose `SKILL.md` opens with a frontmatter block carrying the required `name` and `description` fields, `name` equal to the directory's name, followed by the skill body. Because the skill loader is deliberately lenient (it accepts a `SKILL.md` without a frontmatter block and loads it with no metadata), the system SHALL provide a public validator for a skill directory — usable by the export guard and by tests — that, unlike the loader, SHALL reject a `SKILL.md` with no frontmatter block, a missing or empty `name` or `description`, or a `name` that differs from the directory's name, each with an error naming the offending field. Both exported directories SHALL pass that validator, and loading them through the loader SHALL also succeed.

#### Scenario: Exported skill passes standardized validation

- **GIVEN** a repository where `git paw init` has run
- **WHEN** `.git-paw/skills/orchestrator/` is checked with the skill-directory validator
- **THEN** validation SHALL succeed
- **AND** the parsed `name` SHALL be `orchestrator`

#### Scenario: Exported cli-setup skill passes standardized validation

- **GIVEN** a repository where `git paw init` has run
- **WHEN** `.git-paw/skills/cli-setup/` is checked with the skill-directory validator
- **THEN** validation SHALL succeed
- **AND** the parsed `name` SHALL be `cli-setup`

#### Scenario: Missing frontmatter is rejected by the validator

- **GIVEN** a copy of the exported orchestrator `SKILL.md` with its frontmatter block removed, in a directory named `orchestrator`
- **WHEN** that directory is checked with the skill-directory validator
- **THEN** validation SHALL fail with an error naming the missing frontmatter `name` field

#### Scenario: A name that differs from the directory is rejected

- **GIVEN** a copy of the exported orchestrator skill directory renamed to `supervisor`, its frontmatter still carrying `name: orchestrator`
- **WHEN** that directory is checked with the skill-directory validator
- **THEN** validation SHALL fail with an error naming the `name` field and the directory name
