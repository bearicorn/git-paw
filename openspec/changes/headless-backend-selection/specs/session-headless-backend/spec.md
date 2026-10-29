## ADDED Requirements

### Requirement: A worker runs headless only when its CLI passes the eligibility gate

Before creating any worktree, branch, session file, run state or broker for a headless launch — and before attaching a worker to a headless session with `git paw add` or recovering one — the system SHALL decide, for every worker, whether it may run headless, using a pure function of the worker's resolved CLI record, its effective worker approval level and the support matrix. A worker SHALL be eligible only when all of these hold: (a) its CLI resolves a headless form, a session-id template and a resume template; (b) its effective approval level is not `manual`; (c) that level resolves a non-empty set of approval flags for its CLI, from the built-in table or an `approval_args` override; and (d) when that level is `full-auto`, its CLI declares `sandbox = "native"` or `sandbox = "wrapper"`. When any worker is refused, the command SHALL create nothing, SHALL print every refused worker with each failed condition and the configuration key that would satisfy it, and SHALL exit with code 1. The system SHALL NOT raise a worker's approval level, add a permission-skipping flag, or treat a CLI as sandboxed on the headless backend's behalf.

#### Scenario: The gate decides from the record and level

- **WHEN** eligibility is decided for each of these workers
  - a `claude`-family CLI with built-in templates at `auto`
  - the same CLI at `manual`
  - the same CLI at `full-auto` with no `sandbox`
  - the same CLI at `full-auto` with `sandbox = "wrapper"`
  - a `generic` CLI with no templates at `auto`
  - a `generic` CLI with `headless_args`, `session_id_args` and `resume_args` overrides at `auto` and no `approval_args`
- **THEN** the results SHALL be: eligible; refused for `manual`; refused for a missing sandbox; eligible; refused naming the three missing templates; and refused naming `approval_args`, respectively

#### Scenario: A refusal creates nothing

- **GIVEN** a repository with no session and a config whose worker CLI has no headless form
- **WHEN** `git paw start --backend headless --cli my-agent --branches feat/a` runs
- **THEN** the process SHALL exit with code 1
- **AND** stderr SHALL name `my-agent`, the missing headless form and `[clis.my-agent].headless_args`
- **AND** no worktree, branch, session file, run state or broker process SHALL exist afterwards

#### Scenario: Every refusal is reported at once

- **GIVEN** a headless launch of two workers, one at a CLI with no headless form and one at a CLI with no approval flags
- **WHEN** the launch runs
- **THEN** stderr SHALL name both workers and both conditions in a single run

#### Scenario: Full-auto is never added

- **GIVEN** an eligible worker at `auto`
- **WHEN** its turn plan is built
- **THEN** neither command SHALL contain the CLI's `full-auto` approval flags

#### Scenario: Hot-attach applies the gate

- **GIVEN** a running headless session
- **WHEN** `git paw add feat/c --cli my-agent` runs for a CLI with no headless form
- **THEN** it SHALL exit with code 1 naming the condition
- **AND** no worktree for `feat/c` SHALL be created

### Requirement: Built-in headless facts require matrix evidence

When all three of a worker's headless form, session-id template and resume template come from its CLI family's built-in rows, the worker SHALL be eligible only if the support-matrix row for that family records `headless-worker` as `verified` or `gap`; a `gap` SHALL be printed as a launch notice naming the gap. When any of the three comes from the user's `[clis.<name>]` configuration, the CLI SHALL be treated as user-declared: it SHALL NOT be refused for lack of matrix evidence, and the launch SHALL print a notice that git-paw has not verified this CLI's headless operation.

#### Scenario: An unverified built-in family is refused

- **GIVEN** a matrix in which the `claude` row records `headless-worker` as `unverified`
- **WHEN** eligibility is decided for a `claude` worker at `auto` with no template overrides
- **THEN** it SHALL be refused naming the missing evidence

#### Scenario: A user-declared CLI launches with a notice

- **GIVEN** `[clis.my-agent]` with `headless_args`, `session_id_args`, `resume_args` and `approval_args` for `auto`
- **WHEN** a headless launch of a `my-agent` worker at `auto` runs
- **THEN** the worker SHALL be launched
- **AND** stderr SHALL carry a notice that its headless operation is not verified by git-paw

### Requirement: An auto-level headless worker in a broker session launches with a helper-call notice

