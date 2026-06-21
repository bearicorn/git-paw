## ADDED Requirements

### Requirement: Unattended drive loop runs a supervisor wave to completion

When `git paw start --unattended` is invoked (supervisor mode active), the system SHALL run an in-process **drive loop** that keeps the multi-agent wave moving with no human in the seat: it SHALL poll on an approximately 15-second cadence, sweep the supervisor pane and every coding-agent pane, act on live prompts, detect completion, and then exit with a summary.

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

### Requirement: The drive loop is the sole auto-approver for unattended sessions

For an `--unattended` session the in-process drive loop SHALL be the sole auto-approver. The dashboard's auto-approve thread (which runs inside the `__dashboard` subprocess for non-unattended sessions) SHALL be disabled for unattended sessions so that two approvers never race on the same pane.

#### Scenario: Dashboard auto-approve thread is disabled under --unattended

- **GIVEN** an `--unattended` supervisor session
- **WHEN** the dashboard subprocess starts
- **THEN** the dashboard's auto-approve thread SHALL NOT fire on any pane
- **AND** the in-process drive loop SHALL be the only component sending approval keystrokes

### Requirement: Auto-approve classifier-safe prompts

When a swept pane shows a live prompt the drive loop SHALL classify it via the `auto-approve-classifier`. When the classification is safe, the loop SHALL approve the prompt by sending the agent CLI's documented "approve and remember" keystroke sequence per the `automatic-approval` capability (each logical key sent as a separate `tmux send-keys`). For a 2-option Yes/No prompt the loop SHALL send `"1"` then `Enter` directly rather than `Down`+`Enter`, because on a 2-option prompt `Down`+`Enter` selects the wrong option.

Every auto-approval SHALL be recorded in the broker message log (per `automatic-approval`) before the keystrokes are sent.

#### Scenario: Classifier-safe prompt is auto-approved

- **GIVEN** a coding-agent pane showing a live permission prompt for a classifier-safe command (e.g. `cargo test`)
- **WHEN** the drive loop sweeps the pane
- **THEN** the loop SHALL send the documented approve-and-remember keystroke sequence to that pane
- **AND** the wave SHALL continue without blocking

#### Scenario: Two-option Yes/No prompt is approved with "1" then Enter

- **GIVEN** a live 2-option Yes/No permission prompt classified safe
- **WHEN** the drive loop approves it
- **THEN** the loop SHALL send `"1"` followed by a separate `Enter`
- **AND** SHALL NOT send `Down` then `Enter` for the 2-option prompt

#### Scenario: Approval is logged before keystrokes

- **GIVEN** the drive loop is about to auto-approve a safe prompt
- **WHEN** the action is recorded
- **THEN** the broker log entry SHALL be appended before the `tmux send-keys` call so a crash mid-action still leaves an audit trail

### Requirement: The drive loop covers the supervisor's own pane 0

The drive loop SHALL sweep and act on the supervisor's OWN pane (pane 0), not just the coding-agent panes. The supervisor is an agent that hits permission prompts (e.g. running `sweep.sh`, `cargo test`, `git`); without coverage of pane 0 an unattended supervisor stalls on its first non-allowlisted prompt (v0.6.0 finding W15-3).

#### Scenario: Safe prompt on the supervisor pane is approved

- **GIVEN** an `--unattended` session where the supervisor pane (pane 0) shows a live permission prompt for a classifier-safe command
- **WHEN** the drive loop sweeps pane 0
- **THEN** the loop SHALL approve the supervisor pane's safe prompt
- **AND** the supervisor SHALL NOT stall on that prompt

#### Scenario: Pane 0 with no live prompt is left untouched

- **GIVEN** the supervisor pane (pane 0) is mid-conversation with no live recognized permission prompt
- **WHEN** the drive loop sweeps pane 0
- **THEN** the loop SHALL send no keystrokes to pane 0

### Requirement: Pane-0 approval must not pollute the supervisor's context

