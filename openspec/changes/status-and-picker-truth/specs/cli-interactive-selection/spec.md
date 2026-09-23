## ADDED Requirements

### Requirement: Configured default CLI is pre-selected in the interactive CLI picker

`run_selection()` SHALL accept an optional default CLI name and SHALL pass it to the CLI picker in both assignment modes: to `select_cli()` when the user chooses uniform mode, and to `select_cli_for_branch()` for every branch when the user chooses per-branch mode. The `Prompter` trait's `select_cli_for_branch()` SHALL therefore accept an optional default with the same semantics as `select_cli()`'s. Every caller of the interactive selection flow (`git paw start`, and the supervisor launch's branch selection) SHALL supply the loaded configuration's `default_cli`.

`default_cli` SHALL only choose the picker's starting entry. It SHALL NOT skip the picker: when `--cli` is not given, the mode prompt and the CLI picker (or, in per-branch mode, each per-branch picker) SHALL still appear, and the user can accept the pre-selected CLI or pick any other listed CLI. If `default_cli` is unset, or names a CLI that is not available, the picker SHALL start on the first entry (the existing `select_cli()` behaviour). `--cli` SHALL still skip the CLI prompt entirely.

#### Scenario: default_cli pre-selects in the uniform-mode picker

- **GIVEN** `default_cli = "beta"`, no `--cli` flag, and the user chooses uniform mode
- **WHEN** `run_selection()` is called with the default CLI `"beta"`
- **THEN** `select_cli()` SHALL receive `default = Some("beta")`
- **AND** every selected branch SHALL map to the CLI the picker returns

#### Scenario: default_cli pre-selects in every per-branch picker

- **GIVEN** `default_cli = "beta"`, no `--cli` flag, two selected branches, and the user chooses per-branch mode
- **WHEN** `run_selection()` is called with the default CLI `"beta"`
- **THEN** `select_cli_for_branch()` SHALL receive `default = Some("beta")` for each branch
- **AND** each branch SHALL map to the CLI its own picker returns

#### Scenario: The picker is still shown when default_cli is set

- **GIVEN** `default_cli = "beta"` and no `--cli` flag
- **WHEN** `run_selection()` is called
- **THEN** the mode prompt and the CLI picker SHALL both be invoked
- **AND** a different CLI the user picks there SHALL be the one mapped to the branches

#### Scenario: No default_cli starts the picker on the first entry

- **GIVEN** no `default_cli` and no `--cli` flag, with the user choosing uniform mode
- **WHEN** `run_selection()` is called with no default CLI
- **THEN** `select_cli()` SHALL receive `default = None`

#### Scenario: Per-branch picker starts on a configured default

- **GIVEN** a terminal prompter and a CLI list in which `"beta"` is the second entry
- **WHEN** `select_cli_for_branch()` is called with `default = Some("beta")`
- **THEN** the picker SHALL display with `"beta"` highlighted as the default selection
- **AND** with `default = None` or a CLI that is not listed, it SHALL start on the first entry

#### Scenario: --cli flag wins over default_cli

- **GIVEN** `--cli alpha` and `default_cli = "beta"`
- **WHEN** `run_selection()` is called
- **THEN** no mode prompt or CLI picker SHALL be invoked
- **AND** every selected branch SHALL map to `"alpha"`

#### Scenario: Bare git paw start pre-selects the configured default CLI

- **GIVEN** a repository configuring two available CLIs and `default_cli` naming the one that sorts second in the picker
- **WHEN** the user runs `git paw start --branches <branch> --dry-run` in a terminal, chooses uniform mode, and confirms the CLI picker without moving the cursor
- **THEN** the dry-run plan SHALL map the branch to the configured `default_cli`
