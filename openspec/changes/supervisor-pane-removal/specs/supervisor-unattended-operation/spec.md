## MODIFIED Requirements

### Requirement: Every live permission prompt is escalated without blocking the wave

When a swept coding-agent surface shows a live permission prompt, the drive loop SHALL escalate it for a human decision and SHALL NOT answer it. The loop SHALL NOT block the wave waiting on that decision. It SHALL keep sweeping and progressing the remaining agents. The escalation SHALL carry the verdict label `permission-prompt`, the originating agent, and the prompted command: the command slice, or the file-prompt target path when the prompt has no command header. It SHALL be deduplicated per *Alert dedup keys on command/agent identity, never on boilerplate text*, so one unanswered prompt produces one escalation per dedup window. A surface that is not a coding agent (the dashboard, or a surface whose owner matches no agent worktree) SHALL NOT be scanned for a permission prompt.

Each emitted escalation SHALL be recorded on the broker as a drainable review item. It SHALL appear in the exit summary's escalation list and SHALL be recorded in the manual-decision log (`approval-pattern-surfacing`). It SHALL also produce a `tooling_friction` learning through `sweep.sh learn`. No orchestrator runs inside the session, so the loop SHALL NOT type the escalation into any surface: the broker record, the exit summary and the manual-decision log are the whole escalation, and an orchestrator outside the session (the operator's own agent session, or a person) reads it from the broker and relays it to the human. A wave whose loop recorded any escalation SHALL be reported `escalated-for-review`, not `completed`.

This requirement adds no exit condition. The loop keeps running until completion, the heartbeat, or another existing exit reason, even when every remaining agent is waiting on a prompt.

#### Scenario: A live prompt is escalated without blocking the other agent

- **GIVEN** an `--unattended` session with two agents, one of which shows a live permission prompt for `ls -la`
- **WHEN** the drive loop sweeps the panes and the other agent finishes its work
- **THEN** the loop SHALL record a `permission-prompt` escalation naming the prompting agent and `ls -la`
- **AND** SHALL send no keystroke to the prompting pane
- **AND** the other agent's work SHALL complete and the wave SHALL be reported `escalated-for-review`

#### Scenario: The escalation reaches the broker, the summary and the decision log

- **GIVEN** an `--unattended` session whose agent `feat-a` shows a live permission prompt
- **WHEN** the drive loop escalates it
- **THEN** the broker SHALL receive an `agent.question` naming `feat-a` and the prompted command
- **AND** the exit summary's escalation list SHALL contain the prompt
- **AND** the session's manual-decision log SHALL gain one entry for it

#### Scenario: A permission-prompt escalation is never typed into a surface

- **GIVEN** an `--unattended` session whose agent `feat-a` shows a live permission prompt
- **WHEN** the drive loop escalates it
- **THEN** no keystroke carrying the escalation SHALL be sent to any surface

#### Scenario: A prompt drawn on the dashboard is not escalated

- **GIVEN** a dashboard surface whose broker-log panel quotes a worker's permission prompt at the tail of its capture
- **WHEN** the drive loop sweeps the session
- **THEN** no `permission-prompt` escalation SHALL be attributed to the dashboard

#### Scenario: A file-edit prompt is escalated by its target path

- **GIVEN** a live write-permission prompt with no command header, whose target is `src/lib.rs`
- **WHEN** the drive loop escalates it
- **THEN** the escalation's command SHALL be the target path `src/lib.rs`

#### Scenario: A prompt nobody answers does not end the run

- **GIVEN** an `--unattended` session whose only coding agent sits on an unanswered live prompt
- **WHEN** the drive loop keeps sweeping
- **THEN** the loop SHALL NOT exit for that reason alone
- **AND** it SHALL exit only on completion, the heartbeat, or another existing exit reason

### Requirement: Unattended drive loop runs a supervisor wave to completion

When `git paw start --unattended` is invoked (supervised mode active), the system SHALL run an in-process **drive loop** that keeps the multi-agent wave moving with no human in the seat. It SHALL poll on an approximately 15-second cadence, sweep every coding-agent surface, escalate live permission prompts for a human decision, detect completion, and then exit with a summary. The loop SHALL NOT sweep, capture for a decision, or type into any surface that is not a coding agent (the dashboard, or a surface whose owner matches no agent worktree); no orchestrator runs inside the session.

The drive loop SHALL run in the foreground `git paw start --unattended` process (NOT inside the `__dashboard` subprocess) so that the process's exit status and summary belong to the `--unattended` invocation. The loop SHALL block until an exit condition is reached and SHALL NOT require an attached interactive terminal.

When `--unattended` is absent, the launch SHALL return immediately with the attach hint and run no drive loop.

#### Scenario: --unattended runs a session to completion and exits with a summary

- **GIVEN** an isolated supervised session (private tmux socket, ephemeral broker port, throwaway repo, dummy CLI) launched with `git paw start --unattended`
- **WHEN** the agents reach a completion signal
- **THEN** the drive loop SHALL exit
- **AND** the process SHALL print a summary describing the outcome and per-agent final state
- **AND** the process SHALL exit with status `0` on a clean completion

#### Scenario: Drive loop polls on an approximately 15-second cadence