A send-keys approver MUST NOT blindly type into pane 0 (v0.6.0 finding W15-13): clearing the supervisor's live prompt and polluting its conversation context are different failure modes. When the drive loop approves a prompt on pane 0 it SHALL send ONLY the minimal approval keystrokes that the live prompt consumes, and SHALL NOT send free-text or a trailing newline that would land in the supervisor's prompt box. When pane 0 shows no live recognized permission prompt, the loop SHALL send nothing to pane 0.

#### Scenario: Pane-0 approval sends only the minimal approval keystrokes

- **GIVEN** the supervisor pane shows a live recognized permission prompt classified safe
- **WHEN** the drive loop approves it
- **THEN** the loop SHALL send only the minimal approval keystrokes the prompt consumes
- **AND** SHALL NOT send any free-text characters into the supervisor's prompt box
- **AND** SHALL NOT send a stray trailing newline that would submit unintended input to the supervisor

#### Scenario: No keystrokes when pane 0 is not at a prompt

- **GIVEN** the supervisor pane is producing output but not displaying a live recognized permission prompt
- **WHEN** the drive loop evaluates pane 0
- **THEN** no keystrokes SHALL be sent to pane 0

### Requirement: Act only on a live prompt in the capture tail

The drive loop SHALL treat a pane as showing an actionable prompt ONLY when a recognized prompt footer appears within the last approximately 4 non-blank lines of the pane capture. Prompt-like text that appears earlier in the scrollback (already-resolved history) SHALL NOT trigger any action.

#### Scenario: Prompt footer in the capture tail triggers action

- **GIVEN** a pane capture whose last non-blank lines contain a recognized permission-prompt footer
- **WHEN** the drive loop evaluates the capture
- **THEN** the loop SHALL treat the pane as showing a live prompt and act on it

#### Scenario: Prompt-like text in scrollback is ignored

- **GIVEN** a pane capture containing a recognized prompt footer earlier in the scrollback but whose last approximately 4 non-blank lines show ordinary CLI output (no footer)
- **WHEN** the drive loop evaluates the capture
- **THEN** the loop SHALL NOT treat the pane as showing a live prompt
- **AND** SHALL send no keystrokes to that pane

### Requirement: Each pane is captured explicitly, never via a shell for-loop

The drive loop SHALL capture each pane with an explicit per-pane `tmux capture-pane` invocation. The loop SHALL NOT capture panes via a single shell `for p in …` loop, because shell variable expansion over pane lists trips approval gates and obscures which pane produced which capture.

#### Scenario: Per-pane explicit capture

- **GIVEN** a session with N panes
- **WHEN** the drive loop sweeps the panes in one poll iteration
- **THEN** the loop SHALL issue one explicit `tmux capture-pane` per pane
- **AND** SHALL NOT issue a single `for p in …` shell loop to capture all panes

### Requirement: Pane-to-agent resolution via pane_current_path

The drive loop SHALL resolve each pane to its agent by matching the pane's `pane_current_path` against the session's per-agent `worktree_path`. The loop SHALL NOT resolve panes by pane index or by CLI-argument order, because pane indices are neither alphabetical nor argument-ordered and drift on layout changes. Pane 0 (supervisor) and pane 1 (dashboard) resolve to the repo root.

#### Scenario: Agent resolved by working directory, not index

- **GIVEN** a session whose pane indices do not match the alphabetical or CLI-argument order of agents, and an agent on worktree `/path/to/repo-feat-b`
- **WHEN** the drive loop resolves the pane whose `pane_current_path` is `/path/to/repo-feat-b`
- **THEN** the loop SHALL attribute that pane to the `feat/b` agent
- **AND** SHALL NOT rely on the pane index to make the attribution

#### Scenario: A captured prompt is attributed to the correct agent

