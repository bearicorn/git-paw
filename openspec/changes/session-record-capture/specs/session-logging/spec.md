## MODIFIED Requirements

### Requirement: Create session log directory

The system SHALL create a per-run **session record** directory at
`.git-paw/logs/<session-name>-<started>/` when a session is launched fresh with logging enabled,
where `<started>` is the session's `created_at` timestamp in UTC formatted `YYYYMMDDTHHMMSSZ`. The
record directory SHALL be derived from the saved session's name and creation time alone, so a
recovery, a restart from pause and a hot-attach of the same session SHALL reuse it, and a fresh
launch after `purge` SHALL start a new one. The record directory SHALL be created with owner-only
permissions (mode `0700`). Every per-agent file path in this capability is relative to the record
directory.

#### Scenario: Record directory created on fresh launch
- **GIVEN** `[logging] enabled = true`
- **WHEN** session `paw-myproject` is launched fresh and its `created_at` is `2026-10-18T09:15:00Z`
- **THEN** `.git-paw/logs/paw-myproject-20261018T091500Z/` SHALL be created
- **AND** its permission bits SHALL be `0700`

#### Scenario: Recovery reuses the record
- **GIVEN** a stopped session whose record directory exists
- **WHEN** the session is recovered by `git paw start`
- **THEN** no new record directory SHALL be created
- **AND** capture SHALL continue into the existing record directory

#### Scenario: Fresh launch after purge starts a new record
- **GIVEN** a record directory left by an earlier, purged run of the same repository
- **WHEN** the repository is launched fresh again
- **THEN** a second record directory named with the new start time SHALL be created
- **AND** the earlier record directory SHALL be left unchanged

#### Scenario: Log directory already exists
- **WHEN** the record directory for the session already exists
- **THEN** it SHALL be reused without error

#### Scenario: Logging disabled
- **WHEN** a session is launched with `[logging] enabled = false` or no `[logging]` section
- **THEN** no directory SHALL be created under `.git-paw/logs/`

### Requirement: Attach pipe-pane to capture output

The system SHALL stream each coding agent's terminal output into that agent's capture file in the
session record on **every** path that creates a coding-agent surface: a fresh launch in any launch
mode, a hot-attach through `git paw add`, and a session recovery. Capture SHALL be attached through
the terminal backend's output-stream operation; the tmux backend realizes it with
`pipe-pane -o` (see `tmux-orchestration`). Surfaces that do not run a coding agent (the dashboard,
the broker host) SHALL NOT be captured. No output stream SHALL be attached when logging is disabled
or when `[logging] capture_terminal = false`.

#### Scenario: Capture attached on a bare fresh launch
- **GIVEN** `[logging] enabled = true` and a bare `git paw start` with branches `feat/a` and `feat/b`
- **WHEN** the session is launched
- **THEN** each of the two coding-agent surfaces SHALL have an active output stream
- **AND** the capture files for `feat/a` and `feat/b` SHALL exist in the record directory

#### Scenario: Capture attached on hot-attach
- **GIVEN** a running session with logging enabled
- **WHEN** `git paw add feat/c` attaches a new coding agent
- **THEN** the new surface SHALL have an active output stream into the `feat/c` capture file

#### Scenario: Capture attached on recovery
- **GIVEN** a stopped session with logging enabled whose `feat/a` capture file already has content
- **WHEN** the session is recovered
- **THEN** every recreated coding-agent surface SHALL have an active output stream
- **AND** the existing content of the `feat/a` capture file SHALL be preserved, with new output appended after it

#### Scenario: Non-agent surfaces are not captured
- **GIVEN** a session with the broker enabled and logging enabled
- **WHEN** the session is launched
- **THEN** the dashboard surface SHALL have no output stream attached

#### Scenario: Not attached when logging disabled
- **WHEN** logging is disabled
- **THEN** no output stream SHALL be attached to any surface

#### Scenario: Not attached when terminal capture is turned off
- **GIVEN** `[logging] enabled = true` and `capture_terminal = false`
- **WHEN** a session is launched
- **THEN** no output stream SHALL be attached
- **AND** the record directory and its manifest SHALL still be written

### Requirement: List available log sessions (library API)

The system SHALL enumerate the session records under `.git-paw/logs/`, returning for each its
directory name, its session name, its start time, and whether it is a legacy record. A per-run
record's session name and start time SHALL be parsed from its `<session-name>-<YYYYMMDDTHHMMSSZ>`
name. Any other directory SHALL be reported as a legacy record whose session name is the directory
name and whose start time is the directory's modification time. Records SHALL be returned newest
first.

