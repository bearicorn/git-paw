# cli-resolution Specification

## Purpose
Detect available AI coding CLI binaries by scanning PATH for known names and merging with user-defined custom CLIs from configuration, providing a unified, deduplicated, sorted list, and resolve which CLI to use for each spec-driven branch using a multi-level priority chain that considers command-line flags, per-spec overrides, config defaults, and interactive selection, prompting the user at most once.

## Requirements

### Requirement: Auto-detect known AI CLIs on PATH

The system SHALL scan PATH for the known CLI binaries: `claude`, `codex`, `agy`, `aider`, `vibe`, `qwen`, `amp`, `opencode`, `cline`, `droid`, `pi`, `junie`, `cursor`, `copilot`, `cn`, `kilo`, and `kimi`.

`agy` is the Antigravity CLI (display name "Antigravity"), which replaces the retired Gemini CLI (`gemini`) — Gemini CLI stops serving free / AI Pro / AI Ultra users on 2026-06-18. Removing `gemini` affects auto-detection only; an explicit `default_cli = "gemini"`, a `[clis.gemini]` custom entry, or a saved-session `cli` string still resolves as a pass-through.

#### Scenario: All known CLIs are present on PATH
- **GIVEN** all 17 known CLI binaries exist on PATH
- **WHEN** `detect_known_clis()` is called
- **THEN** it SHALL return a `CliInfo` for each binary with `source = Detected`, a non-empty `display_name`, and a valid `path`

Test: `detect::tests::all_known_clis_detected_when_present`

#### Scenario: No known CLIs are present on PATH
- **GIVEN** PATH contains no known CLI binaries
- **WHEN** `detect_known_clis()` is called
- **THEN** it SHALL return an empty list

Test: `detect::tests::returns_empty_when_no_known_clis_on_path`

#### Scenario: Partial set of CLIs on PATH
- **GIVEN** only a subset of known CLIs exist on PATH
- **WHEN** `detect_known_clis()` is called
- **THEN** it SHALL return only the CLIs that are found

Test: `detect::tests::detects_subset_of_known_clis`

#### Scenario: Antigravity CLI is detected under its `agy` binary name
- **GIVEN** the `agy` binary exists on PATH
- **WHEN** `detect_known_clis()` is called
- **THEN** the result SHALL include a `CliInfo` with `binary_name = "agy"` and `source = Detected`
- **AND** the result SHALL NOT include a CLI with `binary_name = "gemini"` unless `gemini` is supplied as a custom `[clis.*]` entry

Test: `detect::tests::agy_detected_and_gemini_not_known`

### Requirement: Resolve and merge custom CLIs from configuration

The system SHALL resolve custom CLI definitions by looking up commands as absolute paths or via PATH, and merge them with auto-detected CLIs.

#### Scenario: Custom CLIs merged with detected CLIs
- **GIVEN** auto-detected CLIs exist and custom CLI definitions are provided
- **WHEN** `detect_clis()` is called
- **THEN** the result SHALL contain both detected and custom CLIs

Test: `detect::tests::custom_clis_merged_with_detected`

#### Scenario: Custom CLI binary not found
- **GIVEN** a custom CLI definition references a non-existent binary
- **WHEN** `detect_clis()` is called
- **THEN** the missing CLI SHALL be excluded and a warning printed to stderr

Test: `detect::tests::custom_cli_excluded_when_binary_missing`

#### Scenario: Custom CLI resolved by absolute path
- **GIVEN** a custom CLI definition uses an absolute path to an existing binary
- **WHEN** `resolve_custom_clis()` is called
- **THEN** the resolved path SHALL match the absolute path provided

Test: `detect::tests::custom_cli_resolved_by_absolute_path`

### Requirement: Custom CLIs override detected CLIs with the same name

When a custom CLI has the same `binary_name` as a detected CLI, the custom definition SHALL take precedence.

#### Scenario: Custom CLI overrides auto-detected CLI
- **GIVEN** a custom CLI shares a `binary_name` with an auto-detected CLI
- **WHEN** `detect_clis()` is called
- **THEN** the result SHALL contain only the custom version with `source = Custom`

Test: `detect::tests::custom_cli_overrides_detected_with_same_binary_name`

### Requirement: Each CLI result includes all required fields

Every `CliInfo` SHALL have a non-empty `display_name`, `binary_name`, a valid `path`, and a `source` indicator.

#### Scenario: Detected CLI has all fields populated
- **GIVEN** a known CLI binary exists on PATH
- **WHEN** it is detected
- **THEN** all fields (`display_name`, `binary_name`, `path`, `source`) SHALL be populated

Test: `detect::tests::detected_cli_has_all_fields`

#### Scenario: Custom CLI has all fields populated
- **GIVEN** a custom CLI definition is resolved
- **WHEN** it is included in results
- **THEN** all fields SHALL be populated

Test: `detect::tests::custom_cli_has_all_fields`

### Requirement: Display name derivation

