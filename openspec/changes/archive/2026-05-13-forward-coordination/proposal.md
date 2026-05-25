## Why

v0.4.0 dogfood showed agents (and the coordination skill) defaulting to tight pairwise check-ins instead of the parallel-by-default model the broker was designed for. Agents need a way to declare *intent* — "I'm about to touch these files" — so the supervisor and peers can spot collisions *before* edits happen, not after a merge conflict. Today there is no protocol for this: agents either over-coordinate via `agent.blocked`/`agent.question` or silently race on shared files.

This change adds the plumbing — a new `agent.intent` broker message and a coordination skill that teaches forward-looking conflict awareness. The conflict-*detection* algorithm (forward, in-flight, ownership violation) lands in the next change (`conflict-detection`); this one is just protocol and instructions.

## What Changes

- Add a new `BrokerMessage::Intent` variant with serde tag `agent.intent` and an `IntentPayload { files: Vec<String>, summary: String, valid_for_seconds: u64 }`. Validation: `files` non-empty, every entry a non-empty path, `summary` non-empty, `valid_for_seconds` > 0.
- Broadcast `agent.intent` to all peer inboxes (same delivery shape as `agent.artifact` and `agent.verified`). Sender's own inbox is excluded.
- Rewrite the embedded `coordination.md` skill around two phases — *Before you start editing* (publish `agent.intent`, poll once for warnings, decide) and *While you're editing* (re-publish on scope growth, ask peers via `agent.question` on suspected overlap). Existing sections (automatic status, blocked, artifact, cherry-pick, verified/feedback messages) are preserved.
- Update the embedded `supervisor.md` skill with a brief "Watch peer intents" pointer noting that `agent.intent` messages arrive in the supervisor inbox and may be acted on. The full supervisor algorithm (warnings, escalation windows, ownership violations) is deferred to the `conflict-detection` change.
- Update the in-tree user-guide chapter `docs/src/user-guide/coordination.md` to match the new skill content.
- No new CLI flags, no new config keys, no new endpoints. The existing `/publish` and `/messages/{id}` surface carries `agent.intent` unchanged.

Not in scope (deferred to `conflict-detection`):
- Supervisor's overlap-detection logic, escalation windows, and `[supervisor.conflict]` config.
- Watcher-driven in-flight conflict detection.
- `agent.intent` TTL expiry on the supervisor side.

## Capabilities

### New Capabilities
*(none — this change extends existing capabilities)*

### Modified Capabilities
- `broker-messages`: add the `Intent` variant, `IntentPayload` shape, validation rules, `Display` / `status_label()` / `agent_id()` behaviour for the new variant.
- `message-delivery`: add an "Intent messages are broadcast to all other agents" requirement matching the `Artifact`/`Verified` broadcast pattern.
- `agent-skills`: update the embedded coordination skill to include `agent.intent` publish/poll patterns and the *Before/While editing* structure; relax the v0.4 spec scenario that asserts a stale `${GIT_PAW_BROKER_URL}` substring (the actual file uses `{{GIT_PAW_BROKER_URL}}`); add a "Watch peer intents" pointer to the embedded supervisor skill.

## Impact

**Code**:
- `src/broker/message.rs` (or wherever `BrokerMessage` lives) — new variant + payload struct + validation + `Display` + helper methods.
- `src/broker/delivery.rs` (or equivalent) — broadcast routing for the new variant.
- `assets/agent-skills/coordination.md` — substantial rewrite.
- `assets/agent-skills/supervisor.md` — small addition.
- `docs/src/user-guide/coordination.md` — mirror skill content.

**Tests**:
- `tests/broker_messages.rs` (or unit tests in the message module) — round-trip, validation, display, helper-method scenarios for the new variant.
- `tests/message_delivery.rs` — broadcast scenario for `Intent`, including sender-exclusion.
- Skill content tests in `tests/agent_skills.rs` (or similar) — assert new sections exist; drop or update the stale `${GIT_PAW_BROKER_URL}` scenario.

**Backward compatibility**: fully additive on the wire. v0.4 agents that don't know `agent.intent` will see the message type during polling; the existing `Unknown message type is rejected` scenario applies only to *parsing inbound* — agents that simply ignore unknown types in their local handling are fine. The embedded skill is replaced wholesale; user overrides under `<config_dir>/git-paw/agent-skills/coordination.md` continue to win via the existing resolution order, so users who forked the v0.4 skill keep their version until they merge upstream changes manually (release-notes call-out).

**Dependencies**: none added.

**Mismatches surfaced (not fixed by this change — flagged for separate work)**:
1. The embedded `supervisor.md` skill's `agent.verified` and `agent.feedback` curl examples use payload fields `target`/`result`/`notes`/`message`, but `broker-messages` spec defines those payloads as `verified_by`/`message` and `from`/`errors`. The skill diverges from the wire format. Belongs in `v040-hardening` (or its own fix).
2. `agent.question` is referenced by `supervisor.md`, `coordination.md`, and `boot-block-format/spec.md` but is not defined as a `BrokerMessage` variant in `broker-messages/spec.md`. Either a phantom type or a shipped-but-unspec'd variant. Worth verifying against the source code in `v040-hardening`.
3. `agent-skills/spec.md` scenario "Coordination skill retains polling reference" asserts the substring `${GIT_PAW_BROKER_URL}/messages/{{BRANCH_ID}}` but the actual file uses `{{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}` (since the v0.3.0 `${VAR}` → `{{VAR}}` migration). The spec is stale. This change can fix it as a side effect since it touches the same scenario set.
