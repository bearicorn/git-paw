## Purpose

Define the command-line interface for git-paw using clap v4. Declares all subcommands (`start`, `stop`, `purge`, `status`, `list-clis`, `add-cli`, `remove-cli`), their flags, and argument validation. When no subcommand is given, defaults to `start`.
## Requirements
### Requirement: Default to start when no subcommand is given

The system SHALL treat no arguments as equivalent to `start` with no flags.

The system SHALL also accept a hidden `__dashboard` subcommand that does not appear in `--help` output. This subcommand is used internally by pane 0 to run the broker and dashboard.

#### Scenario: No arguments yields None command
- **GIVEN** no arguments are passed
- **WHEN** the CLI is parsed
- **THEN** `command` SHALL be `None` (handled as `Start` in main)

#### Scenario: __dashboard subcommand parses
- **GIVEN** `__dashboard` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Dashboard`

#### Scenario: __dashboard does not appear in help
- **GIVEN** `--help` is passed
- **WHEN** the help text is rendered
- **THEN** the output SHALL NOT contain `__dashboard`

### Requirement: Start subcommand with optional flags

The `start` subcommand SHALL be extended to accept a `--supervisor` flag (boolean, defaults to `false`). The flag MAY be combined with any other `start` flags.

When `--supervisor` is passed, the parsed `StartArgs` struct SHALL have `supervisor: bool` set to `true`.

The `start` subcommand SHALL also accept a `--no-rebase` flag (boolean, defaults to `false`). When `--no-rebase` is passed, the parsed `StartArgs` struct SHALL have `no_rebase: bool` set to `true`. The dispatch SHALL invoke `create_worktree` with `rebase_onto_main = !args.no_rebase` for every worktree creation in the launch. When `--no-rebase` is omitted (i.e. `no_rebase == false`), agent branches SHALL be rebased onto the repository's default branch before their worktrees are opened. When `--no-rebase` is present, agent branches SHALL NOT be rebased, matching the post-`worktree-resume-fix` v0.5.0 behaviour.

The `--no-rebase` flag MAY be combined with any other `start` flags including `--supervisor`, `--from-specs`, `--cli`, and `--branches`.

#### Scenario: Start with --supervisor flag

- **GIVEN** `start --supervisor`
- **WHEN** the CLI is parsed
- **THEN** `supervisor` SHALL be `true`

#### Scenario: Start with --supervisor combined with other flags

- **GIVEN** `start --supervisor --cli claude --branches feat/a,feat/b`
- **WHEN** the CLI is parsed
- **THEN** `supervisor` SHALL be `true`
- **AND** `cli` SHALL be `Some("claude")`
- **AND** `branches` SHALL be `["feat/a", "feat/b"]`

#### Scenario: Start without --supervisor defaults to false

- **GIVEN** `start --cli claude`
- **WHEN** the CLI is parsed
- **THEN** `supervisor` SHALL be `false`

#### Scenario: Start with --no-rebase flag

- **GIVEN** `start --no-rebase`
- **WHEN** the CLI is parsed
- **THEN** `no_rebase` SHALL be `true`

#### Scenario: Start without --no-rebase defaults to false

- **GIVEN** `start --cli claude`
- **WHEN** the CLI is parsed
- **THEN** `no_rebase` SHALL be `false`

#### Scenario: Start with --no-rebase combined with other flags

- **GIVEN** `start --no-rebase --supervisor --from-specs`
- **WHEN** the CLI is parsed
- **THEN** `no_rebase` SHALL be `true`
- **AND** `supervisor` SHALL be `true`
- **AND** `from_specs` SHALL be `true`

#### Scenario: --no-rebase propagates to create_worktree as rebase_onto_main = false

- **GIVEN** `start --branches feat/a --no-rebase` is invoked
- **WHEN** the dispatch reaches the worktree-creation loop
- **THEN** `create_worktree(repo_root, "feat/a", rebase_onto_main)` SHALL be called with `rebase_onto_main = false`

#### Scenario: Default start propagates rebase_onto_main = true

- **GIVEN** `start --branches feat/a` is invoked without `--no-rebase`
- **WHEN** the dispatch reaches the worktree-creation loop
- **THEN** `create_worktree(repo_root, "feat/a", rebase_onto_main)` SHALL be called with `rebase_onto_main = true`

### Requirement: Stop subcommand

The `stop` subcommand SHALL accept an optional `--force` flag (boolean, defaults to `false`). When `--force` is omitted AND stdin is a TTY, `cmd_stop` SHALL render an interactive confirmation prompt describing the destructive nature of stop and pointing at `git paw pause` (soft alternative) and `git paw purge` (full reset). When `--force` is set OR stdin is not a TTY, the prompt SHALL be skipped and the stop SHALL proceed immediately.

The `long_about` help text for `stop` SHALL name all three teardown verbs (`pause`, `stop`, `purge`) with a one-line summary of each, so users can choose the right verb at `--help` time.

#### Scenario: Stop parses without flags

- **GIVEN** `stop` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Stop { force: false }`

