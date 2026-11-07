## ADDED Requirements

### Requirement: The drive loop detects a budget breach

When a budget cap is configured, the drive loop SHALL re-read attributed usage — every worker's, plus
that of each headless orchestrator invocation it started in this run — on a cadence no faster than
its sweep, and SHALL compare the known total against each cap. On the first breach it SHALL record
one `budget-exhausted` escalation naming the cap and the total.

#### Scenario: A breach is escalated once

- **GIVEN** `budget_max_tokens = 1000` and workers whose known usage totals 1200 tokens
- **WHEN** the loop reads usage on several sweeps
- **THEN** it SHALL record exactly one `budget-exhausted` escalation naming the cap and the total

#### Scenario: The token cap counts every token kind

- **GIVEN** `budget_max_tokens = 1000` and one worker with 1 input, 1 output, 900 cache-read and 98
  cache-write tokens
- **WHEN** the loop compares usage with the cap
- **THEN** the cap SHALL be reached

### Requirement: Spend already seen is never subtracted

For the rest of a run the drive loop SHALL keep the last known figures of every session it has
seen, keyed by role, branch and session id, and SHALL count them even after the worker is removed,
swaps CLI or turns unknown. A worker whose known usage turns unknown SHALL be counted once, by its
kept figures and not also as unknown, and warned about once on stderr and in the exit summary.

#### Scenario: A removed, swapped or unknown worker keeps its spend

- **GIVEN** three workers with known usage of 100, 200 and 300 tokens
- **WHEN** the first is removed, the second relaunches on another CLI and the third's usage turns
  unknown
- **THEN** the total SHALL stay at least 600 tokens
- **AND** one warning SHALL name the third worker
- **AND** the third worker SHALL NOT be counted as a worker with unknown usage

### Requirement: Re-reads parse only appended report records

When the loop re-reads a usage report it has read before in the run, it SHALL parse only the
records appended since that read, keeping running totals and each response counted once. A report
that shrank, or whose modification time changed while it did not grow, SHALL be read again from
its start.

#### Scenario: A re-read parses only appended records

- **GIVEN** a usage report already read once in the run
- **WHEN** records are appended and the report is read again
- **THEN** only the appended records SHALL be parsed and the totals SHALL include them

#### Scenario: A rewritten report is read again from its start

- **GIVEN** a usage report already read once in the run
- **WHEN** it is rewritten at the same length with a new modification time
- **THEN** the totals SHALL be those of the rewritten report

### Requirement: A budget breach stops the spend the loop initiates

After a budget breach the drive loop SHALL NOT send re-engagement or correction text, SHALL NOT
start headless orchestrator invocations and SHALL NOT refresh branches or send their notifications
for the rest of the run. It SHALL still submit text a human left in a pane. It SHALL NOT interrupt,
signal or kill any worker on account of the budget.

#### Scenario: A breach under escalate stops initiated spend

- **GIVEN** `budget_max_tokens = 1000`, `budget_on_exhausted = "escalate"` and workers whose known
  usage totals 1200 tokens
- **WHEN** the loop next reads usage
- **THEN** it SHALL record one `budget-exhausted` escalation
- **AND** no re-engagement text SHALL be sent

#### Scenario: A breach still submits a human's buffered text but refreshes no branch

- **GIVEN** a budget breach recorded this run
- **WHEN** a pane holds text a human typed and a merge would refresh a clean branch
- **THEN** the typed text SHALL be submitted
- **AND** no branch SHALL be rebased and no worker notified of a refresh

#### Scenario: A breach starts no orchestrator invocation and touches no worker

- **GIVEN** a budget breach recorded while a judgment call is still waiting for a headless orchestrator invocation
- **WHEN** the loop keeps sweeping
- **THEN** no headless orchestrator invocation SHALL start
- **AND** no keystroke or signal SHALL reach any worker on account of the budget

#### Scenario: No cap leaves the loop unchanged

- **GIVEN** no budget field configured
- **WHEN** the loop runs to completion
- **THEN** its actions, outcome and exit status SHALL be identical to the behaviour before this
  requirement

### Requirement: The exhaustion policy decides whether a budget breach ends the run

With `[supervisor] budget_on_exhausted = "escalate"` (or absent) the drive loop SHALL keep sweeping
after a budget breach until another exit condition. With `"abandon"` it SHALL exit with outcome
`budget-exhausted` and a non-zero status, leaving every worker running.
`[supervisor.correction] on_exhausted` SHALL NOT affect this.

