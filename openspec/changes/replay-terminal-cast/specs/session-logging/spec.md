## MODIFIED Requirements

### Requirement: Derive log file path per pane

The system SHALL derive each coding agent's capture file path in the session record from the agent's
branch name, sanitized for filesystem safety by replacing `/` with `--`, with the `.cast` extension.
Capture files named `<sanitized-branch>.log` in records written before timed capture SHALL remain
readable.

#### Scenario: Simple branch name
- **WHEN** a coding agent is assigned branch `add-auth`
- **THEN** its capture path SHALL be `.git-paw/logs/<record>/add-auth.cast`

#### Scenario: Branch name with slashes
- **WHEN** a coding agent is assigned branch `feat/add-auth`
- **THEN** its capture path SHALL be `.git-paw/logs/<record>/feat--add-auth.cast`

#### Scenario: Branch name with multiple slashes
- **WHEN** a coding agent is assigned branch `feat/auth/jwt`
- **THEN** its capture path SHALL be `.git-paw/logs/<record>/feat--auth--jwt.cast`

### Requirement: Log files contain raw terminal output

Capture files SHALL be asciinema v2 recordings of the raw terminal output, escape sequences
included, with timing. The first line SHALL be a JSON object with `version` `2`, `width`, `height`
and `timestamp` (Unix seconds at recording start). Each subsequent line SHALL be an output event
`[elapsed, "o", data]`, one per chunk read from the surface's output stream, where `elapsed` is the
seconds since `timestamp` and is non-decreasing. `data` is the chunk as a UTF-8 string. A multi-byte
UTF-8 character split across two reads SHALL be emitted whole in the later event, and byte
sequences that are not valid UTF-8 SHALL be replaced with U+FFFD. No stripping or reformatting of
the output SHALL be applied at capture time.

#### Scenario: Header written first
- **WHEN** a recording starts for a surface of 120 columns by 40 rows
- **THEN** the capture file's first line SHALL be a JSON object with `version` `2`, `width` `120` and `height` `40` and a `timestamp`

#### Scenario: Escape sequences preserved
- **WHEN** the agent CLI writes `\x1b[31mred text\x1b[0m`
- **THEN** the concatenated `data` of the output events SHALL contain `\x1b[31mred text\x1b[0m` exactly

#### Scenario: Split UTF-8 character emitted whole
- **WHEN** the two bytes of `é` arrive in two separate reads
- **THEN** no event's `data` SHALL contain a replacement character
- **AND** the event that holds the second byte's read SHALL contain `é`

#### Scenario: Elapsed time is non-decreasing
- **WHEN** a recording holds several output events
- **THEN** each event's `elapsed` SHALL be greater than or equal to the previous event's

### Requirement: List logs for a session

The system SHALL enumerate the capture files within a session record, both `.cast` recordings and
legacy `.log` captures, returning one `LogEntry` per file with its branch name, its path and whether
it is timed.

#### Scenario: Record with timed captures
- **WHEN** the captures of a record holding three `.cast` files are listed
- **THEN** three `LogEntry` items SHALL be returned, each marked timed

#### Scenario: Legacy record
- **WHEN** the captures of a legacy record holding two `.log` files are listed
- **THEN** two `LogEntry` items SHALL be returned, each marked not timed

#### Scenario: Session directory empty
- **WHEN** the captures of a record with no capture files are listed
- **THEN** an empty list SHALL be returned

#### Scenario: Session directory does not exist
- **WHEN** the captures of a nonexistent record are listed
- **THEN** `PawError::SessionError` mentioning the record name SHALL be returned

### Requirement: LogEntry derives branch from filename

The `LogEntry.branch` SHALL reverse the filename sanitization to recover the original branch name,
removing a `.cast` or `.log` extension.

#### Scenario: Simple log filename
- **WHEN** a capture file is named `add-auth.log`
- **THEN** `LogEntry.branch` SHALL be `"add-auth"`

#### Scenario: Sanitized cast filename
- **WHEN** a capture file is named `feat--add-auth.cast`
- **THEN** `LogEntry.branch` SHALL be `"feat/add-auth"`

## ADDED Requirements

### Requirement: Session recorder subcommand

The system SHALL provide a hidden `git paw __record <path> --cols <C> --rows <R>` subcommand that
reads a surface's output stream on stdin until end of input and writes it to `<path>` as an asciinema
v2 recording, flushing each event line as it is written. `__record` SHALL be hidden from
`git paw --help`. It SHALL write to no file other than `<path>`, and it SHALL NOT contact the broker
or the network. It SHALL create `<path>` with owner-only permissions (`0600`). When `<path>` is a
symbolic link, it SHALL exit non-zero without writing anything.

