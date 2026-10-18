## ADDED Requirements

### Requirement: GET /log long-polls on the since cursor

The `GET /log` endpoint SHALL accept an optional `wait` query parameter (whole seconds). When `wait` is present and no log entry with a sequence number greater than `since` exists, the broker SHALL hold the request until such an entry is appended or `wait` seconds elapse, whichever comes first, and SHALL then respond with the same `{"entries": [...], "last_seq": N}` shape as an immediate request — `entries` empty when the wait elapsed with nothing new. The broker SHALL cap the wait at 60 seconds, SHALL reject a non-numeric `wait` with 400, and SHALL NOT hold any broker state lock while waiting. Whenever a `GET /log` response carries no entries — with or without `wait` — its `last_seq` SHALL equal the request's `since` (`0` when `since` is absent), so a reader's cursor never rewinds; a response with entries SHALL report the highest returned sequence number, as before. In every other respect a request without `wait` SHALL behave exactly as before. The drive loop's broker-message observer SHALL request only entries after the last sequence number it has already seen.

#### Scenario: A waiting request returns as soon as an entry is published

- **GIVEN** a broker whose last sequence number is 7
- **WHEN** a client sends `GET /log?since=7&wait=30` and another client publishes a message 2 seconds later
- **THEN** the first request SHALL complete within about 2 seconds with exactly the new entry and `last_seq = 8`

#### Scenario: A waiting request times out empty

- **GIVEN** a broker whose last sequence number is 7 and no publisher
- **WHEN** a client sends `GET /log?since=7&wait=2`
- **THEN** the response SHALL arrive after about 2 seconds with `entries` empty and `last_seq = 7`

#### Scenario: The wait is capped

- **WHEN** a client sends `GET /log?since=0&wait=600` against a broker with no entries after 0
- **THEN** the response SHALL arrive within about 60 seconds

#### Scenario: A non-numeric wait is rejected

- **WHEN** a client sends `GET /log?wait=soon`
- **THEN** the response status SHALL be 400

#### Scenario: A request without wait returns at once

- **GIVEN** a broker with three published messages
- **WHEN** a client sends `GET /log?since=2`
- **THEN** the response SHALL return immediately with the entry at sequence 3 and `last_seq = 3`

#### Scenario: An empty response without wait holds the cursor

- **GIVEN** a broker whose last sequence number is 7
- **WHEN** a client sends `GET /log?since=7` with no `wait`
- **THEN** the response SHALL return immediately with `entries` empty and `last_seq = 7`

#### Scenario: The drive loop reads only new entries

- **GIVEN** a running drive loop whose observer has already consumed entries up to sequence 12
- **WHEN** it polls the broker for new messages
- **THEN** its request SHALL carry `since=12`

### Requirement: GET /status reports wave staleness

The broker SHALL record the time the wave last made progress and SHALL include in every `GET /status` response a `wave` object with `last_progress_unix_secs`, `idle_seconds`, `stall_threshold_seconds`, and `stalled` (true when the threshold is non-zero and `idle_seconds` exceeds it). Progress SHALL be a wave phase advancing or a branch tip moving, and nothing else. A wave phase advances when the broker accepts an `agent.status` whose status or phase differs from that agent's previously recorded values, an `agent.artifact`, an `agent.verified`, an `agent.feedback`, or an `agent.advanced-main`. A branch tip moves when the watcher observes that an agent's worktree `HEAD` commit differs from the one it last observed. Every other accepted message or observation SHALL NOT be progress — in particular an `agent.status` that repeats the agent's recorded status and phase (whatever its modified-file set), an `agent.question` (including the drive loop's escalation of a live permission prompt and its re-escalation of a prompt it already escalated), an `agent.learning` (including the learnings the drive loop records for its own escalations), and a change to an agent's uncommitted modified-file set. A broker that has accepted no message SHALL report its start time as the last progress. Every other field of the `/status` response SHALL be unchanged.

#### Scenario: Repeated identical heartbeats are not progress

- **GIVEN** an agent whose last recorded status is `working` with no modified files, and a stall threshold of 60 seconds
- **WHEN** the agent re-publishes the identical `working` status every 10 seconds for 2 minutes and nothing else happens
- **THEN** `/status` SHALL report `stalled = true` and `idle_seconds` of at least 120

#### Scenario: A real event resets staleness

- **GIVEN** a wave reported `stalled = true`
- **WHEN** an agent publishes an `agent.artifact`
- **THEN** the next `/status` SHALL report `stalled = false` and an `idle_seconds` near 0

#### Scenario: A moved branch tip is progress

- **GIVEN** an agent whose worktree `HEAD` the watcher last observed at commit `A`
- **WHEN** the agent commits and the watcher observes `HEAD` at commit `B`
- **THEN** `last_progress_unix_secs` SHALL advance

#### Scenario: Uncommitted file churn is not progress

- **GIVEN** an agent whose recorded status is `working` and a stall threshold of 60 seconds
- **WHEN** the watcher observes the agent's modified-file set change several times over 2 minutes, with no commit and no status or phase change
- **THEN** `/status` SHALL report `stalled = true`

#### Scenario: The drive loop's own escalations and learnings are not progress

- **GIVEN** an `--unattended` session whose only agent sits on an unanswered live permission prompt, and a stall threshold of 60 seconds
- **WHEN** over 2 minutes the drive loop publishes the prompt's escalation as an `agent.question`, re-escalates it after its dedup window, and records `agent.learning` entries for it, while no agent's phase advances and no branch tip moves
- **THEN** `/status` SHALL report `stalled = true` and an `idle_seconds` of at least 120

#### Scenario: A zero threshold disables the flag

- **GIVEN** `[broker] wave_stall_minutes = 0`
- **WHEN** the wave has been idle for an hour
- **THEN** `/status` SHALL report `stalled = false` and SHALL still report `idle_seconds`

### Requirement: Wave stall threshold is configurable

The `[broker]` table SHALL accept an optional `wave_stall_minutes` integer, defaulting to `30` when absent; `0` SHALL disable the `stalled` flag while keeping `idle_seconds`. A config without the key SHALL load unchanged.

#### Scenario: Absent key uses the default

- **GIVEN** a config with `[broker] enabled = true` and no `wave_stall_minutes`
- **WHEN** the broker starts
- **THEN** `/status` SHALL report `stall_threshold_seconds = 1800`

#### Scenario: Configured threshold is honoured

- **GIVEN** `[broker] wave_stall_minutes = 5`
- **WHEN** the broker starts
- **THEN** `/status` SHALL report `stall_threshold_seconds = 300`

### Requirement: Status surfaces a stalled wave

When the session's broker reports `stalled = true`, `git paw status` SHALL print a line stating that the wave has made no progress and for how long, and `git paw status --json` SHALL include the broker's `wave` object. When the wave is not stalled, the human output SHALL be unchanged.

#### Scenario: A stalled wave is visible in status

- **GIVEN** an active session whose broker reports `stalled = true` with `idle_seconds = 11520`
- **WHEN** `git paw status` is executed
- **THEN** the output SHALL contain a line stating the wave has made no progress for 3h12m

#### Scenario: JSON status carries the wave object

- **GIVEN** an active session with a live broker
- **WHEN** `git paw status --json` is executed
- **THEN** the output SHALL contain a `wave` object with `stalled` and `idle_seconds`

#### Scenario: A progressing wave adds no line

- **GIVEN** an active session whose broker reports `stalled = false`
- **WHEN** `git paw status` is executed
- **THEN** the output SHALL NOT contain a stalled-wave line
