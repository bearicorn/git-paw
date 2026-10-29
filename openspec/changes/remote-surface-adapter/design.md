## Context

After `terminal-port-surface-capabilities`, the terminal port describes each surface by what it
can do: whether it presents an interactive screen and whether it holds a layout position. The
interactive-only operations fail with `PawError::Unsupported` elsewhere. v1.3.0 adds the headless
backend (non-interactive turns, `activity`, owner-only run state under the user data directory),
the drive loop's non-interactive rules and the broker host's inbox-wake thread.

A remote worker's work runs on a vendor's machines. git-paw's view of it, as Fleet's design and its
reviewed plan established for Jules, is a REST conversation: create a session from a pushed branch,
poll its state, list its activities, send it a message. There is no cancel. The session's result is
a patch the vendor hands back. Landing that patch is `remote-work-landing`, and the Jules specifics
are `jules-remote-vendor`. This change is the vendor-neutral part.

Fleet's Jules route (llm-toolbox `fleet/`, built on `worktree-fleet-catalog`, its live run pending)
is the proof of concept for this release. Its live runs answer five questions the vendor's docs leave open; they
are listed verbatim in `jules-remote-vendor`'s design. Two touch this change: event ordering (3) and
question shape and timing (5). The requirements here hold either way: events are ordered by
creation time, never by list position, and a question falls back to a fixed text when no agent
message is found.

## Goals / Non-Goals

**Goals:**
- A third terminal adapter that answers every port operation for a remote surface, honestly.
- A vendor-neutral port, so later vendors (Codex cloud, Copilot's coding agent, Claude Code cloud
  sessions) are new adapters on the same seam, not new machinery.
- A follow that escalates questions, relays answers once, approves nothing, cancels nothing and
  cannot loop forever.
- Every follow decision a pure, table-tested function.

**Non-Goals:**
- Any vendor adapter (`jules-remote-vendor`), selection and the launch gate (`mixed-backend-sessions`,
  `remote-work-landing`), patch landing and verify continuation (`remote-work-landing`).
- Watching a vendor session after git-paw's session ends. Fleet's runner keeps watching for 24 h;
  git-paw has no process that outlives its session, so a session that is still running when the
  follow ends is the human's (its URL is printed and escalated).
- Pull-request handling of any kind (v1.5.0's provider abstraction).

## Decisions

### D1 — A third terminal adapter, with a vendor-neutral port behind it

`RemoteBackend` implements `TerminalBackend`. Behind it sits `RemoteAgentService`, a port in
git-paw's terms, with one in-tree adapter per vendor (Strategy).

*Why an adapter and not a sibling terminal port:* the drive loop, inventory, `status`, `remove`,
`stop`, capture and the broker host already reach every worker through the terminal port. A
sibling port would give each of them a second path. After `terminal-port-surface-capabilities` the
port's operations either mean something for a remote surface or return `Unsupported`.

*Why a second, vendor-facing port behind the adapter:* what a vendor offers (sessions, events,
patches, plans) is not a terminal concept, and the follow needs it. Putting it behind
`RemoteAgentService` keeps the terminal adapter vendor-neutral and makes the next vendor one new
module. Its identifiers name no vendor (audited).

*Alternative rejected:* one terminal adapter per vendor (`JulesBackend`). It would duplicate the
surface mapping, the follow and the state for every vendor.

### D2 — `spawn_surface` creates the session, from a remote task plan

The launch knows the task when it spawns the surface, so the plan carries the prompt, and spawning
creates the vendor session. A plan that names a recorded session re-attaches instead, which is how
recovery never creates a second session. This mirrors v1.3.0's turn plan: an optional,
backend-specific plan on `SurfaceSpec`, required by its backend and ignored by the others.

*Alternative rejected:* create the session on the first submitted message, as headless starts its
first turn. It would postpone creation, and so the eligibility facts (pushed base, no local
commits), past the launch gate that checks them.

### D3 — Text and `Enter` send one message

`send_text` drafts and `Enter` sends the draft, so v0.18.0's split-send-then-`Enter` discipline
holds and every caller that already sends a message to a non-interactive surface works unchanged.
The draft is persisted so that two git-paw processes compose one message, as in v1.3.0.

### D4 — Non-interactive, activity unknown

A remote surface is non-interactive, so no screen predicate ever reads it and no keystroke is ever
sent. Its activity is unknown, because v1.3.0's turn decisions (failed / interrupted /
no-progress) and the inbox wake are about local turns, and none of them is right for a vendor
session: a vendor session cannot read the broker inbox, and its stalls are bounded by the time cap.
`status` reports the vendor's state instead (`mixed-backend-sessions`).

### D5 — The follow runs in the broker host

The broker host runs for every coordinated session, in attended and unattended mode, and sees every
broker message. v1.3.0 already gives it a thread of its own (the inbox wake). The follow is another
such thread with its own cadence.

*Alternatives rejected:*
- **The drive loop.** It runs only under `--unattended`, so an attended session would never hear a
  remote question.
- **A separate `__remote-follow` service.** A second detached process with its own lifecycle across
  start, pause, recovery and stop, for no gain over a thread in a process that already has that
  lifecycle.

A session with a remote worker therefore requires the broker (enforced by the launch gate in
`remote-work-landing`).

### D6 — Questions become `agent.question`; git-paw approves nothing

