## Context

The in-flight conflict detector keeps one structure doing two jobs. The *tracker* holds
"which (pair, file) overlaps exist right now", derived from each agent's current
`modified_files`. The *escalation decision* — whether this conflict has already been put
to a human — is recorded as a flag on that same tracker entry.

Those two things have different natural lifetimes. The overlap is as volatile as
`modified_files`: an agent commits, publishes an empty set, and the overlap vanishes for a
tick. The decision is not volatile at all — a human answered a question about two agents
and a file, and that remains true afterwards.

Because they share a lifetime, a commit discards the decision. The existing requirement
even spells the mechanism out: a triple whose file leaves the intersection is removed,
"the conflict has resolved without escalation". It has not resolved; the agent committed.

Broker log, dogfood session: seq 65 `modified_files = []`, seq 66 the file is back,
seq 71 the identical escalation fires 353s after seq 47 — after the orchestrator had
adjudicated it and published feedback to both agents. Every step is spec-conformant.

## Goals / Non-Goals

**Goals:**

- Separate the volatile overlap from the durable decision.
- Give an orchestrator a way to say "I have answered this" that actually sticks.
- Keep the detector able to re-escalate when a conflict genuinely changes shape.
- Avoid growing the `BrokerMessage` enum if an existing variant will serve.

**Non-Goals:**

- Changing how overlaps are *detected* or how additive-versus-true classification works.
- Suppressing the initial `agent.feedback` warning to the two agents — that is cheap,
  useful, and goes to the agents rather than to a human.
- Persisting decisions across broker restarts. In-session durability is what the defect
  calls for; cross-session persistence is a larger question about where conflict state
  lives, and belongs with the v0.18.0 broker-authority work.
- Auto-resolving conflicts. The human's answer is still the answer.

## Decisions

**Split the decision record from tracker residency.** The minimal correct change: keep the
tracker as the live view of overlaps, and hold escalation decisions in their own map keyed
on `(min_agent, max_agent, file)`. Tracker removal stops discarding decisions, and
`first_seen` moves to the durable side so a commit cannot rewind the clock.

**Invalidate on material change, not on absence.** The question "is this still the same
conflict?" should be answered by the region declarations, which is what the existing
additive/true classification already reasons about — not by whether the file happened to
be in `modified_files` on a given tick. This keeps the detector honest: a conflict that
changes shape can escalate again.

**Prefer an existing broker variant for acknowledgement.** Per AGENTS.md §7, adding a
`BrokerMessage` variant ripples into `agent_id()` / `status_label()` / `validate()`,
`dashboard/broker_log.rs`'s three functions, `learnings.rs::observe()`, and the semantic
routing in `delivery.rs` — and the compiler only catches some of those. `approval-auto`
set the precedent ("Approval gate reuses existing broker variants"), and the detector
already consumes `agent.feedback`. Reuse it. If a new variant proves unavoidable, scope
every site in §7 **before** starting, not on discovery.

**Keep the overlap visible after acknowledgement.** Acknowledging must not delete the
overlap from the tracker — otherwise "I have answered this" becomes "pretend this is not
happening", and the next reader of the dashboard sees nothing. Acknowledgement suppresses
the *question*, not the *fact*.

## Risks / Trade-offs

- **A suppressed escalation is a real risk.** If "material change" is defined too loosely,
  a conflict that has genuinely become dangerous stays silent because someone acknowledged
  an earlier version of it. Lean toward invalidating the acknowledgement: a spurious
  re-escalation costs a turn, a missed one can cost a merge.
- **Unbounded decision records.** Decisions now outlive tracker entries, so they need a
  disposal rule or they accumulate for the session's life. Bounded by the agent-pair ×
  file space, which is small, but it should be deliberate rather than incidental.
- **Acknowledgement is a new verb an orchestrator must know about.** It is useless unless
  the helper exposes it and the bundled orchestration guidance mentions it. Shipping the
  broker side alone would leave the capability unreachable in practice.
- **Interaction with the v0.18.0 broker-authority work.** That cycle moves verdict
  authority into the broker. An acknowledgement record is adjacent to that; keep this
  implementation small and session-local so it does not prejudge where durable conflict
  state eventually lives.
