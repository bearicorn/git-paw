## Purpose

Detect available AI coding CLI binaries by scanning PATH for known names and merging with user-defined custom CLIs from configuration. Provides a unified, deduplicated, sorted list for interactive selection or direct use.

## Requirements

### Requirement: Auto-detect known AI CLIs on PATH

The system SHALL scan PATH for the known CLI binaries: `claude`, `codex`, `gemini`, `aider`, `mistral`, `qwen`, `amp`, and `copilot`.

#### Scenario: All known CLIs are present on PATH
- **GIVEN** all 8 known CLI binaries exist on PATH
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
