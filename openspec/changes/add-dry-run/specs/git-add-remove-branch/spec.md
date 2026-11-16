## ADDED Requirements

### Requirement: add --dry-run refuses exactly where a real add refuses

`git paw add --dry-run` SHALL resolve the session, its effective status, the branch or `--from-spec` spec, the worker and its CLI, the broker agent-id clash and the agent cap exactly as `git paw add` does, and SHALL refuse in every case the real add refuses before it changes anything, with the same error text and a non-zero exit code.

#### Scenario: No session is refused as in a real add

- **GIVEN** a repository with no git-paw session
- **WHEN** the user runs `git paw add feat/x --dry-run`
- **THEN** the command SHALL exit non-zero with the same "no active session" error a real `add` prints

#### Scenario: A stopped session is refused

- **GIVEN** a stopped session
- **WHEN** the user runs `git paw add feat/x --dry-run`
- **THEN** the command SHALL refuse naming the session and `git paw start`, as a real `add` does

#### Scenario: A branch that is already an agent is refused

- **GIVEN** an active session in which `feat/x` is an agent
- **WHEN** the user runs `git paw add feat/x --dry-run`
- **THEN** the command SHALL refuse with the "already an agent" error

#### Scenario: An unknown CLI is refused

- **GIVEN** an active session and no CLI named `nonesuch` detected or in the session
- **WHEN** the user runs `git paw add feat/x --cli nonesuch --dry-run`
- **THEN** the command SHALL refuse listing the detected CLIs

### Requirement: add --dry-run prints the plan in start --dry-run's shape

On success `git paw add --dry-run` SHALL print to stdout a `Dry run — add plan:` header, a `Session:` line, a `Status:` line only when the session is paused (saying the agent starts on the next `git paw start`), then `<branch> → <worker> (<worktree path>)` and the worker's `runs:` line. The worker label, worktree path and `runs:` line SHALL be built by the same functions `start --dry-run` uses, and the command SHALL exit 0.

#### Scenario: Active session plan

- **GIVEN** an active session `paw-proj` and a detected `claude` CLI
- **WHEN** the user runs `git paw add feat/x --cli claude --dry-run`
- **THEN** stdout SHALL contain `Dry run — add plan:`, `Session:  paw-proj`, a line `feat/x → claude (` followed by its worktree path, and a `runs:` line containing the `claude` launch command
- **AND** stdout SHALL NOT contain a `Status:` line

#### Scenario: Paused session plan

- **GIVEN** a paused session
- **WHEN** the user runs `git paw add feat/x --dry-run`
- **THEN** the plan SHALL contain a `Status:` line saying the agent starts on the next `git paw start`

#### Scenario: Profile worker matches start's preview

- **GIVEN** a worker profile `routine` on CLI `claude` with `args = "--model small"`
- **WHEN** the user runs `git paw add feat/x --profile routine --dry-run`
- **THEN** the worker line SHALL read `feat/x → routine [claude] (…)`
- **AND** the `runs:` line SHALL be byte-identical to the one `git paw start --branches feat/x --profile routine --dry-run` prints for `feat/x`

### Requirement: add --dry-run changes nothing

`git paw add --dry-run` SHALL NOT create, rebase or provision a worktree, write `AGENTS.md` or hooks, create or re-tile a pane, write the session file or the per-repo discovery file, contact the broker, or type into any pane. It SHALL NOT take the repository's add/remove lock.

#### Scenario: Nothing is created

- **GIVEN** an active session with two agents
- **WHEN** the user runs `git paw add feat/x --dry-run`
- **THEN** no worktree for `feat/x` SHALL exist, the session file SHALL be byte-identical to before, and the session SHALL still have two agent panes

#### Scenario: A held add/remove lock does not block a preview

- **GIVEN** another `git paw add` holding the repository's add/remove lock
- **WHEN** the user runs `git paw add feat/y --dry-run`
- **THEN** the command SHALL print its plan and SHALL NOT fail with "operation in progress"

### Requirement: add --dry-run --json reports the plan as the add document

With `--json`, `git paw add --dry-run` SHALL print the `cli-machine-output` `add` document with `dry_run` `true`, `worktree_path` the absolute path the worktree would be created at, and `launch_command` the command the `runs:` line shows, and SHALL print nothing else on stdout. A real `git paw add --json` SHALL carry `dry_run` `false` and the `launch_command` it ran.

#### Scenario: Dry-run JSON

- **GIVEN** an active session
- **WHEN** the user runs `git paw add feat/x --cli claude --dry-run --json`
- **THEN** stdout SHALL be one JSON object with `schema_version` 1, `branch` `"feat/x"`, `cli` `"claude"`, `dry_run` `true`, `paused` `false`, an absolute `worktree_path` and a non-empty `launch_command`

#### Scenario: Real add JSON carries dry_run false

- **GIVEN** an active session
- **WHEN** `git paw add feat/x --cli claude --json` succeeds
- **THEN** the document SHALL carry `dry_run` `false` and a `launch_command`

#### Scenario: A refused dry run prints nothing on stdout

- **GIVEN** no session for the repository
- **WHEN** the user runs `git paw add feat/x --dry-run --json`
- **THEN** stdout SHALL be empty and the command SHALL exit non-zero
