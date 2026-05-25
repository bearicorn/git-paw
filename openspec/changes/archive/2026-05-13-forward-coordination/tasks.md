## 1. Broker message type

- [x] 1.1 Add `IntentPayload { files: Vec<String>, summary: String, valid_for_seconds: u64 }` struct in `src/broker/messages.rs` with `serde::{Serialize, Deserialize}`, `Debug`, `Clone`, `PartialEq`, `Eq` derives matching the existing payload conventions.
- [x] 1.2 Add `BrokerMessage::Intent { agent_id: String, payload: IntentPayload }` variant with serde tag `#[serde(rename = "agent.intent")]`.
- [x] 1.3 Add `MessageError::EmptyFiles`, `MessageError::EmptyFilePath`, `MessageError::EmptySummary`, `MessageError::ZeroValidFor` variants (or reuse existing empty-field errors with a field-name discriminator if the codebase uses that pattern — match local convention).
- [x] 1.4 Extend `from_json` (or `validate`) to reject Intent messages with empty `files`, any whitespace-only file path, empty `summary`, or `valid_for_seconds == 0`. `agent_id` validation reuses the existing slug rule.
- [x] 1.5 Extend `BrokerMessage::agent_id()` to return the `agent_id` field for the `Intent` variant.
- [x] 1.6 Extend `BrokerMessage::status_label()` to return `"intent"` for the `Intent` variant.
- [x] 1.7 Extend `Display` for `BrokerMessage` to format `Intent` as `[{agent_id}] intent: {N} files for {valid_for_seconds}s — {summary}` (single line, no ANSI).

## 2. Broker message validation tests

- [x] 2.1 Round-trip serde test for `Intent` with multiple files (asserts JSON contains `"type": "agent.intent"`).
- [x] 2.2 Round-trip test for `Intent` with a single file.
- [x] 2.3 Validation rejection tests: empty `files`, whitespace-only file path, empty `summary`, zero `valid_for_seconds`.
- [x] 2.4 Valid Intent JSON produces a `BrokerMessage` with all fields preserved.
- [x] 2.5 `Display` test asserting the exact string `[feat-auth] intent: 3 files for 900s — wire AuthClient`.
- [x] 2.6 `Display` test for single-file intent.
- [x] 2.7 `status_label()` returns `"intent"`; `agent_id()` returns the agent id.

## 3. Message delivery

- [x] 3.1 Locate the publish handler in `src/broker/delivery.rs` (or wherever `Artifact`/`Verified` broadcast is implemented). Add an `Intent` arm that follows the same broadcast logic: enqueue in every known agent's inbox EXCEPT the sender, skip agents not yet registered, no error on missing inbox.
- [x] 3.2 Update the agent-record update path so `Intent` updates `last_seen` and sets the record `status` field to `"intent"` (via `status_label()`), matching the existing pattern for other message types.

## 4. Message delivery tests

- [x] 4.1 Broadcast scenario: three registered agents, intent from one is received by the other two.
- [x] 4.2 Sender-exclusion scenario: sender does not receive its own intent in its inbox.
- [x] 4.3 Unregistered-target scenario: sender publishes intent while only its own inbox exists; no inbox is created for the unregistered peer; no error.
- [x] 4.4 Agent record update scenarios: publishing `agent.intent` updates `last_seen` and sets `status` to `"intent"`.

## 5. Embedded coordination skill rewrite

- [x] 5.1 Bump frontmatter in `assets/agent-skills/coordination.md`: `compatibility: git-paw v0.5.0+`.
- [x] 5.2 Insert a new `### Before you start editing` section after `### Automatic status publishing` and before `### Check for messages from peers`. Section content: read spec → publish `agent.intent` → poll once for warnings → on overlap decide (wait / split / `agent.question`). Include a `curl` example with `files`, `summary`, `valid_for_seconds: 900`.
- [x] 5.3 Insert a new `### While you're editing` section immediately after `Before you start editing`. Content: re-publish on scope growth; on peer intent overlap send `agent.question` rather than racing. Include the explicit MUST-NOT list (no pairwise check-ins, no waiting for go-ahead, no blocking on broker silence).
- [x] 5.4 Update item 2 of the existing automatic-publishing note to mention intent as a third manual-publish trigger (alongside blocked and exports).
- [x] 5.5 Verify all curl examples in the rewrite use `{{BRANCH_ID}}` and `{{GIT_PAW_BROKER_URL}}` placeholders consistently (no `${...}` regression).
- [x] 5.6 Mirror the changes into `docs/src/user-guide/coordination.md` so the user-guide chapter matches the embedded skill.

## 6. Embedded supervisor skill update

- [x] 6.1 In `assets/agent-skills/supervisor.md`, insert a `### Watch peer intents` section between `### Poll session status and messages` and `### Publish verification outcome`. Content: agent.intent arrives in the inbox; this release has no automatic warning logic; supervisor MAY inspect intents and prompt agents via `agent.feedback` or `agent.question` on observed overlap; full algorithms come in `conflict-detection`.

## 7. Skill-content tests

- [x] 7.1 Update the existing `Coordination skill retains polling reference` test to assert `{{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}` (drop the stale `${...}` form).
- [x] 7.2 New test: skill contains `Before you start editing` heading.
- [x] 7.3 New test: skill contains a curl example for `agent.intent` with `files`, `summary`, and `valid_for_seconds` fields.
- [x] 7.4 New test: skill contains `While you're editing` heading.
- [x] 7.5 New test: skill instructs re-publishing on scope growth.
- [x] 7.6 New test: skill instructs use of `agent.question` (not pairwise blocking) on peer-intent overlap.
- [x] 7.7 New test: skill contains explicit MUST-NOT statements rejecting pairwise check-ins, waiting for go-ahead, and blocking on broker silence.
- [x] 7.8 Existing scenarios (automatic status, blocked/artifact curl, cherry-pick, verified/feedback) still pass against the updated skill.
- [x] 7.9 New test: supervisor skill contains `agent.intent` and `Watch peer intents` heading and notes that automatic conflict-warning logic is not part of this release.

## 8. Release notes & MILESTONE upkeep

- [x] 8.1 ~~Add a bullet to the v0.5.0 release-notes draft (in `MILESTONE.md` …)~~
      N/A — `MILESTONE.md` was removed from the repo before this change landed.
      The release-notes call-out for the coordination.md rewrite and user-fork
      reminder lives in `openspec/changes/forward-coordination/proposal.md`
      §"Backward compatibility" and `design.md` D9. Re-state when CHANGELOG.md
      is regenerated at v0.5.0 release prep time per AGENTS.md §"Cutting a release".
- [x] 8.2 ~~Mark MILESTONE.md item #14…~~
      N/A — `MILESTONE.md` no longer tracked. The stale `${GIT_PAW_BROKER_URL}`
      assertion in `agent-skills/spec.md` is resolved by the delta in
      `openspec/changes/forward-coordination/specs/agent-skills/spec.md` (modified
      scenario "Coordination skill retains polling reference" now asserts
      `{{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}`).

## 9. Quality gates

- [x] 9.1 `just check` — fmt, clippy, all tests green.
- [x] 9.2 `just deny` — supply chain clean.
- [x] 9.3 No new `unwrap()` or `expect()` in non-test code added by this change.
- [x] 9.4 `mdbook build docs/` succeeds.
- [x] 9.5 `openspec validate forward-coordination` passes (it does as of specs artifact).
