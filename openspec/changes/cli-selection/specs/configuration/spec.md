## ADDED Requirements

### Requirement: default_spec_cli config field

The system SHALL support a `default_spec_cli` field in `PawConfig` that specifies the CLI to use for `--from-specs` branches that don't have a `paw_cli` override, bypassing the interactive picker.

#### Scenario: default_spec_cli set
- **WHEN** a config has `default_spec_cli = "claude"`
- **THEN** `PawConfig.default_spec_cli` SHALL be `Some("claude")`

#### Scenario: default_spec_cli absent
- **WHEN** a config has no `default_spec_cli` field
- **THEN** `PawConfig.default_spec_cli` SHALL be `None`

#### Scenario: Merge preserves repo override
- **WHEN** global config has `default_spec_cli = "claude"` and repo config has `default_spec_cli = "gemini"`
- **THEN** the merged config SHALL have `default_spec_cli = Some("gemini")`
