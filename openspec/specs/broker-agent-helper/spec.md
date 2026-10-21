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

### Requirement: sweep.sh detects stuck-on-prompt agents

The bundled `assets/scripts/sweep.sh` helper SHALL detect a
"stuck on prompt" state for each agent pane by inspecting
recent `tmux capture-pane` output. The helper SHALL flag a
pane as stuck when its capture contains documented prompt
markers AND the agent's broker `last_seen_seconds` has not
advanced for more than 30 seconds. Prompt markers SHALL be matched
case-insensitively, as the compiled detectors match them, and the
result SHALL NOT depend on whether the markers were sourced from
the git-paw binary or from the helper's built-in fallback: the
profile stores markers in a normalised case, so a case-sensitive
match against the binary-sourced set silently misses prompts whose
on-screen text is capitalised differently.

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

#### Scenario: Binary-sourced markers detect a capitalised prompt

- **GIVEN** a git-paw binary resolvable on `PATH`, so the helper sources its prompt markers from it
- **AND** an agent whose pane shows `Do you want to proceed?` with a numbered option list and no other approval text, AND whose last_seen is stale
- **WHEN** the sweep runs
- **THEN** the helper SHALL classify the pane as stuck-on-prompt

#### Scenario: Marker source does not change the verdict

- **WHEN** the same capture is evaluated once with the binary-sourced markers and once with the helper's built-in fallback markers
- **THEN** both evaluations SHALL produce the same classification

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

### Requirement: sweep.sh approve refuses pane 0

The `sweep.sh approve <pane>` subcommand SHALL send keystrokes only to a pane that the session's per-repo discovery roster (`agents[]` in `.git-paw/sessions/<session>.json`) lists as a coding agent's `pane_index`. For any other pane — the dashboard, or a pane no roster entry claims — it SHALL send no keystrokes and SHALL report that the pane is excluded from blind send-keys because it is not a coding agent. When no discovery roster can be read, the subcommand SHALL fall back to refusing pane 0. The `<pane>` argument SHALL be a non-negative integer index, checked before the roster: any other value SHALL be refused with a non-zero exit and no keystrokes, and SHALL never be matched against the roster as a pattern. The roster check SHALL run before the live-prompt re-confirm and the option-1 guard, which are otherwise unchanged (*sweep.sh approve re-confirms a live prompt before sending keys*).

#### Scenario: approve of the dashboard pane is rejected

- **GIVEN** a broker-enabled session whose discovery roster lists coding agents at panes 1 and 2
- **WHEN** `sweep.sh approve 0` is invoked
- **THEN** it SHALL send NO keystrokes to pane 0
- **AND** it SHALL report that pane 0 is excluded from blind send-keys because it is not a coding agent

#### Scenario: approve of a pane no roster entry claims is rejected

- **GIVEN** a discovery roster listing coding agents at panes 1 and 2
- **WHEN** `sweep.sh approve 5` is invoked
- **THEN** it SHALL send NO keystrokes

#### Scenario: a non-numeric pane argument is refused

- **GIVEN** a discovery roster listing coding agents at panes 1 and 2
- **WHEN** `sweep.sh approve .` or `sweep.sh approve '[0-2]'` is invoked
- **THEN** it SHALL exit non-zero
- **AND** it SHALL send NO keystrokes to any pane

#### Scenario: approve of a roster agent's pane proceeds

- **GIVEN** a discovery roster listing a coding agent at pane 1 showing a live permission prompt whose option `1` is a one-time "Yes"
- **WHEN** `sweep.sh approve 1` is invoked
- **THEN** it SHALL send `1` and then `Enter` to pane 1

#### Scenario: Without a roster pane 0 is still refused

- **GIVEN** no readable discovery roster for the session
- **WHEN** `sweep.sh approve 0` is invoked
- **THEN** it SHALL send NO keystrokes to pane 0

### Requirement: Conflict-acknowledgement wire form and `sweep.sh conflict-ack`

An orchestrator SHALL be able to record that it has adjudicated an in-flight conflict, and SHALL be able to do so without hand-rolling a raw `curl …/publish` call.

The acknowledgement SHALL ride on the existing `agent.feedback` message rather than a new `BrokerMessage` variant. Its wire form is an `errors` entry of exactly:

```
[conflict-ack] <agent_a> <agent_b> <file>
```

`<agent_a>` and `<agent_b>` are the two agents' slugified ids (which never contain whitespace) and `<file>` is the contested path, which MAY contain whitespace and therefore runs to the END of that `errors` entry. The entry SHALL carry nothing after the path, so any human-readable prose SHALL be a SEPARATE `errors` entry. An `errors` entry that does not begin with the `[conflict-ack]` token, or that is missing any of the three fields, SHALL NOT be treated as an acknowledgement.

