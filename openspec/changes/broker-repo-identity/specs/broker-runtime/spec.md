## MODIFIED Requirements

### Requirement: Stale broker detection

Before binding, the system SHALL probe `GET <config.url()>/status` with a timeout of at most 500 milliseconds and SHALL act on the result as follows:

- **No connection / connection refused** → bind and start a new broker
- **HTTP 200 with a response body containing the marker field `"git_paw": true` AND a `repo_root` equal to the caller's repository** → return a `BrokerHandle` that reattaches to the existing broker without binding
- **HTTP 200 with the marker field but a different `repo_root`** → return an error stating that the port is held by another repository's git-paw broker, naming that repository and the port, and telling the user to set a distinct `[broker] port` for one of them. The system SHALL NOT reattach, because reattaching would route this repository's agents into the other repository's broker while this repository's dashboard renders an empty, unwatched state
- **HTTP 200 with the marker field and no `repo_root`** (a broker from a git-paw version that does not report it) → treat the owner as unknown and return the same error, naming the port; the system SHALL NOT assume the broker belongs to the caller
- **HTTP response without the marker field** → return `Err(BrokerError::PortInUse { .. })` indicating the port is occupied by a foreign process
- **Timeout** → return `Err(BrokerError::ProbeTimeout { .. })` so the user can investigate

#### Scenario: Probe finds no listener and proceeds to bind

- **GIVEN** no process is listening on `127.0.0.1:9119`
- **WHEN** `start_broker` is called with `port = 9119`
- **THEN** the probe returns connection-refused
- **AND** `start_broker` proceeds to bind and returns `Ok(BrokerHandle)`

#### Scenario: Probe finds a live git-paw broker and reattaches

- **GIVEN** a live git-paw broker for the same repository is already running on `127.0.0.1:9119`
- **WHEN** `start_broker` is called with the same config
- **THEN** the probe receives an HTTP 200 with `"git_paw": true` in the body
- **AND** `start_broker` returns `Ok(BrokerHandle)` without binding a new socket

#### Scenario: Probe finds a foreign HTTP server and refuses

- **GIVEN** a non-git-paw HTTP server is bound to `127.0.0.1:9119` and responds to `GET /status` with `404`
- **WHEN** `start_broker` is called with `port = 9119`
- **THEN** the probe receives a response without the `"git_paw"` marker
- **AND** `start_broker` returns `Err(BrokerError::PortInUse { .. })` mentioning the port

#### Scenario: Another repository's broker on the port is refused

- **GIVEN** a live git-paw broker for repository A on `127.0.0.1:9119`
- **WHEN** a session for repository B calls `start_broker` with `port = 9119`
- **THEN** it SHALL return an error naming repository A and port `9119`
- **AND** the error SHALL tell the user to set a distinct `[broker] port`
- **AND** no `BrokerHandle` SHALL be returned for repository B

#### Scenario: A broker that does not report its repository is not assumed to be ours

- **GIVEN** a live git-paw broker on the port whose `GET /status` body has no `repo_root`
- **WHEN** `start_broker` is called
- **THEN** it SHALL return the port-held error rather than reattaching

### Requirement: GET /status returns broker and agent state

The system SHALL expose `GET /status` returning the current state of the broker and all known agents. The response SHALL be HTTP `200 OK` with an `application/json` body containing at least these fields:

- `git_paw: bool` — always `true`; serves as the marker the stale-broker probe checks
- `version: String` — the git-paw crate version (`env!("CARGO_PKG_VERSION")`)
- `uptime_seconds: u64` — seconds since the broker started
- `agents: Array<AgentStatusEntry>` — the list returned by `agent_status_snapshot`
- `repo_root: String` — the canonical path of the repository this broker serves; the stale-broker probe compares it with the caller's repository

The handler MUST be safe to call concurrently. The handler MUST NOT block for more than a few milliseconds.

#### Scenario: Status response contains the marker field

- **GIVEN** a running broker
- **WHEN** `GET /status` is sent
- **THEN** the response status is `200`
- **AND** the response body is JSON
- **AND** the body contains `"git_paw": true`

#### Scenario: Status response contains version and uptime

- **GIVEN** a running broker
- **WHEN** `GET /status` is sent
- **THEN** the response body contains a `version` string field
- **AND** the response body contains a `uptime_seconds` numeric field

#### Scenario: Status response contains empty agents array in Wave 1

- **GIVEN** a running broker (Wave 1, stub `agent_status_snapshot` returns empty)
- **WHEN** `GET /status` is sent
- **THEN** the response body contains `"agents": []`

#### Scenario: Status endpoint is reachable concurrently

- **GIVEN** a running broker
- **WHEN** ten concurrent `GET /status` requests are sent
- **THEN** all ten responses are `200`
- **AND** all ten bodies contain `"git_paw": true`

#### Scenario: Status response identifies the repository

- **GIVEN** a running broker started for a repository
- **WHEN** `GET /status` is sent
- **THEN** the response body contains a `repo_root` field equal to that repository's canonical path

### Requirement: Dashboard subcommand starts broker and dashboard

The system SHALL handle the hidden `__dashboard` subcommand by:

1. Loading `BrokerConfig` from `.git-paw/config.toml`
2. Constructing a `BrokerState` with `log_path` set to `<session_state_dir>/<session_name>.broker.log` — one log per session, so two repositories never write, or purge, the same file
3. Calling `start_broker(config, state.clone())` to obtain a `BrokerHandle`
4. Calling `run_dashboard(state, handle)` which blocks until the user presses `q`
5. Returning `Ok(())` on clean exit

The subcommand SHALL refuse to run if the `$TMUX` environment variable is not set, returning an error indicating it is an internal command intended to run inside tmux.

#### Scenario: Dashboard subcommand starts broker and blocks

- **GIVEN** `$TMUX` is set and `[broker]` config is valid
- **WHEN** `git paw __dashboard` is executed
- **THEN** a broker starts listening on the configured port
- **AND** the dashboard renders in the terminal

#### Scenario: Dashboard subcommand refuses outside tmux

- **GIVEN** `$TMUX` is not set
- **WHEN** `git paw __dashboard` is executed
- **THEN** it returns an error mentioning "internal command" and "tmux"

#### Scenario: Two repositories keep separate broker logs

- **GIVEN** broker-enabled sessions for two repositories, each on its own port
- **WHEN** both brokers record messages
- **THEN** each SHALL write to its own `<session_name>.broker.log`
- **AND** purging one session SHALL NOT delete the other session's log
