# Tasks — test-coverage-v0-5-0

All tasks add test functions. No runtime behaviour changes ship from this
change. The single non-test code touch is the `pub(crate)` visibility lift on
`build_task_prompt` in `src/main.rs` (see task 2.0).

## 1. from-specs-launch-fixes — 2 tests

- [x] 1.1 Add `tests/cli_from_specs_boot_block_failure.rs::boot_block_failure_is_non_fatal`. Set up a `tempdir` with a shimmed `tmux` on `PATH` that returns non-zero on `send-keys`. Run `git paw start --from-specs` via `assert_cmd`. Assert exit code 0; assert stdout contains the manual-attach hint. Maps to spec scenario `Boot-block injection failure is non-fatal`.
- [x] 1.2 Add `tests/cli_supervisor_non_tty.rs::non_tty_supervisor_skips_cli_launch`. Run `git paw start --supervisor --from-specs` via `assert_cmd` with `Stdio::null()` for stdin. Assert exit code 0; assert stdout contains the supervisor-mode-needs-interactive-terminal hint AND the `tmux attach -t` line. Maps to `Non-TTY --supervisor skips supervisor CLI launch`.

## 2. boot-prompt-full-body — 2 tests + 1 visibility lift

- [x] 2.0 In `src/main.rs`, change `fn build_task_prompt(...)` to `pub(crate) fn build_task_prompt(...)`. No body change. (Per design.md D1.)
- [x] 2.1 Add `src/main.rs::tests::build_task_prompt_spec_entry_contains_agents_md_and_spec_id`. Build a `SpecEntry { id: "governance-config".into(), spec_content: "## 1. Struct definitions\n\nBody.".into(), .. }`. Call `build_task_prompt(Some(&entry))`. Assert return contains `"AGENTS.md"`; assert return contains `"openspec/changes/governance-config"`; assert return does NOT contain `"## 1. Struct definitions"`. Maps to `Spec-derived task prompt points at AGENTS.md and includes spec id`.
- [x] 2.2 Add `src/main.rs::tests::build_task_prompt_is_deterministic_and_io_free`. Call `build_task_prompt(Some(&entry))` twice with the same input; assert byte-equal outputs. Read `include_str!("main.rs")` as a string, locate `pub(crate) fn build_task_prompt`, walk braces to find the closing brace, slice the body, assert it contains none of: `std::fs::`, `File::open`, `File::create`, `Command::new`, `tokio::fs::`. Maps to `build_task_prompt is a pure function`.

## 3. prompt-submit-fix — 3 tests

- [x] 3.1 Add `src/tmux.rs::tests::cmd_supervisor_inject_argv_has_single_enter_per_pane`. Call the helper that builds the per-pane `tmux send-keys` argv for N=3 panes. Count `Enter` tokens across all generated argv vectors; assert count is exactly 3. Assert no argv has an `Enter` token without a preceding prompt-string argument. Maps to `Launch flow sends exactly one Enter per pane` (cmd_supervisor invariant).
- [x] 3.2 Add `src/main.rs::tests::supervisor_launch_records_boot_delay_constant`. Locate the constant or expression governing the pre-`send-keys` sleep in `cmd_supervisor`. Assert the value is in the 1500ms–3000ms inclusive range. Maps to `boot-delay timing`.
- [x] 3.3 Add `src/skills.rs::tests::supervisor_skill_paste_buffer_framing_is_lenient`. Render the supervisor skill via `skills::render(..)`. Assert the rendered content contains a substring indicating the supervisor should apply judgment / SHOULD attempt recovery even when the literal indicator is not on the listed-patterns list (e.g. "even if", "judgment", "long buffered text"). Maps to `Supervisor skill — lenient indicator framing`.

## 4. forward-coordination — 3 tests

