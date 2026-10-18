## MODIFIED Requirements

### Requirement: Escalation-first, no blanket-approve when a drive loop is running

When the orchestrator's session context (the repo-root sidecar `.git-paw/AGENTS.local.md`) indicates a drive loop is running (an unattended session), the orchestrator SHALL, each supervision cycle:

1. **Drain the drive loop's escalations first.** Read the loop's escalation and review items from the broker (the `supervisor` inbox, or the event surface) and act on each before the rest of the sweep. For a permission prompt, relay it to the human (in the orchestrator's own session, naming the agent and the command) and approve it with `sweep.sh approve <pane>` only when the human explicitly says so. For any other item, reason about it and publish feedback or an answer.
2. **Then perform its normal sweep** — verification, merge orchestration, conflict handling, detect-stuck, and status publishing — as it otherwise would.

Whether or not a drive loop is running, the orchestrator SHALL NOT approve a permission prompt on its own judgment of the command's safety, and SHALL NOT sweep panes to approve prompts in bulk. In an attended session (no drive loop) it finds stuck-on-prompt agents through its own `sweep.sh detect-stuck` sweep and relays their prompts to the human in the same way.

#### Scenario: With a loop running, escalations are handled before the sweep

- **GIVEN** an orchestrator whose session context indicates a drive loop is running
- **WHEN** it runs a supervision cycle
- **THEN** it SHALL process the loop's escalations before its verify/merge/status sweep
- **AND** SHALL relay each escalated permission prompt to the human in its own session rather than approving it

#### Scenario: An approval follows an explicit human instruction

- **GIVEN** the orchestrator has relayed agent `feat-a`'s permission prompt to the human
- **WHEN** the human tells the orchestrator to approve it
- **THEN** the orchestrator SHALL run `sweep.sh approve <pane>` for `feat-a`'s pane
- **AND** without such an instruction it SHALL NOT run `sweep.sh approve`

#### Scenario: With no loop, the orchestrator still does not approve on its own

- **GIVEN** an orchestrator whose session context does NOT indicate a drive loop
- **WHEN** its sweep finds an agent stuck on a permission prompt
- **THEN** it SHALL relay the prompt to the human
- **AND** SHALL NOT approve it on its own judgment

### Requirement: Never send-keys to the supervisor's own pane

The orchestrator skill SHALL state that keystrokes go only to coding-agent panes and SHALL NEVER be sent to a non-agent surface — pane 0 (the dashboard TUI) or the orchestrator's own terminal — because typing into the dashboard is discarded and typing into its own terminal interrupts its own in-flight command.

#### Scenario: Skill states the never-non-agent-surface rule

- **WHEN** the orchestrator skill's pane-driving section is read
- **THEN** it SHALL state that keystrokes are sent only to coding-agent panes, never to pane 0 or the orchestrator's own terminal, with the rationale

### Requirement: /agents inventory command in the supervisor pane

The orchestrator SHALL recognise an `/agents` directive typed by the operator in the orchestrator's own session and respond with the current agent inventory. The inventory SHALL list every agent registered with the broker plus the orchestrator's own `supervisor` row when present, each coding-agent row carrying `branch_id`, `status`, `last_seen`, `cli`, detected `mode`, and `pane_index`. The `supervisor` row, when present, SHALL carry no pane index: the orchestrator has no pane in the session, and pane 0 is the dashboard.

#### Scenario: User asks for the agent inventory

- **GIVEN** an active supervised session with N agents
- **WHEN** the operator types `/agents` to the orchestrator
- **THEN** the orchestrator SHALL respond with a structured listing of the N agents (plus its own row when registered), each coding-agent row containing `branch_id`, `status`, `last_seen`, `cli`, `mode`, and `pane_index`
- **AND** the `supervisor` row, when listed, SHALL show no pane index (never pane 0)

#### Scenario: Inventory after a mid-session add/remove

- **GIVEN** a session whose agent set has changed via `git paw add` / `git paw remove`
- **WHEN** the operator types `/agents` after the sweep that refreshes the inventory
- **THEN** the listing SHALL reflect the post-change agent set

### Requirement: /tell routing command in the supervisor pane

The orchestrator SHALL recognise a `/tell <agent_id> <prompt>` directive typed by the operator in the orchestrator's own session and route the prompt to the named agent. The directive SHALL be parseable with the agent identifier as the first whitespace-delimited token after `/tell` and the prompt as the remainder of the line (or multi-line content).

#### Scenario: Successful tell to a live agent

- **GIVEN** an active session with agent `feat/auth`
- **WHEN** the operator types `/tell feat/auth rebase onto main` to the orchestrator
- **THEN** the orchestrator SHALL deliver `rebase onto main` to the `feat/auth` agent via the configured delivery mode and acknowledge the routing in its own session

#### Scenario: Tell with multi-line prompt

- **WHEN** the operator types `/tell feat/auth` followed on subsequent lines by multi-line content
- **THEN** the orchestrator SHALL parse the entire content block as the prompt and route it whole
