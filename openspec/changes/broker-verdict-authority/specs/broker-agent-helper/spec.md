## ADDED Requirements

### Requirement: Root-side helper verbs carry the authority credential

The system SHALL provide a hidden `git paw __authority` subcommand that prints the current repository's session authority credential to stdout and nothing else, exits non-zero with an actionable message when no session with a broker exists, and refuses (non-zero, no output) when invoked from a linked worktree rather than the repository's main working tree — decided as git reports it (the checkout's git directory differs from the common git directory, which also holds under `--separate-git-dir`), the same check `git paw setup-cli` uses. Every `sweep.sh` subcommand that publishes a verdict message — `verified`, `feedback-gate`, and `conflict-ack` (an `agent.feedback` from `supervisor` carrying the `[conflict-ack]` token) — SHALL obtain the credential through `git paw __authority` and send it as the `X-Paw-Authority` header; they SHALL NOT read the user-data session file themselves and SHALL NOT embed the credential in the JSON body. The worker-side `broker.sh` SHALL NOT gain any credential-bearing verb. `__authority` SHALL be hidden from `git paw --help`, like the other internal subcommands (`__broker`, `__dashboard`, `__prompt-profile`).

#### Scenario: __authority prints the credential at the repo root

- **GIVEN** a running session with credential `C` and a shell whose cwd is the repository's main working tree
- **WHEN** `git paw __authority` runs
- **THEN** stdout SHALL be exactly `C` followed by a newline
- **AND** the exit status SHALL be 0

#### Scenario: __authority refuses from a linked worktree

- **GIVEN** a shell whose cwd is a `git worktree add`-created worktree of the repository
- **WHEN** `git paw __authority` runs
- **THEN** it SHALL exit non-zero
- **AND** stdout SHALL be empty

#### Scenario: __authority without a session is actionable

- **GIVEN** no session with a broker exists for the repository
- **WHEN** `git paw __authority` runs
- **THEN** it SHALL exit non-zero with a message naming `git paw start`

#### Scenario: sweep.sh verified sends the header

- **GIVEN** a running broker and `sweep.sh verified feat-x "all gates green"` invoked at the repo root
- **WHEN** the subcommand publishes
- **THEN** the request SHALL carry `X-Paw-Authority` equal to the session credential
- **AND** the broker SHALL respond 202

#### Scenario: sweep.sh conflict-ack sends the header

- **GIVEN** a running broker and `sweep.sh conflict-ack feat-x feat-y src/lib.rs` invoked at the repo root
- **WHEN** the subcommand publishes its two acknowledgements
- **THEN** each request SHALL carry `X-Paw-Authority` equal to the session credential
- **AND** the broker SHALL respond 202 to both

#### Scenario: broker.sh has no credential verb

- **WHEN** `broker.sh --help` is inspected
- **THEN** it SHALL list no `verified` or `feedback` subcommand and SHALL reference no authority credential

#### Scenario: __authority is hidden from help

- **WHEN** `git paw --help` is inspected
- **THEN** `__authority` SHALL NOT appear as a listed subcommand
