# skill-standardized Specification

## Purpose
Supports the agentskills.io standardized skill format — a directory containing a `SKILL.md` plus optional resource subdirectories — auto-detecting the directory format and loading/parsing such skills, and validates them against the agentskills.io schema, accepting conforming skills and rejecting those with missing or invalid fields via clear, actionable error messages that name the specific offending fields.
## Requirements
### Requirement: Standardized skill format
The system SHALL support the agentskills.io standardized format for agent skills, which includes a directory structure with SKILL.md as the main file plus optional subdirectories for resources.

#### Scenario: Load standardized skill
- **WHEN** a skill directory contains SKILL.md file
- **THEN** the system SHALL load and parse the skill using the standardized format

#### Scenario: Validate standardized skill structure
- **WHEN** loading a standardized skill
- **THEN** the system SHALL validate that required fields are present

### Requirement: Format detection
The system SHALL automatically detect standardized skill format.

#### Scenario: Format detection for directory-based skills
- **WHEN** a skill is in directory format with SKILL.md
- **THEN** the system SHALL identify it as standardized format

### Requirement: Schema validation
The system SHALL validate that new format skills conform to the agentskills.io schema specification.

#### Scenario: Valid skill passes validation
- **WHEN** a standardized skill conforms to the schema
- **THEN** the system SHALL accept and load the skill

#### Scenario: Invalid skill fails validation
- **WHEN** a standardized skill has missing required fields
- **THEN** the system SHALL reject the skill with clear error message

### Requirement: Validation error reporting
The system SHALL provide clear and actionable error messages when skill validation fails.

#### Scenario: Detailed validation errors
- **WHEN** skill validation fails
- **THEN** the system SHALL report specific missing or invalid fields

### Requirement: Skill frontmatter is parsed without a YAML dependency

The system SHALL parse a standardized skill's frontmatter with a parser that depends on no YAML library, and that supports a defined subset sufficient for the agentskills.io field set. The supported subset SHALL be:

- top-level `key: value` pairs, where the value is a plain scalar, a single-quoted scalar, or a double-quoted scalar, with surrounding whitespace trimmed from plain scalars;
- literal (`|`) and folded (`>`) block scalars for a top-level value, including their strip (`-`) and keep (`+`) chomping indicators;
- full-line and trailing `#` comments outside quoted scalars;
- a one-level `metadata` mapping whose entries are string keys to scalar values, every value being represented as a string.

A top-level key that the system does not consume SHALL be ignored together with any indented content beneath it — such as a block sequence or a nested mapping — so a skill carrying extra fields (for example `allowed-tools:` with a list) loads as before.

The parser SHALL NOT silently misparse. When a field the system consumes (`name`, `description`, `license`, `compatibility`, `metadata`) carries a construct outside the supported subset — for example a flow mapping, an anchor or alias, or a nested mapping inside `metadata` — the system SHALL reject the skill with a validation error that names the field. This applies equally to embedded skills and to user-authored override skills, since the latter are not under the system's control.

The existing validation behaviour is unchanged: a missing or empty `name` or `description` SHALL be rejected with an error naming the missing field, and a skill without frontmatter SHALL load with no metadata.

#### Scenario: Every skill in the repository parses identically

- **GIVEN** every embedded skill and every `SKILL.md` frontmatter block present in the repository
- **WHEN** each is parsed by the dependency-free parser
- **THEN** each SHALL produce the same `name`, `description`, `license`, `compatibility`, and `metadata` values the previous YAML-library parser produced for it

#### Scenario: Unconsumed keys with nested content are skipped

- **GIVEN** frontmatter containing `name`, `description`, and an `allowed-tools:` key followed by an indented block sequence
- **WHEN** it is parsed
- **THEN** `name` and `description` SHALL be populated
- **AND** the sequence SHALL be ignored without error

#### Scenario: The metadata map is read as strings

- **GIVEN** frontmatter whose `metadata:` block contains `author: openspec` and `version: "1.0"`
- **WHEN** it is parsed
- **THEN** `metadata` SHALL be an object mapping `author` to `"openspec"` and `version` to `"1.0"`

#### Scenario: Quoted and block scalars are supported

- **GIVEN** frontmatter whose `description` is given once as a double-quoted scalar containing `: `, and once as a folded block scalar spanning several lines
- **WHEN** each is parsed
- **THEN** the description SHALL equal the scalar's YAML value in both cases

#### Scenario: An unsupported construct in a consumed field is rejected, not guessed

- **GIVEN** frontmatter whose `metadata` value is a flow mapping such as `{ a: 1 }`
- **WHEN** it is parsed
- **THEN** the skill SHALL be rejected with a validation error naming `metadata`
- **AND** no partially-parsed value SHALL be returned

#### Scenario: Missing required fields are still rejected

- **GIVEN** frontmatter without a `description`
- **WHEN** it is parsed
- **THEN** the skill SHALL be rejected with an error naming the missing `description` field

#### Scenario: No YAML library is linked

- **WHEN** the dependency graph is inspected
- **THEN** it SHALL contain neither `serde_yaml` nor `unsafe-libyaml`

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

