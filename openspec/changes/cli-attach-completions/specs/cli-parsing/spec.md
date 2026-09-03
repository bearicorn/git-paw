## ADDED Requirements

### Requirement: Attach subcommand

The `attach` subcommand SHALL reattach the current terminal to the running tmux session for the current repository, resolving the session by the repository path (as `status` does) and invoking the tmux attach path. When no session exists for the repository, or the session is not alive, it SHALL exit with an actionable error naming that no session is running and how to start one. This provides a first-class `git paw attach` in place of the raw `tmux attach -t paw-<project>`.

#### Scenario: Attach reattaches to the repository's running session

- **GIVEN** a running session for the current repository
- **WHEN** the user runs `git paw attach`
- **THEN** the system SHALL attach the terminal to that session

#### Scenario: Attach with no running session errors actionably

- **GIVEN** no running session exists for the current repository
- **WHEN** the user runs `git paw attach`
- **THEN** the system SHALL exit with an error stating no session is running and how to start one

### Requirement: Completions subcommand

The `completions` subcommand SHALL print a shell-completion script for a requested shell (bash, zsh, fish) to stdout, generated from the clap command definition, so a user can install completions through their shell's standard mechanism. A missing or unsupported shell argument SHALL produce an actionable error.

#### Scenario: Completions prints a script for a supported shell

- **WHEN** the user runs `git paw completions bash`
- **THEN** the system SHALL print a bash completion script to stdout and exit successfully

#### Scenario: Completions rejects an unsupported shell

- **WHEN** the user runs `git paw completions <unsupported>`
- **THEN** the system SHALL exit with an actionable error naming the supported shells

### Requirement: No `resume` subcommand; reattach is `attach`, revival is `start`

The CLI SHALL NOT provide a `resume` subcommand. Reattaching the current terminal to a running session SHALL be `git paw attach`; reviving a paused or stopped session SHALL remain `git paw start`. The documentation (CLI reference, README, user guide) SHALL NOT reference a `git paw resume` command.

#### Scenario: `git paw resume` is not a valid subcommand

- **WHEN** the user runs `git paw resume`
- **THEN** the system SHALL reject it as an unknown subcommand

#### Scenario: Docs do not reference a resume command

- **WHEN** the CLI reference and user guide are inspected
- **THEN** they SHALL NOT reference a `git paw resume` command; reattach is documented as `git paw attach` and session revival as `git paw start`
