# supervisor-unattended-operation Specification

## Purpose
Runs an in-process drive loop under `git paw start --unattended` that keeps a supervisor wave moving with no human present — polling every ~15 seconds to sweep all panes, auto-approving classifier-safe prompts (and acting as the sole approver), detecting completion, and exiting with a per-agent summary.
## Requirements
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

When the drive loop sends a nudge to a pane (any text intended to be submitted), it SHALL submit it reliably rather than relying on a single combined text+Enter or a single follow-up `Enter` — because on paste-aware CLIs the text frequently lands in a paste buffer or on a stale input line that a bare `Enter` / `C-m` does not submit. The nudge path SHALL:

1. Send the text, then apply the per-CLI settle delay (`[clis.<name>].submit_delay_ms`, the same resolver the boot prompt uses, falling back to the agnostic default), then send a separate `Enter`.
2. Verify submission by capturing the pane after the `Enter`, allowing the pane to redraw first (the same settle delay), and judging by the **input box's content, never by exact equality with the sent text**: the nudge counts as still present when the input box is non-empty and shows the nudge's leading text (a wrapped rendering shows only its first visual row on the sigil line; continuation rows carry no sigil) or a paste-buffer placeholder matching the profile's `paste_buffer_pattern`; it counts as submitted only when the input box is empty or the pane has left its idle state (a mid-response marker is showing).
3. When the nudge is still present, perform a robust recovery — clear the input line (`C-u`), re-send the text, wait the settle delay, then `Enter` — up to a bounded number of attempts.
4. Never assume a single combined text+Enter, or a lone follow-up `Enter` against a stale line, submits the nudge.
5. Report a nudge still present after every recovery attempt as wedged: escalate it to the broker with verdict `nudge-wedged` AND include that escalation in the exit summary's escalation list, deduped per pane so one stuck line is reported once.

#### Scenario: Nudge applies the per-CLI settle delay between text and Enter

- **GIVEN** the drive loop nudges a pane with submittable text
- **WHEN** the keystrokes are dispatched
- **THEN** the loop SHALL send the text, wait the per-CLI settle delay, then send a separate `Enter`
- **AND** SHALL NOT rely on a single combined text+Enter to submit the nudge

#### Scenario: A stale input line is recovered with clear + re-type + Enter

- **GIVEN** a nudge whose text remains on the pane's input line after the follow-up `Enter`
- **WHEN** the loop verifies submission via `capture-pane` and finds the text still present
- **THEN** it SHALL clear the input line (`C-u`), re-send the text, and send `Enter`, up to a bounded number of attempts
- **AND** a bare `Enter` / `C-m` alone SHALL NOT be treated as sufficient recovery

#### Scenario: A wrapped nudge still on the input line is recognised as unsubmitted

- **GIVEN** a nudge longer than one input-box row, and a post-`Enter` capture whose input box shows the nudge's first row on the sigil line followed by continuation rows without a sigil
- **WHEN** the loop verifies submission
- **THEN** it SHALL judge the nudge still present and run the clear + re-type + `Enter` recovery
- **AND** SHALL NOT report the nudge delivered merely because the sigil row differs from the full text

#### Scenario: A paste-buffer placeholder is recognised as unsubmitted

- **GIVEN** a post-`Enter` capture whose input box shows a paste-buffer placeholder (matching `paste_buffer_pattern`) instead of the nudge text
- **WHEN** the loop verifies submission
- **THEN** it SHALL judge the nudge still present and run the recovery

#### Scenario: An empty input box means the nudge was submitted

- **GIVEN** a post-`Enter` capture whose input box is empty, or whose pane now shows a mid-response marker
- **WHEN** the loop verifies submission
- **THEN** it SHALL report the nudge delivered and send no further keystrokes

#### Scenario: Verification waits for the pane to redraw

- **GIVEN** a nudge dispatched with a non-zero per-CLI settle delay
- **WHEN** the loop verifies submission
- **THEN** the verification capture SHALL be taken after waiting the settle delay following the `Enter`, not immediately

#### Scenario: A wedged nudge appears in the exit summary

- **GIVEN** a nudge still present on the input line after every recovery attempt
- **WHEN** the loop exits and prints its summary
- **THEN** the escalation list SHALL contain a `nudge-wedged` entry naming that pane's agent
- **AND** the same wedged line SHALL NOT produce more than one entry across successive sweeps

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

