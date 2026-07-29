# broker-agent-helper Specification

## Purpose
Provides bundled shell helpers (`broker.sh` for agents, `sweep.sh` for the supervisor) that wrap every agent→broker interaction — status, artifact, blocked, question, intent, poll, and the supervisor status/verified/feedback-gate verbs — so no participant hand-rolls a raw `curl …/publish`. The helpers discover the broker URL from config, shape all JSON internally, and are provisioned into each agent worktree at start/add time. The bundled `sweep.sh` helper additionally detects stuck-agent shapes from live pane capture plus broker heartbeats — stuck-on-prompt (including paste-buffer), no-progress, and blocked-on-supervisor — and publishes deduplicated synthetic `agent.status` messages, while gating its `approve <pane>` subcommand to re-confirm a live prompt before sending keys and to refuse pane 0; the supervisor skill names the helper as the canonical mechanism and forbids inline-bash reinvention.

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

### Requirement: sweep.sh detects stuck-on-prompt agents

The bundled `assets/scripts/sweep.sh` helper SHALL detect a
"stuck on prompt" state for each agent pane by inspecting
recent `tmux capture-pane` output. The helper SHALL flag a
pane as stuck when its capture contains documented prompt
markers AND the agent's broker `last_seen_seconds` has not
advanced for more than 30 seconds.

#### Scenario: Permission prompt with stale heartbeat is detected

- **GIVEN** an agent whose pane shows `Do you want to
  proceed?` (or equivalent permission-prompt pattern) AND
  whose broker last_seen has not advanced for 45 seconds
- **WHEN** the next sweep iteration runs
- **THEN** the helper SHALL classify the pane as
  stuck-on-prompt

#### Scenario: Permission prompt with fresh heartbeat is not stuck

- **GIVEN** an agent whose pane shows a permission prompt AND
  whose last_seen is 5 seconds old
- **WHEN** the sweep runs
- **THEN** the helper SHALL NOT yet classify the pane as
  stuck (the heartbeat may have caught it pre-stall)

#### Scenario: Paste-buffer stall is detected

- **GIVEN** an agent whose pane shows `Pasted text #N` (the
  Claude paste-buffer indicator) AND whose last_seen is
  stale
- **WHEN** the sweep runs
- **THEN** the helper SHALL classify the pane as
  stuck-on-prompt with a `detail` annotation indicating the
  paste-buffer variant

### Requirement: Synthetic agent.status publish on detection

The bundled `sweep.sh` SHALL publish a synthetic
`agent.status` broker message with `phase: "stuck-on-prompt"`
(per [[broker-watcher-and-state]] phase enum) for each
detected stuck-on-prompt agent. The published message SHALL
carry a `detail.captured_prompt` field containing the first
~200 characters of the pane capture so dashboard + MCP
consumers can surface the specific prompt.

#### Scenario: Synthetic publish reaches the broker

- **GIVEN** a detected stuck-on-prompt agent
- **WHEN** the helper publishes
- **THEN** the broker SHALL accept the `agent.status` message
  with `phase: "stuck-on-prompt"` and the documented detail
  fields, and the dashboard SHALL render the supervisor row
  (or the agent row) accordingly

#### Scenario: Dedup prevents spam on repeated detection

- **GIVEN** an agent that remains stuck across multiple
  sweep iterations
- **WHEN** the helper detects the stuck state on iteration N
  AND iteration N+1
- **THEN** the helper SHALL publish only on the first
  detection in the current stuck window; repeated detections
  SHALL NOT produce duplicate broker messages

### Requirement: Supervisor skill directs LLM to use sweep.sh

The bundled `assets/agent-skills/supervisor.md` SHALL include
a "Detecting stuck agents" section that names
`.git-paw/scripts/sweep.sh` (the bundled helper installed by
`git paw init`) as the canonical detection mechanism and
SHALL forbid LLMs from writing inline-bash signature-dedup
monitors as substitutes. The section SHALL document all
detected stuck shapes — stuck-on-prompt, stuck-stream-timeout,
context-bloat, no-progress, and blocked-on-supervisor — and
SHALL state the read-pane-before-classifying rule so the LLM
does not declare an agent idle from counts alone.

