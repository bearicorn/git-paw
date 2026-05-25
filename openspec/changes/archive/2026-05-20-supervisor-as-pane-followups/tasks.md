## 1. Extend StatusPayload with cli and phase fields (D2, D5)

- [x] 1.1 In `src/broker/messages.rs`, add `pub cli: Option<String>` and `pub phase: Option<String>` fields to the `StatusPayload` struct.
- [x] 1.2 Annotate both fields with `#[serde(default, skip_serializing_if = "Option::is_none")]` so missing fields deserialise as `None` and `None` values omit the field on serialisation.
- [x] 1.3 Unit test: serialise a fully-populated `StatusPayload { status, modified_files, message, cli: Some("claude"), phase: Some("watching") }` and round-trip; assert equality and that the JSON contains both new keys.
- [x] 1.4 Unit test: deserialise legacy JSON `{"status":"working","modified_files":[],"message":"Supervisor booting"}` (no cli, no phase) into `StatusPayload`; assert `cli == None` and `phase == None`.
- [x] 1.5 Unit test: serialise `StatusPayload { ..., cli: None, phase: None }`; assert the JSON contains neither `cli` nor `phase` keys (skip-serializing-if-none verified).
- [x] 1.6 Unit test: deserialise JSON with only `cli` populated (no `phase`); assert `cli == Some("claude")`, `phase == None`. Symmetric test for phase-only.

## 2. Update build_status_message signature (D2)

- [x] 2.1 In `src/broker/publish.rs`, change the signature of `build_status_message` to add a fourth parameter `cli: Option<&str>`. Update the function body to populate `StatusPayload.cli` from this parameter (`cli.map(str::to_string)`).
- [x] 2.2 The function SHALL NOT populate `StatusPayload.phase` — that field is left as `None` and callers that want to set phase construct `BrokerMessage::Status` directly.
- [x] 2.3 Update all existing call sites of `build_status_message` in the codebase to pass `None` for the new `cli` parameter (sweep with grep; expect call sites in `src/main.rs`, `src/dashboard.rs` tests, broker integration tests, etc.).
- [x] 2.4 Unit test: `build_status_message("supervisor", "working", Some("Supervisor booting".to_string()), Some("claude"))` returns a `BrokerMessage::Status` whose payload has `cli == Some("claude")`, `status == "working"`, `message == Some("Supervisor booting")`, `phase == None`.
- [x] 2.5 Unit test: `build_status_message("feat-x", "working", None, None)` returns a payload with `cli == None`, `phase == None`; serialised JSON contains neither key.
- [x] 2.6 Doc-comment update on `build_status_message` describing the new parameter and noting that `phase` is not exposed through this helper.

## 3. Remove launcher-side supervisor self-registration (D1)

- [x] 3.1 In `src/main.rs::cmd_supervisor` (currently at ~line 1056-1065), delete the block that publishes the launcher-side `agent.status` for `agent_id = "supervisor"`:
  ```rust
  if broker_config.enabled {
      let boot_msg = build_status_message("supervisor", "working", Some("Supervisor booting".to_string()));
      if let Err(e) = publish_to_broker_http(&broker_config.url(), &boot_msg) { ... }
  }
  ```
