## ADDED Requirements

### Requirement: Spec mode + supervisor mode dispatch

The `start` subcommand dispatch SHALL evaluate the supervisor-mode resolution chain (per `supervisor-cli`) BEFORE branching on `--from-specs`. Specifically:

1. Resolve supervisor-mode-enabled state from the `--supervisor` flag, `--no-supervisor` flag, `[supervisor]` config, and the prompt fallback.
2. If supervisor mode is enabled, route to `cmd_supervisor`. When `--from-specs` is also set, pass `branches_flag = None` so `cmd_supervisor`'s existing `scan_specs(...)` fallback runs to determine branches from configured specs.
3. Otherwise, if `--from-specs` is set, route to `cmd_start_from_specs`.
4. Otherwise, route to bare `cmd_start`.

This ordering ensures `--from-specs --supervisor` (or `--from-specs` with `[supervisor] enabled = true` in config) actually engages supervisor mode end-to-end, rather than silently degrading to spec-mode-without-supervisor.

`--from-specs` combined with `--branches` continues to follow v0.4's existing behaviour (the spec-mode flow ignores explicit branches when from-specs is set); this change does not introduce a new mutual-exclusion error for that combination.

#### Scenario: --from-specs --supervisor engages supervisor mode

- **GIVEN** `git paw start --from-specs --supervisor` is invoked
- **WHEN** the dispatch resolves
- **THEN** the supervisor-mode resolution chain SHALL evaluate `supervisor = true`
- **AND** the dispatch SHALL route to `cmd_supervisor`
- **AND** `cmd_supervisor` SHALL receive `branches_flag = None`, triggering its `scan_specs(...)` fallback

#### Scenario: --from-specs without supervisor uses spec mode

- **GIVEN** `git paw start --from-specs` is invoked, no `--supervisor` flag, and `[supervisor]` config indicates supervisor mode is not enabled (either explicitly false or absent + non-interactive)
- **WHEN** the dispatch resolves
- **THEN** the dispatch SHALL route to `cmd_start_from_specs`

#### Scenario: --from-specs with `[supervisor] enabled = true` config engages supervisor mode

- **GIVEN** `git paw start --from-specs` is invoked with no `--supervisor` flag
- **AND** `.git-paw/config.toml` contains `[supervisor] enabled = true`
- **WHEN** the dispatch resolves
- **THEN** supervisor mode SHALL be active per the resolution chain
- **AND** the dispatch SHALL route to `cmd_supervisor` (not `cmd_start_from_specs`)

#### Scenario: --no-supervisor --from-specs uses spec mode

- **GIVEN** `git paw start --from-specs --no-supervisor` is invoked
- **AND** `[supervisor] enabled = true` is set in config
- **WHEN** the dispatch resolves
- **THEN** supervisor mode SHALL be disabled per the resolution chain
- **AND** the dispatch SHALL route to `cmd_start_from_specs`

#### Scenario: Bare start (no --from-specs, no supervisor) uses cmd_start

- **GIVEN** `git paw start` is invoked with no `--from-specs`, no `--supervisor`, and supervisor mode is not enabled in config
- **WHEN** the dispatch resolves
- **THEN** the dispatch SHALL route to `cmd_start`

### Requirement: Non-TTY launch handling

When a `git paw start` invocation reaches its session-launch step (after worktrees are created, panes added, and `tmux_session.execute()` succeeds), the system SHALL detect whether stdin is connected to a terminal via `std::io::IsTerminal::is_terminal(&std::io::stdin())`.

When stdin is **not** a terminal:
- The system SHALL skip the `tmux::attach(...)` call.
- The system SHALL print an informational message to stdout naming the launched session and the manual-attach command (`tmux attach -t <session>`).
- The system SHALL exit with status `0`.
- For supervisor mode specifically, the system SHALL also skip the foreground supervisor-CLI launch (`Command::new(supervisor_cli).status()`) with an additional hint that supervisor mode requires an interactive terminal.

When stdin **is** a terminal, the launch flow proceeds as before (call `tmux::attach`, run the supervisor CLI in foreground for supervisor mode).

This SHALL apply to all three start paths: `cmd_start`, `cmd_start_from_specs`, and `cmd_supervisor`.

#### Scenario: Non-TTY bare start exits cleanly with attach hint

- **GIVEN** `git paw start --branches feat/x,feat/y` is invoked with stdin redirected from `/dev/null` (or otherwise non-TTY)
- **WHEN** the launch flow completes its session-build steps
- **THEN** the command SHALL exit with status `0`
- **AND** stdout SHALL contain "Session '<name>' started in detached mode."
- **AND** stdout SHALL contain "Attach with:  tmux attach -t <name>"
- **AND** the tmux session SHALL exist and be alive after exit

#### Scenario: Non-TTY --from-specs exits cleanly

- **GIVEN** `git paw start --from-specs` is invoked from a non-TTY context
- **WHEN** the launch flow completes
- **THEN** the command SHALL exit with status `0`
- **AND** the attach-hint message SHALL be printed
- **AND** the tmux session SHALL exist and be alive

#### Scenario: Non-TTY --supervisor skips supervisor CLI launch

- **GIVEN** `git paw start --supervisor --from-specs` is invoked from a non-TTY context
- **WHEN** the launch flow completes
- **THEN** the command SHALL exit with status `0`
- **AND** the foreground supervisor-CLI launch SHALL be skipped
- **AND** stdout SHALL contain a hint indicating supervisor mode requires an interactive terminal
- **AND** stdout SHALL contain the manual-attach command for the launched session

#### Scenario: TTY launch attaches as before

- **GIVEN** `git paw start --branches feat/x,feat/y` is invoked from a real TTY
- **WHEN** the launch flow completes its session-build steps
- **THEN** the system SHALL call `tmux::attach(...)` for the launched session
- **AND** SHALL NOT print the "started in detached mode" hint
