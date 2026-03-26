## MODIFIED Requirements

### Requirement: CLI picker with optional pre-selection

The `select_cli` method on the `Prompter` trait SHALL accept an optional default CLI name for pre-selection in the interactive picker.

#### Scenario: Picker with default pre-selected
- **WHEN** `select_cli()` is called with `default = Some("claude")` and `"claude"` is in the CLI list
- **THEN** the picker SHALL display with `"claude"` highlighted as the default selection

#### Scenario: Picker without default
- **WHEN** `select_cli()` is called with `default = None`
- **THEN** the picker SHALL display with the first item selected (no pre-selection)

#### Scenario: Default CLI not in available list
- **WHEN** `select_cli()` is called with `default = Some("nonexistent")` and that CLI is not available
- **THEN** the picker SHALL display with no pre-selection (graceful fallback)
