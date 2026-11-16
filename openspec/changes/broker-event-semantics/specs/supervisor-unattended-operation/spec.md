## MODIFIED Requirements

### Requirement: Exit summary

On exit the drive loop SHALL print a human-readable summary that states the overall outcome (completed / escalated-for-review / stuck / heartbeat / session-torn-down / budget-exhausted), each agent's RESOLVED final state, the deduped list of escalated prompts awaiting human review, a usage section, and a pointer to the broker log and captured learnings. Each agent's final state SHALL be resolved from terminal artifacts — the most-progressed `WorkerPhase` ever observed for it — rather than the agent's last broker-recorded `agent.status`, so a wave whose workers all finished is not reported as still "working."

The usage section SHALL report attributed token usage (and cost where the CLI reports it) by role — workers grouped by CLI id, and the loop's headless orchestrator invocations — SHALL state how many workers' usage is unknown, and SHALL state that it excludes the operator's own session. When a budget cap is configured it SHALL show each cap beside the known total.

Whether a branch is **merged** SHALL be derived from git refs, never from loop state or broker status alone: a coding agent's branch counts as merged when its tip is an ancestor of the default branch (`git merge-base --is-ancestor <branch> <default>` exits `0`, the default branch resolved through the single default-branch resolver), OR when a merged tip the loop recorded for that branch from an `agent.advanced-main` `merged_sha` is an ancestor of the default branch. In the second case the branch's commits that are not in the default branch SHALL be reported as commits after the merge, not as unmerged work. A `verified` or `done` status SHALL NOT by itself count as merged. A comparison git cannot make (a missing branch, an unresolvable ref, a failed spawn) SHALL count as unmerged.

The summary SHALL print a wave-level line `N/M branches merged` using the ref-derived count, SHALL list every unmerged branch with its count of commits not in the default branch and its tip SHA, SHALL list every merged branch with commits after its merge with that count and its tip SHA, and SHALL annotate each agent's line with its merged / unmerged state alongside its resolved phase. A wave with one or more unmerged branches at exit SHALL NOT be reported `completed`: its outcome SHALL be `escalated-for-review`, with one escalation entry per unmerged branch (verdict `unmerged`, naming the branch, its unmerged-commit count, and its tip). These entries SHALL be added to the summary directly and SHALL NOT be typed into any surface.

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

- **GIVEN** a drive loop whose two coding agents both published `done`, while neither branch tip is an ancestor of the default branch and no merged tip was recorded for either
- **WHEN** the loop exits and prints the summary
- **THEN** the wave-level line SHALL read `0/2 branches merged`
- **AND** each agent's line SHALL show its resolved phase together with `unmerged` and its count of commits not in the default branch
- **AND** the outcome SHALL be `escalated-for-review`, not `completed`
- **AND** the escalation list SHALL contain one `unmerged` entry per branch naming its tip SHA

#### Scenario: A commit after the merge does not unmerge the branch

- **GIVEN** a drive loop that recorded `merged_sha = aaa111` for `feat/x` from an `agent.advanced-main`, `aaa111` is an ancestor of the default branch, and the worker then committed once more on `feat/x`
- **WHEN** the loop exits and prints the summary
- **THEN** `feat/x` SHALL count as merged in the `N/M branches merged` line
- **AND** its line SHALL report one commit after the merge with its tip SHA
- **AND** no `unmerged` escalation entry SHALL be added for `feat/x`

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
