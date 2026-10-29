## MODIFIED Requirements

### Requirement: Auto-detect known AI CLIs on PATH

The system SHALL scan PATH for exactly the binaries named by the rows of the compiled support matrix
(the `cli-support-matrix` capability), in the matrix's row order, and for no other name. The matrix
is the single roster: adding or removing a known CLI is a matrix change, and the Supported AI CLIs
docs page publishes the current roster.

`agy` is the Antigravity CLI (display name "Antigravity"), which replaces the retired Gemini CLI (`gemini`) — Gemini CLI stops serving free / AI Pro / AI Ultra users on 2026-06-18. Removing `gemini` affects auto-detection only; an explicit `default_cli = "gemini"`, a `[clis.gemini]` custom entry, or a saved-session `cli` string still resolves as a pass-through. The same pass-through SHALL hold for any binary later removed from the matrix.

`cursor-agent` is Cursor's agent CLI (display name "Cursor Agent"). The bare `cursor` binary is the Cursor editor launcher on most installations and SHALL NOT be auto-detected as an AI CLI. Removing `cursor` affects auto-detection only: an explicit `git paw start --cli cursor`, a `[clis.cursor]` custom entry, or a saved-session `cli = "cursor"` still resolves as a pass-through. The paths that validate a CLI against the detected roster — spec-driven selection, `git paw add`, and the interactive picker (which `default_cli` only pre-selects within) — require a `[clis.cursor]` entry.

#### Scenario: All known CLIs are present on PATH
- **GIVEN** every support-matrix binary exists on PATH
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

#### Scenario: Cursor's agent CLI is detected under `cursor-agent`, not the editor launcher
- **GIVEN** both a `cursor` and a `cursor-agent` binary exist on PATH
- **WHEN** `detect_known_clis()` is called
- **THEN** the result SHALL include a `CliInfo` with `binary_name = "cursor-agent"`, `display_name = "Cursor Agent"`, and `source = Detected`
- **AND** the result SHALL NOT include a CLI with `binary_name = "cursor"`

Test: `detect::tests::cursor_agent_detected_and_editor_launcher_not_known`

#### Scenario: An explicit `cursor` CLI still passes through
- **GIVEN** no `cursor` entry is auto-detected and no `[clis.cursor]` entry is configured
- **WHEN** a bare session is started with `--cli cursor`
- **THEN** the branch-to-CLI mapping SHALL carry `cursor` unchanged
- **AND** the pane SHALL be launched with `cursor`

Test: `interactive::resolver::tests::explicit_cursor_cli_passes_through_without_detection`, `config::tests::launch_command_falls_back_to_name_without_entry`

#### Scenario: The detection roster is the support matrix
- **WHEN** the binaries auto-detection scans for are compared with the support matrix's rows
- **THEN** they SHALL name the same binaries in the same order

### Requirement: README Supported AI CLIs table matches `src/detect.rs`

The README's Supported AI CLIs section SHALL agree with the compiled support matrix that the
detection roster in `src/detect.rs` derives from. It SHALL do one of two things:

- list exactly the matrix's binaries, each with its derived tier
- carry no per-CLI table, and link to the Supported AI CLIs docs page instead

It SHALL NOT name a binary as auto-detected when the matrix lacks that binary. The mdBook
`docs/src/supported-clis.md` table SHALL carry the same binary set (its full parity with the matrix
is the `cli-support-matrix` docs requirement). A parity test SHALL enforce this.

#### Scenario: README and mdBook CLI tables list every matrix binary and only those

- **WHEN** the README's Supported AI CLIs table (when the README carries one) and `docs/src/supported-clis.md`'s table are inspected
- **THEN** each SHALL contain a `` `<binary>` `` cell for every binary in the support matrix, including `` `cursor-agent` ``
- **AND** neither SHALL contain a `` `cursor` `` binary cell

Test: `tests/supported_clis_docs_sync.rs::readme_and_book_tables_match_known_cli_roster`

#### Scenario: A README table matching the matrix passes

- **GIVEN** a README table that lists every matrix binary with its derived tier and no other binary
- **WHEN** the README parity test runs
- **THEN** it SHALL pass

#### Scenario: A README naming a binary outside the matrix fails

- **GIVEN** a README table that lists a binary the support matrix lacks
- **WHEN** the README parity test runs
- **THEN** it SHALL fail, naming that binary

#### Scenario: A README that links instead of listing passes

- **GIVEN** a README Supported AI CLIs section with no per-CLI table and a link to the Supported AI CLIs docs page
- **WHEN** the README parity test runs
- **THEN** it SHALL pass
