## ADDED Requirements

### Requirement: The drive loop enforces the budget on the spend it initiates

When a budget cap is configured, the drive loop SHALL re-read attributed usage — every worker's, plus
that of each headless orchestrator invocation it started in this run — on a cadence no faster than
its sweep, and SHALL compare the known total against each cap. On the first breach it SHALL record
one `budget-exhausted` escalation naming the cap and the total, and from then on SHALL NOT send
nudges, re-engagement or correction text and SHALL NOT start headless orchestrator invocations. With
`[supervisor.correction] on_exhausted = "escalate"` the loop SHALL keep sweeping until another exit
condition; with `on_exhausted = "abandon"` it SHALL exit with outcome `budget-exhausted` and a
non-zero status. The loop SHALL NOT interrupt, signal or kill any worker on account of the budget.

#### Scenario: A breach under escalate stops initiated spend

- **GIVEN** `budget_max_tokens = 1000`, `on_exhausted = "escalate"` and workers whose known usage
  totals 1200 tokens
- **WHEN** the loop next reads usage
- **THEN** it SHALL record one `budget-exhausted` escalation
- **AND** no nudge or re-engagement text SHALL be sent

#### Scenario: A breach under abandon ends the run

- **GIVEN** `budget_max_tokens = 1000`, `on_exhausted = "abandon"` and known usage of 1200 tokens
- **WHEN** the loop next reads usage
- **THEN** it SHALL exit with outcome `budget-exhausted` and a non-zero status
- **AND** every worker pane SHALL still be running

#### Scenario: No cap leaves the loop unchanged

- **GIVEN** no budget field configured
- **WHEN** the loop runs to completion
- **THEN** its actions, outcome and exit status SHALL be identical to the behaviour before this
  requirement

### Requirement: A budget the loop cannot evaluate is reported

When a budget cap is configured, the drive loop SHALL report at start which workers' usage is unknown
and, when `budget_max_cost` is set, whether any attributed source reports cost. It SHALL evaluate a
cap only against known figures, SHALL label totals as covering known usage only, and SHALL NOT treat
unknown usage as zero in its output.

#### Scenario: Unknown workers are named at start

- **GIVEN** `budget_max_tokens` set and one worker running a `generic`-family CLI
- **WHEN** the drive loop starts
- **THEN** its start output SHALL name that worker as having unknown usage

#### Scenario: An unenforceable cost cap is reported

- **GIVEN** `budget_max_cost` set and no attributed source reporting cost
- **WHEN** the drive loop starts
- **THEN** its start output SHALL state that the cost cap cannot be evaluated

## MODIFIED Requirements

### Requirement: Exit summary

On exit the drive loop SHALL print a human-readable summary that states the overall outcome (completed / escalated-for-review / stuck / heartbeat / session-torn-down / cli-unavailable / budget-exhausted), each agent's RESOLVED final state, the deduped list of escalated prompts awaiting human review, a usage section, and a pointer to the broker log and captured learnings. Each agent's final state SHALL be resolved from terminal artifacts — the most-progressed `WorkerPhase` ever observed for it — rather than the agent's last broker-recorded `agent.status`, so a wave whose workers all finished is not reported as still "working."

The usage section SHALL report attributed token usage (and cost where the CLI reports it) by role — workers grouped by CLI id, and the loop's headless orchestrator invocations — SHALL state how many workers' usage is unknown, and SHALL state that it excludes the operator's own session. When a budget cap is configured it SHALL show each cap beside the known total.

Whether a branch is **merged** SHALL be derived from git refs, never from loop state or broker status: a coding agent's branch counts as merged only when its tip is an ancestor of the default branch (`git merge-base --is-ancestor <branch> <default>` exits `0`, the default branch resolved through the single default-branch resolver). A `verified` or `done` status SHALL NOT by itself count as merged. A comparison git cannot make (a missing branch, an unresolvable ref, a failed spawn) SHALL count as unmerged.

The summary SHALL print a wave-level line `N/M branches merged` using the ref-derived count, SHALL list every unmerged branch with its count of commits not in the default branch and its tip SHA, and SHALL annotate each agent's line with its merged / unmerged state alongside its resolved phase. A wave with one or more unmerged branches at exit SHALL NOT be reported `completed`: its outcome SHALL be `escalated-for-review`, with one escalation entry per unmerged branch (verdict `unmerged`, naming the branch, its unmerged-commit count, and its tip). These entries SHALL be added to the summary directly and SHALL NOT be typed into any pane.

#### Scenario: Summary reports outcome and escalations

- **GIVEN** a drive loop that exits after escalating one permission prompt and completing the rest of the wave
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

#### Scenario: The usage section separates roles and names unknowns

- **GIVEN** a drive loop whose run had two workers on CLI `sonnet` with known usage, one worker with unknown usage, and one headless orchestrator invocation with known usage
- **WHEN** the summary is printed
- **THEN** its usage section SHALL show a `sonnet` worker total, an orchestrator-invocation total, and one worker with unknown usage
- **AND** it SHALL state that the operator's own session is excluded
