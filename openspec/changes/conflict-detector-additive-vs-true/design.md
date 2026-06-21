## Context

The broker's conflict detector (`src/broker/conflict.rs`) tracks three things per agent: active intents (`IntentRecord`, keyed by `agent_id`, with `files: HashMap<String, Option<Vec<Region>>>`), the current modified-file set (`current_files`), and per-pair in-flight state (`in_flight_pairs`, keyed by the triple `(min_agent, max_agent, file)`).

In-flight detection works at **file granularity only**: when two agents both report the same `file` in their `agent.status.modified_files`, the detector records an `InFlightPair { first_seen, escalated }`. On each tick `take_due_escalations(window, now)` returns every triple older than `window_seconds` that has not yet escalated, and the broker emits an `agent.question` to the supervisor inbox for each. There is no inspection of *where* in the file the two agents are working — the escalation fires on filename overlap alone.

The v0.6.0 `conflict-detector-fn-granularity` change already added a `regions` declaration to `agent.intent` files and taught **forward**-conflict detection to use it: `regions_intersect(a, b)` returns whether two region sets intersect (same named region; overlapping `range`; cross-kind = conservative intersect). That machinery is reusable here — the only gap is that the **in-flight escalation decision** never consults it.

The v0.8.0 dogfood showed the cost: 3 of 4 in-flight escalations were additive (disjoint hunks in a shared file) and only 1 was a true collision (both agents inserting at the same anchor). The additive false alarms stall an unattended wave on human input that is not needed.

## Goals / Non-Goals

**Goals:**
- Make the in-flight **escalation decision** region-aware: only true collisions reach the human as an `agent.question`.
- Downgrade additive overlaps (disjoint, well-separated regions) to an informational `agent.feedback` so the supervisor/agents are still told the file is shared, without blocking the wave.
- Guarantee the additive case is recorded (the triple is marked handled) so it neither re-escalates every tick nor is silently dropped.
- Preserve the conservative default: when region data is insufficient to prove disjointness, escalate as today.

**Non-Goals:**
- No change to forward-conflict or ownership-violation behaviour.
- No source parsing or diff/hunk computation — classification uses only the declared `regions` already on the intents, never the file contents on disk.
- No new config field; the existing `[supervisor.conflict] window_seconds` gate is unchanged.
- No wire-format change to any broker message.

## Decisions

### Decision: Classify at escalation time using the intents' declared regions

When a triple `(a, b, file)` becomes due for escalation, look up `a`'s and `b`'s active `IntentRecord.files[file]` regions and classify:

- **TRUE collision** → emit `agent.question` (current behaviour). True iff the two region sets for `file` **intersect** under the existing `regions_intersect` rules: same named region (function/class/block with matching name/anchor), or overlapping `range` intervals, or a cross-kind named-vs-range comparison (conservatively intersecting). Same insertion anchor (two `block { anchor }` with equal anchors) is the canonical true case.
- **ADDITIVE overlap** → emit an informational `agent.feedback` (the new downgrade), NOT an `agent.question`. Additive iff **both** agents declared at least one region for `file` AND the region sets are **disjoint** (do not intersect) — i.e. well-separated hunks (e.g. two non-overlapping `range`s, or two differently-named functions).

**Why reuse `regions_intersect`:** it is the same predicate forward-conflict already uses, so additive-vs-true in-flight and non-conflict-vs-conflict forward classification stay consistent. "Line-adjacent / same-anchor" maps onto its existing rules: same anchor → same `block` name → intersect; adjacent/overlapping line ranges → range overlap → intersect. (If we later want a configurable adjacency gap so ranges N lines apart still escalate, that extends `ranges_overlap`; out of scope here — disjoint declared ranges are treated as additive.)

**Alternative considered — diff the working trees:** compute actual hunk line ranges from each worktree's diff and compare those instead of declared regions. Rejected: the detector is a broker subsystem with no worktree access, it would require parsing git output per tick, and it duplicates the declared-intent mechanism that `conflict-detector-fn-granularity` already established. Declared regions are the contract; agents that want precise classification declare regions (the coordination skill already teaches this).

### Decision: Conservative fallback when regions are absent

If either agent has **no active intent** for `file`, or its intent declares `file` at file level (`regions == None`), the detector **cannot prove** the hunks are disjoint and SHALL escalate as an `agent.question` (today's behaviour). Additive downgrade requires *both* sides to have declared regions. This preserves v0.5.0/v0.8.0 safety: file-level-only intents behave exactly as before.

### Decision: Record the downgrade so it neither repeats nor disappears

The additive case marks the triple as handled — reusing the existing `escalated` flag (the bit means "this triple's escalation decision has been made and acted on", whether the action was a question or a downgrade feedback). This means:
- A downgraded triple is **not re-evaluated** on subsequent ticks (no feedback spam, no later flip to a question while regions are unchanged).
- The overlap is **not silently dropped** — exactly one informational `agent.feedback` is emitted and the tracker still carries the triple until `sweep_in_flight_pairs` removes it when one agent stops touching the file.

The downgrade `agent.feedback` uses `from = "supervisor"` and an error string prefixed with `[conflict-detector]` (per the existing auto-emitted-message conventions), with text indicating "shared file, additive — resolve at merge", the file path, both agent_ids, and the disjoint regions.

### Relationship to `conflict-detector-fn-granularity`

This change is a *consumer* of that change's `regions` field and `regions_intersect` predicate, applied to a new decision point (in-flight escalation) that the v0.6.0 change did not touch. No requirement in `conflict-detector-fn-granularity` changes; its region parsing, four region kinds, and forward-conflict region rules are unchanged.

## Risks / Trade-offs

- **Risk:** an agent declares regions that don't match its actual hunks (lies or drifts), so a real collision is downgraded. → Mitigation: this is the same trust model the forward detector already relies on; the coordination skill forbids manufactured-narrow regions, and any true collision still surfaces at merge time (the downgrade feedback explicitly says "resolve at merge").
- **Risk:** "disjoint but close" ranges (e.g. 3 lines apart) merge cleanly today but might not after edits. → Mitigation: declared disjoint ranges are treated as additive by design; a future configurable adjacency gap can tighten this without a spec change to the additive/true split.
- **Trade-off:** the downgrade reuses the `escalated` flag rather than adding a new state. This keeps the data model unchanged and the sweep/expiry logic untouched, at the cost of the flag name no longer being literally precise (documented in code).
- **Risk:** behaviour change could affect existing in-flight tests that assume file-level escalation. → Mitigation: those tests use file-level intents (no regions) or no intents, which hit the conservative fallback and still escalate; new behaviour only triggers when both sides declared disjoint regions.

## Open Questions

- Should the additive downgrade be suppressible via a config flag (e.g. `downgrade_additive_in_flight = false` to force today's always-escalate)? Deferred — the conservative fallback already covers the no-regions case, and v0.9.0 prioritises unattended operation, which wants the downgrade on by default.