#### Scenario: Recorder produces a playable recording
- **WHEN** `printf 'hello\x1b[1mworld\x1b[0m' | git paw __record out.cast --cols 80 --rows 24` is run
- **THEN** `out.cast` SHALL hold a v2 header with `width` `80` and `height` `24` followed by output events whose data concatenates to `hello\x1b[1mworld\x1b[0m`
- **AND** `out.cast` SHALL have permission bits `0600`

#### Scenario: Recorder is hidden
- **WHEN** `git paw --help` is run
- **THEN** its output SHALL NOT mention `__record`

#### Scenario: Recorder refuses a symlink
- **GIVEN** `out.cast` is a symbolic link to another file
- **WHEN** `git paw __record out.cast --cols 80 --rows 24` is run with input
- **THEN** it SHALL exit non-zero
- **AND** the link target SHALL be unchanged

#### Scenario: Live capture under tmux
- **GIVEN** logging enabled and a launched session under real tmux
- **WHEN** a coding agent's surface prints a marker string
- **THEN** that agent's `.cast` file SHALL contain an output event whose data includes the marker

### Requirement: Recording continues across recovery

The recorder SHALL, when it opens a `<path>` that already starts with a valid asciinema v2 header,
append new output events after the existing ones, measuring `elapsed` from the existing header's
`timestamp`, and SHALL NOT write a second header. When the given size differs from the
header's, the first appended line SHALL be a resize event `[elapsed, "r", "<C>x<R>"]`. When an
existing non-empty `<path>` does not start with a valid v2 header, the recorder SHALL leave the file
unchanged and exit non-zero.

#### Scenario: Append after recovery
- **GIVEN** a `.cast` file whose header `timestamp` is `T` and which holds two events
- **WHEN** the recorder reopens it at `T + 600` seconds and records one chunk
- **THEN** the file SHALL hold one header and three output events, the third with `elapsed` of at least `600`

#### Scenario: Size change recorded on reopen
- **GIVEN** a `.cast` file whose header says `80` by `24`
- **WHEN** the recorder reopens it with `--cols 120 --rows 40`
- **THEN** the first appended line SHALL be a resize event with data `120x40`

#### Scenario: Corrupt existing file left alone
- **GIVEN** an existing non-empty file whose first line is not a v2 header
- **WHEN** the recorder is started on it
- **THEN** it SHALL exit non-zero and the file SHALL be unchanged

### Requirement: Animated replay via --cast

The system SHALL, for `git paw replay <agent> --cast`, play the agent's `.cast` recording from the
selected record with `asciinema play <path>` when an `asciinema` executable is on `PATH`. The player
SHALL be invoked with an argument vector (no shell) and SHALL inherit the terminal. A non-zero player
exit SHALL make the command exit non-zero. When no `asciinema` executable is on `PATH`, the command
SHALL fail with an error that names the `.cast` file path and says that any asciinema v2 player can
play it. When the agent's capture is a legacy `.log` file, the command SHALL fail with an error
saying the capture has no timing and suggesting `--raw`.

#### Scenario: Played through asciinema
- **GIVEN** an agent with a `.cast` capture and an `asciinema` executable on `PATH`
- **WHEN** `git paw replay feat/a --cast` is run
- **THEN** `asciinema` SHALL be invoked with the arguments `play` and the `.cast` path

#### Scenario: Player failure propagates
- **GIVEN** an `asciinema` on `PATH` that exits with status 1
- **WHEN** `git paw replay feat/a --cast` is run
- **THEN** the command SHALL exit non-zero

#### Scenario: No player installed
- **GIVEN** no `asciinema` executable on `PATH`
- **WHEN** `git paw replay feat/a --cast` is run
- **THEN** the command SHALL fail with an error containing the `.cast` file path

#### Scenario: Legacy capture has no timing
- **GIVEN** an agent whose capture is `feat--a.log`
- **WHEN** `git paw replay feat/a --cast` is run
- **THEN** the command SHALL fail with an error suggesting `--raw`

### Requirement: Raw output from a timed recording

The system SHALL, for `git paw replay <agent> --raw` on an agent whose capture is a `.cast`
recording, write to stdout the concatenation of the recording's output-event data in file order, and
nothing else. No header, timing or resize events SHALL be written.

#### Scenario: Raw from a cast
- **GIVEN** a `.cast` whose output events carry `ab`, `\x1b[2J` and `cd`
- **WHEN** `git paw replay feat/a --raw` is run
- **THEN** stdout SHALL be exactly `ab\x1b[2Jcd`
