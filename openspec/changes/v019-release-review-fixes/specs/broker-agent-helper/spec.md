## ADDED Requirements

### Requirement: Bundled helpers bound every broker HTTP call

Every broker HTTP call the bundled helpers make SHALL carry a connect timeout and a total time bound (2s and 10s), so a port that accepts a connection but never answers fails the call instead of hanging its caller. A long-poll MAY raise the total bound past its wait. A loopback `http://host:port` in `GIT_PAW_BROKER_URL` SHALL take precedence over the `[broker]` address in `.git-paw/config.toml`; any other value SHALL be ignored with a warning.

#### Scenario: A broker that accepts but never answers cannot hang a helper

- **GIVEN** a `[broker] port` held by a listener that accepts connections and never responds
- **WHEN** `sweep.sh learn` runs
- **THEN** it SHALL exit non-zero within 20 seconds

Test: `tests/helper_publish_contract.rs::learn_against_a_broker_that_never_answers_returns_within_the_bound`

#### Scenario: Every broker call in the bundled helpers is bounded

- **WHEN** the bundled `_paw_common.sh`, `broker.sh` and `sweep.sh` are read
- **THEN** every `curl` call SHALL pass the shared broker-call bounds

Test: `tests/helper_publish_contract.rs::every_broker_curl_in_the_helpers_passes_the_bounds`

#### Scenario: GIT_PAW_BROKER_URL overrides the configured port

- **GIVEN** a `[broker] port` nothing listens on and `GIT_PAW_BROKER_URL` naming a live broker
- **WHEN** `sweep.sh learn` runs
- **THEN** the learning SHALL reach the broker named by `GIT_PAW_BROKER_URL`

Test: `tests/helper_publish_contract.rs::learn_publishes_to_the_broker_url_override`

#### Scenario: A non-loopback GIT_PAW_BROKER_URL is ignored

- **GIVEN** a live broker on the configured `[broker] port` and `GIT_PAW_BROKER_URL` set to `http://evil.example:1`, `-K/nonexistent` or a URL with a path
- **WHEN** `sweep.sh learn` runs
- **THEN** it SHALL warn that the value is ignored and publish to the configured broker

Test: `tests/helper_publish_contract.rs::a_non_loopback_broker_url_override_is_ignored`

## MODIFIED Requirements

### Requirement: Bundled agent-broker helper script

The system SHALL provide a bundled agent-side broker helper,
`assets/scripts/broker.sh`, that wraps every agent→broker `curl`
interaction the agent is allowed to make. The helper SHALL discover the
broker URL at runtime from `<repo>/.git-paw/config.toml` `[broker]`
(port and bind, defaulting to `http://127.0.0.1:9119`), unless a loopback
`GIT_PAW_BROKER_URL` is set, and SHALL shape all JSON payloads internally,
so callers pass only simple positional arguments. The helper SHALL be a
shell script — NOT a `git paw` subcommand.

#### Scenario: Helper discovers the broker URL from config

- **WHEN** `broker.sh` runs in a repo whose `.git-paw/config.toml`
  `[broker]` section sets a non-default `port`
- **THEN** the helper SHALL target the configured `http://<bind>:<port>`
  broker URL rather than a hardcoded one

#### Scenario: Helper defaults the broker URL when config is absent

- **GIVEN** a repo with no `.git-paw/config.toml`
- **WHEN** `broker.sh` runs
- **THEN** it SHALL default the broker URL to `http://127.0.0.1:9119`

#### Scenario: Helper is a script, not a subcommand

- **WHEN** the agent→broker interaction surface is inspected
- **THEN** it SHALL be a script installed under `.git-paw/scripts/`
- **AND** the system SHALL NOT expose a `git paw publish` (or equivalent
  agent-publish) subcommand on the user-facing CLI