#### Scenario: Stop parses with --force

- **GIVEN** `stop --force` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Stop { force: true }`

#### Scenario: Stop help names all three teardown verbs

- **WHEN** `git paw stop --help` is run
- **THEN** the output SHALL mention `pause` as the soft alternative
- **AND** the output SHALL mention `purge` as the full reset
- **AND** the output SHALL describe what `stop` itself does (kills CLI processes, preserves worktrees)

#### Scenario: Stop with --force from a TTY skips the prompt

- **GIVEN** an active session and `--force` is passed
- **WHEN** `git paw stop --force` is run with stdin attached to a TTY
- **THEN** no interactive prompt SHALL be rendered
- **AND** the session SHALL be killed immediately

### Requirement: Purge subcommand with optional --force flag

The `purge` subcommand SHALL accept an optional `--force` flag (defaults to `false`).

#### Scenario: Purge without --force
- **GIVEN** `purge` is passed without flags
- **WHEN** the CLI is parsed
- **THEN** `force` SHALL be `false`

Test: `cli::tests::purge_without_force`

#### Scenario: Purge with --force
- **GIVEN** `purge --force` is passed
- **WHEN** the CLI is parsed
- **THEN** `force` SHALL be `true`

Test: `cli::tests::purge_with_force`

### Requirement: Status subcommand

The `status` subcommand SHALL parse with no additional arguments.

#### Scenario: Status parses
- **GIVEN** `status` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Status`

Test: `cli::tests::status_parses`

### Requirement: List-CLIs subcommand

The `list-clis` subcommand SHALL parse with no additional arguments.