#### Scenario: Skill prose names the bundled helper

- **WHEN** supervisor.md is inspected
- **THEN** the "Detecting stuck agents" section SHALL name
  the bundled helper's path explicitly and document the
  helper's stuck-detection behaviour for all five shapes

#### Scenario: Skill prose forbids inline-bash reinvention

- **WHEN** the same section is read
- **THEN** the prose SHALL include explicit language
  forbidding inline-bash signature-dedup monitors, with the
  rationale that ad-hoc dedup eats repeat-pattern prompts
  (see v0.6.0 dogfood bug 9)

#### Scenario: Skill prose states the read-pane rule

- **WHEN** the "Detecting stuck agents" section is read
- **THEN** the prose SHALL state that an idle-looking agent
  is classified by reading its live pane, and that a
  prompt-blocked agent SHALL be treated as blocked-on-prompt
  rather than no-progress

### Requirement: Stack-agnostic phrasing in the skill section

The new "Detecting stuck agents" section SHALL pass the
no-language-leak audit from [[lang-agnostic-assets]].

#### Scenario: No-leak audit passes against the new section

- **WHEN** the no-leak audit runs against the updated
  `supervisor.md`
- **THEN** the audit SHALL pass on the rendered skill
  across all supported spec backends

### Requirement: Detector reads live pane state before classifying no-progress

The `sweep.sh` detector SHALL read each agent's live pane
capture and evaluate the pane-marker shapes (stuck-on-prompt,
stuck-stream-timeout, context-bloat) BEFORE it evaluates the
no-progress heuristic. An agent whose pane shows a permission
or paste-buffer marker SHALL be classified as stuck-on-prompt
(routing to the approval path) and SHALL NOT be classified as
no-progress, even when its progress counters are unchanged.
The detector SHALL NOT classify an idle-looking agent from
branch-tip or uncommitted-file counts alone.

#### Scenario: Prompt-blocked agent is classified blocked, not no-progress

- **GIVEN** an agent whose pane shows a permission prompt AND
  whose task-checkbox count and commit count are unchanged
  across the no-progress window
- **WHEN** the detector classifies the pane
- **THEN** the agent SHALL be classified as stuck-on-prompt
- **AND** the agent SHALL NOT be classified as no-progress

#### Scenario: Idle-looking agent with no marker falls through to no-progress

- **GIVEN** an agent whose pane shows no permission, paste,
  stream-timeout, or context-bloat marker
- **WHEN** the detector classifies the pane
- **THEN** the detector SHALL proceed to evaluate the
  no-progress heuristic for that agent rather than declaring
  it stuck-on-prompt

### Requirement: No-progress detection over a heartbeat window

The `sweep.sh` detector SHALL flag an agent as `no-progress`
when, across the configurable no-progress window
(default ~25 minutes, read from `[supervisor]` config when
present), BOTH the agent's completed task-checkbox count AND
its branch commit count are unchanged. The detector SHALL
snapshot `(checkbox_count, commit_count, timestamp)` per agent
and compare against the prior snapshot; a missing prior
snapshot SHALL NOT be treated as no-progress (the first
observation only records state). A `no-progress` detection
SHALL be advisory — it surfaces the state for a nudge or
investigation rather than auto-terminating the agent.

#### Scenario: Both counters unchanged over the window triggers no-progress

- **GIVEN** an agent whose completed-checkbox count AND commit
  count are unchanged from a prior snapshot older than the
  no-progress window AND whose pane shows no stuck marker
- **WHEN** the next sweep evaluates the agent
- **THEN** the detector SHALL classify the agent as
  `no-progress` and publish the synthetic `agent.status` with
  `phase: "no-progress"`