- **GIVEN** two coding-agent panes in different worktrees, one showing a live prompt
- **WHEN** the drive loop captures and resolves the panes via `pane_current_path`
- **THEN** the prompt SHALL be attributed to the agent whose `worktree_path` equals the prompting pane's `pane_current_path`

### Requirement: send-keys nudges send a follow-up Enter

When the drive loop sends a nudge to a pane (any text intended to be submitted), it SHALL send the text and then a SEPARATE `Enter` keystroke, because on paste-aware CLIs the first `Enter` buffers the input rather than submitting it.

#### Scenario: Nudge submits with a separate follow-up Enter

- **GIVEN** the drive loop nudges a pane with submittable text
- **WHEN** the keystrokes are dispatched
- **THEN** the loop SHALL send the text, then a separate `Enter` keystroke
- **AND** SHALL NOT rely on a single combined text+Enter to submit the nudge

### Requirement: Escalation of risky and unknown prompts is non-blocking

When a live prompt is classified `danger` or `unknown` (not safe), the drive loop SHALL escalate it for LATER human review and SHALL NOT block the wave waiting on it. The loop SHALL surface the escalation (via the broker and in the exit summary) and SHALL continue sweeping and progressing the remaining agents. The wave SHALL NOT freeze indefinitely on a single risky prompt.

#### Scenario: Risky prompt is escalated without blocking the wave

- **GIVEN** an `--unattended` session with two agents, one of which shows a live prompt classified `danger`
- **WHEN** the drive loop sweeps the panes
- **THEN** the loop SHALL NOT auto-approve the risky prompt
- **AND** SHALL record the prompt as an escalation for human review
- **AND** SHALL continue progressing the other agent rather than blocking on the risky prompt

#### Scenario: Unknown classification is escalated, not approved

- **GIVEN** a live prompt the classifier returns as `unknown`
- **WHEN** the drive loop evaluates it
- **THEN** the loop SHALL NOT send approval keystrokes
- **AND** SHALL surface the prompt for human review (broker + summary)

### Requirement: Alert dedup keys on command/agent identity, never on boilerplate text

When the drive loop escalates or records an alert, it SHALL dedup repeated alerts on the tuple `(agent_id, shape)` within an approximately 5-minute window, where `shape` is derived from the command/agent identity of the prompt (or a wait-for-clear token) and NOT from the prompt's boilerplate sentence text. The same prompt re-observed every poll SHALL produce ONE alert per window. Two genuinely-distinct prompts from the same agent SHALL NOT be collapsed merely because they share boilerplate text (v0.6.0 finding W15-19).

#### Scenario: Repeated identical alert is deduped within the window

- **GIVEN** an agent whose pane shows the same escalating prompt across several consecutive poll iterations within 5 minutes
- **WHEN** the drive loop observes the prompt on each iteration
- **THEN** the loop SHALL emit exactly one alert for that `(agent_id, shape)` within the window
- **AND** SHALL NOT emit a duplicate alert on each repeat observation

#### Scenario: Two distinct prompts sharing boilerplate are not collapsed

- **GIVEN** the same agent showing two different prompts (different commands/identities) that share the same boilerplate header text within the dedup window
- **WHEN** the drive loop derives the dedup key for each
- **THEN** the keys SHALL differ because `shape` is derived from command/agent identity, not boilerplate
- **AND** the loop SHALL emit a separate alert for each distinct prompt

### Requirement: Stall detection is pane-keyed, not agent-record-keyed

The drive loop's stall/stuck iteration SHALL be keyed on the tmux pane, not on a broker agent record. A pane that has booted but has not yet published any `agent.status` (the boot chicken-and-egg) SHALL still be swept and evaluated for a stall (v0.6.0 finding W15-7). The stuck/bloat determination itself is supplied by the `supervisor-stuck-bloat-detection` capability; this requirement governs that the drive loop feeds it every pane regardless of broker presence.

#### Scenario: Pane with no broker presence is still watched

