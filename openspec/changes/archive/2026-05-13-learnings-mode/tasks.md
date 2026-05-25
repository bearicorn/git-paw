## 1. Configuration

- [x] 1.1 Add `learnings: bool` field to `SupervisorConfig` in `src/config.rs` with `#[serde(default)]` so missing fields default to `false`.
- [x] 1.2 Add `LearningsConfig { flush_interval_seconds: u64 }` struct with `Default` returning `60`. TOML key `[supervisor.learnings_config]`.
- [x] 1.3 Add `pub learnings_config: LearningsConfig` to `SupervisorConfig` with `#[serde(default)]`.
- [x] 1.4 Update config-load tests to cover: section absent → defaults, `learnings = true` only → flush 60s default, custom `flush_interval_seconds` honoured, pre-v0.5 configs load with `learnings = false`.
- [x] 1.5 Update `docs/src/configuration.md` with the new fields.

## 2. Aggregator subsystem

- [x] 2.1 Create `src/broker/learnings.rs` (or `src/broker/learnings/mod.rs` if it grows beyond ~500 lines).
- [x] 2.2 Define `LearningsAggregator` struct holding the trackers from design D2: `pending_blocks`, `feedback_counts`, `conflict_events`, `permission_counts`, `last_flushed_md_idx`, `session_start`. Wrap in `Arc<Mutex<_>>` matching broker concurrency style.
- [x] 2.3 Implement input methods called from the publish-completed event stream:
  - `record_blocked(agent_id, blocked_on, ts)` — populates `pending_blocks`.
  - `record_artifact(agent_id, ts)` — checks for matching pending block, computes stuck duration, accumulates a learning event, clears the entry.
  - `record_feedback(target_agent_id)` — increments `feedback_counts[target]`.
  - `record_verified(target_agent_id)` — emits a recovery-cycles entry if count ≥ 1, clears the entry.
  - `record_detector_message(message)` — classifies `[conflict-detector]`-tagged feedback / question into one of the four conflict categories using the agent → SpecEntry mapping.
  - `record_auto_approve(command_class)` — increments `permission_counts[class]`.
- [x] 2.4 Implement `flush(state)` — appends accumulated entries to the markdown file via the writer, updates `last_flushed_md_idx`. The aggregator does NOT publish to the broker — the markdown file is the only output sink.
- [x] 2.5 Implement `flush_at_shutdown(state)` — same as `flush` but also includes any open stuck-duration entries marked unresolved.

## 3. Markdown writer

- [x] 3.1 Implement `write_session_section(file_path, session_start, learnings)` that opens `.git-paw/session-learnings.md` with `O_APPEND`, writes the H2 header on first call of the session, then writes H3 sections for any non-empty category groupings.
- [x] 3.2 Group learnings by category at flush time per design D7. Empty categories are skipped (no placeholder).
- [x] 3.3 Format each bullet per the design's example.
- [x] 3.4 Test: empty learnings list produces no file content.
- [x] 3.5 Test: H2 header appears once per session, with ISO-8601 UTC timestamp matching the regex from the spec scenario.
- [x] 3.6 Test: subsequent flushes within a session append under the existing H2 (no duplicate H2).
- [x] 3.7 Test: subsequent SESSION (different aggregator instance) starts a new H2 section, prior content unchanged.

## 4. Aggregator wiring

- [x] 4.1 In `src/broker/mod.rs`, conditionally start the aggregator: when `cfg.supervisor.is_some_and(|s| s.enabled && s.learnings)`. Pass `cfg.supervisor.learnings_config.flush_interval_seconds` (or default 60) into the timer.
- [x] 4.2 Hook the aggregator into the publish-completed subscription, alongside the conflict detector. Order should match design D1 (after delivery completes).
- [x] 4.3 On broker shutdown, drive `flush_at_shutdown` before the task exits.
- [x] 4.4 The aggregator does NOT call back into the publish API (no broker variant, no re-entrancy concerns).

## 5. Aggregator unit tests

- [x] 5.1 Stuck-duration: `agent.blocked` followed by `agent.artifact` from the same agent → records stuck-duration entry with correct `duration_seconds`, marked resolved. Markdown bullet appears under "Where agents got stuck".
- [x] 5.2 Stuck-duration unresolved: `agent.blocked` with no following artifact, then shutdown flush → records unresolved entry.
- [x] 5.3 Recovery-cycles: 3 `agent.feedback` to X then `agent.verified` for X → records recovery-cycles entry with `count: 3`.
- [x] 5.4 Recovery-cycles zero: 0 feedback then verified → no entry recorded.
- [x] 5.5 Forward-conflict-intra-spec: detector-tagged feedback to two agents in the same `SpecEntry` family → records `forward-conflict-intra-spec` entry.
- [x] 5.6 Forward-conflict-cross-spec: detector-tagged feedback to two agents in different `SpecEntry` families → records `forward-conflict-cross-spec` entry naming both spec ids.
- [x] 5.7 In-flight-conflict and ownership-violation classification — one test each.
- [x] 5.8 Permission-pattern threshold: 23 hits for `cargo check` → records entry; 2 hits for `git status` → no entry; counter persists across flushes (5+ later → records).
- [x] 5.9 No-learnings session: aggregator runs but no events arrive → flush produces no markdown writes.
- [x] 5.10 Aggregator does NOT publish any broker variant — verify by inspecting the publish call count after a populated flush.

## 6. Integration tests

- [x] 6.1 Start a broker with `[supervisor] enabled = true, learnings = true`. Publish a fixture sequence of events that triggers each deterministic signal. Wait for one flush interval (or trigger flush directly via test-only API).
- [x] 6.2 Assert `.git-paw/session-learnings.md` (in tempdir) contains the expected H2 + H3 sections + bullets.
- [x] 6.3 Assert the broker's message log does NOT contain any `agent.learning` (or other new) variant entries — the aggregator is file-only.
- [x] 6.4 Stop the broker. Assert the shutdown flush captured any remaining open events.
- [x] 6.5 Restart with the same `.git-paw/` and a new session. Assert the previous session's H2 section is unchanged and a new H2 appears at the file end.

## 7. Documentation

- [x] 7.1 Add a "Learnings Mode" section to the supervisor user-guide chapter: how to enable, what gets tracked, where to find the markdown file. Note that v0.6.0 will add programmatic access via MCP (no broker variant in v0.5.0).
- [x] 7.2 Document `[supervisor] learnings = true` and `[supervisor.learnings_config] flush_interval_seconds = ...` in `docs/src/configuration.md`.
- [x] 7.3 Add `.git-paw/session-learnings.md` to the list of generated files documented in the user guide.
- [x] 7.4 `mdbook build docs/` succeeds.

## 8. Release notes

- [x] 8.1 v0.5.0 release notes: announce the opt-in `[supervisor] learnings = true` flag. List the deterministic categories tracked (file-only output). Note that programmatic access (broker variant + MCP) lands in v0.6.0; LLM-driven qualitative signals (recurring failures, ADR drift, doc gaps, scope mistakes) follow when the publish path exists.

## 9. Quality gates

- [x] 9.1 `just check` — fmt, clippy, all tests green.
- [x] 9.2 `just deny` — supply chain clean.
- [x] 9.3 No new `unwrap()` / `expect()` in non-test code (lock acquisition uses the existing precedent if applicable).
- [x] 9.4 `mdbook build docs/` succeeds.
- [x] 9.5 `openspec validate learnings-mode` passes.
- [x] 9.6 Verify no `agent.learning` variant or `LearningPayload` struct exists in `src/broker/messages.rs` after this change.
