## Purpose

Defines git-paw's headless terminal backend: an implementation of the terminal port in which a
worker has no terminal at all. Each worker is a conversation driven turn by turn. Every submitted
message runs one non-interactive invocation of the worker's CLI, detached from any terminal so it
survives the shell that launched it, with its output written to a stream git-paw can read, tail
and record. The backend is an opt-in alternative to tmux, chosen per session. This capability
covers how such a surface is created, how messages reach it, how its turns run, how its activity
and liveness are judged, how it is stopped, and which session-scoped state it keeps.

## ADDED Requirements

### Requirement: A headless surface is a worker conversation driven turn by turn

The headless backend SHALL implement the terminal port such that a surface is a worker conversation rather than a running process. Spawning a surface SHALL record its owner worktree, its label and its non-interactive turn plan — a *first* command for the surface's first turn and a *continuation* command for every later turn — and SHALL start no process. Each message delivered to the surface SHALL run exactly one turn: one invocation of the plan's command for that turn, in the owner worktree, with the message as its standard input. Between turns no process SHALL exist for the surface. Spawning a surface without a turn plan SHALL fail with an error naming the surface.

#### Scenario: Spawning starts no process

- **GIVEN** the headless backend and a turn plan for worktree `/repo/.git-paw/worktrees/feat-x`
- **WHEN** a surface is spawned with that plan
- **THEN** listing the session's surfaces SHALL report the surface with owner `/repo/.git-paw/worktrees/feat-x`
- **AND** no process SHALL have been started for it

#### Scenario: First and continuation turns use their own commands

- **GIVEN** a surface whose plan's first command passes a session-id argument and whose continuation command passes a resume argument, run against a stub CLI that records its arguments
- **WHEN** two messages are delivered one after the other
- **THEN** the first turn SHALL have run the first command
- **AND** the second turn SHALL have run the continuation command

#### Scenario: A spec without a turn plan is refused

- **WHEN** a surface is spawned on the headless backend without a turn plan
- **THEN** the spawn SHALL fail with an error naming the surface
- **AND** no surface SHALL be listed

### Requirement: Text and Enter deliver one message; other keys are unsupported

The headless backend SHALL treat sent text as an unsubmitted draft of the surface: `send_text` SHALL append to the draft, `send_key(Enter)` SHALL submit the whole draft as one message and empty it, and `send_key(CtrlU)` SHALL discard the draft. Every other key SHALL fail with a typed unsupported-operation error naming the backend and the key, and SHALL change nothing. The draft SHALL persist across processes, so text sent by one git-paw process can be submitted by another.

#### Scenario: Text then Enter is one message

- **GIVEN** a headless surface with an empty draft
- **WHEN** the text `please continue` is sent and then `Enter`
- **THEN** exactly one message with the content `please continue` SHALL be delivered to the surface

#### Scenario: Clearing the draft submits nothing

- **GIVEN** a headless surface whose draft holds `stale text`
- **WHEN** `CtrlU` and then `Enter` are sent
- **THEN** no message containing `stale text` SHALL be delivered

#### Scenario: A digit key is unsupported

- **GIVEN** a headless surface
- **WHEN** the key `1` is sent to it
- **THEN** the call SHALL fail with an unsupported-operation error naming the headless backend
- **AND** no message SHALL be delivered and the draft SHALL be unchanged

#### Scenario: A draft written by one process is submitted by another

- **GIVEN** a headless surface to which one git-paw process sent the text `boot prompt` without `Enter`
- **WHEN** a second git-paw process sends `Enter` to the same surface
- **THEN** one message with the content `boot prompt` SHALL be delivered

### Requirement: Messages are delivered in order, exactly once

The headless backend SHALL queue submitted messages and run them as turns in submission order, one turn at a time per surface. A message SHALL run as at most one turn. A message submitted while a turn is running SHALL run after that turn ends, without any further call. Submitting SHALL return only after a turn runner owns the surface, or SHALL fail with an error when no runner could be started within 5 seconds. A submitted message byte-identical to a message still waiting in the surface's queue SHALL be dropped. A message whose turn was started but never recorded as ended SHALL NOT be run again automatically.

#### Scenario: A message sent during a turn runs next

- **GIVEN** a surface whose stub CLI sleeps for two seconds per turn
- **WHEN** message `A` is submitted and, while its turn runs, message `B` is submitted
- **THEN** the turn for `B` SHALL start after the turn for `A` has ended
- **AND** each message SHALL have run exactly one turn

#### Scenario: Identical pending messages collapse

- **GIVEN** a surface that is busy with a turn
- **WHEN** the same message text is submitted twice before that turn ends
- **THEN** exactly one further turn SHALL run with that text

#### Scenario: An interrupted turn is not replayed

