# agent-broker-helper Specification

## Purpose
Provides bundled shell helpers (`broker.sh` for agents, `sweep.sh` for the supervisor) that wrap every agent→broker interaction — status, artifact, blocked, question, intent, poll, and the supervisor status/verified/feedback-gate verbs — so no participant hand-rolls a raw `curl …/publish`. The helpers discover the broker URL from config, shape all JSON internally, and are provisioned into each agent worktree at start/add time.

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

The bundled broker helpers SHALL expose publish subcommands covering the
broker events the boot block, coordination guidance, and supervisor
introspection require, so that NEITHER a coding agent NOR the supervisor
ever needs to hand-roll a raw `curl …/publish` call.

The agent-side `broker.sh` helper SHALL expose `status`, `artifact`,
`blocked`, `question`, and `intent`. The supervisor-side `sweep.sh` helper
SHALL expose `status-publish` (the supervisor `agent.status` verb),
`verified`, and `feedback-gate`. Each subcommand SHALL POST a well-formed
JSON `BrokerMessage` to `<broker-url>/publish` with the publishing agent's
id and the appropriate `payload`, shaping the JSON internally so callers
pass only simple positional/flag arguments. The agent id SHALL be resolved
from an explicit argument (the pre-expanded branch id the boot block passes)
or, absent one, from the current worktree branch; the supervisor verbs
publish as `agent_id = "supervisor"`.

The supervisor-side `sweep.sh status-publish` verb SHALL accept the FULL
`agent.status` payload the introspection taxonomy emits: a free-form
`message` plus an OPTIONAL `phase` label and an OPTIONAL structured `detail`
object. The verb SHALL preserve its plain form `status-publish <message…>`
(no `phase`, no `detail`) byte-for-byte, AND SHALL accept a rich form that
adds a `phase` and/or a `detail` JSON object. When `phase`/`detail` are
supplied the helper SHALL embed them in the published `agent.status` payload;
when they are absent the payload SHALL omit those keys (v0.5.0 wire shape).
A `detail` argument that does not parse to a JSON object SHALL be rejected
(non-zero exit, diagnostic on stderr) rather than published.

Because the bundled helpers cover the full `agent.status` surface (`phase` +
`detail`), the bundled supervisor and coordination skills SHALL route every
broker `agent.status` publish through the helper and SHALL NOT contain a raw
`curl …/publish` example whose body is an `agent.status`.

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

#### Scenario: status-publish plain form preserves the v0.5.0 shape

- **WHEN** `sweep.sh status-publish "merge orchestration complete"` is
  invoked with no phase or detail
- **THEN** the helper SHALL POST an `agent.status` message with
  `agent_id = "supervisor"`, `payload.status = "working"`,
  `payload.modified_files = []`, and `payload.message` set to the given
  text
- **AND** the published payload SHALL contain no `phase` key and no `detail`
  key

#### Scenario: status-publish carries a phase and a structured detail

- **WHEN** `sweep.sh status-publish --phase audit --detail '{"branch":"feat/auth","audit_step":"tests"}' "auditing feat/auth"` is invoked
- **THEN** the helper SHALL POST an `agent.status` message with
  `agent_id = "supervisor"`, `payload.phase = "audit"`, and a
  `payload.detail` object whose `branch` is `"feat/auth"` and whose
  `audit_step` is `"tests"`
- **AND** `payload.message` SHALL be the supplied text

#### Scenario: status-publish rejects a non-object detail argument

- **WHEN** `sweep.sh status-publish --phase audit --detail 'not-json' "msg"`
  is invoked
- **THEN** the helper SHALL exit non-zero and emit a diagnostic on stderr
- **AND** SHALL NOT POST an `agent.status` carrying a string or null
  `detail`

#### Scenario: supervisor skill contains no raw agent.status curl

- **WHEN** the bundled `supervisor.md` is scanned for `curl …/publish`
  examples
- **THEN** no `/publish` example body SHALL be an `agent.status`
  (`"type":"agent.status"`)
- **AND** every documented supervisor `agent.status` emission — boot
  self-register, each phase-taxonomy example, the audit example, the
  `checkpoint` example, and the final-summary status — SHALL use
  `sweep.sh status-publish`

#### Scenario: rich status-publish needs no broad curl grant

- **GIVEN** the supervisor's permission allowlist seeded with the by-path
  grant for `.git-paw/scripts/sweep.sh`
- **WHEN** the supervisor publishes a phase-tagged `agent.status` via
  `sweep.sh status-publish --phase <p> --detail '<obj>' "<msg>"`
- **THEN** the invocation SHALL be covered by the existing by-path grant
- **AND** no broad `curl *` grant SHALL be required to publish the status

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

### Requirement: Helpers provisioned into agent worktrees

`git paw start` and `git paw add` SHALL provision the bundled helper scripts an agent invokes into that agent's worktree at `.git-paw/scripts/`, making them present and executable before the agent boots, so the agent never has to hand-copy a helper from `assets/`. The scripts SHALL be sourced from the same bundled assets `git paw init` uses (matching the running binary's version), and provisioning SHALL be idempotent — attaching to a fresh or reused worktree (re)writes the scripts rather than failing. `broker.sh` SHALL be provisioned whenever the broker is enabled; `docs-fetch.sh` SHALL be provisioned whenever `docs_base_url` is configured (mirroring the docs-fetch skill's injection gate).

#### Scenario: start provisions the broker helper into each worktree

- **GIVEN** a supervisor session with the broker enabled
- **WHEN** `git paw start` sets up an agent's worktree
- **THEN** `<worktree>/.git-paw/scripts/broker.sh` exists and is executable before the agent's boot prompt is submitted
- **AND** the agent does not need to copy the helper from `assets/`

#### Scenario: add provisions the helper into a mid-session worktree

- **WHEN** `git paw add <branch>` attaches a new agent worktree to a broker-enabled session
- **THEN** that worktree's `.git-paw/scripts/broker.sh` exists and is executable, identical to a start-time agent's

#### Scenario: docs-fetch helper provisioned only when configured

- **WHEN** an agent worktree is set up in a project that has configured `docs_base_url`
- **THEN** `<worktree>/.git-paw/scripts/docs-fetch.sh` is provisioned alongside `broker.sh`
- **AND** when `docs_base_url` is unset, `docs-fetch.sh` is not provisioned

#### Scenario: provisioning is idempotent and version-matched

- **WHEN** an agent worktree that already contains `.git-paw/scripts/` is re-attached (a repeat `start`/`add`)
- **THEN** the helper scripts are refreshed from the running binary's bundled assets without error, so a worktree's helper matches the binary that launched the session