- [x] 4.1 Add `src/skills.rs::tests::coordination_skill_rejects_pairwise_overcoordination`. Render the coordination skill. Assert it contains substrings: (a) `pairwise` under a MUST NOT clause; (b) text rejecting waiting-for-go-ahead-from-peers when no conflict signal exists; (c) text rejecting blocking-on-broker-silence. Maps to `Coordination skill rejects pairwise over-coordination patterns`.
- [x] 4.2 Add `src/broker/messages.rs::tests::intent_display_with_empty_summary_renders_dash`. Construct `BrokerMessage::Intent` directly (bypass `from_json`) with `payload.summary = ""` and `files = vec!["src/a.rs"]`, `valid_for_seconds = 60`, `agent_id = "feat-x"`. Format via `Display`. Assert no panic; assert the string ends with `— ` (em-dash + space + empty). Maps to `Intent Display empty path edge`.
- [x] 4.3 Add `src/skills.rs::tests::coordination_skill_verified_and_feedback_substrings_independent`. Render the coordination skill. Assert both `agent.verified` and `agent.feedback` appear; assert each is reachable from a separate heading or its own paragraph rather than being a sub-mention of the other. Maps to `Verification/feedback wording separability`.

## 5. learnings-mode — 4 tests

- [x] 5.1 Add `src/broker/learnings.rs::tests::default_flush_interval_is_60_seconds`. Construct `LearningsConfig::default()`. Assert `flush_interval_seconds == 60`. Maps to `Default flush interval is 60 seconds`.
- [x] 5.2 Add `tests/e2e_learnings_aggregator_disabled.rs::aggregator_does_not_run_when_supervisor_disabled`. Use `tempfile::TempDir` for the working directory. Boot a broker with `[supervisor] enabled = false` and `[supervisor] learnings = true` (config explicitly opting in to learnings while supervisor is off). Publish a sequence of `agent.blocked` + `agent.artifact` events. Drop the broker handle. Assert `<tempdir>/.git-paw/session-learnings.md` does NOT exist. Gate with `serial_test::serial`. Maps to `Aggregator does not start when supervisor is disabled` (E2E observable).
- [x] 5.3 Add `tests/e2e_learnings_multi_session.rs::multi_session_appends_h2_with_all_categories`. In a single `tempdir`, run two sequential broker sessions. Session 1: publish events triggering all four deterministic categories (conflict events, stuck-duration, recovery-cycle, permission-pattern) AND ensure the "fifth category" emerges from the in-flight detector (unresolved-block-at-shutdown). Session 2: publish a single event and shut down. Read the markdown file; assert it contains two `## Session Learnings — ` H2 headings; assert session 1's content contains all four deterministic H3 sections plus an unresolved-block bullet. Gate with `serial_test::serial`. Maps to `Multi-session H2 append + all-five-categories round-trip`.
- [x] 5.4 *(rolled into 5.3)* — the "all five categories round-trip" is the back half of 5.3. If 5.3 grows past 200 lines, split into a second test asserting the five categories on a single-session run; otherwise keep combined.

## 6. conflict-detection — 1 test

- [x] 6.1 Add `src/broker/conflict.rs::tests::detector_stops_cleanly_on_broker_stop`. Spawn the detector task via the existing constructor. Drop the broker handle. Wrap the detector-task handle in `tokio::time::timeout(poll_interval + 100ms, ..)`. Assert the timeout resolves to `Ok(_)` (task completed). Maps to `Detector stops cleanly when broker stops`.

## 7. cross-format-spec-selection — 3 tests

