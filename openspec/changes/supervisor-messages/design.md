## Context

This change extends the v0.3.0 broker message system. The existing `BrokerMessage` enum has three variants (Status, Artifact, Blocked) with established patterns for payload structs, serde tagging, validation, Display, and delivery routing. Adding two more variants follows the exact same patterns — no new architecture.

The supervisor is the only entity that publishes verified/feedback messages. Coding agents receive them but don't publish them. This is enforced by convention (the supervisor's skill template tells it to publish, the coding agents' templates don't), not by code — the broker accepts verified/feedback from any `agent_id`.

## Goals / Non-Goals

**Goals:**

- Add two message variants following established patterns
- Keep routing rules consistent: broadcast for "everyone needs to know" (verified), targeted for "only one agent needs this" (feedback)
- Make the changes purely additive — no existing behavior changes

**Non-Goals:**

- Authentication on who can publish verified/feedback (any agent can; convention enforces this)
- Retry logic for feedback (if the agent doesn't respond, the supervisor escalates to human)
- Verified/feedback affecting the dashboard's status color mapping (already handled — `"verified"` already maps to 🟢)

## Decisions

### Decision 1: VerifiedPayload is minimal

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedPayload {
    /// Who verified this agent's work (typically "supervisor").
    pub verified_by: String,
    /// Optional summary message.
    pub message: Option<String>,
}
```

**Why:**
- The `agent_id` on the envelope identifies which agent was verified
- `verified_by` identifies the verifier (future: could be another agent, not just the supervisor)
- `message` is optional for human-readable context ("all 12 tests pass, spec audit clean")
- No `test_results` struct — that level of detail goes in the broker log, not the message payload

### Decision 2: FeedbackPayload carries a list of error strings

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackPayload {
    /// Who sent the feedback (typically "supervisor").
    pub from: String,
    /// List of error messages the agent should address.
    pub errors: Vec<String>,
}
```

**Why:**
- `errors` as `Vec<String>` is simple and actionable — each string is one issue to fix
- The agent reads the errors, fixes them, and re-publishes `agent.artifact`
- `from` identifies the sender (matches `BlockedPayload.from` pattern)
- No structured error types — the errors come from test output and spec audit, which are free-form text

### Decision 3: Routing follows existing patterns exactly

| Type | Routing | Same as |
|---|---|---|
| `agent.verified` | Broadcast to all agents except sender | `agent.artifact` |
| `agent.feedback` | Deliver to target agent only | `agent.blocked` |

**Why:**
- Verified broadcasts because peers need to know a dependency is safe (e.g. Agent B is blocked on Agent A; when A is verified, B sees it and can proceed with confidence)
- Feedback is targeted because only the agent with failures needs to see them
- Reusing existing routing patterns (broadcast/targeted) means no new delivery logic — just two more match arms

### Decision 4: Display format follows the established convention

```
[feat-errors] verified by supervisor
[feat-errors] verified by supervisor — all tests pass
[feat-errors] feedback from supervisor: 3 errors
```

**Why:**
- Same `[agent_id] type: details` format as status/artifact/blocked
- Verified shows `verified_by` and optional message after `—`
- Feedback shows error count (not the full errors — those are in the payload for the agent, the Display is for the dashboard/log)

### Decision 5: coordination.md documents receiving, not sending

The coding agents' skill template should document that they may receive verified/feedback messages when polling, and how to interpret them:

```markdown
### Messages you may receive

- `agent.verified` — your work has been verified by the supervisor. No action needed.
- `agent.feedback` — your work has issues. Read the `errors` list, fix them, and
  re-publish your `agent.artifact` when done.
```

The supervisor's own skill template (owned by `supervisor-agent` change) documents how to *send* these messages.

## Risks / Trade-offs

- **No authentication** → Any agent can publish `agent.verified` for any other agent. A misbehaving agent could falsely verify itself. **Mitigation:** acceptable for v0.4.0. The supervisor is the only entity with the skill template instructions to publish these. v2.0 A2A protocol could add authentication.

- **Feedback errors are free-form strings** → No structured format means agents can't programmatically parse error types. **Mitigation:** agents are AI — they read natural language. Structured error types would be premature optimization.

## Migration Plan

No migration. Two new enum variants are purely additive. Existing v0.3.0 agents that receive unknown message types in their inbox will see valid JSON with an unrecognized `type` field — they can ignore it. The broker itself handles all five types.