#### Scenario: List-CLIs parses
- **GIVEN** `list-clis` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::ListClis`

Test: `cli::tests::list_clis_parses`

### Requirement: Add-CLI subcommand with required and optional arguments

The `add-cli` subcommand SHALL require `name` and `command` positional arguments and accept an optional `--display-name` flag.

#### Scenario: Add-CLI with required arguments only
- **GIVEN** `add-cli my-agent /usr/local/bin/my-agent`
- **WHEN** the CLI is parsed
- **THEN** `name` SHALL be `"my-agent"`, `command` SHALL be the path, and `display_name` SHALL be `None`

Test: `cli::tests::add_cli_with_required_args`

#### Scenario: Add-CLI with --display-name
- **GIVEN** `add-cli my-agent my-agent --display-name "My Agent"`
- **WHEN** the CLI is parsed
- **THEN** `display_name` SHALL be `Some("My Agent")`

Test: `cli::tests::add_cli_with_display_name`

#### Scenario: Add-CLI missing required arguments is rejected
- **GIVEN** `add-cli` with no positional arguments
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail

Test: `cli::tests::add_cli_missing_required_args_is_rejected`

### Requirement: Remove-CLI subcommand with required argument

The `remove-cli` subcommand SHALL require a `name` positional argument.

#### Scenario: Remove-CLI parses
- **GIVEN** `remove-cli my-agent`
- **WHEN** the CLI is parsed
- **THEN** `name` SHALL be `"my-agent"`

Test: `cli::tests::remove_cli_parses`

### Requirement: Standard flags --version and --help

The CLI SHALL accept `--version` and `--help` flags.

#### Scenario: --version flag is accepted
- **GIVEN** `--version` is passed
- **WHEN** the CLI is parsed
- **THEN** clap SHALL emit a `DisplayVersion` response

Test: `cli::tests::version_flag_is_accepted`

#### Scenario: --help flag is accepted
- **GIVEN** `--help` is passed
- **WHEN** the CLI is parsed
- **THEN** clap SHALL emit a `DisplayHelp` response

Test: `cli::tests::help_flag_is_accepted`

### Requirement: Unknown subcommands are rejected

The CLI SHALL reject unrecognized subcommands with a parse error.

#### Scenario: Unknown subcommand fails
- **GIVEN** an unrecognized subcommand is passed
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail

Test: `cli::tests::unknown_subcommand_is_rejected`

### Requirement: Help output contains all subcommands and quick start

The `start --help` output SHALL list the `--supervisor` flag with a description.

#### Scenario: Start help shows --supervisor flag

- **GIVEN** `start --help` is passed
- **WHEN** the binary runs
- **THEN** stdout SHALL contain `--supervisor`

### Requirement: Version output includes binary name

The `--version` output SHALL include the binary name.

#### Scenario: Version output
- **GIVEN** `--version` is passed
- **WHEN** the binary runs
- **THEN** stdout SHALL contain "git-paw"

Test: `cli_tests::version_output`

### Requirement: No arguments behaves like start

When no subcommand is provided, the binary SHALL behave identically to `start`.

#### Scenario: No args produces same error as start
- **GIVEN** the binary is run with no arguments outside a git repo
- **WHEN** both `git-paw` and `git-paw start` are run
- **THEN** they SHALL produce identical stderr output

Test: `cli_tests::no_args_behaves_like_start`

### Requirement: Subcommands run without error when applicable

Subcommands that don't require a session SHALL succeed in a valid git repo.

#### Scenario: Stop runs without error
- **GIVEN** the binary is run in a git repo
- **WHEN** `stop` is passed
- **THEN** it SHALL succeed

Test: `cli_tests::stop_runs_without_error`

#### Scenario: Status runs without error
- **GIVEN** the binary is run in a git repo
- **WHEN** `status` is passed
- **THEN** it SHALL succeed

Test: `cli_tests::status_runs_without_error`

#### Scenario: List-CLIs runs without error
- **GIVEN** the binary is run in a git repo
- **WHEN** `list-clis` is passed
- **THEN** it SHALL succeed

Test: `cli_tests::list_clis_runs_without_error`

### Requirement: Binary rejects missing required arguments

Subcommands with required arguments SHALL fail when they are missing.

#### Scenario: Add-CLI requires arguments
- **GIVEN** `add-cli` is passed with no arguments
- **WHEN** the binary runs
- **THEN** it SHALL fail with stderr mentioning "required"

Test: `cli_tests::add_cli_requires_arguments`

#### Scenario: Remove-CLI requires argument
- **GIVEN** `remove-cli` is passed with no arguments
- **WHEN** the binary runs
- **THEN** it SHALL fail with stderr mentioning "required"

Test: `cli_tests::remove_cli_requires_argument`

### Requirement: Not-a-repo error from binary

Commands requiring a git repo SHALL fail with an actionable error when run outside one.

#### Scenario: Start from non-git directory
- **GIVEN** the binary is run outside a git repository
- **WHEN** `start` is passed
- **THEN** it SHALL fail with stderr containing "Not a git repository"

Test: `cli_tests::start_from_non_git_dir`

#### Scenario: Unknown subcommand from binary
- **GIVEN** the binary is passed an unrecognized subcommand
- **WHEN** it runs
- **THEN** it SHALL fail with stderr containing "error"

Test: `cli_tests::unknown_subcommand_fails`

### Requirement: Replay subcommand

The `replay` subcommand SHALL accept an optional `<branch>` positional argument, a `--list` flag, a `--color` flag, and an optional `--session` flag.

#### Scenario: Replay with branch
- **WHEN** `replay feat/add-auth` is passed
- **THEN** `branch` SHALL be `Some("feat/add-auth")`, `list` SHALL be `false`, `color` SHALL be `false`

#### Scenario: Replay with --list
- **WHEN** `replay --list` is passed
- **THEN** `list` SHALL be `true` and `branch` SHALL be `None`

#### Scenario: Replay with --color
- **WHEN** `replay feat/add-auth --color` is passed
- **THEN** `color` SHALL be `true`

#### Scenario: Replay with --session
- **WHEN** `replay feat/add-auth --session paw-myproject` is passed
- **THEN** `session` SHALL be `Some("paw-myproject")`

#### Scenario: Replay with no arguments and no --list
- **WHEN** `replay` is passed with no arguments and no `--list`
- **THEN** parsing SHALL fail with an error indicating either a branch or `--list` is required

#### Scenario: Replay help text
- **WHEN** `replay --help` is passed
- **THEN** stdout SHALL contain descriptions of `--list`, `--color`, and `--session` flags with examples

### Requirement: Init subcommand

The `init` subcommand SHALL parse with no required arguments.

#### Scenario: Init parses
- **WHEN** `init` is passed
- **THEN** the command SHALL be `Command::Init`

#### Scenario: Init help text
- **WHEN** `init --help` is passed
- **THEN** stdout SHALL contain a description of project initialization and examples

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

### Requirement: --from-all-specs flag

The `start` subcommand SHALL accept a `--from-all-specs` flag (boolean, default `false`). When passed, the resulting `StartArgs` SHALL indicate the "launch every discovered spec" mode — the v0.4 behaviour previously gated by `--from-specs`.

The flag SHALL appear in `git paw start --help` output with a description naming it as the canonical name for this behaviour.

#### Scenario: --from-all-specs sets the launch-all mode

- **GIVEN** the user invokes `git paw start --from-all-specs`
- **WHEN** the CLI is parsed
- **THEN** the parsed `StartArgs` SHALL indicate the launch-all-discovered-specs mode

#### Scenario: --from-all-specs combined with --supervisor

- **GIVEN** `start --from-all-specs --supervisor`
- **WHEN** the CLI is parsed
- **THEN** both `from_all_specs` and `supervisor` SHALL be `true`

#### Scenario: --from-all-specs appears in help output

- **WHEN** `git paw start --help` is run
- **THEN** the output contains `--from-all-specs`
- **AND** the output describes the flag as launching every discovered spec

### Requirement: --from-specs is a hidden alias of --from-all-specs

The `start` subcommand SHALL accept `--from-specs` as a hidden alias of `--from-all-specs`. When the user passes `--from-specs`, the parsed `StartArgs` SHALL be byte-for-byte identical to the parse result for `--from-all-specs`. No stderr warning SHALL be emitted at runtime; the alias is silent.

The alias SHALL NOT appear in `git paw start --help` output. The alias SHALL be removed in v1.0.0; v0.5.0 keeps it for backward compatibility with v0.4 scripts.

#### Scenario: --from-specs parses identically to --from-all-specs

- **GIVEN** two CLI invocations: `start --from-specs` and `start --from-all-specs`
- **WHEN** both are parsed
- **THEN** the resulting `StartArgs` values SHALL be equal

#### Scenario: --from-specs does not appear in help

- **WHEN** `git paw start --help` is run
- **THEN** the output SHALL NOT contain the substring `--from-specs`

#### Scenario: --from-specs emits no stderr warning

- **GIVEN** the user runs a command containing `--from-specs`
- **WHEN** the CLI parses
- **THEN** no stderr warning SHALL be emitted regarding the flag's deprecation
- **AND** the command proceeds exactly as if `--from-all-specs` had been passed

### Requirement: --specs flag with comma-separated values

The `start` subcommand SHALL accept a `--specs` flag whose value is a comma-separated list of spec names (mirroring the existing `--branches feat/a,feat/b` syntax). The flag SHALL accept zero or more values:

- `--specs` (no values) — indicates the picker mode.
- `--specs NAME` — narrows to a single named spec.
- `--specs NAME1,NAME2,NAME3` — narrows to the listed specs.
- `--specs NAME1,NAME2 --specs NAME3` — equivalent to `--specs NAME1,NAME2,NAME3` if clap's value-accumulation across repetitions is enabled (implementation choice; tests assert behaviour for the comma-separated form).

The parsed value distinguishes three states:
- Flag absent → no spec mode requested.
- Flag present with zero values → picker mode.
- Flag present with one or more values → narrow mode with the listed names.

The flag SHALL appear in `git paw start --help` output.

#### Scenario: --specs with single value parses as narrow

- **GIVEN** `start --specs add-auth`
- **WHEN** the CLI is parsed
- **THEN** `StartArgs` SHALL indicate narrow mode with `["add-auth"]`

#### Scenario: --specs with comma-separated values parses as narrow with multiple names

- **GIVEN** `start --specs add-auth,fix-session,add-logging`
- **WHEN** the CLI is parsed
- **THEN** `StartArgs` SHALL indicate narrow mode with `["add-auth", "fix-session", "add-logging"]`

#### Scenario: --specs with no values parses as picker

- **GIVEN** `start --specs`
- **WHEN** the CLI is parsed
- **THEN** `StartArgs` SHALL indicate picker mode

#### Scenario: --specs absent leaves spec mode unset

- **GIVEN** `start --supervisor` (no `--specs`, no `--from-all-specs`)
- **WHEN** the CLI is parsed
- **THEN** `StartArgs` SHALL indicate no spec mode (falls through to standard branch selection)

### Requirement: --from-all-specs and --specs are mutually exclusive

The system SHALL reject any invocation that combines `--from-all-specs` (or its alias `--from-specs`) with `--specs`. clap's parse step SHALL produce an error before the command runs. The error message SHALL clearly state that the two flags express opposing intents and SHALL list both flags.

#### Scenario: --from-all-specs and --specs together are rejected

- **GIVEN** `start --from-all-specs --specs add-auth`
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail with an error mentioning both `--from-all-specs` and `--specs`

#### Scenario: --from-specs alias and --specs together are also rejected

- **GIVEN** `start --from-specs --specs add-auth`
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail with an error mentioning both flags
- **AND** the alias SHALL enforce the same mutual-exclusion rule as the canonical name

### Requirement: --no-supervisor flag

The `start` subcommand SHALL accept a `--no-supervisor` flag (boolean, default `false`). When passed, the parsed `StartArgs` SHALL have `no_supervisor: bool` set to `true`. The flag SHALL appear in `git paw start --help` output with a description that names the use case (overriding `[supervisor] enabled = true` for a single session).

#### Scenario: --no-supervisor sets the flag

- **GIVEN** the user invokes `git paw start --no-supervisor`
- **WHEN** the CLI is parsed
- **THEN** the parsed `StartArgs.no_supervisor` SHALL be `true`
- **AND** `StartArgs.supervisor` SHALL be `false`

#### Scenario: --no-supervisor absent leaves flag false

- **GIVEN** the user invokes `git paw start` with neither `--supervisor` nor `--no-supervisor`
- **WHEN** the CLI is parsed
- **THEN** `StartArgs.no_supervisor` SHALL be `false`
- **AND** `StartArgs.supervisor` SHALL be `false`

#### Scenario: --no-supervisor appears in help output

- **WHEN** `git paw start --help` is run
- **THEN** the output contains `--no-supervisor`
- **AND** the output describes the flag as disabling supervisor for the session and overriding any `[supervisor] enabled = true` config setting

### Requirement: --supervisor and --no-supervisor are mutually exclusive

The system SHALL reject any invocation that combines `--supervisor` and `--no-supervisor` on the same `start` command. clap's parse step SHALL produce an error before the command runs. The error message SHALL clearly state that the two flags express opposing intents and SHALL list both.

#### Scenario: Both flags together are rejected

- **GIVEN** `start --supervisor --no-supervisor`
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail with an error mentioning both `--supervisor` and `--no-supervisor`

#### Scenario: --no-supervisor combines with other flags

- **GIVEN** `start --no-supervisor --cli claude --branches feat/a,feat/b`
- **WHEN** the CLI is parsed
- **THEN** `no_supervisor` SHALL be `true`
- **AND** `cli` SHALL be `Some("claude")`
- **AND** `branches` SHALL contain `feat/a` and `feat/b`
- **AND** parsing SHALL succeed

### Requirement: Pause subcommand

The `pause` subcommand SHALL parse with no additional arguments and SHALL be visible in `git paw --help` output. The subcommand SHALL include an `about` string ("Pause the session (detaches client, stops broker, leaves CLIs running)") and a `long_about` string that names the RAM trade-off and points the reader at `stop` and (forthcoming v1.0.0) `hibernate` for the destructive and RAM-free alternatives respectively.

The pause subcommand SHALL appear in the root `after_help` quick-start guide alongside `start`, `stop`, and `purge`.

#### Scenario: Pause parses

- **GIVEN** `pause` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Pause`

