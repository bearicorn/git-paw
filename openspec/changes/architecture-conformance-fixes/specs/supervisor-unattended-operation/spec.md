## MODIFIED Requirements

### Requirement: Every live permission prompt is escalated without blocking the wave

When a swept coding-agent surface shows a live permission prompt, the drive loop SHALL escalate it for a human decision and SHALL NOT answer it. The loop SHALL NOT block the wave waiting on that decision. It SHALL keep sweeping and progressing the remaining agents. The escalation SHALL carry the verdict label `permission-prompt`, the originating agent, and the prompted command: the command slice, or the file-prompt target path when the prompt has no command header. It SHALL be deduplicated per *Alert dedup keys on command/agent identity, never on boilerplate text*, so one unanswered prompt produces one escalation per dedup window. A surface that is not a coding agent (the dashboard, or a surface whose owner matches no agent worktree) SHALL NOT be scanned for a permission prompt.

Each emitted escalation SHALL be recorded on the broker as a drainable review item. It SHALL appear in the exit summary's escalation list and SHALL be recorded in the manual-decision log (`approval-pattern-surfacing`). It SHALL also produce a `tooling_friction` learning through `sweep.sh learn`. No orchestrator runs inside the session, so the loop SHALL NOT type the escalation into any surface: the broker record, the exit summary and the manual-decision log are the whole escalation, and an orchestrator outside the session (the operator's own agent session, or a person) reads it from the broker and relays it to the human. A wave whose loop recorded any escalation SHALL be reported `escalated-for-review`, not `completed`, except the start-of-run notice that a cost cap cannot be evaluated (*A budget the loop cannot evaluate is reported*), which alone SHALL NOT change a finished wave's outcome.

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

#### Scenario: An unevaluable cost cap alone leaves a finished wave completed

- **GIVEN** `budget_max_cost` set, no worker CLI reporting cost, and agents that all finish with no other escalation
- **WHEN** the drive loop ends the run
- **THEN** the wave SHALL be reported `completed`, not `escalated-for-review`

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

#### Scenario: Headless orchestrator spend counts toward the breach

- **GIVEN** `budget_max_tokens = 1000`, workers whose known usage totals 700 tokens, and a headless orchestrator invocation the loop started this run that used 400 tokens
- **WHEN** the loop next reads usage
- **THEN** it SHALL record one `budget-exhausted` escalation naming a total of 1100 tokens

#### Scenario: A cost cap is breached by reported cost alone

- **GIVEN** `budget_max_cost = 5.0`, `budget_max_tokens` absent, and workers whose CLIs report a total cost of 6.0
- **WHEN** the loop next reads usage
- **THEN** it SHALL record one `budget-exhausted` escalation naming `budget_max_cost` and the cost total
