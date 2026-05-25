## 1. Panic-surface fix: agents.rs regex compiles

- [x] 1.1 In `src/agents.rs`, add `use std::sync::LazyLock;` (if not already imported).
- [x] 1.2 Define `static SUPERVISOR_PID_REGEX: LazyLock<regex::Regex>` initialised with `LazyLock::new(|| regex::Regex::new(r"PAW_SUPERVISOR_PID=\d+").expect("static regex compiles"))`. Place near the top of the file or in a dedicated `regexes` module within the file.
- [x] 1.3 Define `static LAST_VERIFIED_COMMIT_REGEX: LazyLock<regex::Regex>` initialised with `LazyLock::new(|| regex::Regex::new(r"PAW_LAST_VERIFIED_COMMIT=[^\n]+").expect("static regex compiles"))`.
- [x] 1.4 In `src/agents.rs:343`, replace the inline `regex::Regex::new(r"PAW_SUPERVISOR_PID=\d+").unwrap()` with `&SUPERVISOR_PID_REGEX`.
- [x] 1.5 In `src/agents.rs:357`, replace the inline `regex::Regex::new(r"PAW_LAST_VERIFIED_COMMIT=[^\n]+").unwrap()` with `&LAST_VERIFIED_COMMIT_REGEX`.
- [x] 1.6 Smoke test: call `update_agents_md` twice in succession (existing tests likely exercise this); assert no panic and that subsequent calls reuse the same `LazyLock` instance (no re-compile).

## 2. Panic-surface fix: git.rs path-to-OsStr

- [x] 2.1 In `src/git.rs:245`, replace the `worktree_path.to_str().unwrap()` site by restructuring the `Command::args` call:
  - Before: `.args(["worktree", "remove", "--force", worktree_path.to_str().unwrap()])`
  - After: `.args(["worktree", "remove", "--force"]).arg(worktree_path.as_os_str())`
