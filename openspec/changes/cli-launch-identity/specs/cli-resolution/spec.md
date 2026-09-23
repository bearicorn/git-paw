## MODIFIED Requirements

### Requirement: Auto-detect known AI CLIs on PATH

The system SHALL scan PATH for the known CLI binaries: `claude`, `codex`, `agy`, `aider`, `vibe`, `qwen`, `amp`, `opencode`, `cline`, `droid`, `pi`, `junie`, `cursor-agent`, `copilot`, `cn`, `kilo`, and `kimi`.

`agy` is the Antigravity CLI (display name "Antigravity"), which replaces the retired Gemini CLI (`gemini`) — Gemini CLI stops serving free / AI Pro / AI Ultra users on 2026-06-18. Removing `gemini` affects auto-detection only; an explicit `default_cli = "gemini"`, a `[clis.gemini]` custom entry, or a saved-session `cli` string still resolves as a pass-through.

`cursor-agent` is Cursor's agent CLI (display name "Cursor Agent"). The bare `cursor` binary is the Cursor editor launcher on most installations and SHALL NOT be auto-detected as an AI CLI. Removing `cursor` affects auto-detection only; an explicit `--cli cursor`, `default_cli = "cursor"`, a `[clis.cursor]` custom entry, or a saved-session `cli = "cursor"` still resolves as a pass-through.

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

### Requirement: README Supported AI CLIs table matches `src/detect.rs`

The README's Supported AI CLIs table SHALL list every CLI binary in `src/detect.rs`'s known-CLI roster, and SHALL NOT list a binary that the roster does not auto-detect. The mdBook `docs/src/supported-clis.md` table SHALL carry the same binary set.

#### Scenario: README CLI table mentions opencode, cline, and droid

- **WHEN** the README's Supported AI CLIs table is inspected
- **THEN** it contains the substring `opencode`
- **AND** it contains the substring `cline`
- **AND** it contains the substring `droid`

#### Scenario: README and mdBook CLI tables list every roster binary and only those

- **WHEN** the README's Supported AI CLIs table and `docs/src/supported-clis.md`'s table are inspected
- **THEN** each SHALL contain a `` `<binary>` `` cell for every binary in the known-CLI roster, including `` `cursor-agent` ``
- **AND** neither SHALL contain a `` `cursor` `` binary cell

Test: `tests/supported_clis_docs_sync.rs::readme_and_book_tables_match_known_cli_roster`

## ADDED Requirements

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
