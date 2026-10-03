## MODIFIED Requirements

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

Each helper SHALL be invocable at a single stable path, so that a permission
rule scoped to that exact path can name it. Publishing through a helper SHALL
NOT require a broad `curl *` permission grant.

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
- **THEN** the invocation SHALL begin with that stable helper path, so a
  permission rule scoped to the exact path can name it
- **AND** no broad `curl *` grant SHALL be required to publish the status

### Requirement: sweep.sh approve re-confirms a live prompt before sending keys

The bundled `assets/scripts/sweep.sh` `approve <pane>` subcommand SHALL answer a live permission prompt once, and only when a human or the orchestrator runs it explicitly; no git-paw process SHALL invoke it. It SHALL pass through the approval-send gate: after taking the pane's exclusive claim, and immediately before sending keys, it SHALL run a fresh `tmux capture-pane` of the target pane and SHALL confirm a live permission prompt per the Live-prompt gate (`approval-auto`). It SHALL then always answer with the one-time "Yes" option: the digit `1` followed by a separate `Enter`.

The subcommand SHALL NOT consult a classifier or any other judgement of the pending command, SHALL NOT send a cursor-movement sequence, and SHALL NOT select any option other than `1`. Before sending, it SHALL confirm from the same fresh capture that a numbered option `1` is present and that its line is not a durable grant, meaning it does not contain the resolved prompt profile's broad-grant marker. When option `1` is missing or reads as a durable grant, it SHALL send NO keystrokes and SHALL report why. When the re-confirm capture shows no live prompt (the prompt has cleared), it SHALL send NO keystrokes and SHALL report that the prompt cleared.

#### Scenario: approve sends 1 then Enter when the prompt is still live

- **GIVEN** `sweep.sh approve <pane>` is invoked for a coding-agent pane whose fresh capture shows a live 2-option Yes/No permission prompt
- **WHEN** the subcommand runs
- **THEN** it SHALL send `1` and then a separate `Enter` to the pane via `tmux send-keys`
- **AND** SHALL NOT send `Down`

#### Scenario: approve answers once on a three-option prompt

- **GIVEN** a live prompt whose options are `1. Yes`, `2. Yes, and don't ask again for …` and `3. No`
- **WHEN** `sweep.sh approve <pane>` runs
- **THEN** it SHALL send `1` then `Enter`
- **AND** SHALL NOT send `2`

#### Scenario: approve does not judge the command

- **GIVEN** a live 2-option prompt whose command is `git push --force origin main`
- **WHEN** a human runs `sweep.sh approve <pane>` for it
- **THEN** it SHALL send `1` then `Enter`, exactly as for any other live prompt
- **AND** it SHALL NOT invoke `git paw __classify`

#### Scenario: approve refuses an option 1 that reads as a durable grant

- **GIVEN** a live prompt whose option `1` line contains the resolved broad-grant marker (e.g. `1. Yes, and don't ask again`)
- **WHEN** `sweep.sh approve <pane>` runs
- **THEN** it SHALL send NO keystrokes
- **AND** it SHALL report that option 1 is a durable grant

#### Scenario: approve sends nothing when the prompt has cleared

- **GIVEN** `sweep.sh approve <pane>` is invoked for a pane whose fresh capture no longer shows a live permission prompt
- **WHEN** the subcommand runs
- **THEN** it SHALL send NO keystrokes to the pane
- **AND** it SHALL report that the prompt has cleared so no keys were sent
