# Conflict Detection

When supervisor mode is active, the broker runs an in-process conflict
detector that watches for three failure shapes across parallel agents.
Detected conflicts surface as `[conflict-detector]`-tagged `agent.feedback`
delivered to the involved agents, and — for unresolved in-flight shapes —
as `agent.question` escalations to the supervisor inbox. The detector is
automatic; you only opt out by setting `[supervisor] enabled = false` (or
tuning the knobs in `[supervisor.conflict]`).

## Contents

- [The Three Failure Shapes](#the-three-failure-shapes)
- [The `[conflict-detector]` Tag](#the-conflict-detector-tag)
- [Supervisor Inbox Routing](#supervisor-inbox-routing)
- [Interaction with the Filesystem Watcher](#interaction-with-the-filesystem-watcher)
- [Configuration Knobs](#configuration-knobs)

## The Three Failure Shapes

git-paw's own managed bookkeeping — everything under a worktree's
`.git-paw/` directory (config, bundled scripts, session state) — is excluded
from the overlap computation for **forward** and **in-flight** conflicts.
Every worktree shares this untracked scaffolding, so its presence in
`modified_files` is not evidence of a code conflict: two agents both
reporting `.git-paw/config.toml` never fabricates a warning, while a genuine
source overlap alongside it (`src/a.rs` present in both) still conflicts on
the source path.

### Forward conflict

Two agents publish `agent.intent` payloads whose `files` arrays overlap.
The conflict is *forward* because it surfaces before either agent commits
— intent is the early-warning channel.

**Trigger.** Any non-empty intersection between two active intents (an
intent stays active until its `valid_for_seconds` TTL expires or the agent
publishes a fresh intent, which replaces it). Each pair is warned once;
the warning can fire again only after one side's intent changes its file
set or expires.

**Action.** Each agent receives an `agent.feedback` with the
`[conflict-detector]` tag in its `errors[0]`, naming the peer and listing
the overlapping paths. Neither side blocks; the receiving agents decide
how to retract or reshape their plans.

**Region granularity (v0.6.0).** When both intents declare *regions* for a
shared file (function / class / block / range — see [Declaring
regions](./coordination.md#declaring-regions-v060)), the detector triggers
only when the declared regions intersect, and the warning names the
intersecting regions. When either side omits regions it falls back to the
file-level trigger above, preserving v0.5.0 behaviour. Cross-kind comparisons
(a named region vs a line range) intersect conservatively and append a hint.

**Toggle.** `[supervisor.conflict] warn_on_intent_overlap` (default
`true`). When `false`, the detector still records intents (so in-flight
classification and ownership detection keep working) but no `agent.feedback`
fires for forward shape.

### In-flight conflict

Two agents' current `agent.status` `modified_files` sets overlap on the same
file. The conflict is *in-flight* because both sides are already writing.

**Trigger.** Any non-empty intersection between one agent's latest
`agent.status` `modified_files` and another agent's latest
`agent.status` `modified_files` (each new status replaces that agent's
previous set). `agent.artifact` does not feed this check, and declared
intents only decide how an unresolved overlap is classified (below).

**Action.** Both agents receive `[conflict-detector]`-tagged
`agent.feedback`. The detector starts a `window_seconds` timer; if no
side retracts before it elapses, the detector classifies the overlap from
the two agents' declared intent regions for the shared file and acts:

- **True collision** — the declared regions intersect (same named region,
  same insertion anchor, overlapping line ranges, or a conservative
  cross-kind match), *or* either side declared no regions for the file
  (a file-level intent, or no active intent at all). The detector escalates
  to the supervisor inbox via `agent.question`
  (see [Supervisor Inbox Routing](#supervisor-inbox-routing)).
- **Additive overlap** — *both* agents declared regions for the file and
  the region sets are disjoint (well-separated hunks or differently named
  regions). The detector downgrades to a single informational
  `[conflict-detector]`-tagged `agent.feedback` delivered to both agents
  ("shared file, additive — resolve at merge") and does **not** escalate to
  the human. The overlap is still recorded: the triple is marked decided so
  it neither re-escalates nor re-emits the downgrade on later ticks, and it
  clears only when one agent stops touching the file — so an additive
  overlap is never silently dropped.

The downgrade requires *both* sides to have declared regions; whenever the
region data is insufficient to prove the hunks are disjoint, the detector
escalates, preserving the conservative default.

**Window.** `[supervisor.conflict] window_seconds` (default `120`).

**Escalations persist across commits.** The *overlap* is as volatile as
`modified_files`: an agent commits, the watcher publishes an empty set, and
the overlap vanishes for a tick or two before the file reappears. The
*decision* — that this pair and this file have already been put to a human —
is not volatile, and the detector keeps it separately from the live overlap
view. So a commit does not re-escalate a conflict that has already been
escalated, and it does not rewind the window timer for one that has not: an
overlap first seen 100 seconds ago still escalates at the 120-second mark,
not 120 seconds after the commit. A decision is discarded only once its
conflict is genuinely over — the overlap has been continuously absent for
the escalation window (at least 60 seconds) — or when the two agents'
declared regions for the file describe a materially different overlap.

**Acknowledging an escalation.** Once you have adjudicated an escalation,
record it so the detector stops asking:

```bash
.git-paw/scripts/sweep.sh conflict-ack <agent-a> <agent-b> <file> "optional note"
```

This publishes a `[conflict-ack]`-tagged `agent.feedback` from `supervisor`
to both agents. A note you pass rides along as a second `errors` entry so
the agents see your reasoning; if you pass none, the helper supplies a
default one — agents are taught that `errors` are problems to fix, and a
bare tag line would read as a cryptic error.

While the acknowledgement stands, no further `agent.question` is emitted
for that pair and file — but the detector **keeps tracking** the overlap
rather than dropping it, so a later escalation can still be built on it.
Acknowledging suppresses the question, not the fact. (No view currently
renders the in-flight tracker, so "keeps tracking" means exactly that:
the detector's own state, not something you can read off the dashboard.)

The acknowledgement is invalidated automatically when the two agents'
region declarations for that file change materially — judged by the same
intersection rules that drive the additive-versus-true classification
above. A conflict that genuinely changes shape can therefore escalate
again: an agent re-declaring `agent.intent` with a range that now overlaps
its peer's, or widening its claim from a named region to the whole file.
An intent merely *expiring* is not a change — otherwise every settled
conflict would re-open one TTL later.

**What the supervisor-only rule does and does not do.** The detector
honours an acknowledgement only when the carrying `agent.feedback` has
`from = "supervisor"`. That filter stops an agent's ordinary feedback
from being mistaken for an adjudication. It is **not** an authentication
boundary: the broker does not authenticate senders, `from` is
self-asserted, and `sweep.sh` is present in every worktree — so an agent
that set out to do so could publish `conflict-ack` for a conflict it is
party to. What limits the damage is visibility, not prevention: an
acknowledgement published through the helper reaches **both** named
agents, and **every** acknowledgement — however it was published — stays
in the broker message log. (The detector honours one whatever its
envelope `agent_id`, so a hand-rolled one need not have been delivered to
both; the log is what you audit.) Sender authentication is part of the
v0.18.0 broker-authority work.

### Ownership violation

An agent's `modified_files` (from its `agent.status`) touches a file that
another agent has claimed in its active `agent.intent`. Ownership is
intent-based: a file is *owned* by whichever agent's non-expired intent
declares it.

**Trigger.** Any path in the agent's `modified_files` that lies outside its
own active intent (or the agent has no active intent) and inside another
agent's active intent.

**Action.** The violator receives `agent.feedback` naming the touched file
and the agent whose intent claims it; each violator/file pair is warned
once. When
`[supervisor.conflict] escalate_on_violation = true` (default), the
supervisor inbox also receives a follow-up `agent.question` so a human
can decide whether to override the boundary or block the work.

**Exemption.** The shared spec-task-tracking artifact — the tasks file
git-paw directs every agent to update with its own checkbox (a Spec Kit
`specs/<feature>/tasks.md` or the OpenSpec `tasks.md`) — is exempt from
ownership-violation detection. git-paw's own writeback protocol has every
agent tick its own line in that single shared file, so a writeback there
is expected coordination, not a collision. The exemption only covers that
designated task-tracking artifact; ownership violations on source files
are still reported.

## The `[conflict-detector]` Tag

Every auto-emitted `agent.feedback` from the detector carries a single
`errors` entry that starts with a fixed tag:

```json
{
  "type": "agent.feedback",
  "agent_id": "feat-auth",
  "payload": {
    "from": "supervisor",
    "errors": [
      "[conflict-detector] forward conflict: agent feat-api also intends to modify: src/auth/middleware.rs"
    ]
  }
}
```

The `[conflict-detector]` prefix distinguishes detector output from
human-typed supervisor feedback. Agents (and dashboards) that filter or
classify feedback can match on the tag without parsing payload semantics.

One further tag travels the other way. An `agent.feedback` from
`supervisor` whose `errors` entry reads
`[conflict-ack] <agent-a> <agent-b> <file>` is read by the detector as an
acknowledgement of that conflict (see
[In-flight conflict](#in-flight-conflict)); `sweep.sh conflict-ack` shapes
it for you. The file path runs to the end of its `errors` entry, so the tag
line carries nothing after the path — add any prose as a separate entry.
The detector never emits `[conflict-ack]` itself.

The detector publishes from the `"supervisor"` agent ID (the `from` field
in the payload) — same source as human-authored supervisor feedback —
because routing and display logic already specialise on the supervisor
identity. The tag, not the source, is the discriminator.

## Supervisor Inbox Routing

When an in-flight conflict has not resolved within `window_seconds` **and
is classified a true collision** (see [In-flight conflict](#in-flight-conflict)),
the detector escalates to the supervisor by publishing an `agent.question`
addressed to the *supervisor* (not to either of the conflicting agents).
Additive overlaps are downgraded to an `agent.feedback` and never land in
this inbox. The supervisor pane sees the question in its broker inbox and
can:

1. Type a reply, which the supervisor skill forwards to both involved
   agents via `tmux send-keys` (the same dual-write pattern documented in
   [Agent Coordination § Supervisor Acknowledgement](coordination.md#supervisor-acknowledgement-of-agentquestion)).
2. Resolve the conflict directly by editing one agent's intent or
   pausing the offending agent until the other side commits.

Ownership-violation escalations follow the same routing. Forward conflicts
never escalate — they are advisory.

## Interaction with the Filesystem Watcher

The broker's filesystem watcher polls `git status --porcelain` in each
worktree every ~2 seconds and publishes `agent.status` (with a fresh
`modified_files` array) whenever the set of reported paths — modified,
staged, or untracked — changes. The conflict detector consumes these
auto-published status messages, so in-flight conflicts surface as soon as
edits land on disk — no manual `agent.status` curl required from the agent.

The watcher only reads `git status` — it never stages or commits — so the
detector sees overlaps within one poll of a file being modified, even
before `git add` or `git commit`. This is the mechanism that makes
"in-flight" meaningfully earlier than the post-commit hook's
`agent.artifact { status: "committed" }`.

## Configuration Knobs

All knobs live under `[supervisor.conflict]` in `.git-paw/config.toml`:

```toml
[supervisor.conflict]
window_seconds = 120
warn_on_intent_overlap = true
escalate_on_violation = true
```

| Field | Default | Description |
|-------|---------|-------------|
| `window_seconds` | `120` | Seconds to wait before escalating an unresolved in-flight conflict to the supervisor inbox via `agent.question`. |
| `warn_on_intent_overlap` | `true` | Forward-conflict feedback toggle. When `false`, intents are still tracked but no `agent.feedback` is emitted on intent overlap. |
| `escalate_on_violation` | `true` | Ownership-violation escalation toggle. When `false`, the violator still receives `agent.feedback`, but no follow-up `agent.question` lands in the supervisor inbox. |

The `[supervisor.conflict]` table is fully optional. Setting
`[supervisor] enabled = false` (or omitting the section) disables the
detector subsystem entirely — no auto-emitted feedback fires regardless
of the values above. See [Configuration → Conflict detector tuning](../configuration/README.md#conflict-detector-tuning)
for the canonical field reference.
