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

### Forward conflict

Two agents publish `agent.intent` payloads whose `files` arrays overlap.
The conflict is *forward* because it surfaces before either agent commits
— intent is the early-warning channel.

**Trigger.** Any non-empty intersection between two active intents (an
intent stays active until its `valid_for_seconds` TTL expires, the agent
publishes a fresh intent, or the agent commits).

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
detection keeps working) but no `agent.feedback` fires for forward shape.

### In-flight conflict

One agent publishes `agent.status` or `agent.artifact` whose
`modified_files` overlap with another agent's active intent — or another
agent's recent status / artifact. The conflict is *in-flight* because at
least one side is already writing.

**Trigger.** Any non-empty intersection between an `agent.status` /
`agent.artifact` `modified_files` array and another agent's active intent
or status (whichever is freshest).

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

### Ownership violation

An agent's `modified_files` (in `agent.status` or `agent.artifact`)
touches a path that the spec marks as *owned* by a different change. The
ownership map is built once at session start from the change directories'
`Files owned:` / `Owned files:` declarations.

**Trigger.** Any path in `modified_files` matches an ownership entry that
points to a change other than the sending agent's.

**Action.** The violator receives `agent.feedback` describing the touched
path and the owning change. When
`[supervisor.conflict] escalate_on_violation = true` (default), the
supervisor inbox also receives a follow-up `agent.question` so a human
can decide whether to override the boundary or block the work.

## The `[conflict-detector]` Tag

Every auto-emitted `agent.feedback` from the detector starts its `errors`
array with a fixed tag:

```json
{
  "type": "agent.feedback",
  "agent_id": "feat-auth",
  "payload": {
    "from": "supervisor",
    "errors": [
      "[conflict-detector] forward conflict: feat-api also declares intent over src/auth/middleware.rs",
      "..."
    ]
  }
}
```

The `[conflict-detector]` prefix distinguishes detector output from
human-typed supervisor feedback. Agents (and dashboards) that filter or
classify feedback can match on the tag without parsing payload semantics.

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
do not escalate by default — they are advisory.

## Interaction with the Filesystem Watcher

The broker's filesystem watcher publishes `agent.status` (with a fresh
`modified_files` array) whenever a tracked file changes in a worktree.
The conflict detector consumes these auto-published status messages, so
in-flight conflicts surface as soon as edits land on disk — no manual
`agent.status` curl required from the agent.

The watcher is read-only with respect to git (it watches the working
tree, not the index), so the detector sees overlaps the instant a file
is modified, even before `git add` or `git commit`. This is the
mechanism that makes "in-flight" meaningfully earlier than the
post-commit hook's `agent.artifact { status: "committed" }`.

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
