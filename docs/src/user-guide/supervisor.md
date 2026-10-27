# Supervisor

This chapter mirrors the user-facing prose for the bundled orchestrator
skill (`assets/agent-skills/orchestrator.md`, exported to
`.git-paw/skills/orchestrator/SKILL.md`; `supervisor` remains an alias of its
name) — the doctrine the orchestrator runs against in supervisor mode. Sections
here document the same rules the skill teaches, so users reading the mdBook can
understand the orchestrator's behaviour without opening the skill file
directly.

For the launcher-level "how do I start supervisor mode" walkthrough, see
[Quick Start: Orchestrator Mode](../quick-start-orchestrator.md). For the
broker-side message contract the supervisor exchanges with coding agents, see
[Agent Coordination](coordination.md).

## Which CLI the coding agents launch with

A supervised launch given neither `--branches` nor a spec selection runs the
same interactive selection flow as `git paw start`: the branch picker, the
assignment-mode prompt, and the CLI picker (uniform, or one picker per branch).
Each coding agent then launches with **the CLI that selection mapped to its
branch** — the uniform choice, or that branch's own per-branch choice.
`default_cli` only pre-selects an entry in those pickers; it never skips them.

The session-wide fallback — `--cli`, else `default_cli`, else
`[supervisor].cli` — applies only to launch paths that show no CLI picker,
such as `--branches` without `--cli`; when none of the three is set the
launch fails, naming `--cli` and `default_cli`. Spec-driven branches keep
their own resolution chain (see [Spec-driven launch](spec-driven-launch.md)).

## Session layout: the orchestrator runs outside the session

