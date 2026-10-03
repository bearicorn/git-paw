## ADDED Requirements

### Requirement: Every live permission prompt is escalated without blocking the wave

When a swept pane, a coding-agent pane or the supervisor's own pane 0, shows a live permission prompt, the drive loop SHALL escalate it for a human decision and SHALL NOT answer it. The loop SHALL NOT block the wave waiting on that decision. It SHALL keep sweeping and progressing the remaining agents. The escalation SHALL carry the verdict label `permission-prompt`, the originating agent, and the prompted command: the command slice, or the file-prompt target path when the prompt has no command header. It SHALL be deduplicated per *Alert dedup keys on command/agent identity, never on boilerplate text*, so one unanswered prompt produces one escalation per dedup window.

Each emitted escalation SHALL be recorded on the broker as a drainable review item. It SHALL appear in the exit summary's escalation list and SHALL be recorded in the manual-decision log (`approval-pattern-surfacing`). It SHALL also produce a `tooling_friction` learning through `sweep.sh learn`. The broker record SHALL be the same whether or not an orchestrator is running. When an orchestrator (supervisor CLI) pane is present, the loop SHALL also hand the escalation to that pane (see `supervisor-autonomous-orchestrator`). When none is present, the broker record and the summary are the whole escalation. A wave whose loop recorded any escalation SHALL be reported `escalated-for-review`, not `completed`.

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

#### Scenario: A file-edit prompt is escalated by its target path

- **GIVEN** a live write-permission prompt with no command header, whose target is `src/lib.rs`
- **WHEN** the drive loop escalates it
- **THEN** the escalation's command SHALL be the target path `src/lib.rs`

#### Scenario: A prompt nobody answers does not end the run

- **GIVEN** an `--unattended` session whose only coding agent sits on an unanswered live prompt
- **WHEN** the drive loop keeps sweeping
- **THEN** the loop SHALL NOT exit for that reason alone
- **AND** it SHALL exit only on completion, the heartbeat, or another existing exit reason

## MODIFIED Requirements

### Requirement: Unattended drive loop runs a supervisor wave to completion

When `git paw start --unattended` is invoked (supervisor mode active), the system SHALL run an in-process **drive loop** that keeps the multi-agent wave moving with no human in the seat. It SHALL poll on an approximately 15-second cadence, sweep the supervisor pane and every coding-agent pane, escalate live permission prompts for a human decision, detect completion, and then exit with a summary.

The drive loop SHALL run in the foreground `git paw start --unattended` process (NOT inside the `__dashboard` subprocess) so that the process's exit status and summary belong to the `--unattended` invocation. The loop SHALL block until an exit condition is reached and SHALL NOT require an attached interactive terminal.

When `--unattended` is absent, the launch SHALL behave exactly as the v0.5.0 supervisor launch (return immediately with an attach hint, no drive loop).

#### Scenario: --unattended runs a session to completion and exits with a summary

- **GIVEN** an isolated supervisor session (private tmux socket, ephemeral broker port, throwaway repo, dummy CLI) launched with `git paw start --unattended`
- **WHEN** the agents reach a completion signal
- **THEN** the drive loop SHALL exit
- **AND** the process SHALL print a summary describing the outcome and per-agent final state
- **AND** the process SHALL exit with status `0` on a clean completion

#### Scenario: Drive loop polls on an approximately 15-second cadence

- **GIVEN** a running unattended drive loop
- **WHEN** the loop has not reached any exit condition
- **THEN** the loop SHALL re-sweep all panes on an approximately 15-second interval
- **AND** SHALL continue polling until an exit condition is reached

#### Scenario: Without --unattended the launch returns immediately

- **GIVEN** `git paw start --supervisor` is invoked WITHOUT `--unattended`
- **WHEN** the launch completes its session-build steps
- **THEN** no drive loop SHALL run
- **AND** the launch SHALL return with the standard attach hint (v0.5.0 behaviour unchanged)

### Requirement: The drive loop covers the supervisor's own pane 0