- **GIVEN** a coding-agent pane that has booted but has not yet published an `agent.status` to the broker
- **WHEN** the drive loop runs a poll iteration
- **THEN** the loop SHALL capture and evaluate that pane for a stall
- **AND** SHALL NOT skip the pane merely because no broker agent record exists for it yet

### Requirement: Multiple feedback-fix-reverify cycles are tolerated

The drive loop SHALL tolerate multiple feedback→fix→re-verify cycles per agent. An agent that has received feedback and is iterating SHALL NOT be treated as stuck. The loop SHALL NOT treat "not yet verified after N cycles" as a stuck or exit signal; only the explicit stuck/bloat signal, a genuinely risky prompt, a completion signal, or the heartbeat are exit/re-engage conditions.

#### Scenario: An agent iterating through N feedback cycles is not flagged stuck

- **GIVEN** an agent that has gone through N feedback→fix→re-verify cycles and is still iterating (making progress between cycles)
- **WHEN** the drive loop evaluates the agent
- **THEN** the loop SHALL NOT classify the agent as stuck on the basis of cycle count
- **AND** SHALL continue letting the agent iterate

### Requirement: Completion detection

The drive loop SHALL detect wave completion and exit. Completion SHALL be recognized when either a terminal PASS/FAIL verdict is signalled (e.g. the supervisor publishes a terminal verdict) OR all agents' tasks are checked complete. On completion the loop SHALL stop polling and proceed to the summary.

#### Scenario: PASS/FAIL verdict ends the loop

- **GIVEN** a running unattended drive loop
- **WHEN** a terminal PASS or FAIL verdict is signalled for the wave
- **THEN** the loop SHALL stop polling
- **AND** SHALL proceed to render the exit summary

#### Scenario: All-tasks-checked ends the loop

- **GIVEN** a running unattended drive loop where every agent's assigned tasks are checked complete
- **WHEN** the loop next evaluates completion
- **THEN** the loop SHALL stop polling and proceed to the summary

### Requirement: Heartbeat re-engages the human after prolonged inactivity

When the drive loop has run for approximately 25 minutes without reaching a completion condition, it SHALL re-engage the human by exiting (or surfacing) with a status summary rather than running forever silently.

#### Scenario: Heartbeat fires after prolonged run without completion

- **GIVEN** an unattended drive loop that has run for approximately 25 minutes with no completion signal
- **WHEN** the heartbeat interval elapses
- **THEN** the loop SHALL surface a status summary to re-engage the human
- **AND** SHALL NOT continue running indefinitely without surfacing status

### Requirement: Exit summary

On exit the drive loop SHALL print a human-readable summary that states the outcome (completed / escalated-for-review / stuck / heartbeat), the per-agent final state, the deduped list of escalated prompts awaiting human review, and a pointer to the broker log and captured learnings.

#### Scenario: Summary reports outcome and escalations

- **GIVEN** a drive loop that exits after escalating one risky prompt and completing the rest of the wave
- **WHEN** the summary is printed
- **THEN** the summary SHALL state the overall outcome
- **AND** SHALL list the per-agent final state
- **AND** SHALL include the escalated prompt awaiting human review

### Requirement: The drive loop self-captures qualitative learnings

Because an unattended wave has no human to hand-write findings, the drive loop SHALL self-capture qualitative learnings via the bundled `sweep.sh learn` subcommand (per `learnings-supervisor-observation-channel`) — opportunistically when it absorbs friction during the run, and once at wind-down as a synthesis pass. The loop SHALL NOT hand-roll raw curl to publish `agent.learning`.

#### Scenario: Friction absorbed during the run is recorded as a learning

- **GIVEN** an unattended drive loop that escalates a prompt it could not auto-approve
- **WHEN** the loop absorbs that friction
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
- **THEN** the loop SHALL auto-approve the dummy safe prompt, detect the completion signal, and exit with a summary
- **AND** the test SHALL require no real LLM backend and no interactive terminal