When a headless launch admits a worker at the `auto` level in a session with the Broker layer, the launch SHALL print one notice per such worker's CLI stating that the CLI's own non-interactive policy may refuse the helper-script or MCP calls the worker uses to publish its status, and naming how to allow them: `git paw setup-cli <cli> --allowlist` when that CLI's setup fact row declares the allowlist item, and otherwise the CLI's own configuration or `[clis.<name>].args`. The notice SHALL NOT refuse the launch. The eligibility gate SHALL NOT require a `git paw setup-cli` record, and the launch SHALL NOT write any agent CLI settings file.

#### Scenario: The notice names setup-cli for claude

- **GIVEN** `[broker] enabled = true`, a `claude` worker at `auto` that passes the eligibility gate, and no `git paw setup-cli` record
- **WHEN** `git paw start --backend headless --branches feat/a` runs
- **THEN** the launch SHALL succeed
- **AND** its output SHALL carry one notice naming `git paw setup-cli claude --allowlist`
- **AND** no `.claude/settings.local.json` SHALL be created or modified

#### Scenario: No notice at full-auto

- **GIVEN** `[broker] enabled = true` and a `claude` worker at `full-auto` with `sandbox = "wrapper"` that passes the eligibility gate
- **WHEN** a headless launch runs
- **THEN** no helper-call notice SHALL be printed

### Requirement: The turn plan is composed by the one launch builder

The system SHALL build a headless worker's turn plan with the same launch-command builder every other worker launch uses, in this order: the CLI's executable, its `args`, its headless form, its sandbox arguments, the approval flags resolved for the effective level, and then the rendered session-id arguments for the first command or the rendered resume arguments for the continuation command. Both commands SHALL carry the same session identifier, which SHALL be recorded in the worktree's `cli_session_id`. An empty part SHALL contribute no separator.

#### Scenario: A Claude-family plan

- **GIVEN** a worker using `claude` with no overrides at `auto` and assigned session id `<uuid>`
- **WHEN** its turn plan is built
- **THEN** the first command SHALL be the quoted executable followed by `-p`, `--permission-mode acceptEdits` and `--session-id <uuid>`, in that order
- **AND** the continuation command SHALL be the same with `--resume <uuid>` in place of `--session-id <uuid>`
- **AND** the saved worktree entry SHALL have `cli_session_id = Some(<uuid>)`

#### Scenario: Args precede the headless form and the sandbox argument appears once

- **GIVEN** `[clis.cx] command = "codex"`, `family = "codex"`, `args = "--model m"`, `sandbox = "native"`, and user-declared `headless_args`, `session_id_args` and `resume_args`
- **WHEN** its turn plan is built at `auto`
- **THEN** the first command SHALL contain `--model m` before the headless form
- **AND** it SHALL contain `--sandbox workspace-write` exactly once

### Requirement: A headless session launches without tmux, a dashboard surface or a readiness wait

A headless launch SHALL NOT require `tmux` to be installed and SHALL NOT invoke it. It SHALL open the headless session, set the session environment, start the broker host as a detached service and wait (at most 10 seconds) for its `/status` when the broker is enabled, and then for each worker spawn its surface with its turn plan, attach capture to the session record when capture is on, and submit the worker's boot prompt as its first message. It SHALL create no dashboard surface and SHALL NOT run the launch-readiness gate. It SHALL then save the session and print `git paw status` and `git paw attach` as the next steps. `git paw start --dry-run` with the headless backend SHALL print the backend, each worker's eligibility and each worker's turn plan, and SHALL create nothing.

#### Scenario: Headless launch with no tmux on PATH

- **GIVEN** a repository, an eligible stub CLI, the broker enabled and a `PATH` with no `tmux`
- **WHEN** `git paw start --backend headless --cli stub --branches feat/a,feat/b` runs
- **THEN** it SHALL exit with code 0
- **AND** both workers SHALL have run a first turn whose standard input was their boot prompt
- **AND** the broker SHALL answer `/status`

#### Scenario: Capture holds the first turn

- **GIVEN** `[logging] enabled = true` and an eligible stub CLI that prints `booted`
- **WHEN** a headless session is launched
- **THEN** each worker's capture file in the session record SHALL contain `booted`

#### Scenario: Dry run creates nothing

- **WHEN** `git paw start --backend headless --dry-run --cli stub --branches feat/a` runs
- **THEN** stdout SHALL name the backend, `feat/a`'s eligibility and its two turn commands
- **AND** no worktree, session file or run state SHALL exist afterwards

### Requirement: Starting an existing headless session revives it