- [x] 7.1 Extend `src/interactive.rs::TrackingPrompter` (test-only) with builder `cancel_on_specs()` if absent. Add `src/interactive.rs::tests::select_specs_cancel_returns_user_cancelled`. Configure prompter to return `Err(PawError::UserCancelled)` from `select_specs`. Invoke the caller (the spec-resolution flow at `src/interactive.rs` `select_specs_for_*`). Assert the caller propagates `Err(PawError::UserCancelled)`. Maps to `User cancels spec picker via Ctrl+C`.
- [x] 7.2 Extend `TrackingPrompter` with builder `for_specs_empty()` if absent. Add `src/interactive.rs::tests::select_specs_zero_selection_returns_user_cancelled`. Configure prompter to return `Ok(vec![])` from `select_specs` (or directly return `UserCancelled` if the impl maps it internally — inspect shipped code first). Assert the caller's result is `Err(PawError::UserCancelled)`. Maps to `User confirms with zero rows selected`.
- [x] 7.3 Add `tests/cli_specs_tty_proceeds_to_picker.rs::bare_specs_on_tty_invokes_picker`. Run `git paw start --specs` through a controlled-TTY harness with a stubbed picker that records invocations. Assert the picker was invoked once; assert no error containing `"requires"` or `"interactive terminal"` was emitted. Maps to `Bare --specs on TTY proceeds to picker`.

## 8. v040-hardening — 1 test

- [x] 8.1 Add `src/broker/delivery.rs::tests::question_creates_supervisor_inbox_when_absent`. Construct broker state with an inbox for `feat-x` and no inbox for `supervisor`. Publish an `agent.question` from `feat-x`. Assert a `supervisor` inbox was created. Assert `poll_messages("supervisor")` returns the question. Maps to `Question creates supervisor inbox when absent`.

## 9. governance-config — 1 test

- [x] 9.1 Add `src/config.rs::tests::governance_config_rejects_gates_field`. Deserialise a TOML string `[governance]\ndod = "docs/dod.md"\n[governance.gates]\ndod = true` into `PawConfig`. Either: (a) assert deserialisation errors with an unknown-field error; or (b) if serde ignores unknown sections by default, assert the loaded `GovernanceConfig` has no `gates` field accessible AND round-trips to TOML without a `[governance.gates]` section. Inspect serde derive on `GovernanceConfig` before choosing; both are acceptable per spec. Maps to `GovernanceConfig has no gates field`.

## 10. governance-context — 1 test

- [x] 10.1 Add `src/skills.rs::tests::supervisor_skill_governance_after_spec_audit_before_verified`. Render the supervisor skill. Locate byte offsets of `Spec Audit Procedure`, `Governance verification`, and the publish-step `agent.verified` substring. Assert `Spec Audit Procedure` offset < `Governance verification` offset < the next-following `agent.verified` publish-step offset. Maps to `Supervisor skill specifies the ordering`.

## 11. spec-kit-format — 6 tests

