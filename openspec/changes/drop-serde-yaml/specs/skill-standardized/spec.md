## ADDED Requirements

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
