# safe-process-invocation Specification

## Purpose
Safe construction of the tmux/process commands git-paw builds from untrusted inputs: the tmux session name derived from the repository directory is sanitized to a tmux-safe form at a single construction boundary, and paths and binary invocations typed into a shell (the pipe-pane log path, the dashboard command) are shell-quoted — so repository, branch, or path names containing spaces or shell metacharacters cannot break out of a tmux target or a shell word.

## Requirements
### Requirement: Session names are sanitized to a tmux-safe form

The tmux session name derived from the repository directory SHALL be sanitized so that
it contains only characters valid in a tmux target — no `.`, no `:`, and no whitespace —
before it is used in any tmux command. The name SHALL be constructed exactly once
through a `SessionName` smart constructor (`SessionName::from_project`) so that a raw,
unsanitized directory name can never reach a tmux command. For a well-formed directory
name (only `[A-Za-z0-9_-]`), the sanitized name SHALL be byte-identical to the current
`paw-<name>` output, preserving existing behavior.

#### Scenario: Session name from a directory with a space

- **GIVEN** a repository directory named `My Project`
- **WHEN** a session is created for it
- **THEN** the resulting session name SHALL be a valid tmux target containing no whitespace
- **AND** every `session:0.N` pane target derived from it SHALL resolve to a real pane

#### Scenario: Session name from a dotted directory

- **GIVEN** a repository directory named `my.app`
- **WHEN** a session name is derived
- **THEN** the `.` SHALL be replaced so the name is `paw-my-app`
- **AND** `session:0.N` pane targets SHALL be unambiguous (the `.` no longer collides with tmux's pane separator)

#### Scenario: Well-formed name is unchanged

- **GIVEN** a repository directory named `git-paw`
- **WHEN** a session name is derived
- **THEN** the session name SHALL be exactly `paw-git-paw` (behavior-preserving)

#### Scenario: Collision suffix appends to the sanitized base

- **GIVEN** a sanitized session name whose base is already taken by a live session
- **WHEN** a unique name is resolved
- **THEN** the collision suffix (`-2`, `-3`, …) SHALL be appended to the already-sanitized base, keeping the whole name a valid tmux target

### Requirement: Paths interpolated into shell contexts are quoted

The system SHALL shell-quote any filesystem path interpolated into a shell command body
executed via `/bin/sh -c` (notably the `pipe-pane` logging command `cat >> <path>`) so
that spaces and shell metacharacters in the path are treated literally. For a path
containing no special characters, the quoted form SHALL be behavior-equivalent to the
unquoted form (the shell strips the quotes and the same file is written).

#### Scenario: Logging to a path with a space

- **GIVEN** a repository path that contains a space
- **WHEN** pane logging starts via `pipe-pane`
- **THEN** the emitted `/bin/sh -c` body SHALL quote the path
- **AND** `pipe-pane` SHALL write to the correct single file rather than splitting the path on the space

#### Scenario: Plain path is behavior-equivalent

- **GIVEN** a repository path with no spaces or shell metacharacters
- **WHEN** pane logging starts
- **THEN** the quoted command SHALL write to the same file the unquoted command would have (behavior-preserving)

### Requirement: Commands sent via send-keys are sent literally or shell-quoted

The system SHALL send any command string delivered to a pane via `tmux send-keys` — in
particular the `__dashboard` launch command, which embeds the installed binary path from
`std::env::current_exe()` — literally (using the `-l` flag with a separate `Enter`
keystroke) or with any embedded path shell-quoted, so that a binary path containing a
space still launches the intended command. A binary path with no special characters
SHALL behave exactly as before.

#### Scenario: Dashboard launches from a spaced binary path

- **GIVEN** an installed git-paw binary whose path contains a space
- **WHEN** the `__dashboard` command is sent to its pane
- **THEN** the command SHALL be sent literally (`send-keys -l`) or with the path shell-quoted
- **AND** the dashboard subprocess SHALL launch rather than the shell mis-parsing the path on the space

#### Scenario: Plain binary path is unchanged

- **GIVEN** an installed binary whose path contains no spaces
- **WHEN** the `__dashboard` command is sent
- **THEN** the dashboard SHALL launch exactly as it does today (behavior-preserving)

### Requirement: Injection-prone strings are sanitized at a single construction boundary

The system SHALL sanitize or quote externally-controlled strings that cross into a tmux
target or a shell command context (the repository directory name and interpolated
filesystem paths) at a single construction boundary — a smart-constructor newtype
(`SessionName`) for names and a shell-quoting helper for paths — rather than by ad-hoc
escaping at each call site. A value of the `SessionName` newtype SHALL only ever hold a
tmux-safe string; its constructor SHALL sanitize any input it is given.

#### Scenario: Newtype constructor never yields an unsafe value

- **GIVEN** an arbitrary directory name containing `.`, `:`, or whitespace
- **WHEN** a `SessionName` is constructed from it
- **THEN** the resulting value SHALL contain only tmux-target-safe characters
- **AND** there SHALL be no alternative constructor that bypasses the sanitization

#### Scenario: Single boundary, no ad-hoc call-site escaping

- **GIVEN** the session-name, pipe-pane, and dashboard-command sites
- **WHEN** each builds its tmux/shell string
- **THEN** each SHALL obtain its name via the `SessionName` constructor or its path via the shell-quoting helper
- **AND** no site SHALL interpolate the raw directory name or an unquoted path directly

