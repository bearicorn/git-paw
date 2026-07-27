## MODIFIED Requirements

### Requirement: Auto-detect known AI CLIs on PATH

The system SHALL scan PATH for the known CLI binaries: `claude`, `codex`, `agy`,
`aider`, `vibe`, `qwen`, `amp`, `opencode`, `cline`, `droid`, `pi`, `junie`,
`cursor`, `copilot`, `cn`, `kilo`, and `kimi`.

`agy` is the Antigravity CLI (display name "Antigravity"), which replaces the
retired Gemini CLI (`gemini`) — Gemini CLI stops serving free / AI Pro / AI Ultra
users on 2026-06-18. Removing `gemini` affects auto-detection only; an explicit
`default_cli = "gemini"`, a `[clis.gemini]` custom entry, or a saved-session `cli`
string still resolves as a pass-through.

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
