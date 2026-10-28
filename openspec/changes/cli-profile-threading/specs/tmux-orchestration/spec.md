## ADDED Requirements

### Requirement: First-run dialog recognition comes from the pane CLI's provider

The launch gate SHALL recognise first-run acceptance and folder-trust dialogs, and the affirmative
option to select in each, from the provider record of the CLI being launched. A provider that
declares no dialogs of its own SHALL inherit the embedded default set — the dialogs every pane was
checked for before this change — so launch behaviour is unchanged for every existing CLI. The dialogs
and their affirmative wording SHALL be compiled and SHALL NOT be readable or overridable from a
prompt-shape profile or any configuration file, because answering such a dialog grants a permission.

#### Scenario: Claude's dialogs are recognised for a Claude-family pane

- **GIVEN** a pane launched for a `claude`-family CLI showing Claude Code's folder-trust dialog
- **WHEN** the launch gate polls the pane
- **THEN** it SHALL select the dialog's affirmative option by its text

#### Scenario: A generic CLI keeps today's dialog handling

- **GIVEN** a pane launched for a `generic`-family CLI showing the bypass-permissions acceptance
  dialog the gate recognised before this change
- **WHEN** the launch gate polls the pane
- **THEN** it SHALL answer the dialog exactly as it did before this change

#### Scenario: A profile cannot declare a dialog

- **GIVEN** a `[clis.<name>].prompt_profile` override containing text shaped like a dialog heading
- **WHEN** the launch gate polls that CLI's pane showing that text
- **THEN** no dialog SHALL be recognised from the override's content