A vendor question is published as an `agent.question` from the worker's agent id, with the vendor's
words quoted and the session URL. In an unattended session the drive loop's message decision already
turns a coding agent's `agent.question` into a judgment call for the headless orchestrator
(v0.18.0 `orchestrator-escalation-handoff`). In an attended one the operator answers through the
broker or MCP. A plan awaiting approval is escalated the same way. git-paw never calls a vendor's
plan-approval operation, consistent with v0.17.0's rule that git-paw approves nothing. Sessions are
created with plan approval off, so this state is not expected.

### D7 — Answers are relayed once per distinct question

The relay set is closed: `agent.answer` (and, under the landing rule, `agent.feedback`). An answer
binds to the open escalated question; it is recorded before it is sent, so a crash between the two
loses at most one message and never doubles one (at-most-once, like v1.3.0's "never replay a
turn"). A lost answer leaves the question open, which the time cap and the operator surface.
`agent.blocked` is not relayed: a remote agent cannot unblock a peer through the broker.

### D8 — Bounds, carried from Fleet's reviewed plan

| Rule | Value | Source |
|---|---|---|
| read cadence | at most once per 60 s per session | Fleet design §6 |
| list events only when state or update time changed | — | Fleet review M20 |
| time cap measured from the latest of creation, re-attach, last message sent | `[remote] follow_hours`, default 4 | Fleet design §6; review M8 |
| consecutive read failures | 2 tolerated, 3rd ends | Fleet review M9 |
| authentication failure | ends at once | Fleet plan Task 5 |

The cap restarts only on something a human or the orchestrator did (a re-attach by `start`, an
answer, the one verify continuation), so the follow cannot loop overnight.

### D9 — State: owner-only, outside the repository, recorded before acting

As v1.3.0's headless run state: `<session_state_dir>/<session>.remote/<slug>/`, `0700` / `0600`,
paths from `SessionName` and `BranchSlug`. The session file's `WorktreeEntry.remote` holds only
what recovery needs (vendor, session name, URL, title, base commit). Neither holds a credential.

### D10 — No cancel

The vendor API has no cancel, so `kill` ends the follow and says so. Every command that ends a
remote surface prints its URL and that it may keep running. The time cap, the vendor's own
concurrency limit and the human are the bounds on a session git-paw no longer follows.

`stop`, `pause` and the follow's own end conditions keep the session file's record, so the next
`git paw start` re-attaches. `remove` is the one command that drops the record of a single worker,
after which nothing re-attaches to its session. It therefore refuses, unless `--force`, a remote
worker whose session's last-known state is neither completed nor failed, as it refuses uncommitted
work. Fleet's final review (I-1) found sessions left running unwatched when a task left the remote
route. In git-paw a worker's backend is fixed, so moving the task to a local worker means removing
the remote worker first, and that now takes a deliberate `--force`. `purge` is unchanged: it ends
the whole session by the user's choice and prints every URL.

### D11 — Rendering

Events are sorted by creation time, de-duplicated by identifier, and rendered as
`<time> <kind>: <text>` lines with control characters removed, so vendor text printed by `status`
or `attach` can never carry a terminal escape sequence (in line with v0.21.0's
`core-terminal-output` policy for git-paw's own output). The same rendering feeds `capture_tail`,
`stream_to` and `status`.

## Risks / Trade-offs

Threat analysis (`security-and-safety-review`). Vendor-specific threats (credentials, transport,
host pinning) are in `jules-remote-vendor`, and patch threats are in `remote-work-landing`.

- **R1 — Untrusted vendor text reaches the orchestrator.** A question, plan or event is written by
  the remote agent, and the orchestrator may be a model. → It is quoted and framed as the remote
  agent's words, control characters are removed, and git-paw acts on none of it: it neither
  approves nor answers anything itself.
- **R2 — Orchestrator text reaches the vendor.** An answer may carry anything the orchestrator
  wrote. → Only `agent.answer` addressed to that worker is relayed, once per question. The docs say
  that relayed answers leave the machine.
- **R3 — Publishing on a worker's behalf.** The broker host publishes `agent.question` from the
  remote worker's id. It is not a verdict (`agent.verified` / `agent.feedback`), so the broker's
  verdict authority (v0.18.0 `broker-verdict-authority`) is unaffected.
- **R4 — Runaway spend.** Every message can start vendor work. → At most one answer per question,
  one verify continuation (landing), a time cap that restarts only on outside action, and the
  vendor's own limits (`jules-remote-vendor`).
- **R5 — State tampering by a local worker.** A local worker could forge follow state to suppress
  an escalation or replay an answer. → The state is outside every worktree and owner-only. An
  unsandboxed full-auto worker can write it anyway, as with v1.3.0's run state.
- **R6 — A session nobody follows.** After the cap or `stop`, the vendor session may keep running
  and keep counting against the vendor's limits. → Its URL is escalated and printed; recovery can
  re-attach; `remove` needs `--force` while the session may still be running (D10).

## Migration Plan

Additive and unreachable by users until `remote-work-landing`:

1. The port, its domain types and the fake; the pure follow decision with table tests.
2. `RemoteBackend` (surface mapping, draft, rendering, state) against the fake.
3. The broker host's follow thread; `WorktreeEntry.remote`; `[remote] follow_hours`.
4. Assets, audits, architecture docs.

Rollback: revert. Older binaries ignore `WorktreeEntry.remote` and the run state.

## Open Questions

- **Question shape and timing** (PoC question 5, see `jules-remote-vendor`). The question text is
  the newest agent message at or before the session's last update, with a fixed fallback; the
  cadence is 60 s and the cap 4 h. Both hold whatever the live run shows; the live run may justify
  different defaults.
- **Answers with no open question.** They are not relayed. If dogfood shows orchestrators sending
  unprompted guidance to remote workers, a later change could relay it under the same once-only
  rule.
