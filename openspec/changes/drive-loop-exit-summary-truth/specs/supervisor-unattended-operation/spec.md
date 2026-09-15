## MODIFIED Requirements

### Requirement: Exit summary

On exit the drive loop SHALL print a human-readable summary that states the overall outcome (completed / escalated-for-review / stuck / heartbeat / session-torn-down / cli-unavailable), each agent's RESOLVED final state, the deduped list of escalated prompts awaiting human review, and a pointer to the broker log and captured learnings. Each agent's final state SHALL be resolved from terminal artifacts — the most-progressed `WorkerPhase` ever observed for it — rather than the agent's last broker-recorded `agent.status`, so a wave whose workers all finished is not reported as still "working."

Whether a branch is **merged** SHALL be derived from git refs, never from loop state or broker status: a coding agent's branch counts as merged only when its tip is an ancestor of the default branch (`git merge-base --is-ancestor <branch> <default>` exits `0`, the default branch resolved through the single default-branch resolver). A `verified` or `done` status SHALL NOT by itself count as merged. A comparison git cannot make (a missing branch, an unresolvable ref, a failed spawn) SHALL count as unmerged.

The summary SHALL print a wave-level line `N/M branches merged` using the ref-derived count, SHALL list every unmerged branch with its count of commits not in the default branch and its tip SHA, and SHALL annotate each agent's line with its merged / unmerged state alongside its resolved phase. A wave with one or more unmerged branches at exit SHALL NOT be reported `completed`: its outcome SHALL be `escalated-for-review`, with one escalation entry per unmerged branch (verdict `unmerged`, naming the branch, its unmerged-commit count, and its tip). These entries SHALL be added to the summary directly and SHALL NOT be typed into any pane.

#### Scenario: Summary reports outcome and escalations

- **GIVEN** a drive loop that exits after escalating one risky prompt and completing the rest of the wave
- **WHEN** the summary is printed
- **THEN** the summary SHALL state the overall outcome
- **AND** SHALL list the per-agent final state
- **AND** SHALL include the escalated prompt awaiting human review

#### Scenario: A completed wave is not reported as working

- **GIVEN** a drive loop exiting after every branch was verified and every branch tip is an ancestor of the default branch
- **WHEN** the summary is printed
- **THEN** no agent's final state SHALL read `working`
- **AND** the summary SHALL report the wave as `completed` with an `N/N branches merged` line

#### Scenario: Finished workers with unmerged branches are not reported as merged

- **GIVEN** a drive loop whose two coding agents both published `done`, while neither branch tip is an ancestor of the default branch
- **WHEN** the loop exits and prints the summary
- **THEN** the wave-level line SHALL read `0/2 branches merged`
- **AND** each agent's line SHALL show its resolved phase together with `unmerged` and its count of commits not in the default branch
- **AND** the outcome SHALL be `escalated-for-review`, not `completed`
- **AND** the escalation list SHALL contain one `unmerged` entry per branch naming its tip SHA

#### Scenario: An undeterminable merge comparison counts as unmerged

- **GIVEN** a coding agent whose branch cannot be compared against the default branch (the ref no longer exists, or `git merge-base` fails)
- **WHEN** the summary is derived
- **THEN** that branch SHALL be counted as unmerged
- **AND** its entry SHALL state that the comparison could not be made rather than reporting it merged

#### Scenario: Unmerged entries never reach a pane

- **GIVEN** a drive loop exiting with an orchestrator pane present and at least one unmerged branch
- **WHEN** the `unmerged` escalation entries are added to the summary
- **THEN** no keystroke SHALL be dispatched to the orchestrator pane or any agent pane for them

## ADDED Requirements

### Requirement: CLI unavailability is an explicit non-success exit

The drive loop SHALL recognise a CLI that has stopped serving its pane — a usage or rate limit reached, or authentication lost — by matching the profile-sourced `cli_unavailable_markers` against the pane's capture **tail** (the same tail discipline the live-prompt scan uses), and SHALL surface it as an explicit non-success exit reason instead of letting the wave run on to a `completed` or `heartbeat` exit. When the orchestrator's pane shows a marker, the loop SHALL exit at once with outcome `cli-unavailable`, the summary SHALL name the pane's agent and quote the matching line, and the `--unattended` process SHALL exit with a non-zero status. When a coding agent's pane shows a marker, the loop SHALL record a `cli-unavailable` escalation for that agent (deduped on `(agent_id, shape)` like every other escalation) and SHALL continue driving the remaining agents; a wave carrying such an escalation SHALL NOT be reported `completed`. A marker present only in scrolled-away history, not in the tail, SHALL NOT trigger either action.

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

#### Scenario: A marker outside the capture tail is ignored

- **GIVEN** a pane whose scrolled-away history contains a CLI-unavailability marker while its tail shows a normal idle input box
- **WHEN** the loop sweeps that pane
- **THEN** no `cli-unavailable` exit or escalation SHALL be recorded

#### Scenario: Without a marker the loop behaves as before

- **GIVEN** panes whose captures contain no `cli_unavailable_markers` text
- **WHEN** the loop runs to a completion or heartbeat exit
- **THEN** the outcome and exit status SHALL be identical to the behaviour before this requirement