The drive loop SHALL tolerate multiple feedback→fix→re-verify cycles per agent. An agent that has received feedback and is iterating SHALL NOT be treated as stuck. The loop SHALL NOT treat "not yet verified after N cycles" as a stuck or exit signal. Only the explicit stuck/bloat signal, a live permission prompt (which is escalated), a completion signal, or the heartbeat are exit/re-engage conditions.

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

When the drive loop has observed no progress for approximately 25 minutes without reaching a completion condition, it SHALL re-engage the human by exiting with a status summary (outcome `heartbeat`) rather than running forever silently. The interval SHALL be measured from the most recent progress the loop observed — or from the loop's start, if it has observed none — and NOT from the loop's start alone, so a wave that keeps progressing is never ended by the heartbeat. Progress SHALL be either of: a coding agent's branch tip moving between two ticks (tips compared as full commit SHAs), or a coding agent's phase advancing (its resolved `WorkerPhase` rising). Nothing the drive loop does itself SHALL count as progress: in particular, an escalation it records, its re-escalation of a permission prompt that is still unanswered, and a learning it records SHALL NOT count. A change in pane output alone, or a broker message that does not advance the agent's phase, SHALL NOT count as progress. The phase that advances SHALL be the `WorkerPhase` resolved from the agent's reported `status`; the broker's free-form `phase` label — including the synthetic phases the stuck-detection helper publishes (`stuck-on-prompt`, `stuck-stream-timeout`, `context-bloat`, `no-progress`, `blocked-on-supervisor`) — SHALL NOT count as a phase advance. A branch tip moved by the drive loop's own rebase of that branch (its branch refresh after a merge) SHALL NOT count as progress: the loop SHALL take that branch's next tip read as a fresh baseline. A tip read that fails SHALL be no observation — never progress, and never a reason to abort the loop. A wave whose agents all sit on unanswered permission prompts therefore exits with `heartbeat` one interval after its last progress. The decision whether the heartbeat is due SHALL be a pure function of the last-progress instant, the current instant and the interval. When the heartbeat is due, the loop SHALL take one fresh status observation before exiting. A coding agent's phase advance in that observation SHALL count as progress. A completion condition in it SHALL defer the exit to the loop's next tick, so a terminal status published while a long tick was running is not lost — but at most once until the loop observes progress again, so a completion that never reaches a tick's own status fetch cannot hold the loop open. The behaviour SHALL NOT depend on whether the process has a controlling terminal.

#### Scenario: Heartbeat fires after prolonged run without progress

- **GIVEN** an unattended drive loop whose agents have made no progress for approximately 25 minutes and have not completed
- **WHEN** the heartbeat interval elapses
- **THEN** the loop SHALL exit and surface a status summary with outcome `heartbeat` to re-engage the human
- **AND** SHALL NOT continue running indefinitely without surfacing status

Test: `supervisor::drive::tests::loop_exits_on_heartbeat_when_never_completing` (fake clock); `supervisor::drive::tests::an_unmoving_tip_lets_the_heartbeat_fire_from_the_start` (fake clock); `supervisor::drive::tests::heartbeat_due_is_a_pure_interval_check` (pure); `tests/unattended_drive_e2e.rs::unattended_heartbeat_exits_with_summary`.

#### Scenario: A progressing wave is not ended by the heartbeat

- **GIVEN** an unattended drive loop that has run longer than the heartbeat interval
- **AND** a coding agent's branch tip moved less than one interval ago
- **WHEN** the loop evaluates the heartbeat
- **THEN** the loop SHALL keep driving the wave
- **AND** SHALL exit with `heartbeat` only once a full interval passes with no further progress

Test: `supervisor::drive::tests::branch_tip_movement_defers_the_heartbeat` (fake clock); `supervisor::drive::tests::a_phase_advance_defers_the_heartbeat` (fake clock).

#### Scenario: Only a tip move or a phase advance resets the interval

- **GIVEN** the heartbeat decision with a last-progress instant and the current instant
- **WHEN** the tick observes a branch tip move or a phase advance
- **THEN** the last-progress instant SHALL become the current instant
- **AND** a tick that observes only changed pane output, a repeated `working` status, the loop's re-escalation of an unanswered prompt, or a learning the loop recorded SHALL leave it unchanged

Test: `supervisor::drive::tests::progress_kinds_reset_the_heartbeat` (table-driven, pure).