A supervised session holds the **dashboard at pane 0**, as a full-width top
row, and **one coding agent per pane from pane 1**, row-major below it (at
most five per row). No pane runs an orchestrator CLI. The orchestrator is your
own agent session, run outside tmux at the repository root. The launch
re-renders the exported orchestrator skill
(`.git-paw/skills/orchestrator/SKILL.md`, with the gate commands and the
spec-path doctrine) and writes the session context — the session name, the
broker URL, the helper paths, any `[governance]` documents and, under
`--unattended`, the drive-loop directive — into the gitignored sidecar
`.git-paw/AGENTS.local.md`, and prints both paths with the attach hint. The
orchestrator reads them and drives the run through the bundled helpers
(`.git-paw/scripts/sweep.sh`, `broker.sh`) or the MCP server. See
[Driving the session from your own agent](#driving-the-session-from-your-own-agent). Its
roster row (`supervisor`) appears on the dashboard once it publishes its own
`agent.status`; the launcher never publishes on its behalf.

`[supervisor].cli` (else `default_cli`) names the orchestrator's CLI in that
context and is the coding agents' last CLI fallback; no orchestrator CLI is
needed to launch. `[supervisor].approval` is still parsed but shapes no pane
command — it is reserved for git-paw-initiated orchestrator invocations.

A live session started by an earlier release (orchestrator CLI at pane 0,
dashboard at pane 1, agents from pane 2) keeps working until it ends: every
command derives the agent panes from the session's recorded dashboard index,
and nothing types into that session's first two panes. Recovering it rebuilds
the layout above.

## Driving the session from your own agent

The orchestrator is whatever agent session you already use — any agent CLI,
at the repository root, outside tmux. git-paw gives it three things:

| File | Written by | What it holds |
|---|---|---|
| `.git-paw/skills/orchestrator/SKILL.md` | `git paw init`, refreshed by `git paw start` | The orchestrator skill (the procedure: five gates, spec audit, merge orchestration, stuck detection, routing), rendered with your `[supervisor]` gate commands and configured spec format. It carries no session value, so one file serves every session. |
| `.git-paw/AGENTS.local.md` | `git paw start` | The **session context**: session name, broker URL, helper paths, the discovery file, `[governance]` documents, the drive-loop directive under `--unattended`, and a pointer to `SKILL.md`. The only file carrying this session's values. |
| `AGENTS.md` pointer section | `git paw init` | A short section between `<!-- git-paw-orchestrator:start -->` and `<!-- git-paw-orchestrator:end -->` telling any agent that reads your root `AGENTS.md` that, when a git-paw session is active, it is the orchestrator and should read the two files above. `git paw stop` and `git paw purge` leave it in place. See [AGENTS.md Injection](agents-md.md#orchestrator-pointer-section). |

Both `.git-paw/skills/` exports (the orchestrator skill and the `cli-setup`
skill beside it) are in the standard skill-directory layout, so a skill-aware
CLI can load them directly; copy or symlink them into a vendor skill directory
if your CLI needs that. Over MCP, `get_skill` serves the same skill under the
name `orchestrator` (or its alias `supervisor`).

**The surfaces it uses.**

- **Follow the session** by blocking on the broker's event cursor — the MCP
  `wait_for_events` tool or `.git-paw/scripts/sweep.sh events` — carrying the
  returned cursor forward, and inspect when an event or a `stalled` wave says
  to. `get_session_status`, `sweep.sh status` and `sweep.sh inbox` read the
  roster and the orchestrator's inbox. A sampled pane capture can miss a
  prompt that was answered and replaced between two samples, so capture
  (`sweep.sh capture <pane>`) is for inspection, not detection.
- **Publish verdicts** only through the credential-carrying helpers:
  `sweep.sh verified`, `sweep.sh feedback-gate` and `sweep.sh conflict-ack`
  (in the root session, the MCP `publish_verified` and `publish_feedback` tools
  do the same). The broker refuses a verdict without the session's authority
  credential.
- **Answer and announce** with `sweep.sh answer <agent> <text> [re]` and, after
  each merge, `sweep.sh advanced-main <merged-branch> <summary>`. Its own
  questions and main-side intents go through
  `broker.sh --agent supervisor question|intent`. The skill contains no raw
  HTTP request.
- **Run the session** from the root session with `git paw` commands or the MCP
  `start_session`, `pause_session`, `stop_session` and `purge_session` tools.

**Pairing with `--unattended`.** `git paw start --unattended` adds the
in-process drive loop: it keeps the panes moving and escalates every live
permission prompt to the broker, approving none. The orchestrator reads those
escalations first each cycle, relays each prompt to you in its own session, and
runs `sweep.sh approve <pane>` only on your explicit instruction. It merges as
soon as every expected agent is verified and escalates only on a conflict, an
exhausted correction cycle or a regression (see
[Merge orchestration](#merge-orchestration)). If the loop exits on its
heartbeat, re-run `git paw start --unattended` to re-attach a drive loop to the
live session. The driving session can itself be remote-controlled from a
phone — see [Remote Control](remote-control.md).

## Unattended mode (`--unattended`)

`git paw start --unattended` runs a supervisor wave to completion with **no
human in the seat**. Without it, `git paw start --supervisor` builds the tmux
session and returns immediately with an attach hint, expecting a human (or a
throwaway pane-scraper script) to watch the panes, answer permission prompts,
and notice when the work is done. `--unattended` folds the *driving* half of
that operator loop into the tool itself.

`--unattended` engages supervisor mode (you do not also need `--supervisor`),
then — instead of returning — runs an **in-process drive loop** in the
foreground `git paw start` process. It is designed for detached operation: it
does not attach a terminal and does not replace your shell with an interactive
supervisor CLI. It is mutually exclusive with `--no-supervisor`.

> **`--unattended` approves nothing.** The loop detects live permission prompts
> and escalates every one of them for a human decision; it never answers one.
> For a run that genuinely does not stop for prompts, the decision has to be
> made *before* the prompt exists — in your agent CLI's own allow rules, or by
> launching the agents at `[supervisor] agent_approval = "full-auto"` inside the
> documented [sandbox](./sandbox.md). See [Agent permissions](#agent-permissions).

Each poll (~15 seconds) the loop:

- **Sweeps every coding-agent pane** — and only those. It enumerates panes and
  resolves each to its agent by `pane_current_path` (never by pane index,
  which drifts), capturing each coding pane with its own `capture-pane` call.
  The dashboard, and any pane no agent worktree owns, is never captured,
  escalated or typed into.
- **Acts only on a live prompt** — the prompt's structural markers (numbered
  option glyphs and/or the `Do you want to …` question, with the
  `Esc to cancel` footer) anchored on the modal block itself, searching
  upward from the end of the capture and tolerating up to 13 non-blank
  trailing lines of non-prompt content below it (a task list, a status strip,
  a progress footer), so a CLI that renders chrome under its input area
  doesn't hide a live modal — and so a prompt offering a "don't ask again"
  option is detected too. Beyond that true end, liveness requires the modal's
  actual shape (the footer anchoring a line, with an option list nearby), not
  a bare mention of the footer text in narration or tool output. Prompt-like
  text scrolled well past that bound is ignored.
- **Escalates every live prompt, whatever the command is.** The loop makes no
  judgement about the prompted command — `ls -la` and `rm -rf /` are handled
  identically. The prompt is recorded on the broker as an `agent.question`
  review item (from `supervisor`), written to the
  [manual-decision log](#manual-approvals), captured as a learning, and listed
  in the exit summary. **No keystroke is sent**, to that pane or any other: the
  orchestrator outside the session reads the review item and relays it to the
  human.
- **Submits a directive left buffered on an idle pane.** A pane showing no
  live prompt and no mid-response marker, whose input box still holds
  unsubmitted text, has a stranded directive: the CLI swallowed the first
  `Enter` into its paste buffer. The loop sends a follow-up `Enter` first; if
  the text survives that, it runs the same clear-and-recover sequence nudges
  use (see below), re-typing the text read from the whole input box. A
  paste-buffer placeholder is never cleared — it is escalated instead. Neither
  the live-prompt scan (there is no prompt) nor the ~25-minute stuck detector
  would otherwise surface that stall. A pane that is actively generating is
  never touched.
- **Keeps the wave moving while a prompt waits.** An escalation is
  non-blocking: one agent parked on a prompt never freezes the others, and the
  loop does not wait for anyone to answer. Repeated escalations are
  deduplicated per `(agent, command-shape)` within a five-minute window, so a
  prompt still unanswered after that window is re-escalated as a reminder
  rather than every fifteen seconds. A prompt nobody ever answers does not, on
  its own, end the run — the wave ends on completion or the heartbeat.
- **Leaves judgment calls on the broker** — see
  [The loop is a pump, the supervisor is the brain](#the-loop-is-a-pump-the-supervisor-is-the-brain).
- **Re-engages a gate-failed worker**, when
  [`[supervisor.correction] auto_loopback`](#unattended-correction-loop) is
  enabled — closing the verify↔fix cycle without a human hop.

The loop keeps polling until one of:

1. **Completion** — a terminal PASS/FAIL verdict is signalled, or every agent's
   tasks are checked complete.
2. **Stuck / bloat** — the stuck-detection signal fires.
3. **Heartbeat** — about 25 minutes pass **without progress**, so you are
   re-engaged with a status summary rather than the loop running forever.
   Progress is a coding agent's branch tip moving (a new commit) or its phase
   advancing (e.g. `working` → `committed` → `verified`/`done`); the interval
   is measured from the last progress the loop saw, not from launch. Nothing
   the loop does itself counts — its escalations, its reminder re-escalations
   of an unanswered prompt, the learnings it records, its own rebases of a
   branch after a merge — and neither does changed pane output or a repeated
   `working` status. So a wave that keeps committing is never cut off at 25
   minutes, while a wave whose agents all sit on unanswered prompts still
   exits one interval after its last progress. Before exiting, the loop takes
   one fresh status look, so a `done` published during a long tick still ends
   the run as `completed`.
4. **Session torn down** — the loop's own session instance is purged, stopped,
   or replaced (see [Session-instance lifecycle](#session-instance-lifecycle)
   below).

On exit it prints a summary: the outcome (`completed` /
`escalated-for-review` / `stuck` / `heartbeat` / `session-torn-down`), each
agent's **resolved** final state and its merged/unmerged status, the deduped
list of prompts escalated for human review, a line stating that the
orchestrator runs outside the session and its CLI is not monitored by the
loop, and pointers to the broker log and captured learnings. Because there
is no human to hand-write findings, the loop self-captures qualitative
learnings during the run and once more at wind-down.

A `heartbeat`, `stuck` or interrupted exit ends the *loop*, not the session —
the panes, their conversations and their worktrees are all still there. Re-run
the same `git paw start --unattended` to drive it again; see
[Re-attaching a drive loop to a live session](#re-attaching-a-drive-loop-to-a-live-session).

Each agent's final state is resolved from the most-progressed lifecycle phase
ever observed for it during the run — not simply the last `/status` row — so a
generic `working` heartbeat published after a `verified`/`done` artifact event
can never make a finished worker look unfinished in the summary.

**The merged/unmerged roll-up is ref-derived, never inferred from a worker's
reported status.** A worker reaching `verified`/`done` says nothing about
whether its branch ever reached the default branch — reporting a wave
`completed` on the strength of worker status alone was the failure mode a
0.16.0 external dogfood surfaced: the summary read `completed`, `2/2 branches
merged`, zero escalations, while `main` was unchanged and both branches were
still ahead. At exit the loop asks git directly, once per coding branch,
whether the branch tip is an ancestor of the default branch
(`git merge-base --is-ancestor`); a comparison git cannot make (a deleted
branch, a bad ref) counts as **unmerged** — the fail-safe direction, since this
summary is the input to a destructive `git paw purge`. The wave-level roll-up
line (e.g. `2/2 branches merged`) and each agent's own line (`<agent>: <phase>
· merged` / `<agent>: <phase> · unmerged (N ahead of <default>, tip <sha>)`)
both come from this same ref check, and print for every outcome with coding
agents — a `heartbeat` exit gets the same truth, not only a `completed` one.
**`completed` now implies every branch is merged.** A wave that finishes with
any branch unmerged reads `escalated-for-review` instead, with one
`[unmerged]` escalation entry per unmerged branch, so "Escalations awaiting
human review" can no longer read `0` while work sits un-integrated — check
this list, not just the outcome word, before running `purge`.

### CLI unavailability

A rate-limited or logged-out coding agent is an explicit non-success signal —
not a silent idle pane the loop waits out for up to 25 minutes. Each poll,
every coding-agent pane's capture tail is also scanned for a
CLI-unavailability marker (a usage/session-limit phrase, an organisation
spend-limit phrase, or a login-loss phrase such as an expired/missing
credential). A match becomes a deduped `cli-unavailable` escalation that
quotes the matching line, and the loop keeps driving the rest of the wave; a
wave carrying one is never reported `completed`, and that worker's branch
shows as unmerged in the exit summary regardless. Text on the dashboard —
whose broker log can quote a worker's banner — is never scanned.

The scan tolerates the same bounded trailing region of non-prompt content the
live-prompt gate does, so a banner is still found underneath chrome a CLI
draws below it — current Claude Code's persistent task list, a status strip,
or similar — rather than only a fixed last few lines. A marker separated from
the end of the capture by more than that bound still counts as scrolled away
and is ignored, same as before.

Markers are resolved **per pane, for its own CLI**: a `[clis.<name>]` override of `cli_unavailable_markers` applies to
that CLI's panes, falling back to the embedded Claude Code default (which
includes `You've hit your session limit`, an organisation `org's monthly
spend limit`, an expired/missing login, and an invalid API key) when the CLI
has no override. The override resolves from that pane's own broker-reported
CLI (the `/status` snapshot), so it only takes effect once that pane has
published at least one status — a pane's first sweep of the run, before any
`/status` row exists for it, always falls back to the embedded default for
that one tick. `git paw __prompt-profile --field cli_unavailable_markers`
reads the exact same resolved field the loop does — it is a read path for the
bundled `sweep.sh` helper's *other* detectors (stuck-shape, live-prompt) to
stay in sync with the Rust side, not a second detector: `sweep.sh` itself
does not consult `cli_unavailable_markers` or evaluate a pane for CLI
unavailability today.

A `cli-unavailable` escalation is deduplicated on the agent and the matched
marker — never on the banner line's full text, which can carry a reset time
or countdown — so the same banner observed across sweeps is one escalation,
while two distinct banners on one agent (e.g. a session limit followed later
by a login failure) are two.

The orchestrator runs outside the session, so the loop has no pane to watch
for the orchestrator's own CLI dying — every exit summary says so explicitly
(`The orchestrator runs outside the session; its CLI is not monitored by the
loop.`) instead of implying coverage it does not have.

Nothing in an unattended session approves a prompt — the loop escalates and
the human answers. The dashboard subprocess runs no approver either, attended
or unattended; it renders the broker log and nothing more.

### Re-attaching a drive loop to a live session

A drive loop ends while the session it was driving is still perfectly alive —
on the heartbeat, on a stuck signal, or because you interrupted it. To drive
that session again, **re-run the same `git paw start --unattended` command** in
its repository:

```bash
# Drove the wave the first time; drives it again the second time
git paw start --unattended --from-all-specs
```

The live session is neither relaunched nor forked. Launch-selection flags
(`--branches`, `--specs`, `--from-all-specs`, `--cli`) are never applied to a
session that already exists — attaching a new agent to a live session is
[`git paw add`](./session-lifecycle.md#git-paw-add--hot-attach-an-agent) — and that is exactly what lets the
original launch command double as the re-attach command. No `paw-<project>-2`
is created.

What it does next depends on the session's effective status:

| Status | What `start --unattended` does |
|---|---|
| **Active** | Binds a loop and drives the session as it stands. No pane is created, rebuilt or re-titled, no worktree is touched, and no boot block, prompt or keystroke is injected anywhere. |
| **Paused** | Revives the session exactly as `git paw start` revives it, then drives it. |
| **Stopped** | Recovers the session exactly as `git paw start` recovers it, then drives it. |

A revive or recovery that fails fails the command, without running a loop.

The terminal is **never attached**, even from an interactive shell — the drive
loop occupies the foreground process, exactly as it does on a fresh
`--unattended` launch. The command prints the session name and the
`git paw attach` line for watching it from another terminal.

In every other respect the re-bound loop is the same loop: the same cadence,
the same escalation, correction and heartbeat behaviour, the same exit summary,
and it **approves nothing**. It drives the session's own agent roster, and
reaches the broker at the endpoint **recorded for that session** rather than
the one in your current configuration — so a `[broker] port` changed after
launch cannot point the loop at another repository's broker, or at none.

**Bare sessions are refused.** A session started without supervisor mode
deliberately leaves its boot prompts unsubmitted for a human to send, and the
loop's buffered-input recovery would submit them. `start --unattended` against
one exits non-zero naming the session, stating that only a session launched in
supervised mode can be driven unattended, and pointing at `git paw attach` for
driving it by hand. Nothing about the session is changed. A `--dry-run`
likewise binds nothing and is unaffected.

### One drive loop per session

Two loops on one session would each nudge, submit buffered input and escalate
into the same panes, so at most one may drive a session instance at a time.
Every loop — whether from a fresh launch or a re-attach — holds a **drive-loop
binding**: a lease file at `.git-paw/tmp/drive-loop.binding` (gitignored,
beside the approval claims) naming the session instance and the loop itself. It
is refreshed every poll and released when the loop exits.

A `start --unattended` that finds a live loop already holding the binding
**refuses before changing anything** — no session, pane or worktree state is
touched, and no second loop runs. It exits non-zero with an error naming the
session, the process id holding the binding, and the binding file. Wait for
that loop to exit (or stop that process), then re-run the command.

A loop killed without releasing its binding does not wedge the next one. A
binding that stops being refreshed lapses after the staleness window — the
larger of four poll intervals and two minutes, and of the window the holder
itself recorded, so a loop polling more slowly than the caller is never
displaced while it is alive — and the next `start --unattended` simply takes
it. There is never a lock file to clear by hand. A loop that exits cleanly
releases its binding immediately, so a re-attach right after a heartbeat exit
binds without waiting at all.

Two `start --unattended` runs that reach a lapsed binding at the same instant
still produce exactly one loop: the binding is read, judged and rewritten
under an exclusive lock (`.git-paw/tmp/drive-loop.lock`), so the second runs
its check against the first's result and refuses. That lock file is left in
place between runs; it is empty, and nothing needs to be done about it.

`git paw stop` revokes the session's binding, and recovering a stopped session
supersedes any binding left behind for it. A loop that finds its binding
revoked or taken over exits `session-torn-down` on its next poll without acting
— see [Session-instance lifecycle](#session-instance-lifecycle) below.

> **Deleting `.git-paw/tmp/` ends a running loop.** The binding goes with it
> and the loop exits on its next poll. That is fail-safe rather than
> destructive: a loop that is not running approves nothing, so the panes go on
> waiting for a human exactly as they would have. Re-run
> `git paw start --unattended` to bind a new loop.

> **Upgrading mid-run.** A drive loop started by a git-paw older than this one
> holds no binding, so a re-attach is **not** refused while it is still
> running — and you would end up with two loops driving the same panes. Stop
> that older `--unattended` process before re-attaching to its session.

### Session-instance lifecycle

A loop binds to the *specific session instance* it began driving — not just
the session name — and re-confirms on every tick, **before** it sweeps or
sends anything, that it is still that instance's driver. Two things must hold:

- **The session receipt still describes the instance it bound to.** The
  receipt's creation timestamp is the durable instance token. If the receipt is
  gone, its `created_at` no longer matches (a new session of the same name was
  started after a `purge`), or its status is now `stopped`, the loop is done.
- **The drive-loop binding still names this loop.** `git paw stop` revokes it,
  and a re-attach that recovers a stopped session supersedes it.

Either one failing exits the loop immediately — without sweeping panes or
sending any keystrokes — rather than continuing to drive whatever now occupies
that session name. This is what makes `purge`/`stop` safe to run against a
session still being driven unattended: the old loop notices on its very next
poll and gets out of the way instead of escalating prompts from a session it
no longer owns.

The binding covers the gap the receipt alone cannot see. Recovering a session
keeps its `created_at`, so a session stopped and recovered inside one poll
interval looks *unchanged* to the receipt check — and the loop bound to the
stopped instance would go on driving the recovered one. Revoking the binding on
`stop` is what tells it otherwise.

```bash
# Drive every discovered spec to completion, unattended
git paw start --unattended --from-all-specs

# Or a fixed branch set
git paw start --unattended --branches feat/auth,feat/api
```

Multiple feedback → fix → re-verify cycles per agent are normal and are **not**
treated as a stuck signal — the loop lets agents iterate.

### The loop is a pump, the supervisor is the brain

The drive loop is deliberately mechanical: it cannot decide *anything*. The
orchestrator is the opposite — it can reason about anything — and it runs
outside the session, so the loop never types into it. Every judgment call the
loop meets is a **broker review item**, and nothing is injected into any pane:

| Judgment call | The broker record |
|---|---|
| A live permission prompt — **to relay to the human, not to decide** | the loop's `agent.question` from `supervisor` |
| An ambiguous question | the worker's own `agent.question` |
| A merge decision | the worker's own `committed`/`done` `agent.artifact` |
| A non-converging branch | the loop's correction slow / exhausted escalation |

The loop never waits on any of them: one call being deliberated cannot freeze
the wave, and every record stays on the broker until it is answered. The
orchestrator reads them there — through `sweep.sh`, the broker helper or the
MCP server — and so does a human reading the dashboard's broker log.

**A permission prompt is relayed, not decided.** Deciding it from the pane text
would make the orchestrator a classifier — one reading untrusted output, and so
reachable by prompt injection. The orchestrator puts the prompt in front of the
human, naming the agent and the command, and runs `sweep.sh approve <pane>`
only when the human explicitly says to.

**A nudge is sent reliably, not fired and forgotten.** Typing text into a pane
and immediately following it with `Enter` is not enough: on a paste-aware CLI
the text can land in a paste buffer, and a bare follow-up `Enter` against a
stale input line is a proven no-op — it does not submit anything. So every
nudge — a worker re-engagement, and a stranded directive's recovery — sends the
text, waits the CLI's configured settle delay (the same
`[clis.<name>].submit_delay_ms` the boot prompt uses), then sends `Enter` as
a separate keystroke. Verification waits that same settle delay again, so the
pane has redrawn, before taking the check capture — and judges by the input
box's content, never by comparing it against the sent text verbatim: a long
nudge wraps onto more than one visual row, and only its first row still
carries the prompt sigil, so an exact-equality check would misread a wrapped
line as already delivered. The text counts as still present when the input
box is non-empty and either starts with that first row or shows a
paste-buffer placeholder; it counts as submitted once the box is empty or the
pane has left its idle state. A line still present is recovered by clearing
it (`Ctrl-U`), re-typing the text read from the whole box, and sending
`Enter` again, up to a bounded number of attempts. A pane still wedged after
every attempt is escalated to the broker for human review *and* listed in the
exit summary's escalations — deduplicated per pane, so a persistently stuck
line is reported once rather than on every ~15-second sweep. The whole send —
the first attempt through every recovery attempt — runs under the target
pane's exclusive claim (see "One approver per pane, enforced" below), so a
recovery only ever clears a line the loop itself just put there, never text a
worker agent typed.

**No nudge is sent once the loop is winding down.** The wind-down/exit state
is checked before dispatching any nudge on a tick, so a worker's idle pane
gains no further unsubmitted text on the loop's way out.

**Worker nudges skip finished workers.** Any nudge aimed at a *worker* pane is
suppressed once that worker has **finished** — its broker status is `done` or
`verified`. The signal is deliberately the broker status and never a
pane-content diff: a finished worker's pane is just as unchanging as a stuck
one's, so a diff cannot tell "idle because done" from "idle because stuck". A
worker that later leaves the finished state becomes nudge-eligible again — the
verdict is deferred, not dropped.

Note this is a *narrower* set than the one stall detection treats as terminal,
which also counts `blocked` and `committed`. That is deliberate: those two are
exactly the states a worker occupies while it *awaits* correction — committed
and standing by for re-verification, or blocked and not polling its inbox — and
they are precisely who the
[correction loop](#unattended-correction-loop) exists to re-engage. Suppressing
them would silently disable it for its most common path. Finished and quiet are
different conditions; only the first ends a worker's eligibility for a nudge.

**What the orchestrator does with a judgment call** is documented in the
bundled supervisor skill under *Judgment calls handed to you*: derive spawn order from
the agents' `agent.intent` declarations and the conflict detector's edges; answer
a question from the specs plus cross-agent state; sequence merges after
verifying; and declare a worker unrecoverable only per
[`max_cycles` / `on_exhausted`](#unattended-correction-loop).

**The human is the exception handler.** A human is re-engaged for what the
orchestrator genuinely cannot decide — plus the heartbeat exit described
above, and any run no orchestrator is driving, where the broker review items
and the exit summary are the whole escalation.

### Unattended correction loop

When a supervisor gate fails, git-paw publishes `agent.feedback` to the
branch's inbox — but a *blocked* worker is not polling its inbox, so the
correction stalls until a human re-engages the pane by hand. That single human
hop was the dominant cost measured across the v0.9.0–v0.11.0 dogfoods (4–8
manual verify↔fix cycles per branch).

The `[supervisor.correction]` table promotes that cycle into the drive loop.
Enable it with:

```toml
[supervisor.correction]
auto_loopback = true       # default is false, for back-compat
max_cycles = 5
on_exhausted = "escalate"  # or "abandon"
escalate_after_cycles = 3
```

With `auto_loopback = true`, each poll the loop:

1. **Watches for failing gate verdicts.** A gate failure arrives as an
   `agent.feedback` from the supervisor whose errors carry a `[<gate>]` tag
   naming one of `gate_tags` (default: `[testing]`, `[regression]`,
   `[spec audit]`, `[doc audit]`, `[security audit]`, `[scope]`) — the shape
   `.git-paw/scripts/sweep.sh feedback-gate` publishes. A passing gate
   publishes `agent.verified` instead, so only failures start a cycle. Other
   traffic on the same channel — conflict-detector warnings, peer feedback — is
   never mistaken for a gate verdict. The tag vocabulary is configurable (see
   [`gate_tags`](../configuration/README.md#correction-loop-tuning)) so a
   review process with different gate names can still drive the loop; the
   five-gate verification *framework* itself is unchanged.
2. **Re-engages the worker's pane.** The branch's pane is resolved by
   `pane_current_path`, re-captured immediately before the send, and typed with
   the gate-tagged feedback plus a submitting `Enter` as a separate keystroke.
   If the fresh capture shows the pane is no longer idle (a permission prompt
   went up between the sweep and the send), nothing is typed and the correction
   is retried on the next poll — deferred, never dropped.
3. **Counts the cycle.** Each delivered re-engagement spends one of the
   branch's `max_cycles`. A branch that reaches a terminal PASS leaves the
   cycle and its counter resets, so an unrelated later failure starts from a
   fresh budget.
4. **Flags a slow converger once.** When the count reaches
   `escalate_after_cycles` — and that lands strictly before `max_cycles` — an
   early heads-up is surfaced while re-engagement continues.
5. **Applies `on_exhausted` when the budget is spent.** `escalate` flags the
   branch to the orchestrator/human as unrecoverable; `abandon` marks it failed
   and sends no escalation. Neither re-engages that branch again, and the exit
   summary lists it under *Correction budget exhausted*.

Cycle counts are held in the drive loop's memory and are not persisted —
restarting the supervisor restarts every branch's budget.

The feature is **opt-in**: with `auto_loopback = false`, or with no
`[supervisor.correction]` table at all, the loop sends no re-engagement
keystrokes and the gate feedback is published exactly as it was before this
existed.

When `[supervisor] learnings = true`, exhausting a branch's budget also appends
a `correction_exhausted` learning carrying the branch, the worker CLI, and the
cycle count — the tiered-model tuning signal that tells you a worker's model is
under-powered for a class of task. With learnings off, nothing is recorded.

### The orchestrator's own permission level

No pane runs an orchestrator, so no git-paw launch flag reaches it: the
orchestrator runs in your own agent session, under whatever permission mode
you started that session with. `[supervisor].approval` is still parsed — an
earlier config that sets it loads and launches unchanged, with no warning —
but it changes no pane command; it is reserved for git-paw-initiated
orchestrator invocations. Coding agents resolve their flags from
`agent_approval` alone.

### `auto` resolves to a deterministic permission mode

`agent_approval` accepts three levels: `manual`, `auto`, and `full-auto`.
`full-auto` maps to the CLI's native skip-permissions flag (for example
`claude --dangerously-skip-permissions`); `manual` appends no flags at all. Previously `auto` also appended no
flags, so an `auto` pane silently inherited whatever default its CLI's config
directory happened to carry — sometimes manual approval, sometimes something
else, depending on machine state outside git-paw's control.

`auto` now resolves to an explicit, deterministic flag per CLI wherever one
exists: for Claude this is `--permission-mode acceptEdits` — file edits are
accepted by the CLI without asking, while shell commands still raise a prompt,
which git-paw escalates for you. Codex's `auto` already resolved to
`--sandbox workspace-write`. A CLI with no such mode keeps the empty string and
runs at whatever its own default is, unless you supply a
[`[clis.<name>] approval_args`](../configuration/README.md#custom-clis)
override.

These resolved flags are **git-paw's only influence** on what an agent may run
without asking. Everything the chosen mode leaves prompting stays prompting:
git-paw escalates it and waits for you. See
[Agent permissions](#agent-permissions).

### Launch reliability: acceptance dialogs and dead-pane detection

Two failure modes could previously leave an unattended wave silently dead at
boot:

- **First-run acceptance dialogs.** The first time a CLI launches with a
  bypass-permissions flag against a config directory that has never accepted
  it, some CLIs show a one-time confirmation dialog before their normal UI
  appears — for example Claude Code's bypass-permissions dialog, which
  **defaults its highlighted option to "No, exit"**. A pane launched
  unattended into that dialog exited within seconds without git-paw noticing.
  The launch-readiness gate now recognises this dialog (and a "trust this
  folder?" prompt) as a distinct state and answers it by selecting the
  affirmative option **by its text**, never by a hardcoded digit or a blind
  `Enter` — so the default-to-exit trap is never sprung. A dialog the gate
  cannot resolve fails the launch loudly instead of silently continuing; a
  CLI whose dialog the gate does not recognise falls back to the prior
  readiness/relaunch behaviour unchanged.
- **Panes that reverted to a bare shell.** git-paw launches every pane as a
  shell and `send-keys`s the CLI command into it. When a CLI process exits —
  whether from an unanswered dialog, a crash, or any other reason — the pane
  does not close; it reverts to its underlying interactive shell. `tmux
  has-session` alone cannot tell a live wave from one where every agent's CLI
  has quietly died, because the session (kept up by the dashboard pane) still
  exists. `git paw status` and `git paw doctor` now additionally
  require at least one agent pane whose foreground process is not a bare
  shell before reporting a session healthy, so a fully-dead wave is reported
  `stale`/stopped rather than active. A session within its first ~30 seconds
  is exempted from this check — the CLI needs a moment to start.

## Pane Layout and Labelling

When you attach to a supervisor session, git-paw styles the tmux panes so the
boundary between the top row (the full-width dashboard) and the agent grid
below is easy to see. By default each session gets:

- **Double-line pane borders** (`═║`) instead of tmux's default light `─│`
  lines, so the rows visibly separate even with four or more agents.
- **A per-pane label strip** above every pane — a reverse-video label bar
  showing its index and role:

  ```text
  ╔═ 0: dashboard ══════════════════════════════════╗
  ║ (live agent table)                              ║
  ╚═════════════════════════════════════════════════╝
  ╔═ 1: feat/cold-start ════╗╔═ 2: feat/conflict-det ═╗
  ║ (coding agent)          ║║ (coding agent)         ║
  ╚═════════════════════════╝╚════════════════════════╝
  ```

  Pane `0` is always `dashboard` and the agent panes are labelled with their
  branch id (`feat/<branch>`). The index shown is the
  same one you pass to `tmux send-keys -t <session>:0.<N>` (window 0; the
  session's second window, `broker`, hosts the broker), so you can
  cross-reference the label strip with the broker's `/status` agent listing.
- **An active-pane highlight** — the focused pane's border is cyan-bold while
  inactive panes are dimmed, so you always know which pane has keyboard focus.

These options are applied only to git-paw-managed sessions (`paw-*`); your
other tmux sessions keep their own styling. They apply to plain
`git paw start` sessions too, not just supervisor mode.

Set `[layout].border_affordances = false` in your config to opt out and
inherit your own tmux styling — see the
[Layout configuration reference](../configuration/README.md#layout). Opting out
changes only the styling: git-paw still tags each pane with its role, which is
what `status` and `doctor` read to tell whether a session's agents are running.
On tmux older than 3.2 the double border lines aren't recognised; git-paw
prints a warning and continues with the remaining affordances rather than
failing.

## Understanding what the supervisor is doing

You rarely need to read the orchestrator's own session to know what it is up
to. The supervisor tags each status heartbeat it publishes with a **phase** — a
short label for its current lifecycle activity — and a structured `detail`
body. The dashboard's supervisor row shows the current phase in its status
column, and the MCP `get_session_status` tool returns the same `phase` and
`detail` so external tooling can read it programmatically. (Both surfaces
degrade gracefully: a supervisor that hasn't published a phase renders with
its plain status, exactly as in v0.5.0.)

The supervisor emits these phase-tagged statuses through the bundled
`.git-paw/scripts/sweep.sh status-publish` helper — `--phase <phase>` sets the
label and `--detail '<json-object>'` carries the structured body, while the
plain `status-publish <message>` form (no flags) publishes the v0.5.0 shape
unchanged. The helper shapes the `agent.status` payload internally, so the
supervisor never hand-rolls the JSON. Because the helper always lives at the
stable path `.git-paw/scripts/sweep.sh`, a permission rule in your agent CLI
scoped to that exact path covers every phase without a broad HTTP-client rule.

Every `sweep.sh` publish verb — `status-publish`, `verified`, `feedback-gate`,
`conflict-ack`, `learn`, `answer`, `advanced-main` — checks the broker's HTTP response. When the broker
does not accept the publish (any non-2xx status, or no response at all), the
helper prints the broker's error to stderr and **exits non-zero** instead of
printing its "published" line. A non-zero exit means the event did NOT reach
the broker: fix the cause (a malformed target agent id, a stopped broker) and
re-publish rather than treating the verb as done. The agent-side `broker.sh`
behaves identically.

The phases, with what each one means:

| Phase | The supervisor is… |
|---|---|
| `baseline` | recording the regression baseline on the default branch at boot |
| `sweep` | scanning agent panes and the message stream for new events |
| `audit` | verifying a branch through the gates (the `detail.audit_step` field names which of `tests`, `regression`, `spec`, `docs`, `security` is running) |
| `merge` | cherry-picking or merging a verified branch into the base |
| `feedback` | sending feedback or a proactive route to a peer agent |
| `intent_watch` | tracking declared file intents for overlap |
| `learnings` | recording a structured learning record |
| `checkpoint` | enumerating a multi-publish batch before a risky action, so it can recover from a stream timeout |
| `idle` | waiting for the next event — nothing in flight |

The phase taxonomy is an open set: the supervisor may publish a value not in
this table, and the dashboard renders it as-is. The supervisor emits a status
on every phase transition and at most once per ~30 seconds while it stays in
the same phase, so the row stays current without flooding the
[broker log](dashboard.md). Phases are surfaced for the **supervisor row
only** — coding-agent rows render as they always have. (The one exception is
the supervisor-published `stuck-on-prompt` alert, which appears on a stalled
coding agent's own row.)

## Resolve Pane to Agent via `pane_current_path`

Before the supervisor `tmux capture-pane`s or `tmux send-keys`s a specific
agent, it needs the pane index for that agent. The bundled supervisor skill
is explicit that **pane indices are NOT alphabetical by `agent_id`, NOT in
the CLI-argument order from `git paw start --specs A B C`, and SHALL NOT be
inferred from `git paw status` output or the dashboard's row order** (both
of those are sorted alphabetically by the broker, which has no relationship
to the launcher's internal scan order).

The canonical resolution command queries tmux directly:

```bash
tmux display-message -t <session>:0.<pane> -p '#{pane_current_path}'
```

`<session>` is the session name recorded in `.git-paw/AGENTS.local.md`;
window 0 holds the panes (the session's second window, `broker`, hosts the
broker). The output is the pane's working directory. For coding-agent panes that is
the agent's worktree path — `<repo>/.git-paw/worktrees/feat-<branch>` under
the child placement `git paw init` configures, or `<project>-feat-<branch>`
beside the repo under the sibling placement. Matching it against the
`worktree_path` entries in the per-repo session file
(`.git-paw/sessions/<session>.json`) gives the owning agent's `branch_id` — the
authoritative `agent_id` (with the slash form `feat/<branch>` for git
operations). A pane whose `pane_current_path` is
`<repo>/.git-paw/worktrees/feat-auth` belongs to agent `feat-auth`. Because
the directory name depends on the configured placement, the match is against
the recorded `worktree_path`, never against the directory's basename.

The session file's roster also records a `pane_index`, but that is a
launch-time value and is **not** used for routing — the live
`pane_current_path` match is.

The orchestrator reads the roster when it needs an agent's pane rather than
building a pane map once and reusing it for the session, and in any case
re-reads it after `git paw add` or `git paw remove` — the roster changes and
tmux renumbers panes on layout changes. The roster can also be read through
the helpers or the MCP `get_session_status` tool.

The bundled `.git-paw/scripts/sweep.sh` invokes this command on every sweep
iteration. If the helper is missing for any reason, the supervisor falls
back to invoking `tmux display-message` directly — this is the documented
escape hatch.

The supervisor MUST NOT use `git paw status` output (or the dashboard's row
order) as a mapping source — both are sorted alphabetically by the broker
and have no relationship to the launcher's pane assignment. Always resolve
via `pane_current_path` first.

## Pane-driving and cross-worktree git disciplines

The bundled supervisor skill hardens three behaviours so a multi-agent
session runs without the human approving the supervisor's every command:

- **Use `.git-paw/scripts/sweep.sh` for pane work; never inline loops.**
  Ad-hoc `for p in <panes>; do tmux send-keys -t ...:0.$p ...; done` loops
  trip the `simple_expansion` permission gate on each iteration, so one
  sweep becomes one approval prompt per pane. The helper does the explicit
  per-pane sends with no variable expansion. A single explicit
  `tmux send-keys -t ...:0.<index>` is reserved for pushing one typed reply
  to one pane.
- **Never send-keys to the dashboard (pane 0).** It is a TUI, not an agent;
  keys sent there land in the dashboard, not in any agent. `sweep.sh`
  targets coding-agent panes by design.
- **Cross-worktree git uses `git -C <path>`, never `cd <path> && git`.**
  A bare `cd` trips the untrusted-hooks warning on some CLIs and leaks the
  working directory, so a later mutating git command can land on the wrong
  branch. `git -C` scopes the directory to the single command.

The skill also nudges any agent whose uncommitted working set exceeds ~10
files to commit its completed section, preserving per-section verification
granularity.

## Spec audit governance sub-step

When a project's `.git-paw/config.toml` lists governance documents under
`[governance]` (`adr`, `test_strategy`, `security`, `dod`, `constitution`),
the supervisor reads each doc as part of the
Spec Audit gate and flags drift between the diff and the doc's checklist.
The five canonical doc-checklist examples are:

- **DoD** (Definition of Done) — walk each `- [ ]` item against branch state.
- **ADRs** (Architectural Decision Records) — verify new architectural
  decisions (new deps, new patterns) have a matching ADR.
- **security.md** — walk each security checklist item against the diff.
- **test-strategy.md** — check that test composition matches the documented
  strategy.
- **constitution.md** — check the diff against documented principles
  (e.g. "no panics in library code").

Findings surface as standard `agent.feedback` errors tagged `[doc audit]`
mixed in with other doc-audit gaps. See [Governance](governance.md) for the
config schema and how the supervisor reads each doc.

## Agent permissions

**git-paw does not approve permission prompts, and makes no judgement about
whether an agent's command is safe.** It has no safe-command list, no danger
list and no path-based safety rule. Whether a command runs without asking is
decided entirely by the **agent CLI's own permission system**:

1. **The CLI's resolved permission mode.** `[supervisor] agent_approval` maps
   to each coding agent's native launch flags — see [`auto` resolves to a deterministic permission
   mode](#auto-resolves-to-a-deterministic-permission-mode). This is git-paw's
   only influence on what runs unprompted.
2. **The CLI's own allow and deny rules.** Whatever that CLI reads from its own
   settings. Pre-approve your routine dev commands there; that is the only
   place a standing grant can live. A grant is only as narrow as you write it.
3. **A sandbox**, if you want a kernel-enforced boundary rather than a
   policy one — see [Sandbox](./sandbox.md) and
   [Security Posture](./security-posture.md).

Anything the resolved mode leaves prompting *stays* prompting. git-paw's job
starts there: it detects the live prompt and escalates it for a human decision
(the broker `agent.question` the orchestrator reads, the
[manual-decision log](#manual-approvals), and the exit summary) without
blocking the rest of the wave. Then a human decides, and the approval is typed
only by an explicit [`sweep.sh approve <pane>`](#the-explicit-approve-action).

A fully prompt-free unattended run therefore needs the decision made *before*
the prompt exists: either CLI allow rules covering everything the agents do, or
`agent_approval = "full-auto"` inside a sandbox. Without that, expect
escalations.

> **Deprecated: `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]`.**
> Up to v0.16.0 these configured a git-paw command classifier that answered
> prompts it judged safe. The classifier is retired: three successive security
> reviews each found new command shapes it wrongly called safe, and each fix
> opened the next hole. Both tables **still parse and round-trip**, so existing
> configurations load unchanged — but they have **no effect**, and each
> configuration file that declares them prints one deprecation warning per
> process. Delete them at your leisure, and move any `safe_commands`, `stacks`
> or `extra` entries into your agent CLI's own allow rules. See the
> [configuration reference](../configuration/README.md#deprecated-auto-approval-tables).
>
> The classifier used to choose "Yes, and don't ask again" for prompts it
> judged safe, and your CLI persisted those as **durable grants in its own
> settings file**. Retirement stops new ones; it removes none. Review that file
> and delete what you did not intend — git-paw never edits or deletes it.

### Run dev commands bare — no exit-code-probe wrappers

A CLI allow rule matches a command **prefix**, so it generalises across
argument variants. Wrapping a command in an exit-code probe —
`<cmd> && echo "EXIT $?"`, `<cmd>; echo $?`, `RC=$?; echo "$RC"` — defeats
that: the probe text varies per run, so the CLI's command-string matching never
matches the next invocation and the command re-prompts every time. The bundled
supervisor and coordination skills instruct agents to run dev commands bare and
read the exit status directly; keep your own commands to the same shape so a
grant actually suppresses the prompt.

(git-paw used to normalise the two most common wrappers away before
classifying. With no classifier there is nothing to normalise for, and the
*CLI's* own matching was never forgiving about them anyway. Run them bare.)

### The explicit approve action

`sweep.sh approve <pane>` is the **only** path by which an approval keystroke
reaches a pane, and nothing in git-paw runs it for you. A human runs it, or
tells the orchestrator to run it. It answers the prompt **once**, with option
`1`, and consults nothing about the command.

Four guards sit between that instruction and the keystroke.

**Option 1, never a durable grant.** The helper dispatches the digit `1`
followed by `Enter` as two separate keystrokes, reporting
`approved pane <N> (option 1)`. It never dispatches a blind cursor-movement
sequence — `Down` + `Enter` lands on `No` on a 2-option prompt and takes the
permanent "don't ask again" grant on a 3-option one. Before sending it checks
that a numbered option 1 really is on screen (and sends nothing if not), and
that option 1 does not itself carry the prompt profile's broad-grant wording:
on some prompt shapes the first option *is* "Yes, and don't ask again …", and
pressing it would write a standing permission rule you did not ask for. In that
case the helper sends nothing and says why. git-paw never creates a durable
grant.

**Re-confirm before send.** Immediately before dispatching, the helper takes a
**fresh capture** of the target pane and confirms the prompt's live structural
markers still anchor the modal block (the same anchored search and
bounded-trailing-region tolerance detection uses, not a fixed ~4-line scan). If
the prompt cleared between the decision and the send — the agent moved on, or
you answered it in the pane first — **no keystrokes are sent**, and the helper
reports `cleared before send, no keys sent`. This closes the stray-input race
where the option digit would otherwise land in the CLI's chat box as literal
text, polluting context and leaving dangling unsubmitted commands.

**Coding-agent panes only.** The helper sends keys only to a pane the session's
discovery roster (`.git-paw/sessions/<session>.json`) lists as a coding
agent's `pane_index`. `sweep.sh approve 0` against a broker-enabled session —
pane 0 is the dashboard — sends nothing and reports that the pane is excluded
because it is not a coding agent, whatever the capture shows; so does a pane
no roster entry claims. In a broker-less session the first coding agent sits
at pane 0, and the roster makes it approvable. With no readable roster the
helper falls back to refusing pane 0. The drive loop applies the same
role-based rule: it never types into the dashboard or an unattributed pane.
The refusal cannot be changed by a prompt profile.

**One sender per pane, enforced.** Keystrokes can come from `sweep.sh approve`
and from the drive loop's own nudge and buffered-input recovery. If two land on
one pane, the second falls through onto whatever appears next. So before
sending, every sender takes that pane's **exclusive claim**: an atomic
create-if-absent of `<repo>/.git-paw/tmp/approve-pane-<N>.claim`, held across
the send and released immediately after. Because the file system decides who
wins, exactly one sender can hold a pane at a time, and the exclusion works
across processes and languages — the shell helper and the binary compute the
same path and honour each other's claims.

Taking the claim never blocks. A sender that finds a pane already claimed
**skips it** and moves on, retrying later; `sweep.sh approve` reports
`pane <N> claimed by another approver, no keys sent` and names the claim file.
Release is automatic on every exit path, including an error or a crash
mid-send. If a sender is hard-killed (`SIGKILL`) with a claim held, the next
one steals it after it has gone untouched for 10 seconds — so a crash costs one
pane one sweep, never a permanent wedge. The claim directory is ephemeral and
already gitignored; you never need to manage it by hand, though removing a
stale file is harmless.

No new broker message is involved in any of this: the trigger that a pane is
awaiting approval is the existing `agent.status` with
`phase: "stuck-on-prompt"`, and an escalated prompt uses the existing
`agent.question`.

### What detection still does

Prompt *detection* is the part git-paw keeps, and it is unchanged. The
live-prompt gate decides whether a modal is actually on screen: at the
capture's true end, a textual marker (the `Do you want to …` question or the
`Esc to cancel` footer) within the last ~4 non-blank lines, or a numbered
option line anchoring that tail with a textual marker within the last ~15
non-blank lines. The search runs upward over a bounded trailing region, so a
CLI that renders a task list or status strip under its input area cannot hide a
live modal beneath that chrome; beyond the true end, liveness requires the
modal's actual *shape* (a footer-shaped line plus an option list), never a bare
mention of the footer text in narration or tool output, and a match is rejected
outright if the CLI's idle input box appears below it — a real modal replaces
the input box, so seeing one means the footer is a quoted copy.

Detection reports only that a prompt is present, and what command it is asking
about (the text between the `Bash command` / `Bash(…)` header and the
confirmation question, or the target path of a file-operation prompt). That
command text is used for the escalation message, the dedup key and the
manual-decision log — never to decide anything. A supervisor that merely
*mentions* `rm -rf /` in prose is not a prompt to run it, and a prompt asking
about `ls -la` gets exactly the same treatment as one asking about `rm -rf /`.

The window sizes, the trailing bound and the pane-0 refusal are compiled logic.
A [CLI prompt profile](./cli-prompt-profiles.md) supplies the *literals*
(markers, patterns) and can never widen the gate, change which option
`approve` presses, or lift the pane-0 refusal.

## Out-of-worktree write violations

An observed out-of-worktree write attempt by a coding agent — a permission
prompt asking to write into operator territory, pane narration describing one,
a diff touching operator paths, a stray artifact in the repo-root control dirs
— is a **boundary violation**, and the supervisor skill instructs a two-step
response:

git-paw no longer has a mechanical check for this. The protected-path rule
existed only to stop the retired classifier auto-approving such a write; with
nothing approving automatically, a write prompt into operator territory
reaches a human like every other prompt, and the supervisor relays it rather
than approving it. The doctrine below is what carries the boundary; for
mechanical enforcement use deny rules in your agent CLI's own settings, or the
[sandbox](./sandbox.md).

1. **First attempt — scoped feedback.** The supervisor sends the agent
   `agent.feedback` (via `sweep.sh feedback-gate <agent> scope …`) naming the
   worktree boundary and the attempted path, so the agent can rescope:
   persist inside its worktree, or publish `agent.question` before any
   outside write.
2. **Repeat by the same agent — operator escalation.** A second attempt
   after the scoped feedback escalates to you via `agent.question`, naming
   the agent, both attempted paths, and the feedback already sent — the
   decision (pause, re-brief, remove) is yours, not the supervisor's.

The agent-side counterpart is the coordination skill's memory-isolation
briefing (see [Agent Coordination](coordination.md#memory-isolation)):
persistent artifacts live inside the agent's worktree, operator
configuration directories are off-limits, and a task that seems to require
an outside write becomes a question instead.

## Manual approvals

Every prompt the `--unattended` drive loop escalates is a command that needed
*you*. `git paw approvals` aggregates them, so you can see which prompts recur
and decide which deserve a standing rule in your agent CLI's own allowlist —
closing the loop on repeat friction.

This is written by the **drive loop**, so it covers `--unattended` runs only.
An attended session (`git paw start --supervisor` without `--unattended`) runs
no loop and records nothing; `git paw approvals` still reads any log an
earlier unattended run left behind.

### What gets recorded

Each time the drive loop escalates a live permission prompt, it appends one
JSON line to `.git-paw/sessions/<session>.manual-approvals.jsonl`:

```json
{ "timestamp": "2026-05-29T12:34:56Z", "agent_id": "feat-auth",
  "pattern": "make integration-test", "first_seen": true }
```

The loop runs *outside* the agent's CLI pane, so it cannot see whether you
ultimately pressed Yes or No on the prompt. What it records is the honest,
observable signal: **a command that required a human decision**. A command
escalated seven times is a strong allow-rule candidate regardless of each
individual yes/no — the friction it causes is what you want to remove.

Exactly one line is written per escalation: a prompt re-seen inside the
five-minute dedup window is recorded once, and escalations that are not
permission prompts — a `cli-unavailable` banner, a wedged nudge, a
correction-loop signal — are not recorded here at all.

The `pattern` comes from the escalation's own **prompt slice** — the command
between the `Bash command` / `Bash(…)` header and the confirmation question,
or a file-operation prompt's target path. Surrounding pane text is
never scraped, so an accept-edits diff line or a previous command's output
sitting next to the prompt cannot be recorded as if it were the command
awaiting a decision. When neither source yields something that parses as a
command, **no** line is written and no `permission_pattern` learning is emitted
— an unusable pattern is worse than a missing one, because it pollutes the
promotion report you act on.

### Reporting

```bash
git paw approvals                       # active session, text table
git paw approvals --json                # machine-readable
git paw approvals --session paw-other   # a specific session
git paw approvals --limit 5             # top 5 patterns by count
```

The text table sorts patterns by how often each was escalated, with a
`SUGGEST` hint for which of your agent CLI's own allowlists could absorb it:

```text
PATTERN                       COUNT  SUGGEST
make integration-test             7  user allowlist
./scripts/deploy-staging.sh       3  project allowlist
podman build -t paw-ci .          2  user allowlist
```

A pattern is suggested for the CLI's **project allowlist** when it looks
project-specific (a `./`-rooted script path, or it contains the project or
branch name); otherwise it belongs in your **user allowlist**, where it
benefits every project. Both are rules in the agent CLI's own settings — the
`SUGGEST` column is a hint, not a rule, and `git paw approvals` never edits
any settings file for you.

When `[supervisor] learnings = true`, the first sighting of each pattern also
emits a `permission_pattern` learning so it surfaces in
[Learnings Mode](learnings.md) alongside the other patterns.

### Opting out

Recording is on by default for `--unattended` runs. To disable both the log
writes and the derived learnings emission for a session:

```toml
[supervisor]
manual_approvals_log = false
```

The opt-out affects *writes* only — `git paw approvals` still reads any
pre-existing log. See [Configuration](../configuration/README.md) for the
field reference.

## Repo-configurable gate commands

The supervisor's five verification gates each invoke a configurable command
substituted from `[supervisor]` keys at session boot. The eight keys are:

- `test_command`
- `lint_command`
- `build_command`
- `fmt_check_command`
- `doc_build_command`
- `doc_tool_command` — API-doc generator, separate from `doc_build_command`
- `spec_validate_command`
- `security_audit_command`

When a key is missing or empty, the placeholder renders as `(not configured)`
in the supervisor skill and the supervisor **gracefully skips the tooling
invocation** for that gate — the gate's manual review still applies.
`doc_tool_command` is the one exception: when unset it renders as the empty
string (not `(not configured)`) so the surrounding prose reads naturally for
projects that don't ship an API-doc generator. See
[Configuration](../configuration/README.md) for defaults and examples.

### Polyglot bundled skills

The bundled supervisor and coordination skills ship as a single template
each, but they render per session against the project's stack and the
resolved spec backend. Two placeholders make this work:

- `{{DOC_TOOL_COMMAND}}` substitutes the `doc_tool_command` config value
  above so doc-audit prose names the actual API-doc tool for your stack.
- `{{SPEC_PATH_DOCTRINE}}` renders a per-backend path doctrine: OpenSpec
  sessions see `openspec/changes/<name>/...` references, Spec Kit sessions
  see `.specify/specs/<feature>/...`, Markdown sessions see the flat
  `paw_status: pending` shape, Superpowers sessions see
  `docs/superpowers/plans/*.md`, and multi-backend sessions list each
  present backend's conventions in one paragraph.

A CI no-leak audit asserts that the rendered supervisor skill contains
no stack-specific tokens (`cargo`, `rustdoc`, `.rs:`, `Cargo.toml`,
`rustc`) and no vendor product names outside the explicitly-allowed
`<!-- allowlist-prose -->` span, which carries a deliberate CLI-specific
enumeration (the paste-buffer marker wording).

## Verification runs the whole suite (never fail-fast)

The supervisor's testing gate runs the configured test command in a
whole-suite / no-fail-fast mode. Most test runners stop at the first failing
test group, so a single early failure — typically an environment-specific
**guard test** rather than a code defect — can hide every later suite and
produce a false PASS. A run that aborted early is **incomplete, not a pass**;
a testing PASS requires the full suite to have executed.

For git-paw itself, the no-tmux-server guard test trips whenever any `paw-*`
tmux session exists (which it always does mid-session) and sits in an
early-alphabetical test binary, so a plain `cargo test` / `just check` stops
there. The **`just verify`** recipe runs the trustworthy gate —
`GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast` (the suite is
socket-isolated, so the opt-out is safe) plus lint, deny, and audit — and
the dogfood config sets `[supervisor].test_command = "just verify"` so the
supervisor uses it automatically.

## Per-commit verification

The supervisor verifies each agent's commit **as it lands**, not in batches.
When an agent publishes `agent.artifact { status: "committed" }`, the
supervisor runs that branch's five-gate sweep promptly — independent of
whether other agents have finished. Waiting for a slower agent so several
commits can be verified "together" delays feedback to the agent that already
committed and serialises work that is meant to run in parallel; at five-to-
eight agents that idles the whole session until the slowest one finishes.

To make the trigger an explicit event rather than something the supervisor
has to notice during a pane sweep, the broker posts a `supervisor.verify-now`
message naming the committing branch to the supervisor inbox on every
committed artifact. This is on by default; set
`[supervisor] verify_on_commit_nudge = false` to suppress the nudge and rely
on the supervisor's sweep cadence instead.

Per-branch verifications may run concurrently — each gate sweep runs against
its own branch in its own worktree, so verifying one agent never blocks
starting another's verification. The supervisor may bound how many sweeps run
at once at its discretion. The one legitimate reason to defer a verification
is a genuine dependency (one agent's work needs another's merge in place
first), which the supervisor states explicitly when it defers.

### Verdicts go through the helper

The verdict messages — `agent.verified`, and `agent.feedback` from `supervisor`
— need the session's authority credential, so the supervisor publishes them
only through the root-side `sweep.sh` verbs, which fetch it with
`git paw __authority` and send it as `X-Paw-Authority`:

```bash
.git-paw/scripts/sweep.sh verified <agent-id> "<summary>"
.git-paw/scripts/sweep.sh feedback-gate <agent-id> <gate> "<message>"
.git-paw/scripts/sweep.sh conflict-ack <agent-a> <agent-b> <file> ["<note>"]
```

A raw `curl …/publish` of either message gets `403 {"error":"authority required"}`,
and so does the same verb run from inside a worktree (`__authority` refuses
there). See [Security Posture](./security-posture.md#broker-verdict-authority).

## Recovery from API stream timeouts

The supervisor's own model stream can time out mid-sweep — most often
while it is queueing several `agent.feedback` or `agent.verified`
publishes in one iteration. Left unhandled, the natural reflex is to
fall silent and wait for the next sweep tick, which silently drops the
pending feedback: the agent that was about to hear about a regression
keeps working unaware, and you get no signal anything went wrong.

The bundled supervisor skill teaches an explicit recovery discipline so
a transient timeout no longer loses work:

1. **Recognise the failure shape** — a mid-stream cutoff, or a
   transport / stream error in the CLI output. The phrasing is generic
   so it matches across CLI variants.
2. **Checkpoint before risky actions** — before any iteration with more
   than one intended downstream publish, the supervisor publishes a
   single `agent.status` "checkpoint" enumerating what it is about to
   do, giving the recovery path a re-entry point.
3. **Replay the missing publishes** — on recovery the supervisor
   re-reads its checkpoint and, for each intended target, polls
   `/messages/<target>` since the checkpoint timestamp to see which
   publishes landed, then re-publishes only the missing ones (the
   replay is idempotent, so duplicates are safe).
4. **Never assume a publish succeeded** — a returned `publish` HTTP call
   is not proof the record landed; the supervisor confirms by polling or
   re-publishes idempotently before moving on.

Each successful recovery emits a `recovery_cycles` `agent.learning`
record, so recurrent timeouts surface in the learnings output. See the
"Stream-timeout recovery" section of the bundled
`assets/agent-skills/supervisor.md` skill for the full doctrine, and
[Learnings Mode](learnings.md) for how the `recovery_cycles` records are
aggregated.

## Broker-side conflict detector

Starting with v0.5.0 the broker auto-detects three failure shapes between
parallel agents and emits `agent.feedback` (and, where configured,
`agent.question`) on the supervisor's behalf. All auto-emitted messages
begin with the `[conflict-detector]` token so the supervisor can distinguish
detector output from human-typed feedback. The three failure shapes are:

- **Forward conflict** — two agents publish overlapping `agent.intent`
  declarations.
- **In-flight conflict** — two agents' filesystem-watched
  `modified_files` sets overlap on the same file.
- **Ownership violation** — an agent's `modified_files` include a file
  inside another agent's active intent.

See [Conflict Detection](conflict-detection.md) for the algorithm,
configuration, and escalation behaviour.

## Learnings aggregator

When `[supervisor] learnings = true`, the supervisor session
records deterministic friction signals (stuck durations, recovery cycles,
conflict events, permission patterns) into a markdown file you can review
after the run. The supervisor also records *qualitative* learnings it reasons out over
the run — recurring failure shapes, doc gaps, ADR drift, scope mistakes, and
**tooling friction** with git-paw itself — publishing each through the bundled
`.git-paw/scripts/sweep.sh learn <category> <title> <body-json>` helper (never
a raw HTTP request). It captures them opportunistically during each sweep and again in
a session-end synthesis pass. See [Learnings Mode](learnings.md) for the file
format, the category set, and how to opt in.

## When the user types in your session

The orchestrator runs in your own interactive agent session (outside tmux) —
you can type at any time while its autonomous monitoring loop is running. The supervisor finishes the
current step (spec audit, test run), responds, then resumes the loop. User
input is a high-priority interrupt, not a replacement for the loop.

Each kind of user input maps to an existing mechanism — the supervisor does
not invent new channels:

1. **Status question** ("how's feat-auth going?", "anything blocked?") —
   answered conversationally in the orchestrator's session using `sweep.sh status`,
   `sweep.sh inbox`, and `sweep.sh capture <pane>`. **Nothing is published
   to the broker** — this is a conversation with the user, not a
   session-wide event.
2. **Directive** ("ask feat-auth to use bcrypt", "tell feat-api to skip
   the migration") — published as `agent.feedback` to the named agent
   through `sweep.sh feedback-gate` with the `[directive]` gate, plus a conversational confirmation
   to the user.
3. **Judgment-call ask** ("should we merge feat-a before feat-b?") — the
   supervisor applies its normal escalation rules. If the user has already
   provided enough information to decide, the supervisor answers in its
   session using its reasoning. `agent.question` only fires when the call is
   genuinely ambiguous beyond what the user just provided — typically
   when the user is asking *because they don't know either*.

The mechanisms (`sweep.sh status` / `inbox` or the MCP reads,
`sweep.sh capture`, `sweep.sh feedback-gate`, `tmux send-keys`,
`agent.question`) are unchanged. The addition is *when
to use which* in response to user input.

## Routing through the supervisor

The orchestrator is the single conversational surface: you work in its
session and address individual agents *through* it, rather than tab-switching
into each agent's pane. Two commands you type to the orchestrator drive this.

### `/agents` — the live inventory

Type `/agents` to the orchestrator, in its own session, and it responds with
the current inventory: one row per agent plus the orchestrator's own
`supervisor` row once it has published a status, each coding-agent row carrying
`branch_id`, `status`, `last_seen`, `cli`, the detected `mode`, and
`pane_index`. The `supervisor` row has no pane index — the orchestrator has no
pane, and pane 0 is the dashboard:

```
branch_id   status    last_seen  cli     mode          pane
feat-auth   working   3s         claude  accept-edits  2
feat-api    blocked   90s        claude  unknown       1
supervisor  working   1s         claude  —             —
```

The inventory is composed from broker `GET /status` (`branch_id`, `status`,
`last_seen`, `cli`) joined with the live `pane_current_path` mapping
(`pane_index`). Pane indices are **path-resolved**, never assumed from
ordering — a mid-session `git paw add`/`remove` that renumbers the grid is
reflected correctly after the next sweep. The `mode` column is best-effort
(`accept-edits`, `interactive`, or `unknown`); `unknown` is treated as the
safe case by `/tell`.

**Known limitation — underscored branches show no `pane_index`.** The join
matches an agent's `agent_id` against its worktree directory name. A branch
containing `_` (or a `.`, or upper case) keeps those characters in its
directory while its `agent_id` maps them to `-`, so the two no longer agree
and the pane goes unmatched: the agent still appears in the inventory with its
status and `last_seen`, but `pane_index` is empty and `mode` reads `unknown`.
`/tell` still reaches it — delivery falls back to the broker `agent.feedback`
path, which is the safe route it uses for `unknown` mode anyway. Only the
`send-keys` fast path and the pane-mode column are affected. Avoid `_` in
branch names if you want the pane columns populated.

The supervisor refreshes the inventory at its sweep cadence and caches it in
memory. `/agents` and `/tell` reuse the cached snapshot while it is younger
than `[supervisor.tell] inventory_max_age_seconds` (default 60), re-polling
the broker only when it goes stale. The cache is not persisted — a restarted
supervisor rebuilds it on the first command.

### `/tell <agent> <prompt>` — route a prompt

Type `/tell feat-auth rebase onto main` and the supervisor:

1. **Validates** `feat-auth` against the inventory (the slug `feat-auth` or
   slash form `feat/auth` both work). An unknown target is **not** delivered —
   the supervisor replies with the candidate list of available agents, e.g.
   `unknown target` `feat/ghost`​`; available agents: feat-api, feat-auth`.
2. **Chooses a delivery mode** (see below) and delivers the prompt.
3. **Acknowledges** in its own session and, when learnings mode is on, records the
   routing decision (see [Learnings](./learnings.md)).

`/tell` routes exactly the agent you name and exactly the prompt you type —
it never decides the recipient for you and never pipes your prompt into
another CLI to generate content. There is no bulk broadcast form in this
release; smart, supervisor-decided routing is a future candidate.

### Delivery modes

`[supervisor.tell] mode` selects the default delivery channel:

- **`feedback`** (the default) — the prompt is published as an
  `agent.feedback` broker message and the agent consumes it on its next inbox
  poll. Safe for mixed-mode sessions: the prompt is queued and recorded, never
  raced into a running prompt.
- **`send-keys`** — the prompt is injected directly into the target's pane
  with `tmux send-keys`. Faster, but only safe when the target is in
  accept-edits mode. When you configure `send-keys` but the target's detected
  mode is `interactive` or `unknown`, `/tell` **falls back to `feedback`** and
  prints a note so you can investigate the mode-detection drift. Because of
  this fallback, `send-keys` is best reserved for sessions where your agents
  run in accept-edits mode.

Every `/tell` is auditable: when `[supervisor] learnings = true`, each
invocation appends a line to a `### Supervisor routing` section of
`.git-paw/session-learnings.md` with the timestamp, target, delivery mode, and
the prompt (truncated past 200 characters). With learnings disabled, nothing
is written.

### Proactive routing (opt-in, confirmed)

When a sweep finds an agent blocked on a question you have *already answered*
earlier in the orchestrator's session, the orchestrator may **offer** to
forward that context — it asks you a yes/no question in its own session and
waits. It never routes
on its own: no proactive `/tell` fires without your explicit `y`. Reply `n` (or
anything else) and the offer is dropped. This keeps the pattern from feeling
surveillance-y while it earns trust through dogfooding.

## opsx role gating

The bundled skills teach that `/opsx:verify` and `/opsx:archive` are
supervisor-only, but a coding agent that ignores the prose can still archive a
change — the skill is a convention, not a guard. When the session's spec engine
is OpenSpec, git-paw closes that gap with a **post-commit role-gating guard**.

The guard watches the broker's `agent.artifact { status: "committed" }` events.
When a commit looks like an archive operation **and** the committing agent is
not the supervisor, it reacts per `[opsx] role_gating` (see the
[configuration reference](../configuration/README.md#opsx-role-gating)):

- **`warn`** (the v0.6.0 default) — publishes an `agent.feedback` to the
  offending agent and records an `agent.learning` with category
  `permission_pattern` so the violation surfaces in learnings.
- **`block`** — the warn behaviour, plus an `agent.feedback` to the supervisor
  requesting a revert. The supervisor performs the revert through its
  merge-orchestration skill (confirming with the user first unless
  `[supervisor] auto_revert = true`); git-paw never runs `git revert` itself.
- **`off`** — the guard is disabled.

**Detection heuristic.** A commit is archive activity when **either** signal
fires:

1. its subject line matches the canonical archive shape
   `chore(specs): archive <name>; sync deltas to main specs`, or
2. its diff moves files into `openspec/changes/archive/<name>/` and/or adds a
   main spec under `openspec/specs/<capability>/spec.md`.

The two-signal design is deliberately conservative — it would rather trip on
the supervisor's own archive (which the attribution check then clears, since
`agent_id == "supervisor"`) than miss a coding-agent archive. An unresolvable
worktree is treated as a violation.

**Reading the warning.** The feedback text names the short SHA, the offending
agent, and the trigger so you can spot a false positive at a glance:

```
opsx-role-gating: detected archive activity on commit abc1234 by agent feat-x
  (not the supervisor).
  Reason: commit message matched the archive heuristic ("chore(specs): archive
  feat-x; sync deltas to main specs").
  /opsx:verify and /opsx:archive are supervisor-only ...
```

If a warning is a false positive, the named reason tells you which signal fired
so you can adjust (or flip `role_gating = "off"`). The guard is **inert under
the Spec Kit, Markdown, and Superpowers engines** — those have no `/opsx:`
commands or archive paths, so the capability and its skill sections are scoped
to OpenSpec only.

> **v0.6.0 behaviour change.** `role_gating` defaults to `warn`, so sessions
> where a coding agent archives a change now see guard feedback. Set
> `role_gating = "off"` for the prior (no-guard) behaviour.

## Per-CLI setup (consent-only)

Agents call git-paw's helper scripts constantly, and under most agent CLIs
every call raises a permission prompt. The supervisor does not make those
prompts go away: a helper call runs without one only through the agent CLI's
own permission system and the rules in its settings.

You can pre-approve git-paw's own helper calls in that CLI's allow rules with
[`git paw setup-cli <CLI> --allowlist`](cli-setup.md). It is optional and
consent-only — you run it, it prints its plan first, and `--undo` takes back
exactly what it added. Nothing in the supervisor, the drive loop or the MCP
surface ever runs it, and it refuses to run from a linked worktree.

The embedded `cli-setup` skill carries the rules an agent must follow around
it. `git paw init` and `git paw start` export it to
`.git-paw/skills/cli-setup/SKILL.md` beside the orchestrator skill (main
checkout only), and the MCP `get_skill` tool serves it under the name
`cli-setup`. The
supervisor skill restates the three that bind the orchestrator directly: run
the writing form only when the human asked in the current conversation, pass
`--yes` only after they approved the exact plan shown, and never run it
during an unattended run.

## Merge orchestration

Once every spec'd agent has been verified — the orchestrator published
`agent.verified` for it through `sweep.sh verified` (agents never publish it
themselves) — or the user explicitly asks for a merge, the orchestrator runs the merge orchestration
loop. v0.5.0 removed the Rust auto-merge loop; merging is the orchestrator's
responsibility, performed with shell, git, the configured test command and the
bundled helpers or MCP tools — never a raw HTTP request.

**Trigger.** Either every spec'd agent has been verified (the orchestrator
published `agent.verified` for it), or
the user has explicitly requested the merge.

**Unattended runs merge without asking first.** When the session context
carries the drive-loop directive (`--unattended`), the orchestrator merges as
soon as the trigger holds — fast-forward only — without asking the human
first. It holds a branch back and escalates only on:

- a **conflict** — a non-fast-forward or conflicting branch (skipped, with
  `agent.feedback` to its agent), or a dependency cycle (escalated with
  `agent.question`);
- an **exhausted correction cycle** — a branch whose correction budget ran out
  (escalated with `agent.question`);
- a **regression** — a merge that fails the test command (reverted, with
  `agent.feedback` to its agent).

Every held-back branch is named in the final summary. In an **attended** run
the orchestrator may still confirm the merge order with you before merging.

**Merge order.** The orchestrator reads the broker's message log
(`sweep.sh inbox` / `sweep.sh events`, or the MCP read tools) and builds a dependency graph from `agent.blocked`
events: each event from agent X with `payload.from = Y` is an edge "X
depends on Y". The supervisor then topologically sorts the graph: agents
with no incoming edges merge first; dependents follow.

**Per-branch merge.** For each branch in topological order, the supervisor
checks out `main` and runs:

```bash
git merge --ff-only feat/<branch>
```

Never a merge commit — fast-forward only. If `--ff-only` fails (the branch
diverges from `main`, or there is a conflict), the supervisor SKIPS that
branch and publishes `agent.feedback` to the owning agent asking them to
rebase or resolve. On a successful fast-forward, the supervisor runs the
configured `{{TEST_COMMAND}}`; if tests fail, the supervisor reverts the
merge with `git reset --hard <prev-HEAD>` and publishes `agent.feedback`
tagged `[regression]`. If tests pass, it publishes `agent.advanced-main`
with `sweep.sh advanced-main <merged-branch> <summary>` — the helper resolves
the new default-branch SHA and the base branch name itself — so every dependent agent learns the base moved on its next inbox poll —
see [When Main Advances](coordination.md#when-main-advances) — then moves on to
the next branch.

**Cycle handling.** If the dependency graph has a cycle, the supervisor
does NOT merge any branch in the cycle. Instead, it publishes
`agent.question` (through `broker.sh --agent supervisor question`) to the human and waits for guidance before continuing.

**Final summary.** When the loop completes, the supervisor publishes a
final `agent.status` summarising which branches merged cleanly, which were
skipped (and why), and any regressions encountered.

## Branch refresh (opt-in)

Over a long wave the supervisor merges peer after peer into the default
branch while every other live worker keeps committing on the baseline it
started from, so each branch drifts further from what it will eventually
be merged into. The only *existing* recourse is worker-side and advisory —
see [*When Main Advances*](coordination.md#when-main-advances) in Agent
Coordination — which correctly tells a worker holding uncommitted work not to
auto-rebase, but means refresh only happens when a worker chooses to act.

When `[supervisor] branch_refresh = true` is set, the supervisor's
[unattended drive loop](#unattended-mode---unattended) instead performs the
refresh itself, at a moment it can prove is safe: after a successful merge
(`agent.advanced-main`), for a branch that is provably idle and clean. This
is a supervisor operation, not a worker judgment call — the two disciplines
do not conflict, because the supervisor acts only under exactly the
condition (no uncommitted work) that makes the worker-side "do NOT
auto-rebase" rule unnecessary.

**Default off.** Rewriting a live agent's history is opt-in, permanently —
set `[supervisor] branch_refresh = true` to enable it. With the field
absent or `false`, post-merge behaviour is unchanged from every prior
version.

**Every live worker branch is checked after every merge**, and refreshed
only when ALL of the following hold. Any single failure leaves the branch
untouched — there is no partial or degraded attempt, and the skip is
recorded, not retried or escalated:

1. **Clean.** The worker's working tree has no uncommitted changes, per
   the broker watcher's tracked `modified_files` — never a fresh probe, so
   the supervisor and the watcher can never disagree about what "clean"
   means.
2. **Idle.** The worker's pane is not mid-response.
3. **Claimed.** The supervisor holds the pane's exclusive approval claim
   (the same TOCTOU guard approvals use), narrowing the race window
   between the cleanliness check and the rebase.
4. **Conflict-free.** A `git merge-tree` prediction shows the rebase would
   not conflict — checked BEFORE the rebase starts, so the worktree never
   enters an in-progress rebase state a live agent could observe. A
   prediction that proves wrong once the rebase actually runs is recovered
   with `git rebase --abort`, leaving the branch at its pre-rebase HEAD.
5. **Behind.** The branch actually has commits to pull in; an already
   current branch is left alone.

A branch that has already passed the five-gate supervisor verification and
awaits merge is never refreshed — it has been verified at its current
base, and rewriting it would silently invalidate that verification.

**Notification.** After a successful refresh the affected worker receives
`agent.feedback` tagged `[branch-refresh]` (the same bracket-tag
convention as `[conflict-detector]`) telling it HEAD moved, so it does not
keep reasoning about file contents or commit SHAs from before the
rewrite. This is deliberately NOT `agent.advanced-main` — that event means
"the base moved" and the worker-side discipline above keys off that exact
meaning; conflating it with "your own branch was rewritten" would corrupt
that discipline.
