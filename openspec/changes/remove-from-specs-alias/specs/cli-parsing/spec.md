## ADDED Requirements

### Requirement: Removed --from-specs flag is rejected with a migration message

The `start` subcommand SHALL reject `--from-specs` as a removed flag. When an invocation contains `--from-specs` — alone or combined with any other flag, including `--specs` — the command SHALL fail before any side effect (no worktree, session file, tmux session or config write) with exit code `2`, the code for an argument rejected by parsing, and stderr SHALL name `--from-specs`, state that it was renamed to `--from-all-specs`, and show the corrected invocation. The removed flag SHALL NOT appear in `git paw start --help` output or in any generated shell completion script. `--from-specs` SHALL NOT be an entry of the deprecation registry (`core-deprecation`), so the registry's removal guard passes and the CLI reference's Deprecations section does not list it; its removal is recorded in the stability chapter's list of removed items instead.

#### Scenario: --from-specs fails with a migration message

- **WHEN** `git paw start --from-specs --cli claude` runs in a git repository
- **THEN** the process SHALL exit `2`
- **AND** stderr SHALL contain `--from-specs`, the word `renamed`, and `--from-all-specs`
- **AND** no worktree, session file or tmux session SHALL have been created

#### Scenario: --from-specs combined with --specs reports the removal

- **WHEN** `git paw start --from-specs --specs add-auth` runs
- **THEN** the process SHALL exit `2`
- **AND** stderr SHALL carry the `--from-specs` migration message

#### Scenario: The removed flag is not advertised

- **WHEN** `git paw start --help` is run, and `git paw completions bash`, `zsh` and `fish` are generated
- **THEN** none of the outputs SHALL contain the substring `--from-specs`

#### Scenario: The deprecation registry no longer lists --from-specs

- **WHEN** the deprecation registry is enumerated
- **THEN** no entry SHALL have the item `--from-specs`
- **AND** the CLI reference's Deprecations section SHALL NOT contain `--from-specs`

## RENAMED Requirements

- FROM: `### Requirement: Spec mode + supervisor mode dispatch`
- TO: `### Requirement: Spec mode + coordinated mode dispatch`

## MODIFIED Requirements

### Requirement: Spec mode + coordinated mode dispatch

The `start` subcommand dispatch SHALL evaluate the start-mode resolution chain BEFORE branching on `--from-all-specs`. Specifically:

1. Resolve the start mode — coordinated or bare — from the `--coordinated`, `--bare` and `--unattended` flags, the `default_mode` config key, and the prompt fallback.
2. If coordinated mode is resolved, route to the coordinated launch path. When `--from-all-specs` is also set, pass `branches_flag = None` so the coordinated launch path's existing `scan_specs(...)` fallback runs to determine branches from configured specs.
3. Otherwise, if `--from-all-specs` is set, route to `cmd_start_from_specs`.
4. Otherwise, route to bare `cmd_start`.

This ordering ensures `--from-all-specs --coordinated` (or `--from-all-specs` with `default_mode = "coordinated"` in config) actually engages coordinated mode end-to-end, rather than silently degrading to a bare spec-driven launch.

`--from-all-specs` combined with `--branches` continues to follow v0.4's existing behaviour (the spec-mode flow ignores explicit branches when the launch-all-specs mode is set); this change does not introduce a new mutual-exclusion error for that combination.

#### Scenario: --from-all-specs --coordinated engages coordinated mode

- **GIVEN** `git paw start --from-all-specs --coordinated` is invoked
- **WHEN** the dispatch resolves
- **THEN** the start-mode resolution chain SHALL resolve coordinated mode
- **AND** the dispatch SHALL route to the coordinated launch path
- **AND** the coordinated launch path SHALL receive `branches_flag = None`, triggering its `scan_specs(...)` fallback

#### Scenario: --from-all-specs without coordinated mode uses spec mode

- **GIVEN** `git paw start --from-all-specs` is invoked with no mode flag, and the config does not select coordinated mode (`default_mode = "bare"`, or no `default_mode` and a non-interactive stdin)
- **WHEN** the dispatch resolves
- **THEN** the dispatch SHALL route to `cmd_start_from_specs`

#### Scenario: --from-all-specs with `default_mode = "coordinated"` config engages coordinated mode

- **GIVEN** `git paw start --from-all-specs` is invoked with no mode flag
- **AND** `.git-paw/config.toml` contains `default_mode = "coordinated"`
- **WHEN** the dispatch resolves
- **THEN** coordinated mode SHALL be active per the resolution chain
- **AND** the dispatch SHALL route to the coordinated launch path (not `cmd_start_from_specs`)

#### Scenario: --bare --from-all-specs uses spec mode

- **GIVEN** `git paw start --from-all-specs --bare` is invoked
- **AND** `default_mode = "coordinated"` is set in config
- **WHEN** the dispatch resolves
- **THEN** bare mode SHALL be resolved per the resolution chain
- **AND** the dispatch SHALL route to `cmd_start_from_specs`

#### Scenario: Bare start (no --from-all-specs, no coordinated mode) uses cmd_start