#### Scenario: Re-escalating an unanswered prompt does not defer the heartbeat

- **GIVEN** an unattended drive loop whose only coding agent sits on an unanswered live permission prompt
- **AND** the loop re-escalates that prompt each dedup window and records a learning for it
- **WHEN** a full heartbeat interval passes with no branch tip move and no phase advance
- **THEN** the loop SHALL exit with outcome `heartbeat`

Test: `supervisor::drive::tests::re_escalations_do_not_defer_the_heartbeat` (fake clock).

#### Scenario: Stuck-detection phases are not a phase advance

- **GIVEN** a coding agent whose broker rows carry `status` `working` with a synthetic stuck-detection `phase` such as `stuck-on-prompt`, `context-bloat` or `blocked-on-supervisor`
- **WHEN** the loop resolves the agent's phase
- **THEN** the resolved `WorkerPhase` SHALL NOT rise
- **AND** the tick SHALL NOT count as progress

Test: `supervisor::drive::tests::stuck_detection_statuses_are_not_a_phase_advance`.

#### Scenario: A tip moved by the loop's own branch refresh is not progress

- **GIVEN** an unattended drive loop that rebases a coding branch onto the default branch after another branch merges
- **AND** no agent commits or advances its phase afterwards
- **WHEN** a full heartbeat interval passes after the last real progress
- **THEN** the loop SHALL exit with outcome `heartbeat`
- **AND** the rebased tip SHALL have been taken as a fresh baseline, not counted as progress

Test: `supervisor::drive::tests::a_tip_moved_by_the_loops_own_branch_refresh_is_not_progress` (fake clock).

#### Scenario: A failing tip read is no observation

- **GIVEN** an unattended drive loop whose tip query fails for every coding branch
- **WHEN** a full heartbeat interval passes with no phase advance
- **THEN** the loop SHALL keep running until then without aborting
- **AND** SHALL exit with outcome `heartbeat`

Test: `supervisor::drive::tests::a_failing_tip_read_is_no_observation` (fake clock).

#### Scenario: A completion published during a long tick is not lost to the heartbeat

- **GIVEN** an unattended drive loop whose heartbeat falls due at the end of a tick
- **AND** the tick's status was fetched before a coding agent published `done` with its branch merged
- **WHEN** the loop evaluates the heartbeat
- **THEN** it SHALL take one fresh status observation
- **AND** SHALL end with outcome `completed`, not `heartbeat`

Test: `supervisor::drive::tests::a_heartbeat_takes_a_fresh_look_before_exiting` (fake clock); `tests/unattended_drive_e2e.rs::detached_unattended_run_completes_past_the_heartbeat`.

#### Scenario: The fresh look defers the exit at most once without progress

- **GIVEN** an unattended drive loop whose heartbeat is due
- **AND** every fresh look shows a completion condition that no tick's own status fetch shows
- **WHEN** the heartbeat falls due on the next tick with no progress in between
- **THEN** the loop SHALL exit with outcome `heartbeat`
- **AND** SHALL NOT defer the exit a second time

Test: `supervisor::drive::tests::a_fresh_look_defers_the_heartbeat_at_most_once_without_progress` (fake clock).

#### Scenario: A detached run reaches completion

- **GIVEN** `git paw start --unattended` launched with no controlling terminal and stdin not a TTY, with the heartbeat interval shortened for the test
- **AND** coding agents that keep committing for longer than that interval, then publish a terminal status with their branches merged
- **WHEN** the run proceeds
- **THEN** the loop SHALL NOT exit with `heartbeat`
- **AND** the run SHALL end with outcome `completed`

Test: `tests/unattended_drive_e2e.rs::detached_unattended_run_completes_past_the_heartbeat`.

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

### Requirement: Idle pane with buffered input is submitted

The drive loop SHALL detect a pane that is idle — showing no live-prompt marker and
no mid-response marker — while its input box holds non-empty unsubmitted text, and
SHALL submit that text: first with a follow-up `Enter` (the keystroke that flushes a
swallowed-`Enter` paste), then — when a fresh capture shows the text still present — with
the same bounded clear (`C-u`) + re-type + `Enter` recovery the nudge path uses, re-typing
the text read from the whole input box (the sigil row and its continuation rows). The loop
SHALL NOT clear text it cannot re-type verbatim: for a paste-buffer placeholder it SHALL send
the `Enter` and, if the placeholder remains, escalate rather than send `C-u`. A line still
present after every attempt SHALL be escalated as `nudge-wedged`, deduped per pane, and
SHALL appear in the exit summary. A lone `Enter` SHALL NOT be the only remedy. This recovers
a directive left buffered when the CLI swallowed the first `Enter`, which neither the
live-prompt scan nor the long stuck-detector would otherwise surface.

