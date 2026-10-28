# cli-resolution Specification

## Purpose
Detect available AI coding CLI binaries by scanning PATH for known names and merging with user-defined custom CLIs from configuration, providing a unified, deduplicated, sorted list, and resolve which CLI to use for each spec-driven branch using a multi-level priority chain that considers command-line flags, per-spec overrides, config defaults, and interactive selection, prompting the user at most once. It also covers reliable boot-block submission and CLI-pane launch hardening: injecting the prompt text literally and then sending `Enter` as a separate keystroke after a settle delay resolved per CLI from `[clis.<name>].submit_delay_ms` (with a single CLI-agnostic default and no hardcoded CLI names), clearing the shell input line before the launch command, suppressing known auto-update/confirmation prompts in the launched pane's environment, and verifying the CLI started within a bounded window and retrying the launch once on failure — so a fresh supervisor session boots and all agents self-register unattended.
## Requirements
### Requirement: Auto-detect known AI CLIs on PATH

The system SHALL scan PATH for the known CLI binaries: `claude`, `codex`, `agy`, `aider`, `vibe`, `qwen`, `amp`, `opencode`, `cline`, `droid`, `pi`, `junie`, `cursor-agent`, `copilot`, `cn`, `kilo`, and `kimi`.

`agy` is the Antigravity CLI (display name "Antigravity"), which replaces the retired Gemini CLI (`gemini`) — Gemini CLI stops serving free / AI Pro / AI Ultra users on 2026-06-18. Removing `gemini` affects auto-detection only; an explicit `default_cli = "gemini"`, a `[clis.gemini]` custom entry, or a saved-session `cli` string still resolves as a pass-through.

`cursor-agent` is Cursor's agent CLI (display name "Cursor Agent"). The bare `cursor` binary is the Cursor editor launcher on most installations and SHALL NOT be auto-detected as an AI CLI. Removing `cursor` affects auto-detection only: an explicit `git paw start --cli cursor`, a `[clis.cursor]` custom entry, or a saved-session `cli = "cursor"` still resolves as a pass-through. The paths that validate a CLI against the detected roster — spec-driven selection, `git paw add`, and the interactive picker (which `default_cli` only pre-selects within) — require a `[clis.cursor]` entry.

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

The system SHALL resolve which CLI to use for each spec-driven branch using a 5-level priority chain, from highest to lowest priority. This chain SHALL apply identically to non-supervisor and supervisor-mode spec-driven launches — the `--supervisor` flag SHALL NOT bypass or alter it. In particular, `default_spec_cli` SHALL be honoured for spec branches lacking a `paw_cli` override even when `--supervisor` is combined with `--specs`, so supervisor-mode workers are never silently launched with a different (e.g. unsandboxed) CLI than configured.

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

#### Scenario: default_spec_cli is honoured under --supervisor --specs
- **WHEN** `git paw start --supervisor --specs a,b` runs with no `--cli`, specs `a` and `b` having no `paw_cli`, and `default_spec_cli = "claude-oss"` in config
- **THEN** the `a` and `b` worktree panes SHALL launch the `"claude-oss"` CLI (not a fallback or default CLI), identical to the non-supervisor `--specs` resolution

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

The split-send + resolved delay SHALL apply to every launched coding-agent pane, each resolved from that agent's own CLI. No orchestrator pane exists in the session, so no boot block is submitted for an orchestrator CLI; the orchestrator's CLI is resolved only for git-paw-initiated headless invocations, never for a pane.

#### Scenario: Coding-agent boot blocks are submitted with their own CLI's delay

- **GIVEN** a supervised session with two coding agents running different CLIs
- **WHEN** each agent's boot block is injected
- **THEN** each SHALL be submitted via the split-send using that agent's CLI's resolved delay, so the agent begins work without a manual `Enter`

#### Scenario: No pane receives an orchestrator boot block

- **GIVEN** a supervised session with `[supervisor] cli` set
- **WHEN** the launch injects boot blocks
- **THEN** no split-send SHALL target a pane with the orchestrator CLI's resolved delay

### Requirement: End-to-end boot registration

