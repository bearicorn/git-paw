## Context

Three findings from a v0.4.0 code+spec scan, all small and concrete:

- **Panic surface**: `src/agents.rs:343, 357` (regex on literal patterns, can't fail at runtime), `src/git.rs:245` (`PathBuf` UTF-8 unwrap on input that could legitimately be non-UTF-8). The `expect("broker state lock poisoned")` calls in `src/broker/mod.rs:131, 140` are the documented project precedent for `RwLock` acquisition (per `CLAUDE.md`) and stay.
- **Skill ↔ wire-format drift**: `assets/agent-skills/supervisor.md` curl examples for `agent.verified` (line 30) and `agent.feedback` (line 38) use payload field names that don't match the validated wire format defined in `openspec/specs/broker-messages/spec.md`. Specifically `target/result/notes` vs. `verified_by/message` for verified, and `target/message` vs. `from/errors` for feedback.
- **`Question` variant unspecced**: the `BrokerMessage::Question` variant + `QuestionPayload` + validation + `Display` + helpers all ship in `src/broker/messages.rs`, and routing-to-`"supervisor"`-inbox ships in `src/broker/delivery.rs:130`. None of this is reflected in `openspec/specs/broker-messages/spec.md` or `openspec/specs/message-delivery/spec.md`.

This change closes the three deltas. Bug-fix-shaped, no new behaviour.

## Goals / Non-Goals

**Goals:**
- Eliminate the three fixable non-test panic-surface sites without adding new error paths or visible behaviour changes.
- Update the supervisor skill so an agent following the curl examples literally publishes valid messages.
- Specify the shipped `Question` variant and its delivery so the spec matches the code.
- Keep changes additive at the spec level (ADDED requirements only) so cross-change archive ordering is robust.

**Non-Goals:**
- Refactoring code paths beyond the three targeted sites.
- Renaming variants or payload fields. The wire format is established; this change makes the docs catch up to code.
- Adding new variants.
- Changing lock-acquisition `expect()` calls.
- Sweeping for missing doc comments, unused imports, or other lint cleanups beyond the 3 listed sites.

## Decisions

### D1. `LazyLock<Regex>` for `agents.rs` regex compiles

The two `Regex::new(literal).unwrap()` calls compile statically-correct regex strings — the unwrap is provably safe but trips the panic-surface success criterion. CLAUDE.md explicitly allows `OnceLock` / `LazyLock` initialisation as the documented exception. The fix:

```rust
use std::sync::LazyLock;

static SUPERVISOR_PID_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"PAW_SUPERVISOR_PID=\d+")
        .expect("static regex compiles")  // OK inside LazyLock per CLAUDE.md
});

static LAST_VERIFIED_COMMIT_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"PAW_LAST_VERIFIED_COMMIT=[^\n]+")
        .expect("static regex compiles")
});
```

Call sites use `&SUPERVISOR_PID_REGEX` and `&LAST_VERIFIED_COMMIT_REGEX` instead of compiling fresh. This also avoids the per-call regex-compile cost (small but free).

The `expect("static regex compiles")` inside the `LazyLock` is the CLAUDE.md-allowed exception. Any other `expect` would not be allowed.

### D2. `as_os_str()` for `git.rs:245`

Current code:
```rust
.args([
    "worktree", "remove", "--force",
    worktree_path.to_str().unwrap(),
])
```

`Command::args` accepts an iterator over types implementing `AsRef<OsStr>`. `PathBuf::as_os_str()` returns `&OsStr` directly with no UTF-8 requirement. The fix replaces the slice-of-`&str` with separate `.arg()` calls, since `args()` requires homogeneous element types:

```rust
.args(["worktree", "remove", "--force"])
.arg(worktree_path.as_os_str())
```

This is functionally identical for valid UTF-8 paths (the common case) and works correctly for non-UTF-8 paths (the previously-panicking case). No new error path.

### D3. Skill curl-example fixes

The two wrong examples in `assets/agent-skills/supervisor.md` are corrected to match the wire format:

`agent.verified` (line 30):
```bash
# was: '{"type":"agent.verified","agent_id":"supervisor","payload":{"target":"<agent-id>","result":"pass","notes":""}}'
# now:
'{"type":"agent.verified","agent_id":"<agent-id>","payload":{"verified_by":"supervisor","message":"<summary>"}}'
```

Note that `agent_id` at the top level is the *recipient* per the existing v0.4 convention (the agent being verified). `verified_by` is the sender (`"supervisor"`). The skill update spells this out so users don't get the agent_id semantics wrong.

`agent.feedback` (line 38):
```bash
# was: '{"type":"agent.feedback","agent_id":"supervisor","payload":{"target":"<agent-id>","message":"<what to change>"}}'
# now:
'{"type":"agent.feedback","agent_id":"<agent-id>","payload":{"from":"supervisor","errors":["<error 1>","<error 2>"]}}'
```

Same recipient-vs-sender note. The `errors` field is a list of strings; the example shows two for clarity but a single-error list is also valid.

Surrounding prose (line 95's `result:"pass"` reference and any other reference to the wrong field names) is also corrected to match.

### D4. `Question` variant spec ADDED requirements

The shipped behaviour to capture:
- Variant has serde tag `"agent.question"`, carries `agent_id: String` (the asking agent) and `payload: QuestionPayload`.
- `QuestionPayload` has a single field `question: String`.
- Validation rejects empty/whitespace-only `question`, plus the standard `agent_id` slug rules.
- `Display`: `[{agent_id}] question: {payload.question}`.
- `status_label()` returns `"question"`.
- `agent_id()` returns the variant's `agent_id`.
- Delivery: routed to `"supervisor"` inbox, *creating* the inbox if it doesn't already exist (this is the only variant that creates an inbox on delivery). Not enqueued in any other inbox, including the sender's.

The spec deltas mirror the existing `Verified message variant` / `Feedback message variant` requirement shapes for `broker-messages`, and the `Blocked messages are delivered to the target agent` requirement shape for `message-delivery`.

### D5. ADDED-only delta strategy

All three spec deltas use ADDED requirements, never MODIFIED. Reasons:
- The `Question` requirements are net-new — the spec has no existing block to modify.
- The skill-content correction lands in `agent-skills` as new scenarios asserting the corrected substrings, not as a wholesale rewrite of existing requirements.
- ADDED-only is robust against archive-order surprises where this change might land in any position relative to other v0.5.0 changes that touch the same capabilities.

The skill-content scenarios use string-presence assertions (e.g. "skill contains `verified_by` in a curl example") rather than full-template embedding to keep the delta small.

### D6. Test surface

For each fix, one or two regression tests:
- `agents.rs`: existing tests around `update_agents_md` (the function calling these regexes) should already exercise both code paths; adding a smoke test that calls the function twice in quick succession verifies `LazyLock` is reused.
- `git.rs`: a test that constructs a worktree path containing non-UTF-8 bytes (using `OsString::from_vec` on Linux/macOS) and calls the worktree-remove path, asserting no panic. On Windows, the equivalent check is skipped (the platform's path semantics differ).
- `broker/messages.rs`: explicit round-trip + validation + Display + status_label + agent_id tests for `Question` (the existing `messages.rs:798` `status_label_question` test is one anchor; add the rest).
- `broker/delivery.rs`: explicit test that publishing `agent.question` enqueues exactly into the `"supervisor"` inbox, creates that inbox if it didn't exist, and does NOT enqueue in the sender's inbox.
- `agent-skills`: scenario tests assert the corrected curl examples contain `verified_by` / `from` / `errors`, and do NOT contain the wrong `target` / `result` / `notes` field names.

## Risks / Trade-offs

- **[Risk] Cross-change archive collision on `agent-skills`.** Several v0.5.0 changes (`forward-coordination`, `conflict-detection`, `learnings-mode`, `governance-verification`) all add ADDED requirements to `agent-skills`. This change adds yet another. Multiple ADDED-only deltas archive cleanly because they don't conflict on existing requirement names. → **Mitigation:** ADDED requirement names in this change are distinct (`Supervisor skill — corrected curl examples` or similar) from those in other changes.
- **[Risk] `LazyLock<Regex>` first-use is lazy by definition.** The first call to `update_agents_md` pays a small one-time cost. → **Mitigation:** acceptable; same overhead the unwrap path already has on first call. No change to perceived performance.
- **[Risk] `as_os_str()` API surface variance across Rust versions.** `PathBuf::as_os_str` is stable from 1.0; not a concern. → No mitigation needed.
- **[Risk] Tests on Windows.** The non-UTF-8 path test uses `OsString::from_vec` (Unix-only). → **Mitigation:** gate the test with `#[cfg(unix)]`. Windows uses UTF-16 internally; non-UTF-8 paths exist but are constructed differently. The fix itself works on Windows because `OsStr::AsRef` is platform-agnostic.
- **[Trade-off] Spec ADDED-only vs. MODIFIED.** ADDED produces multiple top-level requirement entries instead of one consolidated block. Cost: spec readers see e.g. `Question message variant`, `Validation for Question variant`, `Display for Question variant`, etc. as separate sections. Acceptable: this is the same shape `Verified message variant` and `Feedback message variant` already use.

## Migration Plan

Pure bug-fix change. No migration step.

1. Land any time after the changes that depend on a stable `agent-skills` requirement set (mostly the v0.5.0 features that ADD to the skill). Practically: archive last among v0.5.0 changes that touch `agent-skills`.
2. Existing user behaviour is unchanged. User-forked supervisor skills with the wrong examples continue to fail validation as they do today; the upstream skill becomes correct.
3. Rollback: revert. Skill regresses; spec catch-up regresses (Question stays unspecced again). Code-level panic-surface fixes can stay even if spec changes are reverted (they're independent commits).

## Open Questions

- **Should `Question` validation include a length cap on the question text?** Decision: not in v0.5.0. The shipped code has no cap; spec catch-up follows shipped behaviour. If users abuse free-text questions, a later change can add a cap.
- **Should the skill update show *both* a single-error and multi-error `agent.feedback` example?** Decision: one example with a two-element `errors` array suffices. Single-error users adapt trivially.
- **Should the inbox-creation-on-Question delivery be replaced with the silent-drop pattern used for `Blocked` (which drops if the target inbox doesn't exist)?** Decision: no. Spec catches up to shipped code; the broker creates the supervisor inbox because the supervisor is the *only* recipient and may not have published a `agent.status` yet at the time the first question arrives. Changing that behaviour is a separate decision.