#### Scenario: A buffered directive on an idle pane is submitted

- **GIVEN** a pane with no live-prompt marker and no mid-response marker whose input box holds non-empty text
- **WHEN** the drive loop sweeps that pane
- **THEN** it SHALL send a follow-up `Enter` to submit the buffered text
- **AND** if a fresh capture then shows the input box empty, it SHALL send nothing further

#### Scenario: A stale line that survives the Enter is cleared, re-typed, and submitted

- **GIVEN** an idle pane whose input box still shows the same text after the follow-up `Enter`
- **WHEN** the drive loop verifies the pane
- **THEN** it SHALL send `C-u`, re-type the text read from the whole input box, and send `Enter`, up to a bounded number of attempts
- **AND** the re-typed text SHALL include every row of the input box, not only the sigil row

#### Scenario: A paste-buffer placeholder is never cleared

- **GIVEN** an idle pane whose input box shows a paste-buffer placeholder matching `paste_buffer_pattern`
- **WHEN** the drive loop sweeps that pane and the placeholder survives the follow-up `Enter`
- **THEN** it SHALL NOT send `C-u`
- **AND** it SHALL escalate the pane as `nudge-wedged`

#### Scenario: A mid-response pane is left alone

- **GIVEN** a pane showing a mid-response marker (the agent is actively generating)
- **WHEN** the drive loop sweeps it
- **THEN** it SHALL NOT send an `Enter` — the buffered-input detector applies only to an idle pane

#### Scenario: A persistently stranded line is escalated once

- **GIVEN** an idle pane whose stranded text survives the `Enter` and every recovery attempt
- **WHEN** the loop sweeps that pane on this and later ticks
- **THEN** it SHALL record one `nudge-wedged` escalation for that pane
- **AND** the exit summary SHALL list it
- **AND** later sweeps SHALL NOT add duplicate entries for the same line

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

### Requirement: The drive loop is bound to a specific session instance

The drive loop SHALL bind to the specific session instance it began driving — identified by a stable instance token (e.g. the session's start receipt, PID, or start-timestamp), not merely the session name — and SHALL stop acting once that instance is purged, stopped, or replaced by a newer session of the same name. A drive loop whose bound session instance no longer exists SHALL exit rather than continue sweeping the session name, so it never sends approvals or nudges to a subsequently-created same-named session's panes.

#### Scenario: A purged session's drive loop does not drive the next same-named session

- **GIVEN** a running unattended drive loop bound to session `paw-x`
- **WHEN** `paw-x` is purged (or stopped) and a new `paw-x` session is later started
- **THEN** the original drive loop SHALL NOT send approvals or nudges to the new session's panes
- **AND** the original drive loop SHALL exit once its bound session instance is gone

#### Scenario: The loop keeps driving its own live session

- **GIVEN** a running drive loop bound to session `paw-x` that is still alive
- **WHEN** the loop ticks
- **THEN** it SHALL continue driving `paw-x` normally

### Requirement: The drive loop stops nudging panes during wind-down

The drive loop SHALL stop sending pane nudges once it has entered its exit / wind-down state (wave complete, stopped, or bound session gone) — it SHALL NOT continue typing nudge text into idle agent panes during or after wind-down. No nudge SHALL be dispatched on a sweep after the loop has decided to exit, so agents are not left with unsubmitted nudge text re-accumulating in their input lines after the loop is gone.

#### Scenario: No nudge is sent after wind-down begins

- **GIVEN** a drive loop that has entered its exit / wind-down state
- **WHEN** a subsequent sweep would otherwise nudge an idle pane
- **THEN** no nudge keystrokes SHALL be dispatched to any pane

#### Scenario: Wind-down does not re-accumulate unsubmitted text

- **GIVEN** idle agent panes at the moment the drive loop winds down
- **WHEN** the loop completes its exit
- **THEN** it SHALL NOT have appended further nudge text to any idle pane's input line during wind-down

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