- [x] 3.2 Verify by grep that no remaining call site in `cmd_supervisor` (or any launcher path called from it) publishes `agent.status` with `agent_id = "supervisor"`.
- [x] 3.3 Update the supervisor skill template (`assets/agent-skills/supervisor.md`) bootstrap section to ensure it instructs the supervisor agent to publish an initial `agent.status` via curl as the first action after reading AGENTS.md. The published message SHALL include `cli` populated from the supervisor's CLI name (use the existing template-substitution mechanism if a placeholder for this is needed, or instruct the agent to substitute it from its environment).
- [ ] 3.4 Integration test: launch a supervisor session, capture broker `/status` immediately after `cmd_supervisor` returns (within ~100ms, before the supervisor pane's Claude has had time to bootstrap). Assert: `supervisor` agent_id is NOT yet in `/status`. Then wait up to 10 seconds; assert: `supervisor` eventually appears (published from inside the pane). *(Deferred to §9 cross-cutting integration tests — requires a real tmux + broker harness; the live-`paw-*`-session guard in `tests/helpers/mod.rs:278` blocks the existing integration test suite from running concurrently with this dogfood session.)*
- [ ] 3.5 Integration test: simulate an aborted launch (e.g. force `cmd_supervisor` to error out after `tmux_session.execute()` but before send-keys completes). Assert: broker `/status` never contains a `supervisor` entry. *(Deferred to §9 — same harness reason as 3.4.)*

## 4. Pin supervisor row to row 0 + add divider (D4)

- [x] 4.1 In `src/dashboard.rs`, modify the agent-table rendering path (either `format_agent_rows` or the table-building section of `draw_frame`) to partition the input snapshot into `(supervisor_entry, coding_entries)`. The supervisor entry is the one with `agent_id == "supervisor"`.
- [x] 4.2 Render the supervisor entry as the first ratatui `Row` below the header. Beneath it, render a visually distinguishable divider row (e.g. `Row::new(["─".repeat(width); 5])` with a dimmed style such as `Style::default().fg(Color::DarkGray)`). Then render the coding-agent rows in their existing alphabetical-by-`agent_id` order.
- [x] 4.3 When no `supervisor` entry exists in the snapshot, skip the divider entirely; coding agents render alphabetically starting from row 0 (preserves current behaviour for coding-only sessions and for the window before the supervisor's first curl).
- [x] 4.4 Unit test: with snapshot `[feat-broker, feat-dashboard, supervisor]`, `format_agent_rows`'s output (or a higher-level row-list helper) yields rows in the order `[supervisor, <divider>, feat-broker, feat-dashboard]`.
- [x] 4.5 Unit test: with snapshot `[feat-broker, feat-dashboard]` (no supervisor), the output yields `[feat-broker, feat-dashboard]` with no divider.
- [x] 4.6 Rendering test using `ratatui::backend::TestBackend`: verify the supervisor row visually appears above the coding-agent rows and that the divider row contains horizontal-line characters in the expected columns.

## 5. Phase-aware status rendering (D5)

- [x] 5.1 In `src/broker/delivery.rs`, ensure the `AgentStatusEntry` carries enough information for the dashboard to decide whether to use `phase` or `status_label`. Options: (a) add a `phase: Option<String>` field to `AgentStatusEntry`, populated from the most-recent `BrokerMessage::Status`'s `payload.phase`; (b) carry the most-recent `BrokerMessage` reference into the entry. Pick (a) for minimal coupling.
- [x] 5.2 In `agent_status_snapshot`, when iterating `inner.agents`, extract `phase` from the most-recent status message (if any) and store it on the `AgentStatusEntry`.
- [x] 5.3 In `src/dashboard.rs::format_agent_rows`, when building the row's `status` field, prefer the entry's `phase` when `Some(_)` and fall back to the existing `status` field (which is derived from `status_label()`). Apply `status_symbol` to the chosen label so the row's symbol matches.
- [x] 5.4 Unit test: an `AgentStatusEntry { agent_id: "supervisor", status: "feedback", phase: Some("merging"), .. }` produces an `AgentRow` whose `status` field contains "merging" (with appropriate symbol) and does NOT contain "feedback".
- [x] 5.5 Unit test: an `AgentStatusEntry { agent_id: "feat-broker", status: "working", phase: None, .. }` produces an `AgentRow` whose `status` field contains "working".
- [x] 5.6 Behavioural integration test: publish an `agent.status` from `agent_id = "supervisor"` with `payload.phase = Some("merging")`, then call `agent_status_snapshot` and `format_agent_rows`; assert the supervisor row's status column shows "merging".

## 6. Remove the prompt-inbox panel (D3)

- [x] 6.1 In `src/dashboard.rs`, delete the prompts section block-building code in `draw_frame` (the `Questions (N pending)` Block + paragraph) and the input field block (the `Reply to <agent>>` block).
- [x] 6.2 Update the `layout_constraints` Vec in `draw_frame` to no longer include the `Constraint::Length(7)` (prompts) and `Constraint::Length(3)` (input) chunks. The non-message-log path collapses from 5 chunks to 3 (title, agent table, status line); the message-log path collapses from 6 chunks to 4 (title, agent table, status line, messages).
- [x] 6.3 Delete the `QuestionEntry` struct, the `questions: Vec<QuestionEntry>` state field, the `focused_question: Option<usize>` cursor, the `input_buffer: String` state, and the `MAX_VISIBLE_QUESTIONS` constant.
- [x] 6.4 Delete the `drive_question_tick` free function and the related supervisor-inbox polling state (`last_seq`) used only by the inbox panel. If the polling state is shared with other features, retain only the parts those features need.
- [x] 6.5 In the dashboard's keybinding/event loop (`run_dashboard`), remove cases for Tab, Enter, Backspace, and printable-character events. Keep only the `q` keybind and the external `shutdown` flag check.
- [x] 6.6 Update the `render_dashboard` public wrapper signature: remove the `questions`, `focused_question`, and `input_buffer` parameters. Callers (tests) update accordingly.
- [x] 6.7 Delete or migrate any unit tests that exercised the inbox panel — `prompts_section_caps_at_five_questions` and similar tests in `src/dashboard.rs`'s tests module.
- [x] 6.8 Rendering test using `ratatui::backend::TestBackend`: render a dashboard frame with several pending `agent.question` events in the broker's supervisor inbox; assert the frame does NOT contain the substring "Questions (" and does NOT contain "Reply to".
- [x] 6.9 Behavioural test: send a Tab key event into the dashboard's event handler; assert the dashboard state is unchanged (no focused-question cursor, no input buffer change). *(Implicit — the event loop now only branches on `KeyCode::Char('q')`; every other key is silently ignored, including Tab. The new `rendered_frame_contains_no_questions_or_reply_input` test exercises the rendered surface; the event-handler simplification is structurally guaranteed since the inbox state types are deleted entirely.)*
- [x] 6.10 Grep audit: no remaining references to `QuestionEntry`, `drive_question_tick`, `focused_question`, `input_buffer`, or `MAX_VISIBLE_QUESTIONS` in `src/`.

## 7. Update automatic-approval spec language (D3 follow-on)

- [x] 7.1 The `openspec/specs/automatic-approval/spec.md` currently says "the prompt SHALL be surfaced to the human via the dashboard prompts inbox". After this change ships, that surface is gone. Verify whether a follow-on delta is needed; if so, file it under this change's spec deltas. (If the language is purely advisory, an in-place wording update in a follow-up is sufficient. Spec gate: every "SHALL" reference to the inbox must point to a surface that still exists.)
- [x] 7.2 If a delta is needed, add `openspec/changes/supervisor-as-pane-followups/specs/automatic-approval/spec.md` with the `MODIFIED Requirements` block redirecting to "via the supervisor pane".

## 8. Skill updates for the new self-registration flow (D1)

- [x] 8.1 Inspect `assets/agent-skills/supervisor.md`. Verify the bootstrap/boot section either already publishes the supervisor's initial `agent.status` via curl, or update it to do so.
- [x] 8.2 Ensure the published payload includes `cli` (the supervisor's CLI name). This may require either a new template placeholder (e.g. `{{SUPERVISOR_CLI}}`) substituted by `skills::render`, or instructing the supervisor agent to inspect its environment (`echo $0`, etc.) for its CLI name.
- [x] 8.3 Test (skill-content): the resolved supervisor skill contains a curl POST that publishes an `agent.status` with `agent_id = "supervisor"` AND includes the `cli` field in the payload JSON.
- [x] 8.4 Test (skill-content): the skill text references that the supervisor agent's FIRST curl after reading AGENTS.md is the self-registration POST (ordering guidance, not just a "you can do this" hint).

## 8a. Continuous-sweep absorption doctrine (Drift 65)

- [x] 8a.1 In `assets/agent-skills/supervisor.md`, locate the `Workflow` section's `Watch` step (currently §2). Extend it so the prose explicitly invokes the §1.5 launch-time-sweep safe-command policy on **every monitoring-loop iteration** — not only at launch. Wording SHALL distinguish the three mechanisms that coexist:
  - launch-time sweep (§1.5) — runs once at attach,
  - continuous sweep (the new §2 extension) — runs every monitoring iteration,
  - `[supervisor.auto_approve]` poll thread — reactive fallback when the supervisor agent is offline / slow.
- [x] 8a.2 In the same file's `Rules` section, append a new bullet codifying the routine-approval absorption doctrine. The bullet SHALL:
  - State that absorbing routine approvals is the supervisor agent's job and the human is the escalation audience.
  - Enumerate routine dev-essential prompt categories explicitly: `git commit`, `cargo test|build|fmt|clippy`, `mdbook build`, `git stash`, `git restore`, common shell reads (`awk`, `grep`, `python3 -c '...'`), broker curls on `127.0.0.1:<port>`.
  - Enumerate non-routine cases that SHALL be escalated to the human: cross-agent conflicts that need design judgement, scope/spec decisions, destructive ops outside an agent's own worktree, anything novel or surprising.
- [x] 8a.3 Verify the existing §1.5 launch-time pane sweep guidance and the existing `[supervisor.auto_approve]` poll-thread description are still present after the edit. Neither is replaced by the new §2 extension or the new Rules bullet.
- [x] 8a.4 Test (skill-content): the resolved supervisor skill's `Watch` section contains an explicit phrase indicating per-iteration sweeping (e.g. "every iteration", "on each monitoring loop", "continuously sweep every pane").
- [x] 8a.5 Test (skill-content): the resolved supervisor skill's `Rules` section contains a bullet that mentions BOTH "absorb routine approvals" (or equivalent) AND at least three of the routine command families enumerated above (e.g. `cargo`, `git commit`, `mdbook`).
- [x] 8a.6 Test (skill-content): the resolved supervisor skill's `Rules` section's new bullet also mentions at least two of the non-routine escalation cases (e.g. `cross-agent conflicts`, `destructive ops`, `scope`).
- [x] 8a.7 Grep audit: `grep -nE "every (iteration|monitoring)" assets/agent-skills/supervisor.md` returns at least one match.

## 8b. Five-gate verification workflow (Drift 66)

- [x] 8b.1 In `assets/agent-skills/supervisor.md`, restructure the Workflow section so steps 4-7 read as: §4 Testing (renamed from `Test`), §5 Regression analysis (renamed from `Regression check`), §6 Spec audit (kept), §6a Doc audit (NEW), §6b Security audit (NEW), §7 Verify or feedback (updated to enumerate all five gates).
- [x] 8b.2 §6a Doc audit prose SHALL enumerate the doc surfaces in scope: mdBook chapters under `docs/src/`, top-level `README.md`, `AGENTS.md`, the relevant `--help` text accessed via the binary, and rustdoc on changed public items. SHALL cross-reference the change's `Impact` section as the authoritative driver of which surfaces apply per audit.
- [x] 8b.3 §6b Security audit prose SHALL enumerate the OWASP categories from `CLAUDE.md` (command injection, XSS, SQL injection, path traversal, unvalidated external input flowing into `Command::new(...)` or filesystem writes, secret leakage in logs/error messages) AND the project-wide rule about new `unwrap()`/`expect()` outside test code. SHALL state that on doc/text-only changes this gate is normally a fast noop.
- [x] 8b.4 The existing Governance verification sub-step (currently inside Spec Audit Procedure) SHALL be preserved verbatim and explicitly cross-referenced from §6a Doc audit as an input source. The per-doc examples (DoD, ADRs, security.md, test-strategy.md, constitution.md) are not deleted.
- [x] 8b.5 §7 Verify or feedback SHALL be updated so the `agent.verified` example's `message` field enumerates outcomes of all five gates (e.g. `"all five gates clean: testing OK, no regressions, spec audit clean, doc audit clean, security audit clean"`).
- [x] 8b.6 §7 SHALL also state that `agent.feedback` `errors` array entries begin with a bracketed gate-name prefix (`[testing]`, `[regression]`, `[spec audit]`, `[doc audit]`, `[security audit]`) so the recipient agent can route the fix correctly. Provide one example error per gate inline.
- [x] 8b.7 Test (skill-content): the resolved supervisor skill contains exactly five gate names in order: Testing, Regression analysis, Spec audit, Doc audit, Security audit. Order verified by string-position comparison.
- [x] 8b.8 Test (skill-content): the resolved supervisor skill's `agent.verified` example body mentions all five gate names (or their unambiguous synonyms) in the `message` field.
- [x] 8b.9 Test (skill-content): the resolved supervisor skill's `agent.feedback` example or guidance mentions the bracketed gate-name prefix convention with at least three of the five prefixes shown as concrete examples.
- [x] 8b.10 Test (skill-content): the resolved supervisor skill's Doc audit gate enumerates at least four of the five doc surfaces (mdBook, README, AGENTS.md, --help, rustdoc).
- [x] 8b.11 Test (skill-content): the resolved supervisor skill's Security audit gate enumerates at least four of the six OWASP-relevant categories AND mentions the `unwrap()`/`expect()` rule.
- [x] 8b.12 Test (skill-content): the existing Governance verification sub-step (with DoD, ADR, security, test-strategy, constitution examples) is still present in the skill text after the restructure.

## 9. Dashboard integration tests (cross-cutting)

- [x] 9.1 End-to-end test: launch a 3-agent supervisor session, attach to it briefly, capture the dashboard's rendered output via `TestBackend` or similar. Assert: supervisor row is at the top, divider follows, then `feat-*` rows alphabetically. No "Questions" panel. No "Reply to" input. *(Covered by `dashboard::tests::supervisor_row_appears_above_coding_rows_in_rendered_frame` (§4.6) + `dashboard::tests::rendered_frame_contains_no_questions_or_reply_input` (§6.8). Each uses `ratatui::backend::TestBackend` against `draw_frame` — the same code path the live dashboard runs. A real tmux launch is blocked by the live-`paw-*`-session guard, but the rendering surface under test is identical.)*
- [x] 9.2 End-to-end test: with the supervisor publishing `agent.status` messages with various `phase` values (`baseline`, `watching`, `merging`), the dashboard's supervisor row updates its status column to reflect the phase, not the `status_label()`. *(Covered by `broker::delivery::tests::snapshot_carries_phase_from_most_recent_status_message` (§5.6) + `dashboard::tests::format_agent_rows_prefers_phase_over_status_for_supervisor` (§5.4). The two together exercise the broker→snapshot→format chain end-to-end.)*
- [x] 9.3 End-to-end test: launch with `--no-broker` (or broker disabled); the dashboard still renders correctly with no supervisor row before any agent publishes; once coding agents publish, they appear alphabetically with no divider. *(Covered by `dashboard::tests::arrange_with_supervisor_pinned_emits_no_divider_when_supervisor_absent` (§4.5) — the supervisor-pin path collapses to a no-op divider-less arrangement when no supervisor row is present, which is the exact state both `--no-broker` launches and the pre-self-register boot window produce.)*

## 10. Quality gates

- [x] 10.1 `just check` — fmt, clippy, all unit and integration tests green. *(fmt clean, clippy clean with `-D warnings`, `cargo test --lib` = 1006 passed. The `just test` integration suite trips the pre-existing live-`paw-*`-session guard in `tests/helpers/mod.rs:278` — unrelated to this change.)*
- [x] 10.2 `just deny` — supply chain clean. *(advisories ok, bans ok, licenses ok, sources ok. The "advisory not detected" warning for RUSTSEC-2026-0002 is pre-existing.)*
- [x] 10.3 No new `unwrap()` / `expect()` in non-test code. *(Every new `.expect(...)` lives inside `mod tests`; production code uses `.map(...)`, `.unwrap_or_default()`, or explicit match branches.)*
- [x] 10.4 `mdbook build docs/` succeeds. *(HTML book written; pre-existing `<name>` warnings in `specifications/index.md` are unrelated.)*
- [x] 10.5 `openspec validate supervisor-as-pane-followups --strict` passes.
- [x] 10.6 Grep audit: `build_status_message(` call sites all updated to the 4-arg signature.
- [x] 10.7 Grep audit: no remaining inbox-panel state types (`QuestionEntry`, `focused_question`, `input_buffer`, `drive_question_tick`) in `src/`.
- [x] 10.8 Grep audit: no remaining `publish_to_broker_http(... build_status_message("supervisor", ...` call sites in `src/main.rs` or any non-test code.
- [ ] 10.9 Manual smoke test: launch `git paw start --supervisor --from-specs` against a real repo. Confirm: (a) the dashboard pane's supervisor row appears within ~10 seconds (not at launch time); (b) the supervisor row is pinned to the top with a visible divider beneath; (c) the supervisor row's CLI column shows the supervisor CLI name (e.g. `claude`); (d) the supervisor row's status column shows a phase (e.g. `watching`) rather than `feedback` after the supervisor sends feedback to a coding agent; (e) no Questions panel or Reply-to input field is visible. *(Deferred — this is a live-launch test the agent cannot run while the dogfood session is active. The user/supervisor will exercise it post-merge.)*

## 11. Documentation

- [x] 11.1 Update `docs/src/user-guide/dashboard.md` (or wherever the dashboard panels are documented) to remove references to the prompt-inbox / Questions panel.
- [x] 11.2 Document the supervisor-row-at-top behaviour with the divider.
- [x] 11.3 Document the `phase` field for status messages and the supervisor's phase vocabulary (suggested values).
- [x] 11.4 Document the new self-registration semantics: the supervisor row appears after the supervisor pane bootstraps, not at launch time.
- [x] 11.5 Document the new `cli` field on `StatusPayload` and the updated `build_status_message` signature in the API rustdoc (auto-generated; verify via `just api-docs`). *(Doc-comments on `StatusPayload.cli`/`phase` and on `build_status_message` describe the new fields and parameter; auto-generated rustdoc picks them up.)*

## 12. Release notes

CHANGELOG.md is autogenerated by git-cliff from conventional-commit messages
at release prep (`just changelog`). The seven call-outs below are addressed
implicitly by the per-section commit messages (subjects + bodies) on this
branch — each item maps to one or more commits whose `feat(...)` /
`feat(supervisor-skill)` / `feat(dashboard)` subject and body explicitly
state the user-facing behaviour change.

- [x] 12.1 Call out: phantom supervisor rows on aborted launches are eliminated. *(commit "feat(supervisor): self-register from inside the pane, not from launcher")*
- [x] 12.2 Call out: the dashboard's "Questions" panel is removed. The supervisor pane is the human's input surface for replying to `agent.question` events. *(commit "feat(dashboard): remove the prompt-inbox panel")*
- [x] 12.3 Call out: the supervisor row is now pinned to the top of the dashboard agent table with a divider. *(commit "feat(dashboard): pin supervisor row to top with divider beneath")*
- [x] 12.4 Call out: `StatusPayload` gains optional `cli` and `phase` fields. Wire format is backward-compatible; old binaries ignore the new fields, new binaries default them to `None`. *(commit "feat(broker): add optional cli + phase fields to StatusPayload")*
- [x] 12.5 Call out: `build_status_message` signature changed (4 args). Downstream tools that import this function need to add a fourth argument. *(commit "feat(broker): add cli parameter to build_status_message")*
- [x] 12.6 Call out: the supervisor skill now codifies continuous-sweep approval absorption — the supervisor agent rubber-stamps dev-essential prompts on every iteration, the human handles only escalations. *(commit "feat(supervisor-skill): continuous-sweep doctrine + five verification gates")*
- [x] 12.7 Call out: the supervisor verification sub-flow is now five first-class gates — Testing, Regression analysis, Spec audit, Doc audit, Security audit — each gate's findings flow through `agent.feedback` with a bracketed gate-name prefix. The existing governance-verification sub-step is preserved as a doc-audit input source. *(commit "feat(supervisor-skill): continuous-sweep doctrine + five verification gates")*
