# stuck-prompt-detection Specification

## Purpose
TBD - created by archiving change auto-approve-scope-v0-6-x. Update Purpose after archive.
## Requirements
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
(per [[supervisor-introspection]] phase enum) for each
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
monitors as substitutes.

#### Scenario: Skill prose names the bundled helper

- **WHEN** supervisor.md is inspected
- **THEN** the "Detecting stuck agents" section SHALL name
  the bundled helper's path explicitly and document the
  helper's stuck-detection behaviour

#### Scenario: Skill prose forbids inline-bash reinvention

- **WHEN** the same section is read
- **THEN** the prose SHALL include explicit language
  forbidding inline-bash signature-dedup monitors, with the
  rationale that ad-hoc dedup eats repeat-pattern prompts
  (see v0.6.0 dogfood bug 9)

### Requirement: Stack-agnostic phrasing in the skill section

The new "Detecting stuck agents" section SHALL pass the
no-language-leak audit from [[lang-agnostic-assets]].

#### Scenario: No-leak audit passes against the new section

- **WHEN** the no-leak audit runs against the updated
  `supervisor.md`
- **THEN** the audit SHALL pass on the rendered skill
  across all supported spec backends

