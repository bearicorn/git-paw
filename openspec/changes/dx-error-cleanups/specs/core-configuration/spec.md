## ADDED Requirements

### Requirement: The config loader returns its warnings as values

Loading configuration SHALL NOT write to stderr. It SHALL return every warning it finds (worktree-runtime warnings, deprecated tables and keys) with the configuration, each naming the file it came from, and SHALL keep no process-global state to deduplicate them. The command that loaded the configuration SHALL emit each warning exactly once through its output sink.

#### Scenario: A deprecated table warns once per command

- **GIVEN** a repository config that declares a deprecated table
- **WHEN** `git paw start` runs and loads the configuration more than once
- **THEN** stderr SHALL contain that warning exactly once

#### Scenario: Two loads in one process each return the warning

- **GIVEN** a config file that produces a warning
- **WHEN** it is loaded twice in one process
- **THEN** each load SHALL return the warning

#### Scenario: An in-process caller captures the warning

- **GIVEN** a config that produces a warning, and a command run in-process with a capturing output sink
- **WHEN** the command loads the configuration
- **THEN** the sink SHALL hold the warning and the process SHALL NOT have written it to stderr