When no explicit display name is provided, the system SHALL derive one by capitalizing the first letter of the binary name.

#### Scenario: Custom CLI defaults to capitalized name
- **GIVEN** a custom CLI definition has no `display_name`
- **WHEN** it is resolved
- **THEN** the `display_name` SHALL be the binary name with the first letter capitalized

Test: `detect::tests::custom_cli_display_name_defaults_to_capitalised_name`

### Requirement: Results sorted by display name

The combined CLI list SHALL be sorted alphabetically by `display_name` (case-insensitive).

#### Scenario: Results are sorted
- **GIVEN** multiple CLIs are detected and/or custom
- **WHEN** `detect_clis()` is called
- **THEN** the results SHALL be sorted by display name

Test: `detect::tests::results_sorted_by_display_name`

### Requirement: CliSource display format

The `CliSource` enum SHALL display as `"detected"` or `"custom"`.

#### Scenario: CliSource display strings
- **GIVEN** `CliSource::Detected` and `CliSource::Custom`
- **WHEN** formatted with `Display`
- **THEN** they SHALL render as `"detected"` and `"custom"` respectively

Test: `detect::tests::cli_source_display_format`


### Requirement: CLI resolution chain for spec-driven launches

The system SHALL resolve which CLI to use for each spec-driven branch using a 5-level priority chain, from highest to lowest priority.

#### Scenario: --cli flag overrides everything
- **WHEN** `--cli claude` is passed and specs have various `paw_cli` values
- **THEN** all branches SHALL use `"claude"` regardless of spec or config values

#### Scenario: paw_cli in spec overrides config
- **WHEN** no `--cli` flag is passed and a spec has `paw_cli: gemini`
- **THEN** that branch SHALL use `"gemini"` regardless of `default_spec_cli` or `default_cli`

#### Scenario: default_spec_cli fills remaining without prompt
- **WHEN** no `--cli` flag, some specs have no `paw_cli`, and `default_spec_cli = "claude"` in config
- **THEN** specs without `paw_cli` SHALL use `"claude"` with no interactive prompt

#### Scenario: default_cli pre-selects in picker
- **WHEN** no `--cli` flag, no `paw_cli`, no `default_spec_cli`, and `default_cli = "claude"` in config
- **THEN** the CLI picker SHALL be shown with `"claude"` pre-selected

#### Scenario: No defaults — full picker
- **WHEN** no `--cli` flag, no `paw_cli`, no `default_spec_cli`, and no `default_cli`
- **THEN** the CLI picker SHALL be shown with no pre-selection

### Requirement: Mixed resolution across specs

The system SHALL handle specs where some have `paw_cli` and others don't in the same launch.

#### Scenario: Mix of paw_cli and default_spec_cli
- **WHEN** 3 specs are launched, 1 has `paw_cli: gemini`, and `default_spec_cli = "claude"`
- **THEN** the gemini spec SHALL use `"gemini"` and the other 2 SHALL use `"claude"`

#### Scenario: Mix of paw_cli and interactive
- **WHEN** 3 specs are launched, 1 has `paw_cli: gemini`, no `default_spec_cli`, and user picks `"claude"` in the prompt
- **THEN** the gemini spec SHALL use `"gemini"` and the other 2 SHALL use `"claude"`

### Requirement: Prompt at most once

The system SHALL prompt the user for CLI selection at most once during a `--from-specs` launch, applying the choice to all branches without a `paw_cli` or `default_spec_cli`.

#### Scenario: Single prompt for remaining branches
- **WHEN** 5 specs are launched, 2 have `paw_cli`, and the picker fires for the remaining 3
- **THEN** the picker SHALL fire once and the chosen CLI SHALL be applied to all 3

### Requirement: Validate resolved CLI names

The system SHALL validate that each resolved CLI name matches an available CLI.

#### Scenario: paw_cli references unknown CLI
- **WHEN** a spec has `paw_cli: nonexistent` and no CLI named `"nonexistent"` is available
- **THEN** the system SHALL return `PawError::CliNotFound("nonexistent")`

#### Scenario: default_spec_cli references unknown CLI
- **WHEN** `default_spec_cli = "nonexistent"` and no such CLI is available
- **THEN** the system SHALL return `PawError::CliNotFound("nonexistent")`

#### Scenario: --cli flag references unknown CLI
- **GIVEN** specs exist
- **WHEN** `--cli nonexistent` is passed and "nonexistent" is not in available CLIs
- **THEN** the system SHALL return `PawError::CliNotFound("nonexistent")`

### Requirement: No prompt when fully resolved

The system SHALL not show any interactive prompt when all branches are resolved via `--cli`, `paw_cli`, or `default_spec_cli`.

#### Scenario: All resolved without prompt
- **WHEN** `--cli claude` is passed
- **THEN** no interactive prompt SHALL be shown

#### Scenario: All resolved via paw_cli and default_spec_cli
- **WHEN** every spec has `paw_cli` or `default_spec_cli` covers the rest
- **THEN** no interactive prompt SHALL be shown
