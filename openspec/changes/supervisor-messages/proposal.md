## Why

The v0.3.0 broker has three message types for agent-to-agent coordination (status, artifact, blocked). v0.4.0 introduces a supervisor that tests and verifies agent work. The supervisor needs to communicate results back: "your work passed" (verified) or "your work has issues, fix them" (feedback). Without these message types, the supervisor has no structured channel to communicate with agents — it would have to resort to `tmux send-keys` which is fragile and unlogged.

## What Changes

- Add `agent.verified` variant to `BrokerMessage`:
  - Published by the supervisor after an agent's work passes tests and spec audit
  - Payload: `agent_id` (the verified agent), `verified_by` (supervisor's agent_id), `message` (optional summary)
  - Routing: broadcast to all agents (peers need to know a dependency is verified and safe to depend on)

- Add `agent.feedback` variant to `BrokerMessage`:
  - Published by the supervisor when an agent's work fails tests or spec audit
  - Payload: `agent_id` (the target agent), `from` (supervisor's agent_id), `errors` (list of error strings)
  - Routing: delivered to the target agent only (only the author needs to see the failures)

- Extend `MessageError` with validation for the new variants (non-empty `errors` list in feedback, non-empty `verified_by`)

- Extend `Display` impl for the new variants:
  - Verified: `[feat-errors] verified by supervisor`
  - Feedback: `[feat-errors] feedback from supervisor: 3 errors`

- Extend `status_label()`: Verified returns `"verified"`, Feedback returns `"feedback"`

- Extend delivery routing in `delivery.rs`:
  - `agent.verified` → broadcast to all agents (same as artifact)
  - `agent.feedback` → deliver to target agent only (same as blocked)

- Dashboard `status_symbol` already maps `"verified"` → `🟢` from v0.3.0 — no dashboard code change needed

- Update `coordination.md` skill template to document the new message types (agents should know they may receive verified/feedback messages when polling)

## Capabilities

### New Capabilities

<!-- None -->

### Modified Capabilities

- `broker-messages`: Add `Verified` and `Feedback` variants to `BrokerMessage`, new payload structs, validation, Display, status_label
- `message-delivery`: Extend routing rules for the two new message types
- `agent-skills`: Update coordination.md with documentation of verified and feedback messages that agents may receive when polling

## Impact

- **Modified files:**
  - `src/broker/messages.rs` — add `VerifiedPayload`, `FeedbackPayload`, two new enum variants, validation, Display, status_label
  - `src/broker/delivery.rs` — add two routing match arms in `publish_message`
  - `assets/agent-skills/coordination.md` — add documentation about receiving verified/feedback messages
- **No new files, no new modules, no new dependencies.**
- **Backward compatible:** existing v0.3.0 agents that don't understand the new message types will see them in their inbox but can ignore them. The JSON `type` field discriminates.
- **Dependents:** `supervisor-agent` (publishes verified/feedback), `spec-audit` (triggers feedback on failure)
