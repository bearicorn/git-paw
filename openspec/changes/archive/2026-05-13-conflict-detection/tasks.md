## 1. Configuration

- [x] 1.1 Add `ConflictConfig` struct in `src/config.rs` (or wherever `SupervisorConfig` lives) with fields `window_seconds: u64`, `warn_on_intent_overlap: bool`, `escalate_on_violation: bool`. Implement `Default` returning `120`/`true`/`true`.
- [x] 1.2 Add `pub conflict: ConflictConfig` to `SupervisorConfig` with `#[serde(default)]` so missing `[supervisor.conflict]` loads defaults. Derives match the existing `SupervisorConfig` derives (`Debug`, `Clone`, `Deserialize`, `Serialize`, etc.).
- [x] 1.3 Update the config-load tests to cover: section absent → defaults, full section → values match, partial fields → other fields default, pre-v0.5 config (no section) loads cleanly.
- [x] 1.4 Update `docs/src/configuration.md` (or the reference page) with the new sub-table, defaults, and what each field controls.

## 2. Tracker module

- [x] 2.1 Create `src/broker/conflict.rs` (promote to `src/broker/conflict/mod.rs` if it crosses ~400 lines).
- [x] 2.2 Implement `IntentRecord { agent_id, files: HashSet<String>, summary, received_at: Instant, valid_for: Duration }`.
- [x] 2.3 Implement `ConflictTracker` with:
  - `intents: HashMap<String, IntentRecord>` — keyed by agent_id
  - `current_files: HashMap<String, HashSet<String>>` — agent_id → currently-modified files (from `agent.status`)
  - `warned_intent_pairs: HashSet<(String, String)>` — lex-ordered pair dedupe for forward conflicts
  - `in_flight_pairs: HashMap<(String, String, String), InFlightPair>` — `(min_id, max_id, file)` → state
  - `warned_violations: HashSet<(String, String)>` — `(violator_id, file)` dedupe
- [x] 2.4 Implement helpers: `insert_intent(msg)`, `update_status(msg)`, `expire_stale_intents(now)`, `forward_overlaps(x_id) -> Vec<(y_id, Vec<file>)>`, `in_flight_overlaps() -> Vec<(min, max, file)>`, `ownership_violations(x_id) -> Vec<(file, owner_y_id)>`.
- [x] 2.5 Unit-test the tracker in isolation: insert/replace, TTL expiry, ordered-pair dedupe, in-flight cleanup when file leaves intersection, ownership lookup.

## 3. Detector loop

- [x] 3.1 Add `pub struct ConflictDetector` in `src/broker/conflict.rs` that owns a `ConflictTracker` (behind `Arc<Mutex<_>>` or equivalent matching the broker's existing concurrency style) and a handle to the broker `State` for publishing.
- [x] 3.2 Implement `ConflictDetector::start(state, config) -> JoinHandle` that spawns a tokio task. The task subscribes to a "publish completed" stream OR tails the message log via cursor (use whichever pattern matches the existing `delivery.rs` / `watcher.rs`).
- [x] 3.3 In the task, on each new message:
  - If `agent.intent`: update tracker, run forward-conflict check (gated by `warn_on_intent_overlap`), emit `agent.feedback` per overlap pair.
  - If `agent.status`: update tracker `current_files`, run in-flight check, run ownership check.
  - On every tick (or on a separate periodic timer): expire stale intents, sweep in-flight pairs whose file is no longer in intersection, run in-flight escalation check (those past `window_seconds` and not yet escalated).
- [x] 3.4 Implement emit helpers `emit_feedback(target_id, error_text)` and `emit_question(question_text)` that build `BrokerMessage::Feedback`/`Question` with `from = "supervisor"`, push through the existing publish API. Ensure these calls do NOT re-trigger the detector (re-entrancy guard — easiest: filter publishes from `agent_id = "supervisor"` out of detector input, OR use a sender flag).
- [x] 3.5 Wire the detector into broker startup in `src/broker/mod.rs`: spawn only when `[supervisor] enabled = true`; pass `config.supervisor.conflict.clone()` in. When the broker shuts down, the spawned task SHALL exit (drop the handle).

## 4. Detector unit tests

- [x] 4.1 Forward-conflict happy path: two agents publish overlapping intents → both receive `agent.feedback` with the right tag and file list.
- [x] 4.2 Forward-conflict dedupe: re-publishing same intent does not re-emit.
- [x] 4.3 Forward-conflict suppression: with `warn_on_intent_overlap = false`, no feedback emitted; tracker still updated.
- [x] 4.4 Forward-conflict on non-overlap: no warnings.
- [x] 4.5 Self-replace: agent re-publishes a different intent for itself; no self-conflict warnings emitted.
- [x] 4.6 TTL expiry: intent older than `valid_for_seconds` does not participate in subsequent overlap checks.
- [x] 4.7 In-flight initial warning: two agents' watcher status overlaps → both receive feedback.
- [x] 4.8 In-flight escalation: simulate elapsed window → `agent.question` published to `"supervisor"` inbox once.
- [x] 4.9 In-flight escalation dedupe: subsequent ticks while still overlapping do not re-emit.
- [x] 4.10 In-flight resolution: one agent's `modified_files` no longer contains the file → triple removed; no escalation.
- [x] 4.11 Ownership violation: violator gets feedback; with `escalate_on_violation = true`, supervisor gets question.
- [x] 4.12 Ownership violation suppression of escalation: with `escalate_on_violation = false`, feedback fires but no question.
- [x] 4.13 Ownership: file inside violator's *own* intent → no violation.
- [x] 4.14 Ownership: file not claimed by anyone → no violation.
- [x] 4.15 Ownership dedupe: second status from same violator on same file does not re-emit.
- [x] 4.16 Detector-disabled (supervisor off): no auto-emitted messages produced for any input.

## 5. Detector integration tests (broker round-trip)

- [x] 5.1 Spin a broker with supervisor enabled. Publish two `agent.intent` via HTTP; poll `feat-x` and `feat-y` inboxes; assert each receives an `agent.feedback` whose `payload.from = "supervisor"` and whose first error string starts with `[conflict-detector]`.
- [x] 5.2 Publish a status that overlaps with another agent's `current_files`; assert in-flight feedback round-trips. Advance simulated time past `window_seconds`; assert the supervisor inbox receives one `agent.question` containing `[conflict-detector]`.
- [x] 5.3 Publish a status from agent Y for a file inside agent X's intent (and outside Y's own intent); assert ownership feedback round-trips. With `escalate_on_violation` toggled off, assert no question hits the supervisor inbox.

