## Why

Wave 1 shipped the broker with stub delivery functions — `publish_message` panics with `todo!()`, `poll_messages` and `agent_status_snapshot` return empty. The HTTP endpoints accept and respond correctly, but nothing actually moves between agents. This change fills in those stubs so agents can discover each other's artifacts, report being blocked, and poll for messages — completing the peer-to-peer coordination loop that is v0.3.0's central feature.

## What Changes

- Replace the `todo!()` body of `publish_message` with real logic that:
  - Updates the sender's `AgentRecord` in `BrokerStateInner` (status, last_seen, last_message)
  - For `agent.status` — updates the agent's record only (no routing to other agents)
  - For `agent.artifact` — broadcasts the message to every other agent's inbox so peers can discover completed work
  - For `agent.blocked` — enqueues the message in the `from` agent's inbox (targeted delivery to the agent that can unblock the sender)
- Replace the empty-return body of `poll_messages` with real logic that drains and returns all queued messages for the given `agent_id`
- Replace the empty-return body of `agent_status_snapshot` with real logic that returns the current `AgentRecord` for every known agent, converted to `AgentStatusEntry` values the dashboard can render
- Add fields to `BrokerStateInner` as needed to support the above (e.g. ensure `queues` and `agents` HashMaps are used correctly)
- Add unit tests for all delivery logic including edge cases (publish to unknown agent, poll with no messages, artifact broadcast skips the sender)

## Capabilities

### New Capabilities

- `message-delivery`: In-memory message queuing and routing logic for the v0.3.0 broker. Covers the three delivery functions (`publish_message`, `poll_messages`, `agent_status_snapshot`), the routing rules per message type, queue drain semantics, agent record lifecycle, and the broadcast-vs-targeted delivery distinction.

### Modified Capabilities

<!-- None — this change fills in stub bodies without changing signatures or HTTP behavior -->

## Impact

- **Modified file (owned by this change — body fill):** `src/broker/delivery.rs` — replaces stub bodies with real implementations. Function signatures, names, and visibility MUST NOT change.
- **Modified file (additive):** `src/broker/mod.rs` — may add new methods on `BrokerState` or new fields to `BrokerStateInner` to support delivery. MUST NOT change existing public API surface.
- **No other files modified.** `src/broker/server.rs` is frozen. `src/broker/messages.rs` is frozen. No new modules, no new dependencies.
- **Depends on:** `message-types` (for `BrokerMessage` and its variants), `http-broker` (for `BrokerState`, `BrokerStateInner`, `AgentRecord`, `AgentStatusEntry`, and the three delivery function signatures).
- **Dependents:** `broker-integration` (Wave 2) — needs delivery to work for end-to-end sessions. `dashboard-tui` — reads `agent_status_snapshot` for the status table.
- **No CLI surface changes.** No new commands, flags, or config fields.
- **Merge order:** `peer-messaging` MUST merge first in Wave 2 (per the TODO Phase 6 plan) because `broker-integration` depends on delivery working for the end-to-end lifecycle.
