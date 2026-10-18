## ADDED Requirements

### Requirement: Launcher mints a per-session authority credential

Every `start` path that launches a fresh session with the broker enabled (bare, spec-driven, supervised) SHALL generate a fresh authority credential of at least 32 bytes of OS-provided randomness, hex-encoded; SHALL hand it to the broker host (`git paw __broker`) on the host's standard input when it starts the host; and SHALL persist that same credential in the user-data `Session` when it saves the session receipt after the panes exist — never earlier, so a launch that fails leaves the persisted session state unchanged (session-state *A failed start preserves persisted session state*). A path that restarts the host for an existing session (recovery, restart from pause) SHALL hand the host the credential its saved `Session` carries, if any, and any later save of that `Session` in the same flow SHALL keep the credential the host persisted rather than overwrite it. The credential SHALL NOT be passed to any process as a command-line argument or environment variable.

#### Scenario: Supervised start records a credential

- **GIVEN** `[broker] enabled = true`
- **WHEN** `git paw start --supervisor` saves session state
- **THEN** the saved `Session` SHALL carry a non-empty `broker_authority`
- **AND** its value SHALL be at least 64 hexadecimal characters

#### Scenario: Bare start with the broker also records a credential

- **GIVEN** `[broker] enabled = true` and no supervisor mode
- **WHEN** `git paw start` saves session state
- **THEN** the saved `Session` SHALL carry a non-empty `broker_authority`

#### Scenario: The host and the receipt hold the same credential

- **GIVEN** a fresh `git paw start` with the broker enabled has completed
- **WHEN** a client POSTs `agent.verified` with `X-Paw-Authority` equal to the saved `Session`'s `broker_authority`
- **THEN** the response SHALL be 202

#### Scenario: A failed start persists no credential

- **GIVEN** a repository with no saved session
- **WHEN** a fresh `git paw start` starts its broker host and then errors before its panes exist
- **THEN** no session receipt, and so no credential, SHALL have been saved for the repository

#### Scenario: Credential never appears on a command line or in the environment

- **WHEN** the launch flow's emitted process invocations — the broker host's included — and session-environment settings are inspected
- **THEN** none SHALL contain the credential value

### Requirement: Broker host loads and enforces the credential

The broker host process (`git paw __broker`) SHALL take the authority credential from its standard input when the launcher hands one, and otherwise SHALL read `broker_authority` from the saved `Session` of the session it was named; it SHALL hand the credential to the `/publish` handler for enforcement. When it is handed no credential and the loaded `Session` carries none (a session saved before this requirement, being recovered or restarted from pause), the host SHALL mint one, persist it to the `Session` before it serves its first request, and enforce it; it SHALL NOT back-fill in any other case. A broker started with no credential at all SHALL accept no verdict message (fail closed). The host SHALL NOT log the credential. The in-process drive loop SHALL attach the credential to every verdict message it publishes.

#### Scenario: Host enforces the launcher's credential

- **GIVEN** a launcher that started the broker host with credential `C` on its standard input
- **WHEN** a client POSTs `agent.verified` with `X-Paw-Authority: C`
- **THEN** the response SHALL be 202

#### Scenario: A session without a credential gains one

- **GIVEN** a `Session` file saved without `broker_authority`
- **WHEN** recovery starts the broker host for that session
- **THEN** the `Session` file SHALL be re-saved with a non-empty `broker_authority`
- **AND** `agent.verified` without that credential SHALL be refused with 403

#### Scenario: Restart from pause keeps a back-filled credential

- **GIVEN** a paused session whose `Session` file has no `broker_authority`
- **WHEN** `git paw start` restarts it from pause and the restart completes
- **THEN** the saved `Session` SHALL carry the credential the host minted
- **AND** `agent.verified` with that credential SHALL be accepted with 202

#### Scenario: The drive loop's branch-refresh notification is accepted

- **GIVEN** an `--unattended` session with branch refresh enabled
- **WHEN** the drive loop publishes its `agent.feedback` refresh notification from `supervisor`
- **THEN** the broker SHALL accept it with 202

### Requirement: Credential is confined to root-side surfaces

The authority credential SHALL be readable only from the user-data `Session` file and the hidden `git paw __authority` subcommand. It SHALL NOT appear in the per-repo discovery JSON (`.git-paw/sessions/<name>.json`), in `GET /status` or `GET /log` responses, in `git paw status --json`, in the MCP `get_session_status` response, in the session's broker log, or in any diagnostic output.

#### Scenario: Discovery file carries no credential

- **GIVEN** a running session with a credential
- **WHEN** `.git-paw/sessions/<name>.json` is inspected
- **THEN** it SHALL NOT contain the credential value or a `broker_authority` key

#### Scenario: Status surfaces carry no credential

- **GIVEN** a running session with credential `C`
- **WHEN** `GET /status`, `git paw status --json`, and MCP `get_session_status` are each invoked
- **THEN** none of the responses SHALL contain `C`

#### Scenario: Broker log carries no credential

- **GIVEN** an accepted `agent.verified` published with the credential
- **WHEN** the broker log is flushed and inspected
- **THEN** it SHALL contain the message and SHALL NOT contain the credential value