#### Scenario: A breach under abandon ends the run

- **GIVEN** `budget_max_tokens = 1000`, `budget_on_exhausted = "abandon"`,
  `[supervisor.correction] on_exhausted = "escalate"` and known usage of 1200 tokens
- **WHEN** the loop next reads usage
- **THEN** it SHALL exit with outcome `budget-exhausted` and a non-zero status
- **AND** every worker pane SHALL still be running

### Requirement: A budget the loop cannot evaluate is reported

When a budget cap is configured, the drive loop SHALL report at start which workers' usage is
unknown. When `budget_max_cost` is set and not every worker CLI reports cost, it SHALL also record
one escalation at start saying the cost cap cannot be evaluated; that escalation SHALL NOT make a
finished wave `escalated-for-review`. It SHALL evaluate a cap only against known figures and SHALL
NOT treat unknown usage as zero.

#### Scenario: Unknown workers are named at start

- **GIVEN** `budget_max_tokens` set and one worker running a `generic`-family CLI
- **WHEN** the drive loop starts
- **THEN** its start output SHALL name that worker as having unknown usage

#### Scenario: An unenforceable cost cap is reported and escalated once

- **GIVEN** `budget_max_cost` set and no worker CLI reporting cost
- **WHEN** the drive loop starts
- **THEN** its start output SHALL state that the cost cap cannot be evaluated
- **AND** it SHALL record exactly one escalation saying so

#### Scenario: An unevaluable cost cap does not change the outcome

- **GIVEN** `budget_max_cost` set, no worker CLI reporting cost, and a wave whose branches all merge
- **WHEN** the loop exits
- **THEN** the outcome SHALL be `completed`
- **AND** the cost-cap escalation SHALL have been published once

## MODIFIED Requirements

### Requirement: Exit summary

On exit the drive loop SHALL print a human-readable summary that states the overall outcome (completed / escalated-for-review / stuck / heartbeat / session-torn-down / budget-exhausted), each agent's RESOLVED final state, the deduped list of escalated prompts awaiting human review, a usage section, and a pointer to the broker log and captured learnings. Each agent's final state SHALL be resolved from terminal artifacts — the most-progressed `WorkerPhase` ever observed for it — rather than the agent's last broker-recorded `agent.status`, so a wave whose workers all finished is not reported as still "working."

The usage section SHALL report attributed token usage (and cost where the CLI reports it) by role — workers grouped by CLI id, and the loop's headless orchestrator invocations — SHALL state how many workers' usage is unknown, and SHALL state that it excludes the operator's own session. When a budget cap is configured it SHALL show each cap beside the known total.

Whether a branch is **merged** SHALL be derived from git refs, never from loop state or broker status: a coding agent's branch counts as merged only when its tip is an ancestor of the default branch (`git merge-base --is-ancestor <branch> <default>` exits `0`, the default branch resolved through the single default-branch resolver). A `verified` or `done` status SHALL NOT by itself count as merged. A comparison git cannot make (a missing branch, an unresolvable ref, a failed spawn) SHALL count as unmerged.

The summary SHALL print a wave-level line `N/M branches merged` using the ref-derived count, SHALL list every unmerged branch with its count of commits not in the default branch and its tip SHA, and SHALL annotate each agent's line with its merged / unmerged state alongside its resolved phase. A wave with one or more unmerged branches at exit SHALL NOT be reported `completed`: its outcome SHALL be `escalated-for-review`, with one escalation entry per unmerged branch (verdict `unmerged`, naming the branch, its unmerged-commit count, and its tip). These entries SHALL be added to the summary directly and SHALL NOT be typed into any surface.

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

#### Scenario: Unmerged entries never reach a surface

- **GIVEN** a drive loop exiting with at least one unmerged branch
- **WHEN** the `unmerged` escalation entries are added to the summary
- **THEN** no keystroke SHALL be dispatched to any surface for them

#### Scenario: The usage section separates roles and names unknowns

- **GIVEN** a drive loop whose run had two workers on CLI `sonnet` with known usage, one worker with unknown usage, and one headless orchestrator invocation with known usage
- **WHEN** the summary is printed
- **THEN** its usage section SHALL show a `sonnet` worker total, an orchestrator-invocation total, and one worker with unknown usage
- **AND** it SHALL state that the operator's own session is excluded