- [x] 2.2 Verify the resulting `Command` builder still produces the expected argv when the worktree path is valid UTF-8 (the common case).
- [x] 2.3 Add a `#[cfg(unix)]`-gated test that constructs an `OsString` from non-UTF-8 bytes (using `std::os::unix::ffi::OsStringExt::from_vec` with a bytes vector containing a non-UTF-8 sequence), wraps it in a `PathBuf`, and confirms the worktree-remove call path does not panic. The test does not require the `git` invocation to succeed (the path won't exist) — only that the argv is constructed without unwrap'ing a non-UTF-8 path.

## 3. Skill curl-example fixes

- [x] 3.1 In `assets/agent-skills/supervisor.md`, locate the `agent.verified` curl example (currently around line 30). Replace the payload `{"target":"<agent-id>","result":"pass","notes":""}` with `{"verified_by":"supervisor","message":"<summary>"}`. Update the surrounding `agent_id` field to `"<agent-id>"` to clarify it's the recipient.
- [x] 3.2 Add a comment or surrounding text near the example clarifying that `agent_id` at the top level is the recipient (the agent being verified), and `verified_by` in the payload is the sender (`"supervisor"`).
- [x] 3.3 In the same file, locate the `agent.feedback` curl example (currently around line 38). Replace the payload `{"target":"<agent-id>","message":"<what to change>"}` with `{"from":"supervisor","errors":["<error 1>","<error 2>"]}`. Update the top-level `agent_id` to `"<agent-id>"`.
- [x] 3.4 Add a comment or surrounding text clarifying the recipient-vs-sender semantics for `agent.feedback` (top-level `agent_id` is the recipient receiving feedback; `from` in the payload is the sender).
- [x] 3.5 Update workflow prose elsewhere in `supervisor.md` that references the wrong field names. Specifically, sweep for `result:"pass"`, `notes:""`, and any `target:"<...>"` references in the verified/feedback context; replace with the correct field names.
- [x] 3.6 Mirror the corrections into `docs/src/user-guide/coordination.md` (or wherever the skill content is doc-mirrored). The doc-mirror should remain consistent with the embedded skill.

## 4. Skill-content tests

- [x] 4.1 Update existing skill-content tests (or add new ones) asserting the embedded `supervisor.md` `agent.verified` example contains `verified_by` and `message` and does NOT contain `target`, `result`, or `notes` as payload field-key tokens.
- [x] 4.2 Update/add tests asserting the `agent.feedback` example contains `from` and `errors`, contains JSON-array brackets `[` and `]` within the example body, and does NOT contain `target` or `message` as payload field-key tokens for Feedback.
- [x] 4.3 Test that the skill includes a comment or surrounding sentence clarifying recipient-vs-sender for both examples.
- [x] 4.4 Test that workflow prose in `supervisor.md` does NOT reference `result:"pass"` or `notes:""` as the verified payload structure.

## 5. Question variant — broker-messages tests

- [x] 5.1 Add or extend tests in `src/broker/messages.rs` covering:
  - Round-trip: `BrokerMessage::Question` with `agent_id = "feat-x"` and a populated `QuestionPayload` survives serialize → deserialize unchanged. Asserts `"type": "agent.question"` in the JSON.
  - Validation: empty `question` is rejected with `MessageError::EmptyQuestionField`.
  - Validation: whitespace-only `question` is rejected.
  - Validation: empty `agent_id` is rejected.
  - `Display`: `[feat-x] question: <text>` exactly. No newlines, no ANSI.
  - `status_label()`: returns `"question"` (existing test at `messages.rs:798` already covers this).
  - `agent_id()`: returns the variant's `agent_id` field.
- [x] 5.2 Tests use the existing test-helper conventions in `messages.rs`.

## 6. Question variant — message-delivery tests

- [x] 6.1 In `src/broker/delivery.rs` tests (or a new test file if conventions allow), add:
  - `Question` from `feat-x` with existing `supervisor` inbox → `poll_messages("supervisor", 0)` returns the question; `poll_messages("feat-x", 0)` returns nothing.
  - `Question` from `feat-x` when no `supervisor` inbox exists yet → publish creates the supervisor inbox; subsequent `poll_messages("supervisor", 0)` returns the question.
  - `Question` does not reach unrelated agents (third agent's inbox unaffected).
  - Sender's agent record (`feat-x`) has `status` set to `"question"` after publishing.
  - Sender's agent record `last_seen` is updated.
- [x] 6.2 At least one test specifically asserts the inbox-creation behaviour distinguishes `Question` from `Blocked` (`Blocked` silently drops on missing target inbox; `Question` creates the supervisor inbox).

## 7. Documentation

- [x] 7.1 Update `docs/src/user-guide/coordination.md` (or wherever the skill is documented) to mirror the corrected curl examples per task 3.6. Include the recipient-vs-sender clarification.
- [x] 7.2 If `docs/src/api/broker-messages.md` (or equivalent broker-message reference doc) exists, add `agent.question` documentation matching the new spec.
- [x] 7.3 `mdbook build docs/` succeeds.

## 8. Release notes

- [x] 8.1 v0.5.0 release notes: brief mention that the `supervisor.md` skill's `agent.verified` and `agent.feedback` curl examples have been corrected; user-forked copies are stale and should be re-merged from upstream.
- [x] 8.2 Note that `agent.question` now has spec coverage (catch-up for v0.4 shipped behaviour; no functional change).
- [x] 8.3 Cross-reference MILESTONE drift items #12 and #13 as resolved.

## 9. Quality gates

- [x] 9.1 `just check` — fmt, clippy, all tests green. Specifically verify no new clippy warnings about `unwrap()` / `expect()` outside the documented `LazyLock`/`OnceLock` exception.
- [x] 9.2 `just deny` — supply chain clean.
- [x] 9.3 Verify the panic-surface scan re-run shows zero `unwrap()` / `expect()` calls in non-test src/ code outside `LazyLock` / `OnceLock` initialisation. The two `expect("broker state lock poisoned")` lock-acquisition uses remain (documented precedent).
- [x] 9.4 `mdbook build docs/` succeeds.
- [x] 9.5 `openspec validate v040-hardening` passes.