#### Scenario: Multiple records
- **WHEN** the records are listed and `.git-paw/logs/` contains `paw-myproject-20261018T091500Z` and `paw-myproject-20261017T080000Z`
- **THEN** both SHALL be returned, `paw-myproject-20261018T091500Z` first
- **AND** each SHALL report session name `paw-myproject`

#### Scenario: Legacy record recognized
- **WHEN** the records are listed and `.git-paw/logs/` contains a directory `paw-other` holding only `.log` files
- **THEN** `paw-other` SHALL be returned as a legacy record with session name `paw-other`

#### Scenario: No records
- **WHEN** the records are listed and `.git-paw/logs/` is empty
- **THEN** an empty list SHALL be returned

#### Scenario: Logs directory does not exist
- **WHEN** the records are listed and `.git-paw/logs/` does not exist
- **THEN** an empty list SHALL be returned (not an error)

## ADDED Requirements

### Requirement: Session record manifest

The system SHALL keep a `manifest.json` file in every session record it creates. The file SHALL be
a versioned wire format carrying `version` (`1`), `session`, `repo_path`, `started_at`, an optional
`ended_at`, and one entry per coding agent with `agent_id`, `branch`, `cli`, an optional
`cli_session_id`, `worktree_path`, `start_commit`, `attached_at`, an optional `detached_at` and an
optional `end_commit`. `start_commit` SHALL be the agent worktree's `HEAD` when the agent is
attached; `end_commit` SHALL be the agent branch's tip when the agent is detached. The manifest
SHALL be written at fresh launch and updated when an agent is hot-attached, removed, or purged and
when the session is purged. Each write SHALL replace the file atomically (write to a temporary file
in the record, then rename). A manifest SHALL load when any optional field is absent, and unknown
fields SHALL be ignored on load.

#### Scenario: Manifest written at fresh launch
- **GIVEN** logging enabled and a fresh launch of branches `feat/a` (CLI `claude`) and `feat/b` (CLI `codex`)
- **WHEN** the session is launched
- **THEN** `manifest.json` SHALL contain `version` `1`, the session name, the start time, and two agent entries
- **AND** each entry's `start_commit` SHALL equal that worktree's `HEAD` at attach time
- **AND** the `feat/a` entry SHALL carry the `cli_session_id` recorded for it and the `feat/b` entry SHALL omit it

#### Scenario: Hot-attach appends an entry
- **GIVEN** a running session with logging enabled and two agents in its manifest
- **WHEN** `git paw add feat/c` attaches a third agent
- **THEN** the manifest SHALL contain three agent entries, the new one with its own `start_commit` and `attached_at`

#### Scenario: Remove records the end of an agent
- **GIVEN** a session whose manifest lists agent `feat/b`
- **WHEN** `git paw remove feat/b` detaches it
- **THEN** the `feat/b` entry SHALL carry `detached_at` and an `end_commit` equal to the `feat/b` tip before any branch deletion

#### Scenario: Purge ends the record
- **GIVEN** a session with logging enabled and two attached agents
- **WHEN** the session is purged
- **THEN** the manifest SHALL carry `ended_at`
- **AND** every agent entry SHALL carry `detached_at` and `end_commit`

#### Scenario: Recovery does not duplicate entries
- **GIVEN** a stopped session whose manifest lists two agents
- **WHEN** the session is recovered
- **THEN** the manifest SHALL still list exactly two agent entries

#### Scenario: Manifest with absent optional fields loads
- **GIVEN** a `manifest.json` with `version` `1` and no `ended_at`, `detached_at`, `end_commit` or `cli_session_id`
- **WHEN** it is loaded
- **THEN** loading SHALL succeed with those fields absent

### Requirement: Broker event record

When logging and the broker are both enabled, the broker host SHALL append every entry of the
broker's message log to `events.jsonl` in the session record, one JSON object per line with the
fields `seq`, `timestamp_unix_secs` and `message`. This is the entry shape `GET /log` returns, and
`message` is the broker wire form of the message. Entries SHALL be appended in message-log order on
the broker's existing flush cadence and at broker shutdown. When the broker host restarts within
the same session, its entries SHALL be appended after the existing lines, even though sequence
numbers restart. When logging is disabled, no event record SHALL be written. In every case, the
broker's existing `broker.log` behaviour SHALL be unchanged.

#### Scenario: Published messages are recorded in order
- **GIVEN** logging and the broker enabled
- **WHEN** three messages are published and the broker flushes
- **THEN** `events.jsonl` SHALL contain three lines in publish order
- **AND** each line SHALL parse as an object with `seq`, `timestamp_unix_secs` and `message`, whose `message` deserializes back into the published broker message

