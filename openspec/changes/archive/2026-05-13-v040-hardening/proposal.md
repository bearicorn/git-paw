## Why

A code-and-spec scan during v0.5.0 planning surfaced a small, concrete set of v0.4.0 hardening items: three panic-surface call-sites in non-test code, two wrong wire-format payload examples in the embedded supervisor skill, and missing spec coverage for the `Question` broker variant that already ships in code. Each is independently small; together they're the v0.4 follow-ups that were called out as MILESTONE drift items #12, #13, and informed the original "v0.4 deep review backlog" framing in the milestone.

This change closes them as a single focused pass before v0.6.0's MCP work begins, so the broker's wire surface and skill examples are accurate going into the next release cycle.

## What Changes

**Panic-surface fixes.** The panic-surface scan found exactly three fixable non-test sites:
- `src/agents.rs:343` and `src/agents.rs:357` — `regex::Regex::new(...).unwrap()` on hardcoded literal patterns. These cannot fail at runtime, but the success criterion ("zero `unwrap()` / `expect()` in non-test code outside `OnceLock` / `LazyLock` initialisation") requires lifting them into static `LazyLock<Regex>` storage. Same behaviour, no new error path.
- `src/git.rs:245` — `worktree_path.to_str().unwrap()` followed by use as a `Command` argument. Replace with `worktree_path.as_os_str()` directly; `Command::args` accepts `OsStr`, so no UTF-8 conversion is needed. Eliminates the panic without introducing an error path.
- `src/broker/mod.rs:131, 140` — `expect("broker state lock poisoned")` is the project's documented lock-acquisition precedent (per `CLAUDE.md` quality-gate guidance). NOT touched.

**Skill wire-format fixes** (MILESTONE drift item #12). The embedded `assets/agent-skills/supervisor.md` skill's curl examples for `agent.verified` and `agent.feedback` use payload field names that do not match the wire format defined in `openspec/specs/broker-messages/spec.md`. An agent following the examples literally would publish messages that fail validation:
- Line 30 (`agent.verified`): payload uses `{"target":"...","result":"pass","notes":""}`. The spec's `VerifiedPayload` requires `verified_by: String` and `message: Option<String>`. Fix to `{"verified_by":"supervisor","message":"<summary>"}`.
- Line 38 (`agent.feedback`): payload uses `{"target":"...","message":"..."}`. The spec's `FeedbackPayload` requires `from: String` and `errors: Vec<String>`. Fix to `{"from":"supervisor","errors":["<error 1>","<error 2>"]}`.
- Update related skill prose where it references the wrong field names (e.g. line 95's `result:"pass"` reference becomes `verified_by:"supervisor"`).
- Mirror updates into `docs/src/user-guide/coordination.md` (or wherever the skill content is doc-mirrored).

**`agent.question` spec coverage** (MILESTONE drift item #13). The `Question` variant exists in `src/broker/messages.rs:158-163` with `QuestionPayload`, validation (`MessageError::EmptyQuestionField`), `Display` formatting, and helper methods. Delivery is implemented in `src/broker/delivery.rs:130`. None of this is documented in `openspec/specs/broker-messages/spec.md` or `openspec/specs/message-delivery/spec.md`. The spec needs to catch up to shipped code:
- Add `Question message variant` requirement to `broker-messages` covering the variant + payload (`question: String`).
- Add `Validation for Question variant` requirement (`question` non-empty after trim, plus `agent_id` slug rules).
- Add `Display for Question variant` requirement matching the existing format.
- Add `status_label for Question variant` and `agent_id for Question variant` requirements matching the existing helpers.
- Add `Question messages are delivered to <target>` requirement to `message-delivery` matching the existing routing logic in `delivery.rs`.

Not in scope:
- Any new behaviour. This change closes drift between code, specs, and skills — no functional changes to the supervisor flow, broker, or skills beyond what already ships.
- Lock-acquisition `expect()` calls at `broker/mod.rs:131, 140`. Documented as the allowed pattern in `CLAUDE.md`.
- Larger code-quality cleanups (rename audit, doc-comment audit, etc.). v0.5.0 hardening is bounded to the three concrete findings above.

## Capabilities

### New Capabilities
*(none)*

### Modified Capabilities
- `broker-messages`: add the `Question` variant requirement set (variant, payload, validation, Display, status_label, agent_id helpers).
- `message-delivery`: add the routing requirement for `Question` matching the existing implementation.
- `agent-skills`: correct the embedded `supervisor.md` skill's `agent.verified` and `agent.feedback` curl examples and surrounding prose so the wire format matches `broker-messages`.

## Impact

**Code**:
- `src/agents.rs` — replace two `Regex::new(...).unwrap()` calls with `LazyLock<Regex>` statics. Add `use std::sync::LazyLock;` if not already imported.
- `src/git.rs:245` — replace `worktree_path.to_str().unwrap()` with `worktree_path.as_os_str()`. Adjust the `Command::args(...)` call's input type accordingly (the slice must allow `&OsStr` entries — likely needs `args(["worktree", "remove", "--force"]).arg(worktree_path.as_os_str())` rewrite).
- `assets/agent-skills/supervisor.md` — fix two curl-example payloads and any prose referencing the wrong field names.
- `docs/src/user-guide/coordination.md` (or wherever the skill is mirrored) — sync the corrections.

**Tests**:
- Confirm `regex::Regex::new` `LazyLock` initialisation doesn't break under concurrent first-use (the existing `agents.rs` tests should already cover the relevant code paths; verify they still pass).
- Add a test asserting `worktree_path` containing non-UTF-8 bytes can be passed to the worktree-remove command without panic.
- Add `agent.question` round-trip test (variant, payload, validation, Display) — though the variant already round-trips in code, an explicit test backstops the new spec scenarios.
- Add `agent.question` delivery test confirming the routing target matches the spec.
- Update skill-content tests so the existing assertions catch the corrected `verified_by` / `from` / `errors` field names rather than the wrong ones.

**Backward compatibility**: all changes are bug fixes or spec-catch-up; no user-facing behaviour changes. Wire format is unchanged (the spec is being aligned with shipped code, not the other way around). User-forked supervisor skills with the wrong examples continue to fail validation as they do today; the upstream skill becomes correct.

**Mismatches resolved**:
- MILESTONE drift item #12: skill ↔ wire-format alignment — resolved by this change.
- MILESTONE drift item #13: `agent.question` spec coverage — resolved by this change.
- MILESTONE drift item #14 (stale `${GIT_PAW_BROKER_URL}` assertion) was already folded into `forward-coordination`; resolved by that change.
