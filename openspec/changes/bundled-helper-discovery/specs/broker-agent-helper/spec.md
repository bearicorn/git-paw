## ADDED Requirements

### Requirement: Bundled helpers read the per-repo discovery file's agent roster

The bundled helper scripts SHALL resolve agents from the per-repo discovery file (`.git-paw/sessions/<session>.json`) using that file's own shape — the `agents` roster, whose entries carry `branch_id`, `worktree_path`, `cli` and `pane_index` — and SHALL NOT read the global session-state shape (`worktrees` entries carrying `branch`). Mapping a pane's working directory to its agent SHALL therefore succeed against a discovery file written by any launch path. Test fixtures that model the discovery file SHALL use the same shape, so the helper's tests cannot pass against a document git-paw never writes.

A helper MAY additionally infer the session from its own environment when it runs inside the session's multiplexer, as a secondary path. When no session can be resolved, the helper SHALL say so explicitly rather than degrading every subcommand to an empty result.

`sweep.sh status` SHALL decide which broker agents are phantoms from the same roster: an agent id is known when it is `supervisor` or equals a roster entry's `branch_id`, whatever branch prefix the agent's branch uses (for example the configurable spec `branch_prefix`). It SHALL NOT recognise agents by a hard-coded branch-name pattern such as `feat[-/]…`. When no discovery file exists, it SHALL fall back to the broker's own agent-id rule — `supervisor`, or a `<prefix>-<name>` / `<prefix>/<name>` slug with any prefix.

#### Scenario: A pane path resolves to its agent from the discovery file

- **GIVEN** a per-repo discovery file whose `agents` roster contains an agent with `branch_id` `feat-x` and `worktree_path` `/repo/.git-paw/worktrees/feat-x`
- **WHEN** the helper resolves the agent for a pane whose working directory is inside that worktree
- **THEN** it SHALL resolve `feat-x`

#### Scenario: Fixtures use the shape git-paw writes

- **WHEN** the helper's tests construct a discovery file
- **THEN** they SHALL use the `agents` roster shape written by the per-repo discovery writer

#### Scenario: No active session is reported explicitly

- **GIVEN** a repository with no active session
- **WHEN** a bundled helper subcommand that needs the session is invoked
- **THEN** it SHALL report that no session was found
- **AND** SHALL NOT exit as though it had done its work

#### Scenario: An agent on a non-feat branch is not a phantom

- **GIVEN** a discovery file whose `agents` roster contains `spec-add-auth` (a spec-driven branch under the default `spec/` prefix) and a broker `/status` listing `supervisor`, `spec-add-auth` and `a`
- **WHEN** `sweep.sh status` is invoked
- **THEN** it SHALL show rows for `supervisor` and `spec-add-auth`
- **AND** SHALL list only `a` as a phantom

#### Scenario: Without a discovery file the broker's id rule decides phantoms

- **GIVEN** no discovery file and a broker `/status` listing `supervisor`, `fix-db-timeout` and `a`
- **WHEN** `sweep.sh status` is invoked
- **THEN** it SHALL show rows for `supervisor` and `fix-db-timeout`
- **AND** SHALL list only `a` as a phantom

## MODIFIED Requirements

### Requirement: Helpers provisioned into agent worktrees

`git paw start` and `git paw add` SHALL provision the bundled helper scripts an agent invokes into that agent's worktree at `.git-paw/scripts/`, making them present and executable before the agent boots, so the agent never has to hand-copy a helper from `assets/`. This SHALL hold on every launch path that creates an agent worktree — the supervisor launch, the bare launch, the spec-driven launch with or without supervisor mode, and `git paw add` — since an agent's need for the helper does not depend on how its session was started. The scripts SHALL be sourced from the same bundled assets `git paw init` uses (matching the running binary's version), and provisioning SHALL be idempotent — attaching to a fresh or reused worktree (re)writes the scripts rather than failing. `broker.sh` SHALL be provisioned whenever the broker is enabled; `docs-fetch.sh` SHALL be provisioned whenever `docs_base_url` is configured (mirroring the docs-fetch skill's injection gate).

#### Scenario: start provisions the broker helper into each worktree

- **GIVEN** a supervisor session with the broker enabled
- **WHEN** `git paw start` sets up an agent's worktree
- **THEN** `<worktree>/.git-paw/scripts/broker.sh` exists and is executable before the agent's boot prompt is submitted
- **AND** the agent does not need to copy the helper from `assets/`

#### Scenario: A bare launch provisions the helper

- **GIVEN** a broker-enabled session launched with `git paw start --branches <branch>` (no supervisor mode, no specs)
- **WHEN** the agent worktrees are set up
- **THEN** each worktree's `.git-paw/scripts/broker.sh` exists and is executable before that agent's pane is launched

#### Scenario: A spec-driven launch without supervisor mode provisions the helper

- **GIVEN** a broker-enabled session launched with `--specs` and no supervisor mode
- **WHEN** the agent worktrees are set up
- **THEN** each worktree's `.git-paw/scripts/broker.sh` exists and is executable before that agent's prompt is submitted
- **AND** no agent needs to copy the helper from `assets/` in order to publish its first status

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

Each helper SHALL be invocable at a single stable path, so that the command
classifier can recognise it by that path. The absence of a broad `curl *`
grant SHALL NOT depend on any seeded settings-file allowlist.

The `broker.sh intent` subcommand SHALL always publish a `valid_for_seconds`
field, because the broker requires it: when the caller supplies no TTL the
helper SHALL send a default of `900` seconds. Every publish subcommand of both
helpers SHALL check the broker's HTTP response: when the broker does not accept
the publish (any non-2xx status, or no response), the helper SHALL print the
broker's error to stderr and exit non-zero, and SHALL NOT report the publish as
done.

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
  summary, the files the agent is about to touch, and a
  `valid_for_seconds` field (the supplied value, or the default)

#### Scenario: intent without a TTL is accepted by the broker

- **GIVEN** a running broker
- **WHEN** `broker.sh intent "wire auth" src/auth.rs` is invoked with no TTL
- **THEN** the published `agent.intent` SHALL carry `valid_for_seconds = 900`
- **AND** the broker SHALL accept it with `202`

#### Scenario: A rejected publish is not silent

- **GIVEN** a running broker that answers a publish with `400`
- **WHEN** a `broker.sh` or `sweep.sh` publish subcommand sends that publish
- **THEN** the helper SHALL exit non-zero
- **AND** SHALL print the broker's error to stderr

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

- **GIVEN** a supervisor invoking the bundled helper at its stable path
  `.git-paw/scripts/sweep.sh`
- **WHEN** the supervisor publishes a phase-tagged `agent.status` via
  `sweep.sh status-publish --phase <p> --detail '<obj>' "<msg>"`
- **THEN** the invocation SHALL be recognisable by that stable helper path
- **AND** no broad `curl *` grant SHALL be required to publish the status
- **AND** the guarantee SHALL NOT depend on a seeded settings-file allowlist
