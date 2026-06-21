## MODIFIED Requirements

### Requirement: Synthetic agent.status publish on detection

The bundled `sweep.sh` SHALL publish a synthetic
`agent.status` broker message with a stuck-state `phase` (per
[[supervisor-introspection]] phase enum) for each detected
stuck agent. The supported stuck-state phase values SHALL be
`stuck-on-prompt`, `stuck-stream-timeout`, `context-bloat`,
`no-progress`, and `blocked-on-supervisor`. The published
message SHALL carry a `detail` object describing the evidence
for that shape — at minimum a `captured_prompt` field
containing the first ~200 characters of the pane capture for
the pane-marker shapes (`stuck-on-prompt`,
`stuck-stream-timeout`, `context-bloat`) so dashboard + MCP
consumers can surface the specific cause.

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
  detection in the current stuck window per `(agent_id, shape)`;
  repeated detections of the same shape SHALL NOT produce
  duplicate broker messages

#### Scenario: Each new shape publishes its own phase

- **GIVEN** an agent detected in a `no-progress`,
  `context-bloat`, or `blocked-on-supervisor` state
- **WHEN** the helper publishes the synthetic `agent.status`
- **THEN** the message's `phase` SHALL be the matching value
  (`no-progress`, `context-bloat`, or `blocked-on-supervisor`)
  and the broker SHALL accept it without a validation error

## ADDED Requirements

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

## MODIFIED Requirements

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
