# selftest Specification

## Purpose
Provides `git paw selftest`, which runs an isolated end-to-end session lifecycle (start → add/remove roster transitions → stop) against a throwaway repo under `.git-paw/tmp/` using a dummy CLI, a private tmux socket, and an ephemeral broker port, then reports a single pass/fail verdict without any real LLM backend or interactive terminal.

## Requirements
### Requirement: `git paw selftest` subcommand

The CLI SHALL provide a `selftest` subcommand that runs an isolated, end-to-end session lifecycle against a throwaway repository and a dummy CLI, then reports a single pass/fail verdict. The subcommand SHALL parse with no required arguments and SHALL exit `0` when the lifecycle completes successfully and non-zero when any lifecycle step fails.

The subcommand SHALL appear in `git paw --help` output and SHALL carry an `about` string and a `long_about` string with at least one usage example, matching the project's CLI help conventions.

The lifecycle SHALL NOT require a real AI CLI backend (LLM), SHALL NOT require an interactive terminal, and SHALL NOT touch the user's default tmux socket, real sessions directory, or live `paw-*` sessions.

#### Scenario: selftest parses with no arguments

- **GIVEN** `selftest` is passed to the CLI
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Selftest`

#### Scenario: selftest appears in help output

- **WHEN** `git paw --help` is run
- **THEN** stdout SHALL list a `selftest` subcommand with a description

#### Scenario: selftest help text describes the isolated lifecycle

- **WHEN** `git paw selftest --help` is run
- **THEN** stdout SHALL describe that the command runs an isolated session lifecycle with a dummy CLI and no real LLM backend
- **AND** stdout SHALL contain at least one usage example

#### Scenario: selftest reports pass and exits zero on a healthy build

- **GIVEN** tmux is available on PATH
- **WHEN** `git paw selftest` is run
- **THEN** the command SHALL exit with status `0`
- **AND** stdout SHALL contain a pass indication (e.g. "selftest passed")

#### Scenario: selftest reports failure and exits non-zero when a lifecycle step fails

- **GIVEN** a lifecycle step (start, roster check, or stop) fails during the run
- **WHEN** `git paw selftest` completes
- **THEN** the command SHALL exit with a non-zero status
- **AND** stderr SHALL name the failing step

### Requirement: selftest harness runs an isolated session lifecycle with a dummy CLI

The `selftest` harness SHALL exercise a full session lifecycle without launching any real AI agent. It SHALL:

- Create a throwaway git repository under `.git-paw/tmp/` (a path inside the current repository's git-paw working tree) so the selftest artifacts are namespaced and easily cleaned up.
- Launch the session using a dummy CLI command (e.g. `cat` or `sh`) in place of a real AI CLI, so no LLM process is spawned and the session boots deterministically in detached (non-TTY) mode.
- Drive the lifecycle steps `start` → roster check → `stop` (or `purge`) against the throwaway repo.
- Clean up all artifacts it created (the throwaway repo under `.git-paw/tmp/`, the tmux session on its private socket) on both the success and failure paths.

The harness SHALL report which lifecycle step failed when the verdict is fail.

#### Scenario: selftest boots the session with a dummy CLI, not a real LLM

- **GIVEN** `git paw selftest` is run
- **WHEN** the harness launches the session
- **THEN** the session SHALL be launched with a dummy CLI command (e.g. `cat`/`sh`)
- **AND** no real AI CLI process SHALL be spawned

#### Scenario: selftest exercises a full start → stop lifecycle

- **GIVEN** tmux is available
- **WHEN** `git paw selftest` runs
- **THEN** the harness SHALL start a session against the throwaway repo
- **AND** SHALL tear the session down (stop or purge) before reporting its verdict

#### Scenario: selftest namespaces its throwaway repo under .git-paw/tmp/

- **GIVEN** `git paw selftest` is run from within a git repository
- **WHEN** the harness creates its throwaway repository
- **THEN** the throwaway repository SHALL live under `.git-paw/tmp/`
- **AND** the harness SHALL remove that directory after the run completes (pass or fail)

### Requirement: selftest verifies add/remove roster transitions

The `selftest` harness SHALL exercise the session-management roster transitions that previously required a live session to validate. After starting a session it SHALL add at least one agent worktree, observe that the roster grows, then remove that agent worktree and observe that the roster shrinks, asserting the observable roster state at each transition. This SHALL close git-paw-add's deferred live-verification by making the add/remove transitions observable through `git paw selftest` with no real LLM backend.

#### Scenario: selftest observes the roster grow on add

- **GIVEN** an isolated selftest session has started with an initial roster
- **WHEN** the harness performs an add of a new agent worktree
- **THEN** the observed roster SHALL include the newly added agent

#### Scenario: selftest observes the roster shrink on remove

- **GIVEN** an isolated selftest session with an added agent worktree
- **WHEN** the harness performs a remove of that agent worktree
- **THEN** the observed roster SHALL NOT include the removed agent
- **AND** the remaining roster entries SHALL be unchanged

### Requirement: selftest uses a private tmux socket and an ephemeral broker port

The `selftest` harness SHALL isolate its tmux and broker resources so it never collides with the user's live session or with a concurrently running selftest. Specifically the harness SHALL:

- Use a private tmux socket dedicated to this run, established via a per-run `TMUX_TMPDIR` tempdir (equivalently `tmux -L <unique-name>`), NOT the user's default tmux socket.
- Remove the `TMUX` and `TMUX_PANE` environment variables from the child process environment so the spawned tmux process does not detect itself as running inside the caller's existing tmux session and attach to the parent's server.
- Allocate the broker port as an OS-assigned ephemeral port (bind `127.0.0.1:0` and read back the assigned port) rather than a fixed or PID-derived port, so concurrent selftest runs never collide on the broker port.

#### Scenario: selftest session lands on its private socket, not the default socket

- **GIVEN** `git paw selftest` is run
- **WHEN** the harness boots its tmux session
- **THEN** the session SHALL appear only on the harness's private tmux socket
- **AND** SHALL NOT appear on the user's default tmux socket

#### Scenario: selftest strips TMUX and TMUX_PANE from the child environment

- **GIVEN** `git paw selftest` is run from inside an existing tmux session (with `TMUX` and `TMUX_PANE` set)
- **WHEN** the harness spawns its tmux/`git paw` child processes
- **THEN** those child processes SHALL have `TMUX` and `TMUX_PANE` removed from their environment
- **AND** the harness session SHALL still boot on its own private socket

#### Scenario: selftest claims an OS-assigned ephemeral broker port

- **GIVEN** two `git paw selftest` runs are launched concurrently
- **WHEN** each harness allocates its broker port
- **THEN** each SHALL bind `127.0.0.1:0` to obtain a distinct OS-assigned port
- **AND** neither run SHALL fail with an "address already in use" broker-port error

