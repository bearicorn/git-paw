## ADDED Requirements

### Requirement: Test-strategy skill provided in the agentskills.io format

The repository SHALL provide a `test-strategy` agent skill at
`.agents/skills/test-strategy/SKILL.md` following the agentskills.io standard: a skill
directory containing a `SKILL.md` whose YAML frontmatter has a `name` matching the parent
directory and a non-empty `description`. The skill body SHALL encode the layer-decision
procedure (unit / integration / e2e / asset-parity), behavioral-only assertion rules, an
anti-pattern catalog, and the consolidation routing (delete / table-ify / collapse-to-integration
/ replace-with-e2e / keep / protect-sole-guard).

#### Scenario: Skill is present and standard-conformant

- **GIVEN** the repository checkout
- **WHEN** `.agents/skills/test-strategy/SKILL.md` is loaded
- **THEN** it SHALL parse as a valid agentskills.io skill with `name = "test-strategy"` (matching the folder) and a non-empty `description`

### Requirement: CI validates all repository skills

CI SHALL validate every skill directory under `.agents/skills/` against the agentskills.io
format (name↔folder match, required frontmatter present and within length limits) and SHALL
fail the build when a skill is non-conformant. The check MAY reuse git-paw's own skill-loading
and validation path (dogfooding `skill-validation`) rather than an external tool.

#### Scenario: A malformed skill fails the build

- **GIVEN** a skill under `.agents/skills/` whose `SKILL.md` is missing a required frontmatter field or whose `name` does not match its folder
- **WHEN** the skill-validation CI check runs
- **THEN** it SHALL report the offending skill and exit non-zero

#### Scenario: Conformant skills pass

- **GIVEN** all skills under `.agents/skills/` conform to the agentskills.io format
- **WHEN** the skill-validation CI check runs
- **THEN** it SHALL exit zero

### Requirement: Contributor guidance references the strategy

`AGENTS.md` (or the CONTRIBUTING guide it points to) SHALL direct contributors and agents to
the `test-strategy` skill as the canonical procedure for authoring and consolidating tests.

#### Scenario: Guidance points to the skill

- **WHEN** `AGENTS.md`'s testing guidance is read
- **THEN** it SHALL reference the `test-strategy` skill as the procedure for deciding a test's type and writing it behaviorally