#### Scenario: Pause accepts no flags

- **GIVEN** `pause --anything` is passed (any flag)
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail with an unknown-argument error

#### Scenario: Pause appears in help

- **WHEN** `git paw --help` is run
- **THEN** the output SHALL list a `pause` subcommand
- **AND** the output SHALL include the `pause` line in the quick-start `after_help` block

#### Scenario: Pause help text names the RAM trade-off

- **WHEN** `git paw pause --help` is run
- **THEN** the output SHALL mention that CLI processes remain running
- **AND** the output SHALL mention the RAM-allocation trade-off (or words conveying "RAM stays held")
- **AND** the output SHALL suggest `git paw stop` for the RAM-releasing alternative

### Requirement: `git paw purge` interactive confirmation SHALL honour `y`+Enter under all conditions

The `cmd_purge` interactive confirmation prompt SHALL be reliably readable by the dialoguer `Confirm` widget regardless of preceding stderr output. When the unmerged-commits warning has been written to stderr immediately before the prompt, the warning writer SHALL flush stderr before the prompt's `interact()` call begins, so the user's `y`+Enter input is not racing the warning's buffered bytes.

#### Scenario: Purge with unmerged commits and `y`+Enter proceeds

- **GIVEN** a session with at least one branch carrying commits not in `main`
- **AND** `git paw purge` is invoked from a TTY
- **WHEN** the prompt "Purge is irreversible. Continue?" appears and the user types `y` followed by Enter
- **THEN** the purge SHALL proceed (kill tmux session + remove worktrees + delete session JSON)
- **AND** the exit code SHALL be 0
- **AND** stdout SHALL contain `Purged session 'paw-...'`

