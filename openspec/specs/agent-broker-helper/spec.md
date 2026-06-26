# agent-broker-helper Specification

## Purpose
TBD - created by archiving change agent-broker-helper. Update Purpose after archive.
## Requirements
### Requirement: Bundled agent-broker helper script

The system SHALL provide a bundled agent-side broker helper,
`assets/scripts/broker.sh`, that wraps every agent→broker `curl`
interaction the agent is allowed to make. The helper SHALL discover the
broker URL at runtime from `<repo>/.git-paw/config.toml` `[broker]`
(port and bind, defaulting to `http://127.0.0.1:9119`) and SHALL shape
all JSON payloads internally, so callers pass only simple positional
arguments. The helper SHALL be a shell script — NOT a `git paw`
subcommand.

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

### Requirement: Helper publish subcommands

The `broker.sh` helper SHALL expose publish subcommands covering the
agent→broker events the boot block and coordination guidance require:
`status`, `artifact`, `blocked`, `question`, and `intent`. Each
subcommand SHALL POST a well-formed JSON `BrokerMessage` to
`<broker-url>/publish` with the agent's id and the appropriate
`payload`, resolving the agent id from an explicit argument (the
pre-expanded branch id the boot block passes) or, absent one, from the
current worktree branch.

#### Scenario: status publishes agent.status

- **WHEN** `broker.sh status booting` is invoked
- **THEN** the helper SHALL POST an `agent.status` message to
  `/publish` with `payload.status = "working"`, the given message, and
  `modified_files = []`

#### Scenario: artifact publishes the code-less done fallback

- **WHEN** `broker.sh artifact` is invoked
- **THEN** the helper SHALL POST an `agent.artifact` message with
  `payload.status = "done"` and the `exports` and `modified_files`
  fields, using the same JSON shape as the prior raw-curl done fallback

#### Scenario: blocked publishes dependency information

- **WHEN** `broker.sh blocked <needs> <from>` is invoked
- **THEN** the helper SHALL POST an `agent.blocked` message whose
  `payload` carries the supplied `needs` and `from` values

#### Scenario: question publishes agent.question

- **WHEN** `broker.sh question "<text>"` is invoked
- **THEN** the helper SHALL POST an `agent.question` message whose
  `payload.question` is the supplied text

#### Scenario: intent publishes agent.intent

- **WHEN** `broker.sh intent <summary> <files> [valid_for_seconds]` is
  invoked
- **THEN** the helper SHALL POST an `agent.intent` message carrying the
  summary, the files the agent is about to touch, and (when supplied) a
  `valid_for_seconds` field

### Requirement: Helper poll subcommand

The `broker.sh` helper SHALL expose a `poll` subcommand that performs a
read of the agent's own broker inbox so the agent can observe peer
artifacts and any feedback/inbox messages routed to it. The read SHALL
target `GET <broker-url>/messages/<agent-id>` and SHALL accept an
optional `since` cursor.

#### Scenario: poll reads the agent inbox

- **WHEN** `broker.sh poll` is invoked for an agent
- **THEN** the helper SHALL issue a `GET` against
  `<broker-url>/messages/<agent-id>` and emit the returned messages

#### Scenario: poll honours a since cursor

- **WHEN** `broker.sh poll <n>` is invoked
- **THEN** the request SHALL include `since=<n>` so only messages newer
  than the cursor are returned

### Requirement: Helper convention discipline

The `broker.sh` helper SHALL avoid the stdin-claiming
`interpreter - <<` heredoc shape (e.g. `python3 - <<'PY'`), passing any
embedded interpreter script via `-c "$(cat <<'EOF' … EOF)"` so an
upstream pipe's stdin is never swallowed. A convention test SHALL fail
if such a shape is reintroduced, identifying the offending line.

#### Scenario: No stdin-claiming heredoc shape in the helper

- **WHEN** the `broker.sh` convention test scans the script body
- **THEN** it SHALL report no `interpreter - <<` (e.g. `python3 - <<`)
  occurrence on a non-comment line

#### Scenario: Reintroduced heredoc shape fails the test

- **GIVEN** a synthetic script body containing a `python3 - <<'PY'`
  block
- **WHEN** the convention scanner runs against it
- **THEN** it SHALL flag the offending shape and identify the line