#### Scenario: Shutdown flushes pending entries
- **GIVEN** logging and the broker enabled
- **WHEN** a message is published and the broker host shuts down before its next periodic flush
- **THEN** that message SHALL be present in `events.jsonl`

#### Scenario: A broker host restart appends
- **GIVEN** an `events.jsonl` holding two lines from a previous broker host lifetime in the same session
- **WHEN** the broker host restarts and one more message is published and flushed
- **THEN** `events.jsonl` SHALL hold three lines, the new line last

#### Scenario: No event record without logging
- **GIVEN** the broker enabled and logging disabled
- **WHEN** messages are published
- **THEN** no `events.jsonl` SHALL be written

#### Scenario: No event record without the broker
- **GIVEN** logging enabled and the broker disabled
- **WHEN** a session is launched
- **THEN** no `events.jsonl` SHALL be written
- **AND** the record directory and its manifest SHALL still be written

### Requirement: Session record files are owner-only

The system SHALL create every file it writes into a session record (the manifest, the event record
and the agents' capture files) with owner-only permissions (mode `0600`). It SHALL refuse to create
or write a record file or record directory through a symbolic link: when the target path is a
symbolic link, that file SHALL be skipped with a warning on stderr, the link and its target SHALL be
left unmodified, and the launch SHALL proceed.

#### Scenario: Record files are owner-only
- **GIVEN** logging and the broker enabled
- **WHEN** a session is launched and the broker flushes
- **THEN** the manifest, the event record and every capture file SHALL have permission bits `0600`

#### Scenario: Symlinked capture path is refused
- **GIVEN** a recovered session whose record holds a symbolic link at the `feat/a` capture path, pointing at a file outside the record
- **WHEN** the session is recovered
- **THEN** no output stream SHALL be attached for `feat/a` and a warning naming the path SHALL be printed on stderr
- **AND** the link target's content SHALL be unchanged
- **AND** the other agents SHALL be captured normally

### Requirement: Session record retention

When `[logging] retain_sessions = N` is configured, the system SHALL, during a fresh launch and
after creating the new record, delete the oldest session records under `.git-paw/logs/` so that at
most `N` records remain, ordering by start time as reported by the record enumeration. The record
being launched SHALL never be deleted. Only entries that are real directories directly under
`.git-paw/logs/` SHALL be candidates. Symbolic links and plain files SHALL never be deleted, and
deletion SHALL NOT follow symbolic links inside a record. The number of records pruned SHALL be
reported on stderr. When `retain_sessions` is unset, no record SHALL ever be deleted. Recovery,
hot-attach and restart from pause SHALL never prune.

#### Scenario: Oldest records are pruned
- **GIVEN** `retain_sessions = 2` and three existing records
- **WHEN** a fresh launch creates a fourth record
- **THEN** the two oldest records SHALL be deleted
- **AND** the new record and the newest pre-existing record SHALL remain
- **AND** stderr SHALL report that two records were pruned

#### Scenario: Unset retention keeps everything
- **GIVEN** `retain_sessions` unset and ten existing records
- **WHEN** a fresh launch creates a new record
- **THEN** all eleven records SHALL remain

#### Scenario: Symlinks and files are never pruned
- **GIVEN** `retain_sessions = 1`, a symbolic link and a plain file under `.git-paw/logs/`, and one older record
- **WHEN** a fresh launch creates a new record
- **THEN** the older record SHALL be deleted
- **AND** the symbolic link, its target and the plain file SHALL be left unchanged

#### Scenario: Recovery never prunes
- **GIVEN** `retain_sessions = 1` and two existing records
- **WHEN** a stopped session is recovered
- **THEN** both records SHALL remain

### Requirement: Session-logging documentation states the privacy posture

The session-logging chapter of the user guide SHALL state that session records can contain secrets
(terminal output, prompts, command output, file contents). It SHALL state that git-paw writes them
only under `.git-paw/logs/` with owner-only permissions, that `git paw init` adds `.git-paw/logs/`
to `.gitignore`, and that git-paw never transmits a session record anywhere. It SHALL document how
to bound records with `retain_sessions` and `capture_terminal`.

#### Scenario: Privacy section present
- **WHEN** `docs/src/user-guide/session-logging.md` is read
- **THEN** it SHALL contain a section stating that records may contain secrets, are owner-only, are gitignored and are never transmitted
- **AND** it SHALL name `retain_sessions` and `capture_terminal`
