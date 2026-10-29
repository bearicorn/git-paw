## ADDED Requirements

### Requirement: --worker-backend selects a worker's backend

The `start` subcommand SHALL accept a repeatable `--worker-backend <BRANCH>=<BACKEND>`, and the `add` subcommand a single `--worker-backend <BACKEND>`, whose accepted backends are exactly `tmux`, `headless` and `remote`. A value that is not of that form, or names any other backend, SHALL be rejected by argument parsing with exit code 2 before any side effect. On `start`, a `<BRANCH>` that is not among the branches the launch creates, or a branch named twice, SHALL fail with exit code 1 before anything is created. A worker the flag does not name SHALL use the session's backend. Both flags SHALL appear in their subcommand's `--help` with a description naming the three values, stating that the session's backend is the default, and pointing to the remote-workers chapter, and the `start` long-about examples SHALL include one example with a remote worker.

#### Scenario: The flags parse every value

- **GIVEN** the `start` and `add` parsers
- **WHEN** `git paw start --branches feat/a,feat/b,feat/c --worker-backend feat/b=headless --worker-backend feat/c=remote` and `git paw add feat/d --worker-backend remote` are parsed
- **THEN** the parsed start arguments SHALL map `feat/b` to headless and `feat/c` to remote
- **AND** the parsed add arguments SHALL carry the remote backend

#### Scenario: An unknown backend is a parse error

- **WHEN** `git paw start --branches feat/a --worker-backend feat/a=zellij` runs
- **THEN** it SHALL exit with code 2
- **AND** stderr SHALL name the accepted values `tmux`, `headless` and `remote`
- **AND** no worktree, session file or terminal session SHALL be created

#### Scenario: A branch the launch does not create is refused

- **GIVEN** a repository with no session
- **WHEN** `git paw start --branches feat/a --worker-backend feat/z=remote` runs
- **THEN** it SHALL exit with code 1 naming `feat/z`
- **AND** no worktree, session file or terminal session SHALL be created

#### Scenario: Help documents the flag

- **WHEN** `git paw start --help` and `git paw add --help` are run
- **THEN** each output SHALL contain `--worker-backend`, `tmux`, `headless` and `remote`
