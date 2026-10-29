## Purpose

Defines sessions whose workers run on different terminal backends side by side — tmux panes,
headless turn-driven conversations and remote vendor sessions — so a user can offload some branches
while others run locally. It covers how each worker's backend is chosen, checked and recorded, which
combinations a session may hold, how one routing backend serves the whole session, and how the
drive loop and the lifecycle commands treat each worker by its own backend. The session's own
backend (tmux or headless) still hosts the session, its broker host and its services.

## ADDED Requirements

### Requirement: Each worker runs on its own backend, defaulting to the session's

Every worker of a session SHALL run on exactly one worker backend — `tmux`, `headless` or `remote` — chosen when the worker is launched or attached and otherwise equal to the session's backend. A tmux worker SHALL be admitted only into a session whose backend is tmux, because only a tmux session can hold a pane; headless and remote workers SHALL be admitted into a session of either backend. A worker's backend SHALL be fixed for the worker's life: recovery, restart from pause and revival SHALL use the recorded backend, and no command SHALL move a worker to another backend.

#### Scenario: An unnamed worker uses the session's backend

- **GIVEN** a tmux launch of `feat/a` and `feat/b` with `--worker-backend feat/b=headless`
- **WHEN** the session is saved
- **THEN** `feat/a` SHALL be a tmux worker and `feat/b` a headless worker

#### Scenario: A tmux worker in a headless session is refused

- **GIVEN** a repository with no session
- **WHEN** `git paw start --backend headless --branches feat/a,feat/b --worker-backend feat/b=tmux` runs
- **THEN** it SHALL exit with code 1 naming `feat/b` and the reason that a headless session holds no panes
- **AND** no worktree, branch, session file, run state or broker process SHALL exist afterwards

#### Scenario: Recovery keeps each worker's backend

- **GIVEN** a stopped tmux session whose `feat/a` is a tmux worker and `feat/b` a headless worker
- **WHEN** `git paw start` recovers it
- **THEN** `feat/a` SHALL be relaunched in a pane and `feat/b` respawned as a headless surface

### Requirement: Every worker passes its own backend's gate before anything is created

Before creating any worktree, branch, session file, run state, broker or vendor session for a launch or an attach, the system SHALL check every planned worker against the gate of its own backend — the headless eligibility gate for every headless worker, whatever the session's backend, and the remote eligibility gate for every remote worker — together with the combination rule. When any worker is refused, the command SHALL create nothing, SHALL print every refused worker with each failed condition and the key or flag that would satisfy it, and SHALL exit with code 1.

#### Scenario: A headless worker in a tmux session is gated

- **GIVEN** a tmux launch of `feat/a` and `feat/b` with `--worker-backend feat/b=headless`, where `feat/b`'s CLI has no headless form
- **WHEN** the launch runs
- **THEN** it SHALL exit with code 1 naming `feat/b`, the missing headless form and `[clis.<name>].headless_args`
- **AND** no worktree for `feat/a` or `feat/b` SHALL exist afterwards

#### Scenario: Refusals of different backends are reported together

- **GIVEN** a launch with one headless worker whose CLI has no headless form and one remote worker whose branch has local commits
- **WHEN** the launch runs
- **THEN** stderr SHALL name both workers and both conditions in a single run
- **AND** the process SHALL exit with code 1

### Requirement: The session file records a worker's backend when it differs from the session's

The `WorktreeEntry` SHALL record a worker's backend in an optional `backend` field, written only when it differs from the session's backend and annotated `#[serde(default, skip_serializing_if = "Option::is_none")]`, so a session whose workers all use its backend is saved byte-identically to before and an entry without the field loads with the session's backend.

#### Scenario: A mixed session records the odd worker only

- **GIVEN** a tmux session with tmux worker `feat/a` and remote worker `feat/r`
- **WHEN** its session file is saved and inspected
- **THEN** `feat/r`'s entry SHALL contain `"backend": "remote"`
- **AND** `feat/a`'s entry SHALL contain no `backend` key

#### Scenario: A single-backend session file is unchanged

- **GIVEN** a tmux session all of whose workers are tmux workers
- **WHEN** it is saved by this release and by the previous release
- **THEN** the two files SHALL be byte-identical

### Requirement: One routing backend serves every session