#### Scenario: Purge with unmerged commits and `n`+Enter cancels

- **GIVEN** same setup as above
- **WHEN** the user types `n` followed by Enter
- **THEN** the purge SHALL NOT proceed
- **AND** the exit code SHALL be 0
- **AND** stdout SHALL contain `Purge cancelled.`
- **AND** the session worktrees SHALL still be on disk

#### Scenario: Purge with bare Enter (no y/n) defaults to no

- **GIVEN** same setup
- **WHEN** the user types Enter without first typing `y` or `n`
- **THEN** the prompt SHALL default to false (No)
- **AND** the purge SHALL NOT proceed
- **AND** stdout SHALL contain `Purge cancelled.`

### Requirement: `git paw purge --force` SHALL propagate `--force` to `git worktree remove` and emit per-worktree progress

When `git paw purge` is invoked with `--force`, the underlying `git worktree remove` invocations SHALL pass `--force` so the removal succeeds on worktrees with uncommitted changes. The command SHALL also emit per-worktree progress messages to stderr (e.g. `Removing worktree <path>...` before each removal and `done (<elapsed>s)` after) so the user can distinguish a slow-but-progressing removal from an actual hang.

#### Scenario: `purge --force` removes dirty worktrees

- **GIVEN** a session with one worktree containing uncommitted edits
- **WHEN** `git paw purge --force` is invoked
- **THEN** the dirty worktree SHALL be removed successfully
- **AND** the exit code SHALL be 0
- **AND** the underlying `git worktree remove` invocation SHALL include the `--force` flag

