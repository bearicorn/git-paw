## Why

The broker's in-flight conflict detector escalates **any** same-file overlap between two agents to the human as an `agent.question` once the `window_seconds` window elapses. It has no notion of *additive vs conflicting* hunks. In the v0.8.0 dogfood, 3 of 4 such escalations were purely additive (different structs in `src/config.rs`; shared functions with hunks 75–112 lines apart in `src/main.rs`; `supervisor.md` hunks at L291 vs L454) — all clean merges that needed no human input. Only one (`coordination.md`, both inserting at anchor L82) was a true collision. In an unattended wave these false alarms stall the run waiting on input that is not needed.

## What Changes

- Before escalating an in-flight same-file overlap to a human `agent.question`, the detector consults the two agents' active-intent **region declarations** for the overlapping file (the `regions` already carried on `agent.intent` per `conflict-detector-fn-granularity`) and classifies the overlap as **additive** (disjoint, well-separated regions) or **true** (overlapping or line-adjacent ranges / same insertion anchor / same named region).
- **True collisions** SHALL continue to escalate as an `agent.question` to the supervisor inbox (unchanged behaviour).
- **Additive overlaps** SHALL be downgraded to an informational `agent.feedback` ("shared file, additive — resolve at merge") and SHALL NOT escalate to the human.
- The downgrade path still **records** the overlap as handled (the triple is marked escalated/resolved so it is not re-evaluated every tick), so an additive overlap is never silently dropped and never re-escalates.
- When neither agent declared regions for the shared file (file-level intents only, or no active intent), the detector SHALL fall back to escalating as an `agent.question` — the conservative, current behaviour — since it cannot prove the hunks are disjoint.

This refines, and does not replace, the v0.6.0 region-granularity work: that change taught *forward*-conflict detection to use regions; this change teaches the *in-flight escalation decision* to use the same regions.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `conflict-detection`: the **In-flight conflict detection** requirement changes — the window-elapsed escalation step now classifies the overlap additive-vs-true using the agents' declared regions and only escalates true collisions as an `agent.question`; additive overlaps downgrade to an informational `agent.feedback` and are recorded (not re-escalated, not dropped).

## Impact

- `src/broker/conflict.rs`: the in-flight escalation path (`take_due_escalations` / the tick that consumes it) gains an additive-vs-true classification step that reads `IntentRecord.files` regions for the overlapping file; a new informational `agent.feedback` variant text for the additive-downgrade case.
- No wire-format change: `agent.intent` `regions` already exist (`conflict-detector-fn-granularity`); `agent.feedback` / `agent.question` shapes are unchanged.
- No config change: gated by the existing `[supervisor.conflict] window_seconds`; behaviour is identical when `[supervisor] enabled = false`.
- Tests in `src/broker/conflict.rs` (unit) covering disjoint → downgrade, overlapping/same-anchor → escalate, and downgrade-records-the-overlap.
