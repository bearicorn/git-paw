## MODIFIED Requirements

### Requirement: Human-readable output carries no escape sequences when not on a terminal

Every git-paw subcommand's human-readable output SHALL contain no terminal escape sequences (no ESC byte) when its standard output is not a terminal, and SHALL contain none when the `NO_COLOR` environment variable is set to a non-empty value. The only exceptions SHALL be output the user explicitly requested as the recorded terminal bytes: `git paw replay <agent> --raw`, which is written unmodified, escape sequences included, whether or not stdout is a terminal and whether or not `NO_COLOR` is set, and `git paw replay <agent> --cast`, whose player is given the recording unmodified, escape sequences included. A standing test SHALL run each non-interactive subcommand with stdout piped, and again with `NO_COLOR=1`, and fail on any ESC byte.

#### Scenario: Piped output is plain

- **WHEN** `git paw status`, `git paw doctor`, `git paw list-clis` and `git paw --help` run with stdout piped to a file
- **THEN** none of the outputs SHALL contain an ESC byte

#### Scenario: NO_COLOR suppresses escapes

- **GIVEN** `NO_COLOR=1` in the environment
- **WHEN** `git paw --help` runs
- **THEN** its output SHALL contain no ESC byte

#### Scenario: The replay timeline is plain when the record holds escapes

- **GIVEN** a session record whose broker events and terminal capture contain escape sequences
- **WHEN** `git paw replay` runs with stdout piped to a file
- **THEN** the output SHALL contain no ESC byte

#### Scenario: Raw replay output is passed through unmodified

- **GIVEN** `NO_COLOR=1` in the environment and a capture file for `feat/a` containing `\x1b[31mred\x1b[0m`
- **WHEN** the user runs `git paw replay feat/a --raw` with stdout piped to a file
- **THEN** the output SHALL be byte-identical to the capture file, escape sequences included

#### Scenario: Cast playback is given the recording unmodified

- **GIVEN** `NO_COLOR=1` in the environment, an `asciinema` on `PATH`, and a `.cast` recording for `feat/a` whose output events contain `\x1b[31mred\x1b[0m`
- **WHEN** the user runs `git paw replay feat/a --cast`
- **THEN** `asciinema` SHALL be invoked on that `.cast` file, left unmodified with its escape sequences
