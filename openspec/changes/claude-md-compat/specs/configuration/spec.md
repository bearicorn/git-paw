## ADDED Requirements

### Requirement: ClaudeMdMode config field

The system SHALL support a `claude_md` field in `PawConfig` with values `"symlink"`, `"copy"`, or `"skip"`, defaulting to `"skip"` when absent.

#### Scenario: Field absent defaults to skip
- **WHEN** a config file has no `claude_md` field
- **THEN** `PawConfig.claude_md` SHALL default to `ClaudeMdMode::Skip`

#### Scenario: Field set to symlink
- **WHEN** a config file has `claude_md = "symlink"`
- **THEN** `PawConfig.claude_md` SHALL be `ClaudeMdMode::Symlink`

#### Scenario: Field set to copy
- **WHEN** a config file has `claude_md = "copy"`
- **THEN** `PawConfig.claude_md` SHALL be `ClaudeMdMode::Copy`

#### Scenario: Merge preserves repo override
- **WHEN** global config has `claude_md = "skip"` and repo config has `claude_md = "symlink"`
- **THEN** the merged config SHALL have `ClaudeMdMode::Symlink`

#### Scenario: Round-trip serialization
- **WHEN** a config with `claude_md = "copy"` is saved and loaded
- **THEN** the value SHALL be preserved