#### Scenario: `purge --force` emits per-worktree progress to stderr

- **GIVEN** a session with two or more worktrees
- **WHEN** `git paw purge --force` is invoked
- **THEN** stderr SHALL contain a `Removing worktree <path>...` line for each worktree being removed
- **AND** stderr SHALL contain a `done` or completion marker after each removal
- **AND** the order SHALL match the worktree iteration order

#### Scenario: `purge` without `--force` does NOT pass `--force` to `git worktree remove`

- **GIVEN** a session with one worktree containing uncommitted edits
- **WHEN** `git paw purge` (no `--force`) is invoked and the user confirms with `y`
- **THEN** the underlying `git worktree remove` SHALL NOT include the `--force` flag
- **AND** if `git worktree remove` fails because of the dirty state, the failure SHALL be reported to stderr as `warning: failed to remove worktree '<path>': <git error>` per the existing error-handling path
- **AND** purge SHALL continue with the remaining worktrees

### Requirement: `git paw stop` and `git paw purge` SHALL strip the supervisor boot-block injection from AGENTS.md

`cmd_stop` and `cmd_purge` (`src/main.rs`) SHALL invoke a helper that removes the supervisor-pane boot-block injection block from `<repo>/AGENTS.md`. The block is bounded by HTML comment markers `<!-- git-paw:start -->` ... `<!-- git-paw:end -->` (or similar — the actual marker strings are owned by the injection code path and SHALL match exactly). The helper SHALL be idempotent and SHALL preserve all surrounding content byte-for-byte.

