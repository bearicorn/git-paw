## Context

After v0.18.0 the drive loop is observe → decide → apply. The shell lists surfaces and owners,
captures tails, fetches `/status`, reads new log entries and the clock. It then calls the pure
decisions — `decide_prompt`, `decide_nudge` and `summarize` — and applies the returned
`DriveEffect`s through the ports, logging each one. Today every observation is screen-derived:

- the live-prompt gate and `has_approval_marker` find a live permission prompt, which
  `decide_prompt` escalates (`permission-prompt`, with a manual-decision record and a learning);
  since v0.17.0 `classifier-retirement` nothing answers it;
- `input_box_text`, `pane_has_buffered_input` and `pane_is_mid_response` handle buffered input
  and the GP-19 nudge verification and recovery (`send_nudge`, `submit_buffered_input`);
- `cli_unavailable_line` handles CLI death.

Completion is already broker-derived (`detect_completion` over `/status`, merge truth from git).
Escalations are published as `BrokerMessage::Question` with a verdict label, which is an open
string set (`permission-prompt`, `unmerged`, `cli-unavailable`, `correction-exhausted`,
`correction-slow`, `nudge-wedged`).

Stuck detection for tmux lives in `sweep.sh` (`detect-stuck`, reading `no_progress_window_seconds`).
The Rust loop never emits `DriveOutcome::Stuck`. v0.18.0 `unattended-heartbeat-progress` makes the
heartbeat measure time since the last progress: a branch tip moved or a phase advanced. Nothing the
loop does itself (an escalation, a re-escalation, a learning) counts.

The broker routes four kinds of message to one worker's inbox (`delivery.rs::route_message`):

- `Feedback` and `Answer` to the envelope's `agent_id`;
- `Blocked` to `payload.from`;
- a branch-scoped `Learning` to its branch.

`Artifact`, `Verified`, `Intent` and `AdvancedMain` are broadcasts. A worker drains its inbox
itself (`GET /messages/:id`), but only while it is in a turn. An idle tmux worker is reached
today only by typing into its pane: `/tell` send-keys mode, or the drive loop's hand-offs.

## Goals / Non-Goals

**Goals:**
- Headless workers are driven correctly by the existing loop, with tmux byte-identical.
- No automatic action can kill or replay anything on a headless worker (and, as everywhere since
  v0.17.0, nothing approves).
- An idle headless worker is reachable by the same broker messages that reach a tmux worker.
- Every new judgment is a pure, table-tested decision.

**Non-Goals:**
- Making the backend selectable, or the permission policy (`headless-backend-selection`).
- Moving the drive loop into a detached service (see Open Questions).
- Parsing a CLI's non-interactive output for denied actions or costs (to verify per CLI).
- New `BrokerMessage` variants or a new `DriveEffect` variant.

## Decisions

### D1 — The observation carries interactivity and activity; decisions branch on them

`SurfaceObservation` gains `interactive: bool` and `activity: SurfaceActivity`. tmux observations
are `interactive = true` and `Unknown`, so every existing row is unchanged and the existing
tables keep passing. On a non-interactive surface:

- `decide_prompt` returns no effects and no state change, even if the tail contains text shaped
  like a permission prompt: no `permission-prompt` escalation, no manual-decision record, no
  learning. A non-interactive CLI has no prompt to show, so anything prompt-shaped in its output
  stream is the agent's own text, and escalating it would be a phantom.
- The buffered-input row never fires.

On interactive (tmux) surfaces `decide_prompt` is unchanged: every live prompt is still escalated.

*Why not skip non-interactive surfaces in the shell?* The decision table is the documented
contract. Putting the rule there makes it testable with data and keeps the shell free of
judgment.

### D2 — Nudges keep their effect; only verification differs

Correction re-engagement and any other drive-loop text stay `SubmitText { surface, text }`
(send text, then `Enter`). The shell applies it through the port. On a non-interactive surface the
outcome is `delivered` when both calls succeed, and `failed` with the port's error otherwise.
There is no settle delay, no re-capture, no `C-u` / re-type and no `nudge-wedged` path.

*Alternative rejected:* a new `DeliverMessage` effect. It duplicates `SubmitText`, and the
backend already gives text plus `Enter` the meaning "submit a message".

### D3 — Turn outcomes: escalate, resume once, never destroy

A new pure decision, `decide_turn(&TurnState, &TurnObservation, &TurnPolicy, now)`, runs for
non-interactive surfaces only:

| Observation | Effect |
|---|---|
| `Failed`, and the tail matches the CLI's unavailable markers | escalate `cli-unavailable`, deduped on (agent, marker) as today |
| `Failed` otherwise | escalate `turn-failed` with the exit code, once per failed turn |
| `Interrupted`, first time for this agent | `SubmitText(continuation text)` |
| `Interrupted`, again | escalate `turn-interrupted` |
| `Busy { last_output }` older than `no_progress_window_seconds` | escalate `no-progress`, once per window |
| `AwaitingInput`, or `Busy` with recent output | nothing |

The rationale:

- **A failed turn is not retried.** Retrying an auth or rate-limit failure burns spend and hides
  the cause.
- **An interrupted turn is not replayed.** Its message may already have had side effects. Instead
  the worker is told it was interrupted and asked to check its own state and continue, which is
  the bundled continuation text.
