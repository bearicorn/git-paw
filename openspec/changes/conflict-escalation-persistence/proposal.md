## Why

A human who adjudicates an in-flight conflict is asked about it again after the next
commit, indefinitely, and nothing they can do records the decision.

The detector is not missing dedup. `broker-conflict-detection`'s *In-flight conflict
detection* already requires "Escalation is emitted only once per triple". The defect is
that the dedup key is a triple whose lifetime any commit destroys: the same requirement
says a triple is removed from the tracker when its file "no longer appears in the
intersection" of the two agents' `modified_files` — "the conflict has resolved without
escalation". A commit empties `modified_files`, so the triple is dropped, and the next
status republishes it as a brand-new triple with `first_seen` reset.

Observed verbatim in the dogfood broker log: seq 65 `feat-worker-session-id-capture`
publishes `modified_files = []` (its commit), seq 66 it republishes with
`src/commands/supervisor.rs`, seq 71 the window elapses and the identical escalation fires
again — 353s after seq 47, and after the orchestrator had already adjudicated it and
published `agent.feedback` to both agents. **Both escalations are spec-conformant.**

The workflow requires agents to commit as they go, so the conflict clock resets precisely
when an agent is making the most progress. The cost falls hardest on an external
orchestrator — the v0.18.0 direction — because each re-fire consumes a full turn to
triage before discovering it is one already answered.

## What Changes

- An escalation decision SHALL survive its triple leaving and re-entering the tracker. A
  commit, or any transient emptying of `modified_files`, SHALL NOT reset the conflict's
  escalation state or its `first_seen` clock.
- A decision record SHALL be keyed on the conflict's durable identity — the agent pair and
  the file — rather than on tracker residency, and SHALL be cleared only when the conflict
  is genuinely over or its region declarations materially change.
- The detector SHALL accept an **acknowledgement** that suppresses re-escalation for a
  given pair and file until the declarations materially change, so an orchestrator or
  human who has adjudicated a conflict can record that fact. Today there is no way to say
  "I have answered this."
- Re-escalation after a material change SHALL remain possible: this narrows repetition, it
  does not silence the detector.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `broker-conflict-detection`: *In-flight conflict detection* — escalation state persists
  across transient tracker removal, and an acknowledgement suppresses re-escalation.

## Impact

- `src/broker/conflict.rs` — the in-flight tracker's removal path and the escalation
  decision record; these become separately-lived rather than one lifetime.
- `src/broker/messages.rs` — if acknowledgement is carried as a message, this touches the
  `BrokerMessage` enum, which per AGENTS.md §7 ripples into `agent_id()` /
  `status_label()` / `validate()`, `dashboard/broker_log.rs`
  (`message_bit` / `type_short` / `derive_summary`), `broker/learnings.rs::observe()`,
  and the semantic routing in `broker/delivery.rs`. **Prefer reusing an existing variant**
  — the detector already consumes `agent.feedback`, and `approval-auto` has precedent for
  "Approval gate reuses existing broker variants". Adding a variant is the fallback, and
  if taken, every site above must be scoped before implementation starts.
- `.git-paw/scripts/sweep.sh` / `assets/scripts/sweep.sh` — an orchestrator needs a way to
  send the acknowledgement; the helper is where that belongs.
- **Delta coordination**: `broker-conflict-detection` is not touched by any other
  in-flight v0.17.0 change.
