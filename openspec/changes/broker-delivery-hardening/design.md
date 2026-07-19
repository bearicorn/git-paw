## Context

Two delivery defects surfaced in the v0.11.0 unattended dogfood, both in
`src/broker/delivery.rs`. `poll_messages` returns `last_seq = 0` on an empty
poll; a client (`sweep.sh`) that advances with `cursor = last_seq` snaps to `0`
and re-reads from the start, re-serving a resident `Question` and hiding the
`Artifact`/`VerifyNow` messages behind it. Separately, a blocked agent
re-publishes an unanswered `Question` every poll, flooding the supervisor inbox.

## Goals / Non-Goals

**Goals:**
- Make the poll cursor monotonic so `cursor = last_seq` is always safe.
- Stop identical unanswered questions from piling up in the supervisor inbox.
- Prove a `Question` cannot wedge a mixed inbox (e2e).

**Non-Goals:**
- No wire-format change; `since == 0` still returns the whole inbox.
- No change to routing targets (who receives what).
- The client-side re-publish suppression (asker checks its inbox for a matching
  `agent.answer` before re-asking) is complementary skill guidance, not part of
  this capability spec.

## Decisions

- **Cursor = `max(since, highest_returned)`.** Advancing with `cursor = last_seq`
  becomes idempotent and never regresses. Alternative considered: a server-side
  per-agent "delivered" cursor (ack-on-deliver) — rejected because it adds write
  state to a read-only poll and breaks the non-destructive re-read contract.
- **Dedup keyed on `(agent_id, question text)` resident in the supervisor
  inbox.** Simplest correct interim matching the dogfood learning ("dedup
  identical question payloads"). Alternative: answer-aware suppression — better
  placed agent-side (the asker owns the knowledge that its question was
  answered), so it rides the `coordination` skill, not the broker.

## Risks / Trade-offs

- [Dedup drops a question the agent legitimately wants to re-ask] → Mitigation:
  suppression is scoped to a *still-resident* identical question; once the
  supervisor drains or answers it, a re-ask enqueues normally.
- [Monotonic cursor changes the `last_seq = 0`-on-empty behaviour] → Backward
  compatible on the wire: `since == 0` still returns the full inbox, so clients
  that never assigned `cursor = last_seq` are unaffected; clients that did are
  fixed.

## Open Questions

- Dedup residency scope — entire undrained inbox (recommended, no timer) vs a
  bounded time window. Resolve at apply; default to inbox-residency.
