## MODIFIED Requirements

### Requirement: Stop subcommand

The `stop` subcommand SHALL accept an optional `--force` flag (boolean, defaults to `false`).
`stop` is **non-destructive** — it kills the session's CLI processes but preserves all worktrees
and session state, and is recoverable via `git paw start` — so `cmd_stop` SHALL NOT render a
confirmation prompt; the stop SHALL proceed immediately regardless of `--force` and regardless of
whether stdin is a TTY. The `--force` flag is accepted for backward-compatibility and symmetry with
`purge` but is currently a **no-op** (reserved).

The `long_about` help text for `stop` SHALL name all three teardown verbs (`pause`, `stop`, `purge`)
with a one-line summary of each, so users can choose the right verb at `--help` time, and SHALL note
that `--force` is a no-op for `stop` because `stop` is non-destructive.

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

#### Scenario: Stop never renders a confirmation prompt

- **GIVEN** an active session
- **WHEN** `git paw stop` is run — with or without `--force`, with stdin attached to a TTY or not
- **THEN** no interactive prompt SHALL be rendered
- **AND** the session's CLI processes SHALL be killed immediately
- **AND** the worktrees and session state SHALL be preserved
