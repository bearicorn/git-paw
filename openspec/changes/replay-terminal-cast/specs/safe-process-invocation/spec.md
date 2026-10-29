## MODIFIED Requirements

### Requirement: Paths interpolated into shell contexts are quoted

The system SHALL shell-quote any filesystem path interpolated into a shell command body executed via
`/bin/sh -c`, so that spaces and shell metacharacters in the path are treated literally. This covers
in particular both the installed binary path and the capture path in the `pipe-pane` recorder
command `<exe> __record <path> …`. Where the terminal backend expands its own format syntax in that
body (tmux's `#`), the path SHALL also be escaped for that syntax. For a path containing no special
characters, the quoted form SHALL be behavior-equivalent to the unquoted form (the shell strips the
quotes and the same file is written).

#### Scenario: Logging to a path with a space
- **GIVEN** a repository path that contains a space
- **WHEN** pane logging starts via `pipe-pane`
- **THEN** the emitted `/bin/sh -c` body SHALL quote the capture path
- **AND** the recorder SHALL write to the correct single file rather than splitting the path on the space

#### Scenario: Binary path with a space
- **GIVEN** an installed git-paw binary whose path contains a space
- **WHEN** pane logging starts via `pipe-pane`
- **THEN** the emitted body SHALL quote the binary path
- **AND** the recorder SHALL start and write the capture file

#### Scenario: Path containing a format character
- **GIVEN** a repository path that contains `#`
- **WHEN** pane logging starts via `pipe-pane`
- **THEN** the recorder SHALL write to the capture file under that exact path

#### Scenario: Plain path is behavior-equivalent
- **GIVEN** a repository path with no spaces, shell metacharacters or format characters
- **WHEN** pane logging starts
- **THEN** the quoted command SHALL write to the same file the unquoted command would have (behavior-preserving)