## 6. Supervisor skill rewrite

- [x] 6.1 In `assets/agent-skills/supervisor.md`, locate the v0.4 `### Conflict detection` section ("Compare the `modified_files` arrays …"). Remove it.
- [x] 6.2 Insert (or extend the existing `### Watch peer intents` section from `forward-coordination`) with a new sub-section that documents the broker-side detector: forward / in-flight / ownership; the `[conflict-detector]` tag; the supervisor agent's role focus on `agent.question` escalations and repeat-violator follow-up; explicit "do not duplicate by manual `modified_files` comparison."
- [x] 6.3 Mirror the change into `docs/src/user-guide/supervisor.md` if such a chapter exists; otherwise add a paragraph to `docs/src/user-guide/coordination.md` summarizing the new auto-detection behaviour.

## 7. Skill-content tests

- [x] 7.1 Test: supervisor skill contains substring `[conflict-detector]`.
- [x] 7.2 Test: supervisor skill contains text indicating the broker auto-emits feedback for forward, in-flight, and ownership conflicts.
- [x] 7.3 Test: supervisor skill does NOT contain the v0.4 substring "Compare the `modified_files` arrays from every `agent.artifact` event" (or any equivalent manual-comparison instruction as a primary detection path).
- [x] 7.4 Existing supervisor-skill scenarios (Spec Audit, Watch peer intents from `forward-coordination`) still pass.

## 8. Release notes & MILESTONE upkeep

- [x] 8.1 Add a v0.5.0 release-notes bullet: "Broker auto-detects forward / in-flight / ownership conflicts when supervisor is enabled. Auto-emitted feedback is tagged `[conflict-detector]` and uses `from: \"supervisor\"`."
- [x] 8.2 Add a release-notes bullet: "Configurable via `[supervisor.conflict]` table — `window_seconds` (default 120), `warn_on_intent_overlap` (default true), `escalate_on_violation` (default true)."
- [x] 8.3 Mark MILESTONE.md item #15 (v0.4 supervisor.md "### Conflict detection") as resolved by this change once the rewrite ships.

## 9. Quality gates

- [x] 9.1 `just check` — fmt, clippy, all tests green.
- [x] 9.2 `just deny` — supply chain clean.
- [x] 9.3 No new `unwrap()` / `expect()` in non-test code added by this change. Lock acquisition uses the existing `expect("broker state lock poisoned")` precedent if applicable, but no new uses outside that documented pattern.
- [x] 9.4 `mdbook build docs/` succeeds.
- [x] 9.5 `openspec validate conflict-detection` passes (it does as of specs artifact).