When `git paw start` runs in a repository whose session is on the headless backend, the system SHALL use the recorded backend; a `--backend` naming a different backend SHALL fail with exit code 1 and an error naming both backends. For a live headless session it SHALL deliver the bundled continuation message to every worker whose activity is interrupted, start the broker host if it is not alive, and then, in an interactive terminal, follow the workers' output as `git paw attach` does; in a non-interactive context it SHALL do the same revival and then refuse to fork, as for tmux. For a headless session whose run state is gone, it SHALL re-open the session and respawn each worker's surface with its recorded session identifier, delivering the continuation message as its first message.

#### Scenario: Interrupted workers are resumed

- **GIVEN** a live headless session with worker `feat-a` interrupted and worker `feat-b` awaiting input
- **WHEN** `git paw start` runs
- **THEN** `feat-a` SHALL run a turn whose message is the continuation text
- **AND** `feat-b` SHALL run no turn

#### Scenario: A conflicting backend is refused

- **GIVEN** a live headless session for the repository
- **WHEN** `git paw start --backend tmux` runs
- **THEN** it SHALL exit with code 1 naming `headless` and `tmux`
- **AND** no tmux session SHALL be created

#### Scenario: Lost run state is recovered with resume

- **GIVEN** a saved headless session whose run state was deleted and whose worker `feat-a` has `cli_session_id = Some(<uuid>)`
- **WHEN** `git paw start` runs
- **THEN** `feat-a`'s first new turn SHALL run its continuation command carrying `<uuid>`

### Requirement: Lifecycle commands act through the session's recorded backend

`git paw add`, `remove`, `pause`, `stop`, `purge`, `status` and `attach` SHALL construct the terminal backend from the session's recorded backend and SHALL NOT require `tmux` for a headless session. For a headless session, `remove` SHALL kill the worker's surface before removing its worktree; `pause` SHALL stop the broker host service and SHALL NOT stop running turns; `stop` SHALL close the headless session, keeping the session file for recovery; and `purge` SHALL close the session and remove its run state together with the rest of the session's state.

#### Scenario: Stop ends every headless process

- **GIVEN** a headless session with a busy worker and the broker host running
- **WHEN** `git paw stop` runs
- **THEN** no turn runner, CLI or broker host process of the session SHALL remain
- **AND** the session file SHALL record the session as stopped

#### Scenario: Pause keeps turns running

- **GIVEN** a headless session with a busy worker
- **WHEN** `git paw pause` runs
- **THEN** the broker port SHALL be free
- **AND** the busy worker's turn SHALL end normally

#### Scenario: Remove kills the surface first

- **GIVEN** a headless session whose worker `feat-b` is busy
- **WHEN** `git paw remove feat/b` runs
- **THEN** `feat-b`'s CLI process SHALL have exited before its worktree is removed

### Requirement: sweep.sh's terminal subcommands refuse on a headless session

The bundled `sweep.sh` SHALL read the discovery file's `backend` and, when it is `headless`, its `snapshot`, `capture`, `approve` and `detect-stuck` subcommands SHALL exit non-zero without invoking `tmux`, printing a message that names `git paw status` and `git paw attach`. Its broker subcommands SHALL behave unchanged for every backend.

#### Scenario: capture refuses on headless

- **GIVEN** a discovery file with `"backend": "headless"`
- **WHEN** `sweep.sh capture 1` runs
- **THEN** it SHALL exit non-zero, name `git paw status` and `git paw attach`, and invoke no `tmux` command

#### Scenario: A broker subcommand still works

- **GIVEN** a headless session with the broker enabled
- **WHEN** `sweep.sh status-publish` runs with valid arguments
- **THEN** it SHALL publish to the broker as it does for a tmux session

### Requirement: The headless chapter documents selection, the gate and the limits

The user guide SHALL include a headless-backend chapter that documents `--backend` and `default_backend`, every eligibility condition with the configuration key that satisfies it, the turn model (messages become turns, idle workers are woken for addressed messages, nothing is approved by git-paw), the lifecycle differences (no dashboard surface, `attach` follows output, no tmux required), that `auto` refuses every action outside the CLI's own policy, the residual risks of sandboxed `full-auto` with nobody watching, which environment a turn inherits, and that native Windows is not supported.

#### Scenario: The chapter covers the gate

- **WHEN** the headless chapter is read
- **THEN** it SHALL name each eligibility condition and its configuration key
- **AND** it SHALL state that git-paw approves nothing for a headless worker

#### Scenario: The chapter is built

- **WHEN** `mdbook build docs/` runs
- **THEN** it SHALL succeed with the headless chapter listed in `SUMMARY.md`
