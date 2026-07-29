# cli-resolution Specification

## Purpose
Detect available AI coding CLI binaries by scanning PATH for known names and merging with user-defined custom CLIs from configuration, providing a unified, deduplicated, sorted list, and resolve which CLI to use for each spec-driven branch using a multi-level priority chain that considers command-line flags, per-spec overrides, config defaults, and interactive selection, prompting the user at most once. It also covers reliable boot-block submission and CLI-pane launch hardening: injecting the prompt text literally and then sending `Enter` as a separate keystroke after a settle delay resolved per CLI from `[clis.<name>].submit_delay_ms` (with a single CLI-agnostic default and no hardcoded CLI names), clearing the shell input line before the launch command, suppressing known auto-update/confirmation prompts in the launched pane's environment, and verifying the CLI started within a bounded window and retrying the launch once on failure — so a fresh supervisor session boots and all agents self-register unattended.

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

### Requirement: Boot prompt submitted via split-send + settle delay

The boot-injection path SHALL inject the boot block into a pane
literally and then submit it with a SEPARATE `Enter` sent after
a settle delay, rather than a same-call trailing `Enter`. This
split is what reliably submits a large paste across CLIs
(W15-1: a same-call trailing `Enter` left the boot block
unsubmitted on a custom CLI). The mechanism SHALL contain no
hardcoded CLI names.

#### Scenario: Boot block is injected then submitted separately

- **WHEN** a boot block is injected into a pane
- **THEN** the system SHALL first send the prompt text
  (literally, no `Enter`), then after the settle delay send
  `Enter` as a separate keystroke

#### Scenario: Mechanism is CLI-name-free

- **WHEN** the submit path is inspected
- **THEN** it SHALL NOT branch on any specific CLI name — the
  same split-send applies to every CLI

### Requirement: Settle delay is config-driven with an agnostic default

The settle delay SHALL be resolved per CLI from
`[clis.<name>].submit_delay_ms`, falling back to a single
CLI-agnostic default (`DEFAULT_SUBMIT_DELAY_MS`) for any CLI
without an override. The resolver SHALL key on the leading
binary token of the CLI command (so a CLI string carrying
flags still matches its config entry).

#### Scenario: Unconfigured CLI uses the agnostic default

- **GIVEN** a CLI with no `[clis.<name>].submit_delay_ms`
  configured (or no `[clis.<name>]` entry at all)
- **WHEN** the settle delay is resolved
- **THEN** it SHALL equal `DEFAULT_SUBMIT_DELAY_MS`

#### Scenario: Per-CLI override is honoured

- **GIVEN** `[clis.mycli].submit_delay_ms = 2500`
- **WHEN** the settle delay for `mycli` is resolved
- **THEN** it SHALL be 2500

#### Scenario: Resolver keys on the binary, not the flags

- **GIVEN** `[clis.mycli].submit_delay_ms = 2500`
- **WHEN** the delay is resolved for the CLI command
  `"mycli --some-flag"`
- **THEN** it SHALL be 2500 (the leading token `mycli`
  matched the config entry)

#### Scenario: No CLI name resolves to a hardcoded value

- **GIVEN** an empty `[clis]` config
- **WHEN** the delay is resolved for any CLI id (including
  names that might otherwise be special-cased)
- **THEN** every CLI SHALL resolve to the same
  `DEFAULT_SUBMIT_DELAY_MS` — there is no built-in per-name
  table

### Requirement: Profile applies to supervisor and agent panes

The split-send + resolved delay SHALL apply to every launched
pane, including the supervisor pane (itself a CLI instance).
The supervisor's delay is resolved from the supervisor CLI;
the agents' delay from the agent CLI.

#### Scenario: Supervisor pane boot block is submitted

- **GIVEN** any supervisor session
- **WHEN** the supervisor pane's boot block is injected
- **THEN** it SHALL be submitted via the split-send using the
  supervisor CLI's resolved delay, so the supervisor begins
  its loop without a manual `Enter`

### Requirement: End-to-end boot registration

The system SHALL boot a fresh supervisor session such that all
coding agents register with the broker without manual
intervention, for any CLI given an adequate settle delay
(default or configured) and broker-curl seeding.

#### Scenario: All agents register unattended

- **GIVEN** a fresh supervisor session with N agents and
  broker enabled, using a CLI whose settle delay is adequate
- **WHEN** the session launches
- **THEN** within a bounded window the broker `/status` SHALL
  list all N coding agents (plus the supervisor) with no human
  `Enter` or permission approval required

### Requirement: Clean the shell input line before the CLI-launch command

The system SHALL ensure a pane's shell input line is clean before sending
the CLI-launch command — by sending a clearing keystroke (e.g. `C-u`/`C-c`)
and/or a leading newline — so a pending shell startup prompt (auto-update
confirmation, MOTD, etc.) cannot swallow the leading character of the launch
command and strand the pane at a bare shell.

#### Scenario: Launch keystroke is not corrupted by a startup prompt

- **GIVEN** a pane whose interactive shell shows a startup prompt (e.g.
  `[oh-my-zsh] Would you like to update? [Y/n]`) at launch time
- **WHEN** git-paw sends the CLI-launch command
- **THEN** the pane SHALL clear the pending prompt first so the full launch
  command (not a keystroke-truncated variant like `laude-oss`) reaches the
  shell and the CLI starts

### Requirement: Suppress shell startup prompts in the launched pane

The system SHALL suppress known shell auto-update / confirmation prompts in
the pane it launches where it controls the pane environment (e.g. exporting
`DISABLE_AUTO_UPDATE=true` or the equivalent), so such a prompt cannot fire
mid-launch. The system SHALL NOT modify the user's global shell
configuration.

#### Scenario: Auto-update prompt suppressed for the launched pane

- **WHEN** git-paw launches a CLI pane
- **THEN** it SHALL set the pane environment so the shell's auto-update
  prompt does not fire during launch, without editing the user's `~/.zshrc`
  or global oh-my-zsh settings

### Requirement: Verify the CLI started and retry once

The system SHALL verify, within a bounded window after the launch keystroke,
that the pane's CLI actually started (the shell prompt was replaced by the
CLI), and SHALL retry the launch once if the first attempt did not take.

#### Scenario: Failed launch is retried

- **GIVEN** a pane where the first CLI-launch attempt did not start the CLI
  (the shell prompt is still present after the bounded window)
- **THEN** git-paw SHALL send the launch command once more before giving up,
  so a single swallowed attempt does not permanently strand the pane

### Requirement: README Supported AI CLIs table matches `src/detect.rs`

The README's Supported AI CLIs table SHALL list every CLI defined
in `src/detect.rs`. Currently that count is 10 entries; the v0.4
table of 7 entries SHALL be expanded to include `opencode`,
`cline`, and `droid` (plus any further additions present in
`src/detect.rs` at archive time).

#### Scenario: README CLI table mentions opencode, cline, and droid

- **WHEN** the README's Supported AI CLIs table is inspected
- **THEN** it contains the substring `opencode`
- **AND** it contains the substring `cline`
- **AND** it contains the substring `droid`