The system SHALL boot a fresh supervisor session such that all
coding agents register with the broker without a human `Enter`,
for any CLI given an adequate settle delay (default or
configured). The agents' boot publish goes through the bundled
`broker.sh` helper at its stable path. Whether that call runs
without a permission prompt is decided by the agent CLI's own
permission system: its resolved permission mode, or an allow
rule for the helper's exact path in that CLI's settings.
Registration SHALL NOT depend on a git-paw command classifier.
When the CLI prompts for the helper, git-paw SHALL NOT answer
the prompt; under `--unattended` the drive loop escalates it
(`supervisor-unattended-operation`).

#### Scenario: All agents register unattended

- **GIVEN** a fresh supervisor session with N agents and
  broker enabled, using a CLI whose settle delay is adequate
  and whose permission system lets the `broker.sh` helper run
- **WHEN** the session launches
- **THEN** within a bounded window the broker `/status` SHALL
  list all N coding agents (plus the supervisor) with no human
  `Enter` or permission approval required

#### Scenario: A prompted boot publish is escalated, not answered

- **GIVEN** an `--unattended` session whose agent CLI raises a
  permission prompt for its `broker.sh` boot publish
- **WHEN** the drive loop sweeps that pane
- **THEN** the prompt SHALL be escalated
- **AND** no keystroke SHALL be sent to that pane

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

The README's Supported AI CLIs table SHALL list every CLI binary in `src/detect.rs`'s known-CLI roster, and SHALL NOT list a binary that the roster does not auto-detect. The mdBook `docs/src/supported-clis.md` table SHALL carry the same binary set.

#### Scenario: README CLI table mentions opencode, cline, and droid

- **WHEN** the README's Supported AI CLIs table is inspected
- **THEN** it contains the substring `opencode`
- **AND** it contains the substring `cline`
- **AND** it contains the substring `droid`

Test: `tests/supported_clis_docs_sync.rs::readme_and_book_tables_match_known_cli_roster` (`opencode`, `cline`, and `droid` are roster members, so the per-binary missing-cell assertion covers them)

#### Scenario: README and mdBook CLI tables list every roster binary and only those

- **WHEN** the README's Supported AI CLIs table and `docs/src/supported-clis.md`'s table are inspected
- **THEN** each SHALL contain a `` `<binary>` `` cell for every binary in the known-CLI roster, including `` `cursor-agent` ``
- **AND** neither SHALL contain a `` `cursor` `` binary cell

Test: `tests/supported_clis_docs_sync.rs::readme_and_book_tables_match_known_cli_roster`

### Requirement: Per-CLI session-id and resume argument templates

The `[clis.<name>]` table SHALL accept two optional string templates, `session_id_args` (arguments
that assign a session identifier at launch) and `resume_args` (arguments that resume an existing
session), each containing the placeholder `{{SESSION_ID}}`. Resolution for a CLI SHALL consult, in
order: (1) the `[clis.<name>]` override when present; (2) the built-in table; (3) no template. The
built-in table SHALL contain, at minimum, `claude` → `session_id_args = "--session-id {{SESSION_ID}}"`
and `resume_args = "--resume {{SESSION_ID}}"`; every CLI without a built-in row and without an override
SHALL resolve to no template. A template that omits `{{SESSION_ID}}` SHALL be rejected at config load
with an error naming the key. The built-in rows SHALL be verified against the CLI's upstream
documentation at implementation time.

#### Scenario: Claude resolves the built-in templates

- **WHEN** templates are resolved for `claude` with no config override
- **THEN** `session_id_args` SHALL render to `--session-id <id>` and `resume_args` to `--resume <id>`
  for the given id

#### Scenario: Override wins over the built-in row

- **GIVEN** `[clis.claude]` defining `resume_args = "--continue-session {{SESSION_ID}}"`
- **WHEN** the resume template is resolved for `claude`
- **THEN** it SHALL render to `--continue-session <id>`

#### Scenario: Unknown CLI resolves no template

- **WHEN** templates are resolved for `my-agent` with no `[clis.my-agent]` templates
- **THEN** both `session_id_args` and `resume_args` SHALL resolve to none

#### Scenario: Template without the placeholder is rejected

- **GIVEN** `[clis.my-agent]` defining `session_id_args = "--session"` (no `{{SESSION_ID}}`)
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `session_id_args`

### Requirement: Worker session id is assigned at launch

