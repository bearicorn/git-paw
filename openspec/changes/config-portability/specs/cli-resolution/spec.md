## MODIFIED Requirements

### Requirement: Resolve and merge custom CLIs from configuration

The system SHALL resolve custom CLI definitions and merge them with auto-detected CLIs. A `command` that is an absolute path SHALL be used as given; a `command` that contains a path separator but is not absolute SHALL be resolved against the repository's main working tree, regardless of the current directory or worktree; a bare name SHALL be looked up on PATH. A definition that does not resolve SHALL be excluded and returned to the caller as unresolved; a launch or the picker SHALL print one warning for it, and `git paw doctor` SHALL report it as a check instead of printing.

#### Scenario: Custom CLIs merged with detected CLIs
- **GIVEN** auto-detected CLIs exist and custom CLI definitions are provided
- **WHEN** `detect_clis()` is called
- **THEN** the result SHALL contain both detected and custom CLIs

Test: `detect::tests::custom_clis_merged_with_detected`

#### Scenario: Custom CLI binary not found
- **GIVEN** a custom CLI definition references a non-existent binary
- **WHEN** `detect_clis()` is called
- **THEN** the missing CLI SHALL be excluded from the result and returned as unresolved

Test: `detect::tests::custom_cli_excluded_when_binary_missing`

#### Scenario: Custom CLI resolved by absolute path
- **GIVEN** a custom CLI definition uses an absolute path to an existing binary
- **WHEN** `resolve_custom_clis()` is called
- **THEN** the resolved path SHALL match the absolute path provided

Test: `detect::tests::custom_cli_resolved_by_absolute_path`

#### Scenario: Relative command resolves against the main working tree
- **GIVEN** `[clis.wrapped] command = "scripts/agent-wrap"` and an executable `scripts/agent-wrap` in the main working tree
- **WHEN** the CLI is resolved from a subdirectory or from an agent worktree
- **THEN** it SHALL resolve to `<main working tree>/scripts/agent-wrap`
- **AND** the pane SHALL be launched with that absolute path

## ADDED Requirements

### Requirement: A configured CLI absent on this machine

When a CLI named by `default_cli` or `default_profile` does not resolve, the picker SHALL open without that pre-selection and print one notice naming the CLI and the key. When a CLI that would launch without a prompt (`default_spec_cli`, a preset's or a worker profile's CLI) does not resolve, the launch SHALL fail before creating any worktree, naming the CLI, the key, and the remedies `--cli`, installing it, or `[clis.<name>] command`. `--cli` SHALL keep its pass-through.

#### Scenario: Absent default_cli does not pre-select

- **GIVEN** `default_cli = "claude-oss"` that does not resolve, and `claude` on PATH
- **WHEN** `git paw start` shows the picker
- **THEN** no entry SHALL be pre-selected and one notice SHALL name `claude-oss` and `default_cli`

#### Scenario: Absent default_spec_cli refuses before any worktree exists

- **GIVEN** `default_spec_cli = "claude-oss"` that does not resolve and pending specs without `paw_cli`
- **WHEN** `git paw start --specs` runs
- **THEN** the launch SHALL fail naming `claude-oss`, `default_spec_cli` and the remedies
- **AND** no worktree or session SHALL have been created