#### Scenario: Stop strips the boot-block injection

- **GIVEN** a session in which `cmd_supervisor` or `cmd_start` injected a `<!-- git-paw:start -->`...`<!-- git-paw:end -->` block into `AGENTS.md`
- **WHEN** `git paw stop` (with or without `--force`) is invoked
- **AND** the teardown completes successfully
- **THEN** the resulting `AGENTS.md` SHALL contain no `<!-- git-paw:start -->` marker
- **AND** no `<!-- git-paw:end -->` marker

#### Scenario: Purge strips the boot-block injection

- **GIVEN** the same setup
- **WHEN** `git paw purge` (with or without `--force`) is invoked
- **THEN** `AGENTS.md` SHALL contain neither marker after the purge completes

#### Scenario: Stop/purge on AGENTS.md without markers is a no-op

- **GIVEN** an `AGENTS.md` with no `<!-- git-paw:start -->` marker
- **WHEN** `git paw stop` or `git paw purge` runs the cleanup helper
- **THEN** `AGENTS.md` SHALL be byte-identical to its pre-cleanup state
- **AND** the helper SHALL return success

### Requirement: `git paw init` SHALL be idempotent and additive on existing config files

`src/init.rs::run_init` SHALL parse the existing `.git-paw/config.toml` (if any) and compare its top-level keys/tables against the bundled-default schema. The init flow SHALL append commented stanzas ONLY for keys/tables missing from the user's config. It SHALL NEVER:

1. Modify the value of an existing key.
2. Add a second occurrence of any top-level table (e.g. a second `[supervisor]`) when the user already has that section commented OR uncommented.
3. Re-order or reformat existing keys/sections.
4. Strip existing user comments or blank lines.

When every bundled-default key is already present in the user's config, init SHALL print `config.toml already has all default keys; no changes` and return Ok without writing.

Init invocations SHALL be idempotent: running `git paw init` a second time on a config that the first run produced SHALL leave the file byte-identical.

#### Scenario: First init writes a complete commented default config

- **GIVEN** a fresh repo with no `.git-paw/config.toml`
- **WHEN** `git paw init` is invoked
- **THEN** the file SHALL be created
- **AND** SHALL parse as valid TOML
- **AND** SHALL contain commented stanzas for every bundled-default top-level key/section

#### Scenario: Second init on the just-written file is a no-op

- **GIVEN** the same repo after the first init
- **WHEN** `git paw init` is invoked again
- **THEN** the file SHALL be byte-identical to the first-run output
- **AND** the exit SHALL be 0