- **GIVEN** `git paw start` is invoked with no `--from-all-specs`, no `--coordinated`, and coordinated mode is not selected in config
- **WHEN** the dispatch resolves
- **THEN** the dispatch SHALL route to `cmd_start`

### Requirement: Non-TTY launch handling

When a `git paw start` invocation reaches its session-launch step (after worktrees are created, panes added, and `tmux_session.execute()` succeeds), the system SHALL detect whether stdin is connected to a terminal via `std::io::IsTerminal::is_terminal(&std::io::stdin())`.

When stdin is **not** a terminal:
- The system SHALL skip the `tmux::attach(...)` call.
- The system SHALL print an informational message to stdout naming the launched session and the manual-attach command (`tmux attach -t <session>`).
- The system SHALL exit with status `0`.

When stdin **is** a terminal, the bare launch paths proceed as before and call `tmux::attach`.

The coordinated launch path SHALL NOT call `tmux::attach` and SHALL NOT run any agent CLI in the foreground, whether or not stdin is a terminal. It SHALL print a line naming the launched session; without `--unattended` it SHALL then print the manual-attach command and return, and with `--unattended` it SHALL run the drive loop, which requires no terminal.

This SHALL apply to all three start paths: `cmd_start`, `cmd_start_from_specs`, and the coordinated launch path.

#### Scenario: Non-TTY bare start exits cleanly with attach hint

- **GIVEN** `git paw start --branches feat/x,feat/y` is invoked with stdin redirected from `/dev/null` (or otherwise non-TTY)
- **WHEN** the launch flow completes its session-build steps
- **THEN** the command SHALL exit with status `0`
- **AND** stdout SHALL contain "Session '<name>' started in detached mode."
- **AND** stdout SHALL contain "Attach with:  tmux attach -t <name>"
- **AND** the tmux session SHALL exist and be alive after exit

#### Scenario: Non-TTY --from-all-specs exits cleanly

- **GIVEN** `git paw start --from-all-specs` is invoked from a non-TTY context
- **WHEN** the launch flow completes
- **THEN** the command SHALL exit with status `0`
- **AND** the attach-hint message SHALL be printed
- **AND** the tmux session SHALL exist and be alive

#### Scenario: Non-TTY coordinated launch runs no foreground CLI

- **GIVEN** `git paw start --coordinated --from-all-specs` is invoked from a non-TTY context
- **WHEN** the launch flow completes
- **THEN** the command SHALL exit with status `0`
- **AND** no agent CLI SHALL be run in the foreground
- **AND** stdout SHALL contain a line naming the launched session
- **AND** stdout SHALL contain the manual-attach command for the launched session

#### Scenario: TTY launch attaches as before

- **GIVEN** `git paw start --branches feat/x,feat/y` is invoked from a real TTY
- **WHEN** the launch flow completes its session-build steps
- **THEN** the system SHALL call `tmux::attach(...)` for the launched session
- **AND** SHALL NOT print the "started in detached mode" hint

### Requirement: --from-all-specs flag

The `start` subcommand SHALL accept a `--from-all-specs` flag (boolean, default `false`). When passed, the resulting `StartArgs` SHALL indicate the "launch every discovered spec" mode.

The flag SHALL appear in `git paw start --help` output with a description naming it as the canonical name for this behaviour.

#### Scenario: --from-all-specs sets the launch-all mode

- **GIVEN** the user invokes `git paw start --from-all-specs`
- **WHEN** the CLI is parsed
- **THEN** the parsed `StartArgs` SHALL indicate the launch-all-discovered-specs mode

#### Scenario: --from-all-specs combined with --coordinated

- **GIVEN** `start --from-all-specs --coordinated`
- **WHEN** the CLI is parsed
- **THEN** both `from_all_specs` and `coordinated` SHALL be `true`

#### Scenario: --from-all-specs appears in help output

- **WHEN** `git paw start --help` is run
- **THEN** the output contains `--from-all-specs`
- **AND** the output describes the flag as launching every discovered spec

### Requirement: --from-all-specs and --specs are mutually exclusive

The system SHALL reject any invocation that combines `--from-all-specs` with `--specs`. clap's parse step SHALL produce an error before the command runs. The error message SHALL clearly state that the two flags express opposing intents and SHALL list both flags.

#### Scenario: --from-all-specs and --specs together are rejected

- **GIVEN** `start --from-all-specs --specs add-auth`
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail with an error mentioning both `--from-all-specs` and `--specs`

## REMOVED Requirements

### Requirement: --from-specs is a hidden alias of --from-all-specs

**Reason**: The alias was a v0.5.0 compatibility shim for v0.4 scripts, warned as deprecated through the `core-deprecation` registry since v0.24.0, and scheduled for removal at v1.0.0. Freezing it into the 1.x contract would keep two spellings of one flag for the whole major version.

**Migration**: Replace `--from-specs` with `--from-all-specs` in every invocation; behaviour is identical. Passing `--from-specs` now fails with exit code `2` and a message naming the replacement (*Removed --from-specs flag is rejected with a migration message*).