The system SHALL assign each worker a session identifier at launch: when the launch flow attaches a
worker (initial `start`, spec-driven `start`, `git paw add`) and the worker's CLI resolves a
`session_id_args` template, it SHALL generate a fresh version-4-formatted UUID from OS-provided
randomness, append the rendered `session_id_args` to the CLI launch command after any approval flags,
and record the id in the worktree's `cli_session_id`. When the CLI resolves no
`session_id_args` template, the launch command SHALL be unchanged and `cli_session_id` SHALL remain
`None`. The identifier SHALL be generated without adding a dependency.

#### Scenario: Claude worker is launched with an assigned id

- **GIVEN** a worker on `feat/x` with `cli = "claude"` and no override
- **WHEN** the launch command is built
- **THEN** it SHALL contain `--session-id <uuid>` where `<uuid>` matches the v4 UUID shape
- **AND** the saved `WorktreeEntry` for `feat/x` SHALL have `cli_session_id = Some(<uuid>)`

#### Scenario: Each worker gets a distinct id

- **GIVEN** three Claude workers launched in one session
- **WHEN** their `cli_session_id` values are compared
- **THEN** all three SHALL differ

#### Scenario: CLI without an assignment template launches unchanged

- **GIVEN** a worker with `cli = "codex"` and no `[clis.codex].session_id_args`
- **WHEN** the launch command is built
- **THEN** it SHALL be identical to the pre-change command
- **AND** the saved entry's `cli_session_id` SHALL be `None`

#### Scenario: Assignment follows approval flags

- **GIVEN** a Claude worker at `agent_approval = "full-auto"`
- **WHEN** the launch command is built
- **THEN** `--dangerously-skip-permissions` SHALL precede `--session-id <uuid>` in the command

### Requirement: Panes launch the configured command of a CLI

Every launch path that spawns an AI CLI into a tmux pane — the bare start, the spec-driven start, the supervisor pane, `git paw add`, and session recovery (worker and supervisor panes) — SHALL build the pane's launch command from the CLI's resolved `[clis.<name>].command`, not from the `[clis.<name>]` key. The leading whitespace-delimited token of the CLI identity is looked up in the merged `[clis]` table; when it names an entry, that token SHALL be replaced by the entry's `command` and any trailing text SHALL be preserved verbatim. When it names no entry (a built-in detected CLI, or an explicit pass-through name), the identity SHALL be used unchanged.

The substituted `command` SHALL be emitted as exactly one shell word that the pane's shell resolves to the configured string: it SHALL be single-quoted (with embedded single quotes escaped) whenever it contains any character outside the portable safe set `A-Z a-z 0-9 _ - . / + , : @ %`, so a path containing spaces or shell metacharacters is neither word-split nor expanded. Approval flags and session-id / resume arguments SHALL be appended after the substituted command exactly as they are appended after the bare CLI today.

#### Scenario: A custom CLI whose key differs from its command launches the command

- **GIVEN** `[clis.claude-variant] command = "claude"`
- **WHEN** a session is started with a worker on `feat/x` using CLI `claude-variant`
- **THEN** the pane launch command SHALL begin with `claude`
- **AND** it SHALL NOT contain `claude-variant`

Test: `tests/cli_launch_identity_e2e.rs::worker_pane_launches_configured_command_not_key`

#### Scenario: A CLI without a `[clis]` entry launches by name

- **GIVEN** no `[clis.codex]` entry
- **WHEN** a worker is attached with CLI `codex`
- **THEN** the pane launch command SHALL be `codex` exactly as before this change

Test: `config::tests::launch_command_falls_back_to_name_without_entry`

#### Scenario: A command path containing a space is one shell word

- **GIVEN** `[clis.my-agent] command = "/opt/My Tools/my-agent"` and an executable stub at that path
- **WHEN** the launch command is built and run through a real POSIX shell
- **THEN** the launch command SHALL begin with `'/opt/My Tools/my-agent'`
- **AND** the shell SHALL execute the stub at that exact path (not `/opt/My` with an argument)

Test: `config::tests::launch_command_quotes_path_with_space_as_one_shell_word`

#### Scenario: Trailing text after the identity is preserved

- **GIVEN** `[clis.claude-oss] command = "/usr/local/bin/claude"`
- **WHEN** the launch command is built for the identity `claude-oss --model opus`
- **THEN** it SHALL be `/usr/local/bin/claude --model opus`