#### Scenario: Init preserves a user-authored `[supervisor]` block

- **GIVEN** a `.git-paw/config.toml` containing only:
  ```toml
  [supervisor]
  enabled = true
  cli = "claude-oss"
  ```
- **WHEN** `git paw init` is invoked
- **THEN** the resulting file SHALL contain `enabled = true` and `cli = "claude-oss"` byte-identical to the input
- **AND** SHALL NOT contain a second `[supervisor]` section header (commented or uncommented)
- **AND** SHALL parse as valid TOML (no `duplicate key` errors)

#### Scenario: Init appends missing top-level sections

- **GIVEN** a `.git-paw/config.toml` containing only `branch_prefix = "feat/"`
- **WHEN** `git paw init` is invoked
- **THEN** the resulting file SHALL preserve `branch_prefix = "feat/"` byte-identical
- **AND** SHALL gain commented stanzas for every bundled-default section the user is missing (`[broker]`, `[dashboard]`, `[supervisor]`, etc.)
- **AND** SHALL parse as valid TOML

#### Scenario: Init never modifies existing user values

- **GIVEN** a `.git-paw/config.toml` with `[broker] port = 9200` (non-default port)
- **WHEN** `git paw init` is invoked
- **THEN** the resulting file SHALL still have `port = 9200`
- **AND** SHALL NOT introduce a second `port` key or a commented `# port = 9119` stanza inside `[broker]`

### Requirement: --unattended flag

The `start` subcommand SHALL accept an `--unattended` flag (boolean, default `false`). When passed, the parsed `StartArgs` SHALL have `unattended: bool` set to `true`. When omitted, `unattended` SHALL be `false`.

The flag SHALL appear in `git paw start --help` output with a description naming the use case: run a supervisor wave to completion with no human babysitting (auto-approve classifier-safe prompts, escalate the rest for later review, detect completion, exit with a summary).

`--unattended` engages supervisor mode: when `--unattended` is passed, the supervisor-mode resolution chain SHALL resolve supervisor mode active (equivalent to `--supervisor`) so the dispatch routes to the supervisor launch path. `--unattended` MAY be combined with `--supervisor`, `--from-all-specs` (and its alias `--from-specs`), `--specs`, `--cli`, and `--branches`.

`--unattended` SHALL NOT be combined with `--no-supervisor`; the system SHALL reject any invocation that combines `--unattended` with `--no-supervisor` with a parse error naming both flags, because they express opposing intents.

#### Scenario: --unattended sets the flag

- **GIVEN** the user invokes `git paw start --unattended`
- **WHEN** the CLI is parsed
- **THEN** the parsed `StartArgs.unattended` SHALL be `true`

#### Scenario: --unattended absent leaves flag false

- **GIVEN** the user invokes `git paw start` without `--unattended`
- **WHEN** the CLI is parsed
- **THEN** `StartArgs.unattended` SHALL be `false`

#### Scenario: --unattended combines with other flags

- **GIVEN** `start --unattended --from-specs --cli claude`
- **WHEN** the CLI is parsed
- **THEN** `unattended` SHALL be `true`
- **AND** the spec-mode launch-all state SHALL be set
- **AND** `cli` SHALL be `Some("claude")`
- **AND** parsing SHALL succeed

#### Scenario: --unattended appears in help output

- **WHEN** `git paw start --help` is run
- **THEN** the output SHALL contain `--unattended`
- **AND** the output SHALL describe the flag as running the wave to completion without human babysitting

#### Scenario: --unattended engages supervisor mode

- **GIVEN** `git paw start --unattended --branches feat/a,feat/b` is invoked
- **WHEN** the dispatch resolves the supervisor-mode resolution chain
- **THEN** supervisor mode SHALL resolve active
- **AND** the dispatch SHALL route to the supervisor launch path (`cmd_supervisor`)

#### Scenario: --unattended with --no-supervisor is rejected

- **GIVEN** `start --unattended --no-supervisor`
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail with an error mentioning both `--unattended` and `--no-supervisor`