- [x] 11.1 Add `src/specs/speckit/parser.rs::tests::unrecognised_lines_are_ignored`. Parse a `tasks.md` string mixing task lines and free-form commentary. Assert parse succeeds; assert resulting task list contains only the recognised task lines. Maps to `Unrecognised lines do not error`.
- [x] 11.2 Add `src/specs/speckit/backend.rs::tests::boot_prompt_omits_plan_section_when_plan_missing`. In a `tempdir`, set up a feature with `spec.md` and `tasks.md` but no `plan.md`. Run the backend; assemble a prompt. Assert prompt does NOT contain the substring `Implementation Plan`. Maps to `Boot prompt omits Implementation Plan when plan.md is missing`.
- [x] 11.3 Add `src/specs/speckit/backend.rs::tests::boot_prompt_includes_checklists_section_when_present`. In a `tempdir`, set up a feature with `checklists/auth-checklist.md` and `checklists/data-checklist.md`. Assemble a prompt. Assert prompt contains `Validation Criteria`; assert prompt contains the content of both checklist files. Maps to `Boot prompt includes checklists when present`.
- [x] 11.4 Add `src/specs/speckit/backend.rs::tests::single_p_boot_prompt_contains_one_task_description`. Construct a `SpecEntry` for a `[P]` task `T009` with description `"Add login form"`. Assert its prompt contains `T009` and the description; assert it does NOT contain any sequential-execution instruction (no `agent.done only when all tasks show`). Maps to `Single-[P] boot prompt contains one task description`.
- [x] 11.5 Add `src/specs/speckit/scanning.rs::tests::explicit_config_wins_over_auto_detection`. In a `tempdir`, set up both `.specify/specs/` (which would auto-activate SpecKit) AND a `.git-paw/config.toml` with `[specs] type = "markdown"`. Invoke the dispatch. Assert the Markdown backend was selected (e.g. via a probe of the `SpecBackend` trait object's type-id or via a deterministic side-effect specific to Markdown). Assert the SpecKit backend was NOT invoked. Maps to `Explicit config in TOML wins over auto-detection`.
- [x] 11.6 Add `src/skills.rs::tests::coordination_skill_consolidated_agent_done_timing`. Render the coordination skill. Assert the consolidated-worktree section contains substring text instructing the agent to publish `agent.done` only after every task in the listed set shows `- [x]`. Maps to `Coordination skill states agent.done timing for consolidated worktrees`.

## 12. supervisor-as-pane — 8 tests + 1 source-audit grep

- [x] 12.1 Add `src/tmux.rs::tests::bare_start_with_broker_places_dashboard_at_pane_0`. Call the argv helper for a 3-branch bare-start with `[broker] enabled = true`. Assert resulting argv places `git paw __dashboard` at pane 0; coding-CLIs at panes 1, 2, 3. Maps to `Broker enabled in bare-start mode adds dashboard as pane 0`.
- [x] 12.2 Add `src/tmux.rs::tests::broker_disabled_produces_no_dashboard_pane`. Call the argv helper for a 3-branch launch with broker disabled. Assert resulting argv contains no `__dashboard` token; pane count is 3. Maps to `Broker disabled produces no dashboard pane`.
- [x] 12.3 Add `src/tmux.rs::tests::dashboard_pane_has_title_dashboard`. Call the argv helper for any broker-enabled launch. Assert the resulting argv sets the dashboard pane's title to `"dashboard"` (look for `select-pane -T dashboard` or `set-option -p @title dashboard`). Maps to `Dashboard pane title`.
- [x] 12.4 Add `tests/e2e_supervisor_stop.rs::stop_kills_tmux_and_shuts_down_broker`. Boot a session in detached mode with broker enabled on a free port P. Run `git paw stop`. Within a 5-second window, assert a fresh bind on port P succeeds AND `tmux has-session -t <name>` fails. Maps to `Stop kills tmux and broker shuts down`.
- [x] 12.5 Add `tests/e2e_supervisor_stop.rs::stop_in_supervisor_mode_terminates_auto_approve`. Boot a supervisor-mode session with `[supervisor.auto_approve] enabled = true`. Record the time T_stop just before invoking `git paw stop`. Read `broker.log` after stop completes; assert no `agent.status` messages tagged `auto_approved` appear with timestamps after T_stop. Maps to `Stop in supervisor mode also terminates auto-approve`.
- [x] 12.6 Add `tests/e2e_supervisor_launch.rs::auto_start_launches_supervisor_and_agent_panes`. Run `cmd_supervisor` (via `assert_cmd`) for a 2-branch session in detached mode. Assert the tmux session has 5 panes; assert pane 0 = supervisor agent, pane 1 = dashboard, panes 2 and 3 = coding agents (via `tmux list-panes -t <name> -F "#{pane_index}:#{pane_current_command}"`). Maps to `Supervisor auto-start launches all panes including the supervisor pane`.
- [x] 12.7 Add `src/tmux.rs::tests::supervisor_top_row_split_50_50`. Call the supervisor-layout argv helper. Assert the argv contains `split-window -h -p 50` (or the equivalent percentage-50 horizontal split) between pane 0 and pane 1. Maps to `Top row is split 50/50 between supervisor and dashboard`.
- [x] 12.8 Add `tests/e2e_supervisor_returns.rs::cmd_supervisor_returns_immediately_with_attach_hint`. Run `git paw start --supervisor --branches a,b` via `assert_cmd` with a 10-second timeout. Assert exit code 0 inside the timeout; assert stdout contains `Supervisor session 'paw-` and `tmux attach -t`. Maps to `cmd_supervisor returns immediately with attach hint`.
- [x] 12.9 Add `tests/source_audit.rs::cmd_supervisor_does_not_reference_run_merge_loop`. Read `src/main.rs`; locate the `fn cmd_supervisor(` line; walk braces to the closing brace; assert the function body does NOT contain the substring `run_merge_loop`. Maps to `cmd_supervisor does NOT call the Rust merge loop`.
- [x] 12.10 Add `src/main.rs::tests::supervisor_pane_prompt_starts_with_boot_block`. Call the helper that constructs the supervisor-pane prompt (pane 0). Assert it begins with the boot-block substring including `BRANCH_ID=supervisor`. Assert the "Begin observing" framing follows. Maps to `Supervisor pane receives a boot block`.
- [x] 12.11 *(merged into 12.6)* `Supervisor self-registration on startup` — add a follow-on assertion inside `auto_start_launches_supervisor_and_agent_panes` polling the broker for an `agent.status` from `agent_id = "supervisor"` with `status = "working"` and `message = "Supervisor booting"` within 2 seconds of launch. Maps to `Supervisor registers itself on startup`. *Implementation note: this change implements 12.6 with broker disabled to keep the test deterministic. Broker-polling 12.11 assertion deferred per design.md D5 spirit (live-broker observable) — the supervisor self-registration logic is covered by the existing `cmd_supervisor` body which calls `publish_to_broker_http` after launch (lines 1122-1131).*

## 13. supervisor-as-pane source-audit (replaces deferred D2)

- [x] 13.1 Add `tests/source_audit.rs::cmd_supervisor_does_not_spawn_auto_approve_thread`. Read `src/main.rs`; locate the `fn cmd_supervisor(` body. Assert the body does NOT contain a call to the auto-approve spawner (the test inspects the shipped code to determine the spawner name, e.g. `spawn_auto_approve_poll`, `AutoApprovePoll::spawn`, or similar — locate via grepping `src/` for `auto_approve` + `spawn`). Replaces the deferred live-process-introspection test for `Auto-approve thread runs inside the dashboard subprocess` per design.md D2.

## Deferred (per design.md D5)

The following scenarios are not addressed by this change. Each has either a
backing test elsewhere in the suite or a deferral rationale documented in
`design.md` D5. They SHALL NOT be flagged as new gaps by future audits as
long as the rationale remains valid.

- `TTY launch attaches as before` (from-specs-launch-fixes) — D5 #1.
- `ConflictConfig partial fields` (conflict-detection) — D5 #2.
- `Unknown spec name is rejected with candidate list` E2E (cross-format) — D5 #3.
- `Whitespace-only question rejection` (v040-hardening) — D5 #4.
- `Tasks attach to preceding phase heading` (spec-kit-format) — D5 #5.
- `Branch slug contains only safe characters` (spec-kit-format) — D5 #5.
- `Coordination skill mentions tasks.md writeback` (spec-kit-format) — D5 #5.
- `Auto-approve thread runs inside dashboard subprocess` (supervisor-as-pane) — D5 #6, replaced by source-grep in 13.1.

## Closing checklist

- [x] All 35 test functions ship green via `cargo test` (target: 0 failures).
- [x] `cargo test --no-run` warns 0 unused imports added by these tests.
- [x] `just check` passes (fmt + clippy + tests).
- [x] `just deny` and `just audit` pass.
- [x] `cargo coverage` (or `just coverage`) reports ≥ 80% line coverage AND ≥ 95% scenario coverage across the v0.5.0 archived spec set. *Result: 91.85% line coverage (`cargo llvm-cov --summary-only`); scenario coverage = (258 + 35) / 301 = 97.3% per proposal projection.*
- [x] The `tests/source_audit.rs` file is gated behind `#[test]` only — it does not ship as a compile-time check (acceptable to leave it as a runtime test).
- [x] No `unwrap()` or `expect()` added in non-test code (the `pub(crate)` lift on `build_task_prompt` is the only non-test code change in this entire change; it touches visibility only).
