## Purpose

Interactive selection prompts for choosing branches and AI CLIs. Supports uniform (same CLI for all branches) and per-branch assignment modes, with CLI flags that skip prompts. Logic is separated from UI via the `Prompter` trait for testability.

## Requirements

### Requirement: CLI flags skip all prompts when both provided

When both `--cli` and `--branches` flags are provided, the system SHALL skip all interactive prompts and map the CLI to all specified branches.

#### Scenario: Both flags skip all prompts
- **GIVEN** `--cli alpha` and `--branches feature/auth,fix/api` flags
- **WHEN** `run_selection()` is called
- **THEN** it SHALL return mappings without invoking any prompts

Test: `interactive::tests::both_flags_skips_all_prompts_and_maps_cli_to_all_branches`

### Requirement: CLI flag skips CLI prompt but prompts for branches

When only `--cli` is provided, the system SHALL prompt for branch selection but skip CLI selection.

#### Scenario: CLI flag provided, branches prompted
- **GIVEN** `--cli alpha` flag and no branches flag
- **WHEN** `run_selection()` is called
- **THEN** branch selection SHALL be prompted and the flag CLI SHALL be used

Test: `interactive::tests::cli_flag_skips_cli_prompt_but_prompts_for_branches`

### Requirement: Branches flag skips branch prompt but prompts for CLI

When only `--branches` is provided, the system SHALL skip branch selection but prompt for CLI assignment.

#### Scenario: Branches flag provided, CLI prompted in uniform mode
- **GIVEN** `--branches` flag and no CLI flag
- **WHEN** user selects uniform mode
- **THEN** the selected CLI SHALL be mapped to all flagged branches

Test: `interactive::tests::branches_flag_skips_branch_prompt_but_prompts_for_cli_uniform`

### Requirement: Uniform mode maps same CLI to all branches

In uniform mode, the system SHALL assign the selected CLI to every selected branch.

#### Scenario: Uniform mode selection
- **GIVEN** user selects uniform mode, picks 2 branches and 1 CLI
- **WHEN** `run_selection()` completes
- **THEN** both branches SHALL be mapped to the same CLI

Test: `interactive::tests::uniform_mode_maps_same_cli_to_all_selected_branches`

### Requirement: Per-branch mode maps different CLIs to each branch

In per-branch mode, the system SHALL prompt for a CLI for each selected branch individually.

#### Scenario: Per-branch mode selection
- **GIVEN** user selects per-branch mode with 2 branches
- **WHEN** different CLIs are chosen for each branch
- **THEN** each branch SHALL be mapped to its respective CLI

Test: `interactive::tests::per_branch_mode_maps_different_cli_to_each_branch`

#### Scenario: Per-branch mode with branches flag
- **GIVEN** branches provided via flag and per-branch mode selected
- **WHEN** different CLIs are chosen
- **THEN** each flagged branch SHALL be mapped to its selected CLI

Test: `interactive::tests::per_branch_mode_with_branches_flag`

### Requirement: Error when no CLIs available

The system SHALL return `PawError::NoCLIsFound` when the CLI list is empty.

#### Scenario: Empty CLI list
- **GIVEN** no CLIs available
- **WHEN** `run_selection()` is called
- **THEN** it SHALL return `Err(PawError::NoCLIsFound)`

Test: `interactive::tests::no_clis_available_returns_error`

### Requirement: Error when no branches available

The system SHALL return `PawError::BranchError` when the branch list is empty.

#### Scenario: Empty branch list
- **GIVEN** no branches available
- **WHEN** `run_selection()` is called
- **THEN** it SHALL return `Err(PawError::BranchError)`

Test: `interactive::tests::no_branches_available_returns_error`

### Requirement: User cancellation propagates as PawError::UserCancelled

The system SHALL propagate cancellation (Ctrl+C or empty selection) as `PawError::UserCancelled`.

#### Scenario: User cancels branch selection
- **GIVEN** user presses Ctrl+C during branch selection
- **WHEN** `run_selection()` is called
- **THEN** it SHALL return `Err(PawError::UserCancelled)`

Test: `interactive::tests::user_cancels_branch_selection_returns_cancelled`

#### Scenario: User selects no branches
- **GIVEN** user confirms with zero branches selected
- **WHEN** `run_selection()` is called
- **THEN** it SHALL return `Err(PawError::UserCancelled)`

Test: `interactive::tests::user_selects_no_branches_returns_cancelled`

#### Scenario: User cancels CLI selection
- **GIVEN** user presses Ctrl+C during CLI selection
- **WHEN** `run_selection()` is called
- **THEN** it SHALL return `Err(PawError::UserCancelled)`

Test: `interactive::tests::user_cancels_cli_selection_returns_cancelled`

### Requirement: Subset branch selection

The system SHALL support selecting a subset of available branches.

#### Scenario: Selecting one of two branches
- **GIVEN** 2 available branches
- **WHEN** user selects only the second
- **THEN** only that branch SHALL appear in the result

Test: `interactive::tests::selecting_subset_of_branches_works`

### Requirement: CliMode display format

The `CliMode` enum SHALL display as human-readable descriptions.

#### Scenario: CliMode display strings
- **GIVEN** `CliMode::Uniform` and `CliMode::PerBranch`
- **WHEN** formatted with `Display`
- **THEN** they SHALL render as `"Same CLI for all branches"` and `"Different CLI per branch"`

Test: `interactive::tests::cli_mode_display`

### Requirement: CliInfo display format

`CliInfo` SHALL display as the binary name when it matches the display name, or as `"DisplayName (binary)"` when they differ.

#### Scenario: Same display and binary name
- **GIVEN** a `CliInfo` where `display_name` equals `binary_name`
- **WHEN** formatted with `Display`
- **THEN** it SHALL render as just the binary name

Test: `interactive::tests::cli_info_display_same_names`

#### Scenario: Different display and binary name
- **GIVEN** a `CliInfo` where `display_name` differs from `binary_name`
- **WHEN** formatted with `Display`
- **THEN** it SHALL render as `"DisplayName (binary_name)"`

Test: `interactive::tests::cli_info_display_different_names`

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