Test: `config::tests::launch_command_preserves_trailing_text`

#### Scenario: Supervisor pane launches the configured command

- **GIVEN** `[supervisor] cli = "claude-oss"` and `[clis.claude-oss] command = "claude"` with `approval_args` resolving a non-empty flag for the supervisor's approval level
- **WHEN** the supervisor session launches
- **THEN** pane 0's launch command SHALL be `claude` followed by that flag

Test: `tests/cli_launch_identity_e2e.rs::supervisor_pane_launches_configured_command`

#### Scenario: Recovery relaunches the configured command

- **GIVEN** a saved session whose worktree entry has `cli = "claude-oss"` and `cli_session_id = Some(<id>)`, and `[clis.claude-oss] command = "claude"` with a resolvable `resume_args`
- **WHEN** the session is recovered
- **THEN** the worker pane's launch command SHALL begin with `claude` followed by the rendered resume arguments

Test: `tests/cli_launch_identity_e2e.rs::recovery_launches_configured_command_with_resume_args`

### Requirement: The CLI identity, not the launch command, keys per-CLI resolution

The `[clis.<name>]` key (or the built-in / pass-through CLI name) SHALL remain the CLI's identity. Approval-flag resolution, session-id and resume template resolution, prompt-profile resolution (including the readiness gate's choice of readiness markers), settle-delay resolution, MCP-capability resolution, and the `cli` recorded in each saved `WorktreeEntry` SHALL all be keyed by that identity, never by the substituted launch command. The readiness gate SHALL relaunch the substituted launch command when a pane is still a bare shell.

#### Scenario: Saved worktree entry records the key

- **GIVEN** `[clis.claude-oss] command = "claude"`
- **WHEN** a session is started with a worker using CLI `claude-oss`
- **THEN** the saved session's `WorktreeEntry.cli` for that worker SHALL be `claude-oss`

Test: `tests/cli_launch_identity_e2e.rs::saved_session_records_cli_key`

#### Scenario: Per-CLI overrides resolve against the key

- **GIVEN** `[clis.claude-oss] command = "claude"` with `approval_args = { auto = "--paw-oss-sentinel" }`
- **WHEN** a session is started with a worker using CLI `claude-oss` at `agent_approval = "auto"`
- **THEN** the pane launch command SHALL be `claude --paw-oss-sentinel`

Test: `tests/cli_launch_identity_e2e.rs::worker_overrides_resolve_by_key_and_launch_command`

#### Scenario: Readiness gate uses the identity's prompt profile

- **GIVEN** `[clis.claude-oss] command = "claude"` with a `prompt_profile.readiness_markers` override
- **WHEN** the readiness gate polls the pane for CLI identity `claude-oss`
- **THEN** it SHALL classify readiness using the override's markers
- **AND** a bare-shell relaunch SHALL send the launch command `claude`

Test: `tmux::tests::gate_pane_resolves_profile_by_identity_and_relaunches_command`

### Requirement: Per-CLI headless invocation template

The `[clis.<name>]` table SHALL accept an optional string `headless_args` giving the arguments that make the CLI run one non-interactive turn reading its prompt from stdin. Resolution for a CLI SHALL consult, in order: (1) the `[clis.<name>]` override; (2) the built-in table, which SHALL contain at minimum `claude` → `-p` and `codex` → `exec`; (3) none. The built-in rows SHALL be verified against each CLI's upstream documentation at implementation time. An empty override string SHALL be rejected at config load with an error naming the key.

#### Scenario: Claude and Codex resolve through the built-in table

- **WHEN** `headless_args` is resolved for `claude` and for `codex` with no overrides
- **THEN** the results SHALL be `-p` and `exec` respectively

#### Scenario: Override wins over the built-in row

- **GIVEN** `[clis.claude]` defining `headless_args = "-p --output-format text"`
- **WHEN** `headless_args` is resolved for `claude`
- **THEN** the result SHALL be `-p --output-format text`

#### Scenario: Unknown CLI resolves none

- **WHEN** `headless_args` is resolved for `my-agent` with no `[clis.my-agent].headless_args`
- **THEN** the result SHALL be none

#### Scenario: Empty override is rejected

- **GIVEN** `[clis.my-agent] headless_args = ""`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `headless_args`