- **Nothing kills.** A hung turn is the operator's or orchestrator's call, and `git paw remove` or
  `stop` always works.

The time of last output is part of the `Busy` activity the headless backend reports (the output
stream's modification time — an observed fact passed in, with `now`). tmux reports `Unknown`, so
the row never fires there; tmux stuck detection stays in `sweep.sh`.

### D4 — Progress is unchanged for headless waves

v0.24.0's progress set (a branch tip moved, a phase advanced) applies to headless waves as it is.
An earlier draft added "a turn started or ended" because tmux counted dispatched approvals as
progress and headless had none; since v0.17.0 no wave dispatches approvals and v0.24.0 counts only
tips and phases, so headless and tmux are on equal footing. Counting turn boundaries would also
let git-paw's own deliveries (a wake, the continuation text) start turns that keep a wave alive,
which v0.24.0 rules out for the loop's own actions.

### D5 — The broker host owns inbox wakes, in every coordinated headless session

The host already runs for every session with a broker, and it sees every routed message. It no
longer hosts an approver (v0.17.0 deleted the attended auto-approve thread, whose cadence came from
the now no-effect `[supervisor.auto_approve] stall_threshold_seconds`), so the wake loop is a
thread of its own with a fixed tick of at most 15 seconds, read from no deprecated key. Each tick it
calls the pure `decide_wake(&WakeState, &WakeObservation, now)`, where the observation is:

- per worker, its activity;
- the highest directed-message sequence waiting in its inbox, as counted by the delivery layer;
- whether a `budget-exhausted` escalation has been logged.

It returns `SubmitText(wake text)` for each worker that is `AwaitingInput` and has a directed
message newer than the last one it was woken for. The decision records that sequence, so wakes
happen at most once per message, even when the worker ignores its inbox. No wakes are issued
after a budget breach.

"Directed" is decided by an exhaustive `match` over `BrokerMessage`:

- **wakes:** `Feedback`, `Answer`, `Blocked`;
- **does not wake:** `Status`, `Artifact`, `Verified`, `Intent`, `AdvancedMain`, `Learning`
  (informational), `Question` and `VerifyNow` (orchestrator-bound).

A new variant must choose, which is the ripple noted in the proposal.

*Alternatives rejected:*
- **Waking from the drive loop.** It runs only under `--unattended`, so an attended headless
  worker would never hear an answer.
- **Waking from the turn runner at turn end.** The runner would need the broker URL and HTTP, and
  it cannot see messages that arrive while no turn runs.
- **A wake on every broadcast.** Spend for no action.

Duplicate texts from a wake and a nudge in the same tick are collapsed by the adapter's
identical-pending rule.

### D6 — Agent-facing texts are bundled assets

The three texts are bundled assets, loaded with `include_str!`:

- `assets/headless-boot-addendum.md` — appended to the boot block of a headless worker only;
- `assets/headless-continue.md` — the interrupted-turn continuation;
- `assets/headless-wake.md` — "messages are waiting in your inbox; read them and act".

They name no CLI, vendor, stack or commit convention, and refer to "your coordination
instructions" rather than to `broker.sh` or MCP. The same text is therefore correct for every
layer combination of v1.0.0 `layer-isolation`, and the no-language-leak audit covers them.

## Risks / Trade-offs

- **[No prompt reaches a human for a headless worker]** → By design: a non-interactive CLI
  refuses what its policy does not allow instead of prompting, and git-paw approves nothing for any
  worker since v0.17.0. The launch gate in `headless-backend-selection` admits only workers whose
  CLI enforces a declared non-interactive policy, and only sandboxed workers at `full-auto`.
- **[A worker ignores its inbox after a wake]** → At most one wake per message. The no-progress
  and heartbeat rules surface the stall.
- **[The continuation text causes a repeated side effect]** → It asks the worker to inspect its
  state first. That is one bounded attempt, then an escalation.
- **[Marker drift]** → A CLI's non-interactive error text may differ from its TUI banner, and the
  `cli-unavailable` row would then fall back to `turn-failed`. That is still loud. Per-CLI fixtures
  are to be verified by the v0.23.0 audit's `headless-worker` column.
- **[Wake cadence adds latency]** → One wake-loop tick (at most 15 s). This is acceptable for
  coordination messages.

## Migration Plan

1. Observation fields + decision rows for D1/D2 with tmux rows unchanged; suite green.
2. `decide_turn` + `TurnObservation` (activity, output position); fake-backend tests.
3. Wake loop in the broker host; delivery-layer directed counts; integration test.
4. Assets + boot-block addendum + audits; docs.

Rollback: revert. tmux sessions are unaffected at every step.

## Open Questions

- **Detached drive loop.** The drive loop still runs in the foreground `start --unattended`
  process, so closing that terminal ends the loop while headless workers keep going. A detached
  drive-loop service is the obvious next step: run it with the adapter's `spawn_service`, and
  write its summary to the session record. *Recommendation:* a follow-up change after v1.3.0
  dogfood, once v0.18.0's heartbeat fix is proven. It needs a decision on where the exit summary
  and exit code go.
- **Denied actions.** A non-interactive CLI reports a refused action in its output. Surfacing
  those refusals as escalations needs a per-CLI reader of the output format. That is to be
  verified per CLI and not specced here.
