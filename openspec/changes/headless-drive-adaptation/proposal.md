## Why

`headless-backend-adapter` gives git-paw a worker with no screen: it receives messages, runs one
turn per message, and reports busy / awaiting input / failed / interrupted. The coordination
machinery still assumes panes. The drive loop:

- reads screens for permission prompts (which it escalates for a human since v0.17.0), input
  boxes and redraws;
- verifies that a nudge was submitted by re-capturing the input box (GP-19).

A further assumption does not hold without a screen: a worker that ended its turn waiting for an
answer is only reached when something types into it.

On a headless surface none of these hold: there are no prompts, sending text *is* starting a
turn, and an idle worker stays idle until it is sent a message. This change adapts the loop and
the broker host to that model. It keeps the observe → decide → apply shape from v0.18.0
`drive-loop-functional-core`, and it changes nothing for tmux surfaces.

## What Changes

- **No prompt scanning on a non-interactive surface.** The prompt decision returns no effects for
  a surface whose backend reports it non-interactive: it does not scan the output stream for a
  permission prompt, escalates none, and sends nothing. A non-interactive CLI shows no prompt to
  answer; permissions for headless workers come only from the CLI's own non-interactive policy,
  which `headless-backend-selection` gates at launch. tmux surfaces keep escalating every live
  prompt exactly as v0.17.0 specifies.
- **Nudges are messages.** Correction re-engagement and other drive-loop text use the existing
  submit-text effect. On a non-interactive surface the delivery counts as done when the port
  accepts it, with no input-box re-capture, no `C-u` / re-type recovery, and no buffered-input
  `Enter`. The GP-19 bug class does not exist there.
- **Turn outcomes are observed, never acted on destructively.**
  - A *failed* turn is escalated as `cli-unavailable` when the output tail carries the CLI's
    unavailable marker (the existing verdict and dedup), and as `turn-failed` otherwise.
  - An *interrupted* turn is resumed once with a bundled continuation message. A second
    interruption is escalated as `turn-interrupted`.
  - A *busy* surface whose output has not grown for `[drive] no_progress_window_seconds` is
    escalated as `no-progress`.

  No outcome kills a worker or re-runs its message.
- **Progress is unchanged.** The heartbeat counts only a branch tip moving or a phase advancing
  (v0.24.0), for headless and tmux waves alike. A turn starting or ending is not progress, so a
  wake or continuation git-paw itself delivers cannot keep a wave alive.
- **Inbox wake.** In a headless session the broker host starts a turn for a worker that is
  awaiting input when a message addressed to it waits in its broker inbox: `agent.feedback`,
  `agent.answer`, or a directed `agent.blocked`. Broadcasts never wake a worker.
  - It wakes at most once per message.
  - It never wakes after a budget-exhausted escalation, or while the session is paused.
  - It runs in attended and unattended sessions alike, because no other process reaches an idle
    headless worker.
- **Headless boot guidance.** A headless worker's boot block gains a bundled, project-agnostic
  addendum: nobody can answer an interactive prompt; an action its policy does not allow is
  refused; when it needs input it reports that as its instructions describe and ends its turn,
  and it will be resumed with a message. The continuation and wake texts are bundled assets too.
- All new decisions are pure functions. That covers the non-interactive prompt rule, turn-outcome
  handling, no-progress and wake. They are table-tested against data, and the shell
  applies their effects through the terminal port and the broker client.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `supervisor-unattended-operation`: added — non-interactive surfaces are neither scanned for
  permission prompts nor escalated; nudges are delivered as messages; turn outcomes and
  no-progress escalate.
- `broker-runtime`: added — the broker host wakes an idle headless worker for a directed inbox
  message, on its own cadence.
- `boot-block`: added — headless workers receive the non-interactive addendum.
- `core-lang-agnostic`: added — the addendum, continuation and wake texts are bundled agnostic
  assets covered by the no-language-leak audit.
- `code-architecture`: added — the headless drive and wake decisions are pure functions.

## Impact

- **Builds on (must rebase over):**
  - `headless-backend-adapter` (v1.3.0): `activity`, interactivity, the fake's non-interactive
    mode.
  - v0.18.0:
    - `drive-loop-functional-core`: `decide_prompt` (escalate-only since v0.17.0),
      `decide_nudge`, the `DriveEffect` enum, the effect log. This change adds observation fields
      and decision rows, and no new effect variant.
    - `broker-host-separation`: the `__broker` host, which gains the wake loop as a thread with
      its own tick (there is no auto-approve thread to share since v0.17.0).
    - `terminal-backend-seam`.
    - `orchestrator-escalation-handoff`: escalations from this change flow into its hand-off
      like any other.
  - v0.17.0:
    - `classifier-retirement`: git-paw approves nothing; every live prompt on an interactive
      surface is escalated (*Every live permission prompt is escalated without blocking the
      wave*). The attended auto-approver and its `stall_threshold_seconds` cadence are gone, so
      this change adds no approver rule and gives the wake loop its own cadence.
    - `idle-pane-nudge-submission`: its input-box verification and recovery stay tmux-only.
    - `cli-unavailable-detection-gaps`: the marker scan and `(agent, marker)` dedup reused for
      failed turns.
  - v0.19.0 `worker-usage-budget`: the budget-exhausted escalation suppresses wakes.
  - v0.21.0 `supervisor-vocabulary-split`: `[drive] no_progress_window_seconds`.
  - v0.18.0 `unattended-heartbeat-progress`: the progress definition (a branch tip moving or a
    phase advancing; nothing the loop does itself), which this change leaves unchanged.
  - v0.20.0 `skill-package-layout`: if bundled prose moved into packages, the three new assets
    follow it.
- **Code:** `src/supervisor/drive.rs` and the decision module from `drive-loop-functional-core`
  (observation gains `activity` and interactivity; new decision rows); the `__broker` host
  (`src/commands/broker_host.rs`) gains a wake-loop thread with its own tick; `src/skills.rs` /
  boot-block rendering (addendum); new `assets/headless-boot-addendum.md`,
  `assets/headless-continue.md`, `assets/headless-wake.md`.
- **Enum-variant ripple:** `BrokerMessage` is not changed, but the wake decision adds one new
  compile-forcing exhaustive match over it (which messages wake a worker). Every future variant
  must decide whether it wakes. The site is to be added to AGENTS.md §7's `BrokerMessage` list.
  `SpecBackendKind` is untouched.
- **Escalation verdicts:** three new labels (`turn-failed`, `turn-interrupted`, `no-progress`) on
  the existing `Question`-based escalation path; no wire change.
- **Security / safety:** git-paw approves nothing for any worker since v0.17.0. For a headless
  worker it also escalates no prompt, because a non-interactive CLI shows none: the CLI's declared
  non-interactive policy is the only gate on its actions. That is why the launch gate in
  `headless-backend-selection` exists. Wakes and continuations only deliver bundled text; they
  never approve or widen anything. Budget breach and pause stop all automatic turns.
- **Tests:** decision tables (fixtures, no fakes); fake-backend drive-loop tests; a broker-host
  wake integration test with the headless adapter and a stub CLI; asset audits.
- **Docs:** `docs/src/user-guide/supervisor.md` (coordinated mode) and the unattended chapter
  gain a headless section; the full headless chapter ships in `headless-backend-selection`.
