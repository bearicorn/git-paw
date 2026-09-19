## MODIFIED Requirements

### Requirement: CLI unavailability is an explicit non-success exit

The drive loop SHALL recognise a CLI that has stopped serving its pane — a usage or rate limit reached, or authentication lost — by matching `cli_unavailable_markers` against the pane's capture **tail**, and SHALL surface it as an explicit non-success exit reason instead of letting the wave run on to a `completed` or `heartbeat` exit. The tail SHALL tolerate the same bounded trailing region of non-prompt content (a task list, a status strip) that the live-prompt gate tolerates, so content a CLI draws beneath its banner does not hide the banner; a marker separated from the end of the capture by more than that bounded region SHALL still count as scrolled away.

The markers SHALL be resolved for the CLI running in that pane: a `[clis.<name>]` override of `cli_unavailable_markers` SHALL apply, falling back to the embedded default when the CLI has no override. The drive loop and the bundled helper script SHALL therefore classify the same pane identically for this field.

When the orchestrator's pane shows a marker, the loop SHALL exit at once with outcome `cli-unavailable`, the summary SHALL name the pane's agent and quote the matching line, and the `--unattended` process SHALL exit with a non-zero status. When a coding agent's pane shows a marker, the loop SHALL record a `cli-unavailable` escalation for that agent and SHALL continue driving the remaining agents; a wave carrying such an escalation SHALL NOT be reported `completed`. A `cli-unavailable` escalation SHALL be deduplicated on the agent and the matched marker, and SHALL NOT be keyed on any other content of the capture — so the same banner seen across sweeps is one escalation, and two different banners on one agent are two.

When the roster contains no orchestrator pane, coding-pane detection SHALL work unchanged, and the exit summary SHALL state that no orchestrator pane was monitored for CLI unavailability, so the summary does not imply coverage it does not have.

#### Scenario: A rate-limited orchestrator ends the run as cli-unavailable

- **GIVEN** an unattended drive loop whose orchestrator pane's capture tail shows `You've hit your session limit`
- **WHEN** the loop sweeps that pane
- **THEN** the loop SHALL exit with outcome `cli-unavailable`
- **AND** the summary SHALL name the orchestrator and quote the matching line
- **AND** the `--unattended` process SHALL exit with a non-zero status
- **AND** the summary SHALL NOT read `completed`

#### Scenario: A rate-limited worker is escalated while the wave keeps moving

- **GIVEN** an unattended drive loop with two coding agents, one of whose pane tails shows a CLI-unavailability marker
- **WHEN** the loop sweeps the panes
- **THEN** it SHALL record a `cli-unavailable` escalation naming that agent
- **AND** it SHALL keep sweeping and approving the other agent's prompts
- **AND** the eventual summary SHALL NOT report the wave `completed`

#### Scenario: A banner above a task-list footer is still detected

- **GIVEN** a coding agent's capture ending with `You've hit your session limit` followed by a six-line task-list region
- **WHEN** the loop sweeps that pane
- **THEN** it SHALL record a `cli-unavailable` escalation for that agent

#### Scenario: A marker outside the capture tail is ignored

- **GIVEN** a pane whose scrolled-away history contains a CLI-unavailability marker, separated from the end of the capture by more than the bounded trailing region, while its tail shows a normal idle input box
- **WHEN** the loop sweeps that pane
- **THEN** no `cli-unavailable` exit or escalation SHALL be recorded

#### Scenario: A per-CLI marker override is honoured by the loop

- **GIVEN** a coding agent running a CLI whose `[clis.<name>]` profile overrides `cli_unavailable_markers` with `out of tokens`
- **AND** that agent's capture tail shows `out of tokens`
- **WHEN** the loop sweeps that pane
- **THEN** it SHALL record a `cli-unavailable` escalation for that agent
- **AND** the bundled helper SHALL classify the same capture identically

#### Scenario: The same banner across sweeps is one escalation

- **GIVEN** a coding agent whose tail shows a session-limit banner on consecutive sweeps, while earlier transcript lines in the capture change between those sweeps
- **WHEN** the loop sweeps the pane repeatedly
- **THEN** exactly one `cli-unavailable` escalation SHALL be recorded for that agent and marker

#### Scenario: Two different banners on one agent are two escalations

- **GIVEN** a coding agent whose tail first shows a session-limit banner and later an authentication-failure banner
- **WHEN** the loop sweeps the pane across both
- **THEN** two distinct `cli-unavailable` escalations SHALL be recorded

#### Scenario: No orchestrator pane is reported honestly

- **GIVEN** an unattended drive loop whose roster contains coding agents and no orchestrator pane
- **AND** one coding agent's tail shows a CLI-unavailability marker
- **WHEN** the loop sweeps and later exits
- **THEN** it SHALL record a `cli-unavailable` escalation for that agent
- **AND** the exit summary SHALL state that no orchestrator pane was monitored for CLI unavailability

#### Scenario: Without a marker the loop behaves as before

- **GIVEN** panes whose captures contain no `cli_unavailable_markers` text
- **WHEN** the loop runs to a completion or heartbeat exit
- **THEN** the outcome and exit status SHALL be identical to the behaviour before this requirement
