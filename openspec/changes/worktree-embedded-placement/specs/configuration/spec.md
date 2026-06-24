## ADDED Requirements

### Requirement: Worktree placement configuration field

The system SHALL support an optional `worktree_placement` field on
`PawConfig` accepting the string values `"child"` or `"sibling"`. When the
field is absent, the effective placement SHALL be `sibling`, preserving
v0.7.0 behaviour. The field SHALL participate in the repo-overrides-global
merge as a scalar (repo value wins). A default-valued field SHALL be
skipped on serialization so that a config without an explicit
`worktree_placement` round-trips without the field appearing.

#### Scenario: worktree_placement set to child

- **GIVEN** a TOML file with `worktree_placement = "child"`
- **WHEN** the config is loaded
- **THEN** the effective placement SHALL be `child`

#### Scenario: worktree_placement set to sibling

- **GIVEN** a TOML file with `worktree_placement = "sibling"`
- **WHEN** the config is loaded
- **THEN** the effective placement SHALL be `sibling`

#### Scenario: worktree_placement absent defaults to sibling

- **GIVEN** a TOML file with no `worktree_placement` field
- **WHEN** the config is loaded
- **THEN** the effective placement SHALL be `sibling`

#### Scenario: Repo placement overrides global

- **GIVEN** global config has `worktree_placement = "sibling"` and repo config has `worktree_placement = "child"`
- **WHEN** configs are merged
- **THEN** the merged placement SHALL be `child`

#### Scenario: Placement config survives round-trip

- **GIVEN** a `PawConfig` with `worktree_placement = "child"`
- **WHEN** it is serialized to TOML and re-parsed
- **THEN** the re-parsed placement SHALL be `child`

#### Scenario: Pre-existing config without the field loads unchanged

- **GIVEN** a `.git-paw/config.toml` written before this field exists
- **WHEN** the config is loaded
- **THEN** loading SHALL NOT error
- **AND** the effective placement SHALL be `sibling`