- **GIVEN** a surface whose turn was started and whose runner was killed before the turn ended
- **WHEN** a new message is submitted to the surface
- **THEN** only the new message SHALL run a turn
- **AND** the interrupted message SHALL NOT run again

### Requirement: Turns run detached from any terminal and outlive their launcher

Each turn SHALL run under a turn runner that git-paw starts as the leader of a new session, with no controlling terminal and with its standard input not connected to any terminal. The turn's standard input SHALL receive the message and then end-of-file, so a CLI that tries to read further input reads end-of-file instead of waiting. A turn SHALL keep running, and queued messages SHALL keep being delivered, after the process that submitted them exits and after the terminal it ran in is closed. The turn command SHALL be run through the system shell exactly as the tmux backend's pane would run it, and the message text SHALL never appear in any process's arguments.

#### Scenario: A turn survives the launching terminal

- **GIVEN** a headless surface submitted a message from a process attached to a pseudo-terminal, whose stub CLI writes a marker file after five seconds
- **WHEN** the submitting process exits and its pseudo-terminal is closed immediately after submitting
- **THEN** the marker file SHALL exist after the stub's delay

#### Scenario: The message is read from stdin, never argv

- **GIVEN** a stub CLI that records its arguments and its standard input
- **WHEN** a message `secret-ish text` is delivered
- **THEN** the recorded standard input SHALL equal `secret-ish text`
- **AND** no recorded argument SHALL contain `secret-ish text`

#### Scenario: A turn has no controlling terminal

- **GIVEN** a stub CLI that records whether it can open `/dev/tty`
- **WHEN** a message is delivered
- **THEN** the stub SHALL report that no controlling terminal is available

### Requirement: Turn output streams to the surface and to its capture destination

The headless backend SHALL append everything a turn writes on standard output and standard error, in order of arrival, to the surface's output stream, which `capture_tail` and `attach` read. When an output destination has been set with `stream_to`, every turn that starts afterwards SHALL also append its output to that destination. The destination SHALL be created with owner-only permissions when absent, appended to when present, and SHALL NOT be written when it is a symbolic link; a destination that cannot be written SHALL be reported in the surface's diagnostics and SHALL NOT fail the turn.

#### Scenario: Output before the first byte is captured

- **GIVEN** a surface whose output destination was set before its first message
- **WHEN** its first turn prints `hello` and exits
- **THEN** the destination SHALL contain `hello`
- **AND** the surface's output stream SHALL contain `hello`

#### Scenario: A later turn appends

- **GIVEN** a destination that already holds the output of an earlier turn
- **WHEN** another turn prints `again`
- **THEN** the destination SHALL hold the earlier content followed by `again`

#### Scenario: A symlinked destination is refused

- **GIVEN** an output destination that is a symbolic link to a file outside the session record
- **WHEN** a turn runs
- **THEN** the link target SHALL be unchanged
- **AND** the turn SHALL still complete and its output SHALL be in the surface's output stream

### Requirement: Capture reads the tail of the output stream

`capture_tail` on a headless surface SHALL return the last requested lines of the surface's output stream, across turn boundaries, and an empty result for a surface that has run no turn. It SHALL start no process and change no state.

#### Scenario: The tail spans turns

- **GIVEN** a surface whose first turn printed `one` and whose second turn printed `two`
- **WHEN** the last two lines are captured
- **THEN** the result SHALL be `one` followed by `two`

#### Scenario: A fresh surface captures nothing

- **WHEN** a surface that has run no turn is captured
- **THEN** the result SHALL be empty

### Requirement: Activity and liveness are derived from the runner and the turn record

The headless backend SHALL report each surface's activity as exactly one of: *busy* — a turn runner owns the surface, carrying the time the surface's output stream last grew; *interrupted* — a turn started and has no recorded end, or a message is waiting with no runner; *failed* — the last turn ended with a non-zero exit code or a signal, carrying that code or signal; *awaiting input* — otherwise, including a surface that has run no turn. The surface SHALL report as alive exactly when its activity is busy or awaiting input. The determination SHALL be a pure function of the observed lock, turn record and queue, and observing a surface SHALL start no process and change no state.

#### Scenario: Activity follows the turn lifecycle

- **WHEN** activity is derived for each of these observations
  - runner lock held
  - lock free, last turn started with no end
  - lock free, a message waiting
  - lock free, last turn ended with exit code 2
  - lock free, last turn ended with exit code 0, nothing waiting
  - no turn ever run, nothing waiting
- **THEN** the results SHALL be busy, interrupted, interrupted, failed with code 2, awaiting input, and awaiting input respectively
- **AND** the surface SHALL be reported alive for the first, fifth and sixth only

#### Scenario: A crashed CLI reads as failed

- **GIVEN** a stub CLI that exits with code 1
- **WHEN** a message is delivered and its turn ends
- **THEN** the surface's activity SHALL be failed with code 1
- **AND** the surface SHALL NOT be reported alive