The drive loop SHALL sweep the supervisor's OWN pane (pane 0), not just the coding-agent panes. On pane 0 it SHALL detect CLI unavailability, escalate a live permission prompt (under the agent id `supervisor`), and submit a buffered directive left on an idle input box. It SHALL NOT send approval keystrokes to pane 0. When pane 0 shows a live permission prompt, the loop SHALL NOT inject a hand-off or nudge into pane 0 while that prompt is up. The escalation is then carried by the broker and the exit summary.

#### Scenario: A live prompt on the supervisor pane is escalated, not answered

- **GIVEN** an `--unattended` session where the supervisor pane (pane 0) shows a live permission prompt
- **WHEN** the drive loop sweeps pane 0
- **THEN** the loop SHALL record a `permission-prompt` escalation for agent `supervisor` on the broker and in the summary
- **AND** SHALL send no keystroke to pane 0

#### Scenario: Pane 0 with no live prompt is left untouched

- **GIVEN** the supervisor pane (pane 0) is mid-conversation with no live recognized permission prompt
- **WHEN** the drive loop sweeps pane 0
- **THEN** the loop SHALL send no keystrokes to pane 0

### Requirement: Multiple feedback-fix-reverify cycles are tolerated

The drive loop SHALL tolerate multiple feedback→fix→re-verify cycles per agent. An agent that has received feedback and is iterating SHALL NOT be treated as stuck. The loop SHALL NOT treat "not yet verified after N cycles" as a stuck or exit signal. Only the explicit stuck/bloat signal, a live permission prompt (which is escalated), a completion signal, or the heartbeat are exit/re-engage conditions.

#### Scenario: An agent iterating through N feedback cycles is not flagged stuck

- **GIVEN** an agent that has gone through N feedback→fix→re-verify cycles and is still iterating (making progress between cycles)
- **WHEN** the drive loop evaluates the agent
- **THEN** the loop SHALL NOT classify the agent as stuck on the basis of cycle count
- **AND** SHALL continue letting the agent iterate

### Requirement: The drive loop self-captures qualitative learnings

Because an unattended wave has no human to hand-write findings, the drive loop SHALL self-capture qualitative learnings via the bundled `sweep.sh learn` subcommand (per `learnings-supervisor-observation-channel`). It SHALL do so opportunistically when it escalates a prompt or absorbs other friction during the run, and once at wind-down as a synthesis pass. The loop SHALL NOT hand-roll raw curl to publish `agent.learning`.

#### Scenario: An escalated prompt is recorded as a learning

- **GIVEN** an unattended drive loop that escalates a live permission prompt
- **WHEN** the loop records that friction
- **THEN** the loop SHALL record a qualitative learning via `sweep.sh learn`
- **AND** SHALL NOT publish the learning via raw curl

#### Scenario: Wind-down synthesis records durable learnings

- **GIVEN** an unattended drive loop reaching an exit condition
- **WHEN** the loop performs its wind-down summary
- **THEN** the loop SHALL run a session-end synthesis pass that records durable qualitative learnings via `sweep.sh learn`
- **AND** SHALL dedup them against learnings already recorded in-session

### Requirement: The in-tool loop replaces external pane-scraping monitor scripts

The `--unattended` drive loop SHALL be the supported mechanism for running a wave to completion without a human, replacing the external `.git-paw/scripts/wave*-monitor.sh` pane-scraping monitors. The in-tool loop SHALL be testable end-to-end via the `selftest` harness (dummy CLI, private tmux socket, ephemeral broker port, throwaway repo) with no real LLM backend and no interactive terminal.

#### Scenario: Drive loop is exercised end-to-end without a real LLM

- **GIVEN** the `selftest` isolation harness with a dummy CLI scripted to emit a permission prompt and a completion signal
- **WHEN** an `--unattended` drive loop runs against the isolated session
- **THEN** the loop SHALL escalate the dummy prompt without sending it any keystroke, detect the completion signal, and exit with a summary
- **AND** the test SHALL require no real LLM backend and no interactive terminal

### Requirement: Approval on a pane is guarded by an exclusive per-pane claim