- **GIVEN** a running unattended drive loop
- **WHEN** the loop has not reached any exit condition
- **THEN** the loop SHALL re-sweep all coding-agent surfaces on an approximately 15-second interval
- **AND** SHALL continue polling until an exit condition is reached

#### Scenario: The dashboard surface is never swept for a decision

- **GIVEN** a running unattended drive loop whose dashboard surface shows text resembling a live permission prompt
- **WHEN** the loop sweeps the session
- **THEN** it SHALL NOT escalate, nudge or submit input on the dashboard surface

#### Scenario: Without --unattended the launch returns immediately

- **GIVEN** `git paw start --supervisor` is invoked WITHOUT `--unattended`
- **WHEN** the launch completes its session-build steps
- **THEN** no drive loop SHALL run
- **AND** the launch SHALL return with the standard attach hint

### Requirement: CLI unavailability is an explicit non-success exit

The drive loop SHALL recognise a coding agent's CLI that has stopped serving its pane — a usage or rate limit reached, or authentication lost — by matching `cli_unavailable_markers` against that agent's capture **tail**, and SHALL surface it as an explicit non-success outcome instead of letting the wave run on to a `completed` exit. The tail SHALL tolerate the same bounded trailing region of non-prompt content (a task list, a status strip) that the live-prompt gate tolerates, so content a CLI draws beneath its banner does not hide the banner; a marker separated from the end of the capture by more than that bounded region SHALL still count as scrolled away.

The markers SHALL be resolved for the CLI running in that pane: a `[clis.<name>]` override of `cli_unavailable_markers` SHALL apply, falling back to the embedded default when the CLI has no override. The drive loop and the bundled helper script SHALL therefore classify the same pane identically for this field.

When a coding agent's pane shows a marker, the loop SHALL record a `cli-unavailable` escalation for that agent and SHALL continue driving the remaining agents; a wave carrying such an escalation SHALL NOT be reported `completed`. A `cli-unavailable` escalation SHALL be deduplicated on the agent and the matched marker, and SHALL NOT be keyed on any other content of the capture — so the same banner seen across sweeps is one escalation, and two different banners on one agent are two. Text on a surface that is not a coding agent (the dashboard, whose broker log can quote a worker's banner) SHALL NOT be scanned for markers. The orchestrator runs outside the session, so the exit summary SHALL state that the orchestrator's own CLI is not monitored by the loop.

#### Scenario: A rate-limited worker is escalated while the wave keeps moving

- **GIVEN** an unattended drive loop with two coding agents, one of whose pane tails shows a CLI-unavailability marker
- **WHEN** the loop sweeps the panes
- **THEN** it SHALL record a `cli-unavailable` escalation naming that agent
- **AND** it SHALL keep sweeping the other agent's pane and escalating its prompts
- **AND** the eventual summary SHALL NOT report the wave `completed`

#### Scenario: A banner above a task-list footer is still detected

- **GIVEN** a coding agent's capture ending with `You've hit your session limit` followed by a six-line task-list region
- **WHEN** the loop sweeps that pane
- **THEN** it SHALL record a `cli-unavailable` escalation for that agent

#### Scenario: A marker outside the capture tail is ignored

- **GIVEN** a pane whose scrolled-away history contains a CLI-unavailability marker, separated from the end of the capture by more than the bounded trailing region, while its tail shows a normal idle input box
- **WHEN** the loop sweeps that pane
- **THEN** no `cli-unavailable` escalation SHALL be recorded

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

#### Scenario: A banner quoted by the dashboard does not trigger

- **GIVEN** a dashboard surface whose broker-log panel displays a worker's `You've hit your session limit` line at the tail of its capture
- **WHEN** the loop sweeps the session
- **THEN** no `cli-unavailable` escalation or exit SHALL be attributed to the dashboard

#### Scenario: The summary does not claim orchestrator coverage

- **GIVEN** an unattended drive loop whose coding agents show no marker
- **WHEN** the loop exits and prints its summary
- **THEN** the summary SHALL state that the orchestrator's CLI is not monitored by the loop

#### Scenario: Without a marker the loop behaves as before

- **GIVEN** panes whose captures contain no `cli_unavailable_markers` text
- **WHEN** the loop runs to a completion or heartbeat exit
- **THEN** the outcome and exit status SHALL be identical to the behaviour before this requirement

### Requirement: Exit summary

On exit the drive loop SHALL print a human-readable summary that states the overall outcome (completed / escalated-for-review / stuck / heartbeat / session-torn-down), each agent's RESOLVED final state, the deduped list of escalated prompts awaiting human review, and a pointer to the broker log and captured learnings. Each agent's final state SHALL be resolved from terminal artifacts — the most-progressed `WorkerPhase` ever observed for it — rather than the agent's last broker-recorded `agent.status`, so a wave whose workers all finished is not reported as still "working."

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

## REMOVED Requirements

### Requirement: The drive loop covers the supervisor's own pane 0

**Reason**: No orchestrator CLI runs inside the session, so there is no supervisor pane to sweep; pane 0 is the dashboard in every broker-enabled mode.

**Migration**: The drive loop sweeps coding-agent surfaces only. Live permission prompts are escalated per *Every live permission prompt is escalated without blocking the wave*. Other judgment calls are recorded as broker review items (this change) and handed to a headless orchestrator invocation by `orchestrator-escalation-handoff`.