### Requirement: Kill and close stop process groups and remove run state

Killing a headless surface SHALL, when a runner owns it, send a termination signal to the runner's process group, wait up to 5 seconds for the runner to release the surface, then forcibly terminate the group, and SHALL then remove the surface's run state so it is no longer listed. The backend SHALL signal a process group only while its runner holds the surface's lock. Closing a headless session SHALL kill every surface, stop every service and remove the session's run state; afterwards the session SHALL NOT be reported alive. A session SHALL be reported alive from the time it is opened until it is closed, whether or not any turn is running.

#### Scenario: Killing a busy surface stops its CLI

- **GIVEN** a surface whose stub CLI runs a long turn
- **WHEN** the surface is killed
- **THEN** the stub CLI process SHALL have exited within 10 seconds
- **AND** the surface SHALL no longer be listed

#### Scenario: Close removes everything

- **GIVEN** an open headless session with two surfaces, one busy, and one service
- **WHEN** the session is closed
- **THEN** no runner, CLI or service process of the session SHALL remain
- **AND** the session SHALL NOT be reported alive
- **AND** its run state SHALL no longer exist

#### Scenario: An idle session is alive

- **GIVEN** an open headless session whose surfaces are all awaiting input
- **WHEN** session liveness is queried
- **THEN** the session SHALL be reported alive

### Requirement: Attach follows worker output read-only

`attach` on a headless session SHALL write to its own standard output each new line of every surface's output stream, prefixed with that surface's label, until it is interrupted, and SHALL then return without affecting any turn or service. It SHALL send no input to any surface and SHALL write no terminal control sequences. Detaching clients and applying a layout SHALL be no-ops for the headless backend.

#### Scenario: Attach shows labelled output

- **GIVEN** a session with surfaces labelled `feat-a` and `feat-b`
- **WHEN** attach is running and `feat-a`'s turn prints `building`
- **THEN** attach SHALL print a line containing `feat-a` and `building`

#### Scenario: Interrupting attach leaves turns running

- **GIVEN** attach is following a session with a busy surface
- **WHEN** attach is interrupted
- **THEN** the busy surface's turn SHALL continue and SHALL end normally

### Requirement: Session environment never persists the caller's environment

The headless backend SHALL store, as the session's environment, only the variables git-paw sets through the port's session-environment operation, and SHALL apply them over the environment of the process that starts each turn runner. It SHALL NOT write any other environment variable of any git-paw process to disk.

#### Scenario: Session variables reach turns

- **GIVEN** a headless session whose session environment sets `GIT_PAW_BROKER_URL`
- **WHEN** a turn runs a stub CLI that records its environment
- **THEN** the recorded environment SHALL contain that `GIT_PAW_BROKER_URL` value

#### Scenario: Caller variables are not persisted

- **GIVEN** a git-paw process whose environment contains `EXAMPLE_TOKEN=abc123`
- **WHEN** it opens a headless session and spawns and messages a surface
- **THEN** no file in the session's run state SHALL contain `abc123`

### Requirement: Run state is owner-only and outside the repository

The headless backend SHALL keep a session's run state under the user data directory next to the session file, never inside the repository or any worktree. It SHALL create its directories with mode `0700` and its files with mode `0600`, and SHALL build every run-state path from the validated session-name and branch-slug types, so no path component can traverse outside the run state.

#### Scenario: Run state permissions

- **GIVEN** an open headless session with one surface that has run a turn
- **WHEN** the run state is inspected
- **THEN** every directory SHALL have mode `0700` and every file mode `0600`
- **AND** no run-state file SHALL be inside the repository

### Requirement: Headless services run detached and stop gracefully

The headless backend SHALL run a service — such as the broker host — under a turn runner in service mode: in its own session, with no controlling terminal, and as the runner's child so that the service is never reparented to init while its runner lives. `service_alive` SHALL report whether that runner holds the service's lock. Stopping a service SHALL send `SIGHUP` to the service's process group, which the broker host treats as a graceful shutdown with a final flush. A service runner SHALL stop its service within 10 seconds of finding that its session's run state no longer exists.

#### Scenario: The broker host survives the launcher

- **GIVEN** a headless session whose broker host was started as a service by a `git paw` process attached to a pseudo-terminal
- **WHEN** that process exits and the pseudo-terminal is closed
- **THEN** the broker SHALL still answer `GET /status` 10 seconds later

#### Scenario: Stopping the broker host flushes its log

- **GIVEN** a headless broker host service that has accepted a message
- **WHEN** the service is stopped
- **THEN** the broker's port SHALL be free within 5 seconds
- **AND** the session's broker log SHALL contain the accepted message

#### Scenario: Removed run state stops an orphaned service

- **GIVEN** a running headless service whose session run state is deleted by hand
- **WHEN** 10 seconds have passed
- **THEN** the service process SHALL no longer be running