Every command that acts on a session SHALL construct one terminal backend for it from the session record: the adapter of the session's backend together with one adapter for each other worker backend the session holds. Every surface operation SHALL be performed by the adapter that created the surface. Opening the session, setting its environment, probing its liveness, its services and its layout SHALL be performed by the session backend's adapter. Listing the session's surfaces SHALL return the surfaces of every adapter; closing the session SHALL close it in every adapter; presenting the session SHALL first print every remote worker's label, last-known vendor state and URL and then present the session through the session backend's adapter. For a session whose workers all use the session's backend, every operation issued and its order SHALL be identical to the behavior before this requirement. The construction of this routing backend SHALL be the only place that matches on the set of worker backend kinds.

#### Scenario: A surface operation reaches only its own adapter

- **GIVEN** a routing backend over a fake tmux adapter and a fake remote adapter, with one surface in each
- **WHEN** text and `Enter` are sent to the remote surface
- **THEN** only the remote adapter SHALL have recorded the text and the key
- **AND** the tmux adapter SHALL have recorded no operation

#### Scenario: Listing returns every worker

- **GIVEN** a tmux session with tmux worker `feat/a`, headless worker `feat/h` and remote worker `feat/r`
- **WHEN** the session's surfaces are listed
- **THEN** the listing SHALL report one surface owned by each of the three worktrees
- **AND** only `feat/a`'s surface SHALL carry a layout position

#### Scenario: A single-backend session issues the same operations

- **GIVEN** a tmux launch plan whose workers are all tmux workers
- **WHEN** the session is built through the routing backend and the emitted tmux command strings are compared with the strings emitted before this requirement
- **THEN** they SHALL be identical in content and order

### Requirement: The drive loop treats each worker by its own backend

The drive loop SHALL treat each worker by its own backend: it SHALL evaluate permission prompts, buffered input and every other screen predicate only for tmux workers; SHALL apply the headless turn-outcome decisions only to headless workers; and SHALL send no text and no key to a remote worker's surface. For a remote worker, re-engagement after a failed gate SHALL be the remote verify continuation of remote-worker landing, and the drive loop SHALL NOT count it as a correction cycle. Completion detection, merge detection, the heartbeat's measure of progress and the exit summary SHALL treat workers of every backend alike.

#### Scenario: No drive-loop text reaches a remote worker

- **GIVEN** `auto_loopback` enabled, the fake backend, and a remote worker whose branch has just failed a gate
- **WHEN** the drive loop runs one sweep and its correction pass
- **THEN** the fake SHALL have recorded no text or key operation for the remote worker's surface
- **AND** the gate's `agent.feedback` for the worker SHALL still have been published

#### Scenario: A headless worker in a tmux session gets turn decisions

- **GIVEN** a tmux session whose headless worker `feat/h` is scripted interrupted for the first time
- **WHEN** the drive loop runs one sweep
- **THEN** it SHALL deliver the bundled continuation text to `feat/h` as one message

#### Scenario: A tmux worker in a mixed session still escalates prompts

- **GIVEN** a tmux session with a remote worker and a tmux worker whose screen shows a live permission prompt
- **WHEN** the drive loop runs one sweep
- **THEN** it SHALL publish a `permission-prompt` escalation for the tmux worker
- **AND** no keystroke SHALL be sent to either worker

### Requirement: Lifecycle commands act on each worker through its own backend

`git paw start` (launch, recovery, restart from pause and revival), `add`, `remove`, `pause`, `stop` and `purge` SHALL act on each worker through its own backend: a tmux worker as a pane, a headless worker as a headless surface (respawned with the continuation message when recovered), and a remote worker as a remote surface (re-attached when recovered, as remote-worker landing specifies). Every worker, whatever its backend, SHALL be registered with the broker host as a launched pane worker is. A session whose backend is headless SHALL NOT require `tmux`.

#### Scenario: Stop ends every kind of worker

- **GIVEN** a tmux session with a busy tmux worker, a busy headless worker and a followed remote worker
- **WHEN** `git paw stop` runs
- **THEN** the tmux session SHALL no longer exist
- **AND** no headless turn runner or CLI process of the session SHALL remain
- **AND** the remote worker's follow SHALL have ended, and stop's output SHALL name its vendor session URL
- **AND** the session file SHALL record the session as stopped

#### Scenario: A remote worker is registered with the broker host

- **GIVEN** a tmux launch with remote worker `feat/r` and the broker enabled
- **WHEN** the launch completes
- **THEN** the broker host SHALL have accepted a `POST /watch` for `feat-r`
