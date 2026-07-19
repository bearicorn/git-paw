## Why

The v0.11.0 unattended-operation dogfood (`session-learnings.md`, 2026-07-14/15)
surfaced two broker-delivery defects that corrupt the supervisor's view of agent
traffic:

- **Cursor regression (30× observed, hid real traffic):** `poll_messages`
  returns `last_seq = 0` on an empty poll. A client that advances with
  `cursor = last_seq` therefore snaps back to `0` and re-reads its inbox from the
  start every cycle — re-serving the same `Question` ~30× while never surfacing
  the `Artifact` and `VerifyNow` messages queued behind it in the log.
- **Question re-publish flood (7× observed):** a blocked agent whose `Question`
  is never acknowledged re-publishes the identical payload every poll, filling
  the supervisor inbox with duplicates.

## What Changes

- **MODIFY** the `message-delivery` requirement *"Cursor-based message polling"*:
  the returned cursor SHALL be monotonic — the greater of `since` and the highest
  returned sequence — so it never regresses to `0` and a client that stores
  `cursor = last_seq` can never re-read already-seen messages.
- **ADD** a `message-delivery` requirement *"Duplicate question suppression"*:
  the broker SHALL suppress enqueuing a `Question` whose `(agent_id, payload)` is
  identical to one already resident (unanswered) in the supervisor inbox, so a
  re-polling blocked agent cannot flood it.
- **E2E:** a supervisor inbox containing a `Question` followed by an `Artifact`
  SHALL fully drain across successive polls — the `Question` never wedges the
  cursor.
- Complementary (agent-side, no capability spec here): the `coordination`
  skill guidance suppresses client re-publish when a matching `agent.answer`
  (shipped v0.11.0) is already in the asker's inbox.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `message-delivery`: cursor monotonicity (`poll_messages` return value) and
  broker-side deduplication of identical unanswered `Question` payloads.

## Impact

- **Code:** `src/broker/delivery.rs` — `poll_messages` cursor return; `Question`
  routing dedup check against the resident supervisor inbox.
- **Tests:** new unit test for the monotonic cursor + empty-poll case; new e2e
  draining a mixed (`Question` → `Artifact`) inbox; unit test for question dedup.
- **Docs:** the `poll_messages` rustdoc cursor contract.
- No wire-format change; `since == 0` still returns the full inbox (backward
  compatible for existing clients).