#### Scenario: Movement in either counter is not no-progress

- **GIVEN** an agent whose commit count advanced (or whose
  completed-checkbox count advanced) since the prior snapshot
- **WHEN** the next sweep evaluates the agent
- **THEN** the detector SHALL NOT classify the agent as
  `no-progress`

#### Scenario: First observation only records state

- **GIVEN** an agent with no prior progress snapshot on file
- **WHEN** the sweep evaluates the agent
- **THEN** the detector SHALL record the current
  `(checkbox_count, commit_count, timestamp)` and SHALL NOT
  classify the agent as `no-progress` on this first observation

### Requirement: Blocked-on-supervisor timeout detection

The `sweep.sh` detector SHALL detect a `blocked-on-supervisor`
state for an agent that has an unanswered `agent.blocked`
event whose `payload.from` identifies the supervisor (or whose
pane shows it is awaiting supervisor input), where the
unanswered duration exceeds the configurable
blocked-on-supervisor window (default ~15 minutes). On
detection the helper SHALL publish a synthetic `agent.status`
with `phase: "blocked-on-supervisor"` so the supervisor (or
the unattended drive loop) is forced to answer rather than
leaving the agent waiting indefinitely.

#### Scenario: Long-unanswered supervisor block is detected

- **GIVEN** an agent whose latest `agent.blocked` event names
  the supervisor as the blocker AND has gone unanswered longer
  than the blocked-on-supervisor window
- **WHEN** the next sweep evaluates the agent
- **THEN** the detector SHALL classify the agent as
  `blocked-on-supervisor` and publish the synthetic
  `agent.status` with `phase: "blocked-on-supervisor"`

#### Scenario: Recently-blocked agent is not yet flagged

- **GIVEN** an agent that published an `agent.blocked` naming
  the supervisor only seconds ago
- **WHEN** the sweep evaluates the agent
- **THEN** the detector SHALL NOT yet classify the agent as
  `blocked-on-supervisor` (the window has not elapsed)

### Requirement: sweep.sh approve re-confirms a live prompt before sending keys

The bundled `assets/scripts/sweep.sh` `approve <pane>` subcommand SHALL pass through the `broker-mediated-approvals` approval-send gate. Immediately before sending the sticky-yes keystrokes (`Down` then `Enter`), the subcommand SHALL run a fresh `tmux capture-pane` of the target pane and SHALL confirm a live permission-prompt marker is present within the last 4 non-blank lines of that capture. When the re-confirm capture shows no live prompt in the tail (the prompt has cleared), the subcommand SHALL send NO keystrokes and SHALL report that the prompt cleared.

#### Scenario: approve sends keys only when the prompt is still live

- **GIVEN** `sweep.sh approve <pane>` is invoked for a coding-agent pane whose fresh capture shows a permission-prompt marker within the last 4 non-blank lines
- **WHEN** the subcommand runs
- **THEN** it SHALL send `Down` then `Enter` to the pane via `tmux send-keys`

#### Scenario: approve sends nothing when the prompt has cleared

- **GIVEN** `sweep.sh approve <pane>` is invoked for a pane whose fresh capture no longer shows a permission-prompt marker in the last 4 non-blank lines
- **WHEN** the subcommand runs
- **THEN** it SHALL send NO keystrokes to the pane
- **AND** it SHALL report that the prompt has cleared so no keys were sent

### Requirement: sweep.sh approve refuses pane 0

The `sweep.sh approve <pane>` subcommand SHALL refuse to send keystrokes when the supplied pane index is 0 (the supervisor's own pane). It SHALL send no keystrokes and SHALL report that pane 0 is excluded from blind send-keys.

#### Scenario: approve 0 is rejected

- **GIVEN** `sweep.sh approve 0` is invoked
- **WHEN** the subcommand runs
- **THEN** it SHALL send NO keystrokes to pane 0
- **AND** it SHALL report that pane 0 is excluded from blind send-keys