The acknowledgement SHALL be honoured only when the carrying `agent.feedback` has `from = "supervisor"`. This filter stops an agent's ordinary feedback from being mistaken for an adjudication. It is NOT an authentication boundary: the broker does not authenticate senders and `from` is self-asserted, so a deliberately forged acknowledgement is possible. A helper-published acknowledgement reaches BOTH named agents; EVERY acknowledgement, however it was published, is retained in the broker message log — the detector honours one whatever its envelope `agent_id` — so a forgery is auditable after the fact. Sender authentication is out of scope for this capability.

The supervisor-side `sweep.sh` helper SHALL expose a `conflict-ack <agent-a> <agent-b> <file> [note…]` subcommand that shapes and publishes this message to BOTH named agents with `from = "supervisor"`. When no `note` is supplied the helper SHALL supply a default human-readable one naming the file and stating that there is nothing for the recipient to fix — agents are taught that `agent.feedback` `errors` are problems to fix, so a bare tag line would read as a cryptic error. A missing `<agent-a>`, `<agent-b>`, or `<file>` SHALL be rejected (non-zero exit, usage diagnostic on stderr) and SHALL publish nothing.

The bundled coordination guidance SHALL tell agents that a `[conflict-ack]` `errors` entry is informational: nothing to fix, no re-published `agent.artifact`, and no pausing of edits on that file.

#### Scenario: The helper publishes a parseable acknowledgement to both agents

- **GIVEN** a running broker
- **WHEN** `sweep.sh conflict-ack feat-x feat-y src/a.rs` runs
- **THEN** exactly two `agent.feedback` messages SHALL be published, one addressed to `feat-x` and one to `feat-y`
- **AND** each SHALL carry `from = "supervisor"` and a first `errors` entry equal to `[conflict-ack] feat-x feat-y src/a.rs`

#### Scenario: A note rides as a separate errors entry

- **WHEN** `sweep.sh conflict-ack feat-x feat-y src/a.rs "feat-x owns this until it merges"` runs
- **THEN** each published message SHALL carry two `errors` entries: the `[conflict-ack]` tag entry, then the note
- **AND** the tag entry SHALL remain parseable as an acknowledgement for `(feat-x, feat-y, src/a.rs)`

#### Scenario: A default note is supplied when none is given

- **WHEN** `sweep.sh conflict-ack feat-x feat-y src/a.rs` runs with no note argument
- **THEN** each published message SHALL still carry a second `errors` entry naming `src/a.rs` and stating there is nothing for the recipient to fix
- **AND** that entry SHALL NOT itself parse as an acknowledgement

#### Scenario: A missing argument is rejected without publishing

- **WHEN** `sweep.sh conflict-ack feat-x feat-y` runs with no file argument
- **THEN** the command SHALL exit non-zero with a usage diagnostic on stderr
- **AND** no message SHALL be published

#### Scenario: The coordination guidance tells agents the tag is informational

- **WHEN** the bundled coordination skill is read
- **THEN** its description of `agent.feedback` SHALL name the `[conflict-ack]` entry as an exception to "errors are problems to fix"
- **AND** SHALL state that the agent is to do nothing in response — no fix, no re-published `agent.artifact`, and no pausing of its edits on that file

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

### Requirement: sweep.sh events long-polls the broker log

The bundled `sweep.sh` SHALL provide an `events <since> [wait]` subcommand that calls the session broker's `GET /log?since=<since>&wait=<wait>` (default wait 30 seconds) and the broker's `/status`, and prints one JSON object `{entries, last_seq, epoch, wave}` to stdout, where `epoch` identifies the running broker instance so a caller can tell a restarted broker from the one its cursor came from. It SHALL resolve the session and broker the same way its other subcommands do, SHALL print a JSON object with empty `entries` and `no_session: true` when no broker is reachable, and SHALL send no keystrokes and publish nothing.

#### Scenario: events prints new entries and the cursor

- **GIVEN** a live broker at sequence 3
- **WHEN** `sweep.sh events 3 5` runs and an agent publishes during the wait
- **THEN** stdout SHALL be a JSON object whose `entries` holds the new message and whose `last_seq` is 4

#### Scenario: events reports a stalled wave on timeout

- **GIVEN** a live broker whose wave is stalled and no publisher
- **WHEN** `sweep.sh events 3 1` runs
- **THEN** stdout SHALL contain `"entries": []`, `last_seq` 3 and a `wave` object with `stalled` true

#### Scenario: events without a session degrades

- **GIVEN** no active session for the repository
- **WHEN** `sweep.sh events 0` runs
- **THEN** stdout SHALL be a JSON object with `no_session` true and the exit status SHALL be 0

#### Scenario: events reports the broker epoch

- **GIVEN** a live broker
- **WHEN** `sweep.sh events 0 1` runs
- **THEN** stdout SHALL carry an `epoch` field identifying that broker instance