Before any component sends keystrokes that answer a prompt or submit input in an agent pane, it SHALL acquire an **exclusive per-pane claim**, and SHALL release it once those keystrokes are sent. This covers the explicit `sweep.sh approve <pane>` action and the drive loop's nudge, stale-input recovery and buffered-input submit. The claim SHALL be an atomic filesystem claim, created via an atomic create-if-absent so two concurrent acquirers cannot both succeed, keyed by pane and located under a gitignored runtime directory. The in-process drive loop (Rust) and the bundled `sweep.sh` helper (shell) SHALL honour the claim identically, so senders in DIFFERENT processes cannot both type into the same pane. Acquisition SHALL be **non-blocking**: if the claim is already held, the sender SHALL skip that pane for this attempt (it SHALL NOT wait) and rely on a later tick or a re-run, preserving the non-blocking escalation guarantee. The claim SHALL be released even on error (RAII in Rust, `trap` in shell), so a crashed or interrupted sender does not permanently wedge a pane.

#### Scenario: An approver holds the pane's claim before sending

- **GIVEN** `sweep.sh approve N` about to approve the live prompt on pane N
- **WHEN** it dispatches the approval keystroke(s)
- **THEN** it SHALL hold pane N's exclusive claim for the duration of the send

#### Scenario: Two senders race on one pane — exactly one sends

- **GIVEN** two senders concurrently attempting to type into the same pane N
- **WHEN** both try to acquire pane N's claim
- **THEN** exactly one SHALL acquire the claim and send its keystrokes
- **AND** the other SHALL fail to acquire and SHALL NOT send any keystroke

#### Scenario: Acquisition is non-blocking when the claim is held

- **GIVEN** pane N's claim is currently held by another sender
- **WHEN** an approver attempts to approve pane N
- **THEN** it SHALL skip pane N for this attempt rather than block

#### Scenario: The Rust loop and the shell helper are mutually exclusive

- **GIVEN** the in-process drive loop holds pane N's claim while it submits buffered input or recovers a nudge
- **WHEN** `sweep.sh approve N` runs in a separate process
- **THEN** it SHALL observe the claim as held and SHALL NOT send an approval keystroke
- **AND** the same exclusion SHALL hold in the reverse direction (shell holds it, Rust skips its send)

#### Scenario: The claim is released after the send and on error

- **GIVEN** a sender that acquired pane N's claim and then finished sending (or hit an error)
- **WHEN** the attempt returns
- **THEN** pane N's claim SHALL be released so a later sender can acquire it
- **AND** a sender that panics/errors mid-send SHALL NOT leave pane N permanently claimed

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
- **AND** it SHALL keep sweeping the other agent's pane and escalating its prompts
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

## REMOVED Requirements

### Requirement: The drive loop is the sole auto-approver for unattended sessions

**Reason**: No component auto-approves any more, so there is no approver race to prevent.
**Migration**: See `approval-auto`, *Approval keystrokes are sent only by an explicit approve action*.

### Requirement: Auto-approve classifier-safe prompts

**Reason**: The drive loop no longer classifies or approves prompts.
**Migration**: Every live prompt is escalated (*Every live permission prompt is escalated without blocking the wave*). For prompt-free unattended runs, use the agent CLI's own allow rules, or `full-auto` inside a sandbox.

### Requirement: Pane-0 approval must not pollute the supervisor's context

**Reason**: The drive loop sends no approval keystrokes to pane 0, so there is no pane-0 approval to keep minimal.
**Migration**: Pane-0 behaviour is specified by *The drive loop covers the supervisor's own pane 0* (modified) and `approval-auto`, *Blind send-keys excludes the supervisor pane 0*.

### Requirement: Escalation of risky and unknown prompts is non-blocking

**Reason**: Its trigger was a `danger` / `unknown` classifier verdict, which no longer exists. It also described the loop as the sole approver of classifier-safe prompts.
**Migration**: Replaced by *Every live permission prompt is escalated without blocking the wave*. The broker record, the orchestrator hand-off and the non-blocking guarantee carry over unchanged.

### Requirement: Auto-approval prefers a durable grant for a safe prompt

**Reason**: git-paw never selects a "don't ask again" grant now.
**Migration**: None. Durable grants are a decision for the agent CLI's own permission settings, made by the user.
