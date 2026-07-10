# test-coverage-v0-5-0 Specification

## Purpose
Records the scenario-to-test mapping that backfills test coverage for the v0.5.0 archived changes, naming the specific `#[test]` functions that must exist for each previously-uncovered spec scenario. This is a bookkeeping spec, not a runtime capability.

## Requirements
### Requirement: Scenario-to-test mapping for from-specs-launch-fixes

The test suite SHALL contain at least one `#[test]` per uncovered scenario for the archived change `2026-05-10-from-specs-launch-fixes`. The mapping is:

- `Boot-block injection failure is non-fatal` →
  `tests/cli_from_specs_boot_block_failure.rs::boot_block_failure_is_non_fatal`
  (asserts exit code 0 + attach-hint on stdout when `tmux send-keys`
  returns non-zero — simulated via a shimmed `PATH` tmux).
- `Non-TTY --supervisor skips supervisor CLI launch` →
  `tests/cli_supervisor_non_tty.rs::non_tty_supervisor_skips_cli_launch`
  (assert_cmd run with `Stdio::null()` for stdin; assert stdout contains the
  supervisor-mode-needs-interactive-terminal hint AND the attach command).

The `TTY launch attaches as before` scenario is intentionally deferred per
design.md D5 item 1.

#### Scenario: Boot-block injection failure non-fatal test exists

- **WHEN** the test suite for this capability is enumerated
- **THEN** `tests/cli_from_specs_boot_block_failure.rs::boot_block_failure_is_non_fatal` is present
- **AND** the test asserts the process exit code is `0` even when the simulated `tmux send-keys` invocation returns non-zero
- **AND** the test asserts stdout contains the manual-attach hint

#### Scenario: Non-TTY supervisor-skip test exists

- **WHEN** the test suite for this capability is enumerated
- **THEN** `tests/cli_supervisor_non_tty.rs::non_tty_supervisor_skips_cli_launch` is present
- **AND** the test asserts the foreground supervisor-CLI launch is skipped under non-TTY stdin
- **AND** the test asserts stdout contains the supervisor-mode-requires-interactive-terminal hint

### Requirement: Scenario-to-test mapping for boot-prompt-full-body

The test suite SHALL contain the following tests for the archived change `2026-05-11-boot-prompt-full-body`:

- `Spec-derived task prompt points at AGENTS.md and includes spec id` →
  `src/main.rs::tests::build_task_prompt_spec_entry_contains_agents_md_and_spec_id`.
- `build_task_prompt is a pure function` →
  `src/main.rs::tests::build_task_prompt_is_deterministic_and_io_free`.

The visibility of `build_task_prompt` SHALL be `pub(crate) fn` (per
design.md D1) so the tests can call it directly without going through
`cmd_supervisor`.

#### Scenario: Spec-id substring test exists

- **GIVEN** a `SpecEntry` with `id = "governance-config"` and a non-empty `spec_content`
- **WHEN** `build_task_prompt(Some(&entry))` is called from the test
- **THEN** the returned string contains the substring `AGENTS.md`
- **AND** the returned string contains the substring `openspec/changes/governance-config`
- **AND** the returned string does NOT contain the first body line of `spec_content` in raw form (no truncated heading like `## 1. Code fix`)

#### Scenario: Purity test exists

- **WHEN** `build_task_prompt(Some(&entry))` is called twice with the same `SpecEntry` input
- **THEN** the two returned strings are byte-equal (deterministic output)
- **AND** static inspection of the function body (between its opening and closing braces in `src/main.rs`) does NOT contain `std::fs::`, `File::open`, `File::create`, or `Command::new` tokens

### Requirement: Scenario-to-test mapping for prompt-submit-fix

For the archived change `2026-05-11-prompt-submit-fix`, the test suite SHALL
contain:

- `Launch flow sends exactly one Enter per pane` (cmd_supervisor invariant)
  → `src/tmux.rs::tests::cmd_supervisor_inject_argv_has_single_enter_per_pane`
  (asserts the argv returned by the helper that builds the per-pane
  send-keys arguments contains exactly one `Enter` token per pane).
- `Boot-delay timing` (the ~2 second sleep before send-keys) →
  `src/main.rs::tests::supervisor_launch_records_boot_delay_constant`
  (asserts the sleep duration constant referenced by `cmd_supervisor` is in
  the documented 1.5s–3s window).
- `Supervisor skill — lenient indicator framing` substring →
  `src/skills.rs::tests::supervisor_skill_paste_buffer_framing_is_lenient`
  (asserts the rendered supervisor skill content contains a substring
  indicating the supervisor SHOULD apply judgment beyond the listed
  indicator patterns).

#### Scenario: Single-Enter invariant test exists

- **WHEN** the helper that builds the per-pane `tmux send-keys` argv for `cmd_supervisor` is invoked for N panes
- **THEN** the assertion in `src/tmux.rs::tests::cmd_supervisor_inject_argv_has_single_enter_per_pane` SHALL count exactly N `Enter` tokens in the combined argv set
- **AND** SHALL assert no additional standalone `Enter` invocations are recorded

#### Scenario: Boot-delay timing test exists

- **WHEN** `src/main.rs::tests::supervisor_launch_records_boot_delay_constant` is run
- **THEN** the constant or expression governing the pre-`send-keys` sleep SHALL be asserted to be in the 1.5s–3s window
- **AND** the test SHALL fail if the constant is removed or set outside the window

#### Scenario: Lenient indicator-framing substring test exists

- **WHEN** the rendered supervisor skill content is inspected
- **THEN** it contains substring text indicating the supervisor SHOULD apply judgment if a pane shows long buffered text without a follow-up response, even when the literal indicator string is not on the listed-patterns list

### Requirement: Scenario-to-test mapping for forward-coordination

The test suite SHALL contain the following tests for the archived change `2026-05-13-forward-coordination`:

- `Coordination skill rejects pairwise over-coordination patterns` →
  `src/skills.rs::tests::coordination_skill_rejects_pairwise_overcoordination`
  (assert the rendered coordination skill content contains all three MUST NOT
  substrings: pairwise check-ins on every change, waiting for go-ahead from
  peers when no conflict signal exists, blocking on broker silence).
- `Intent Display empty path edge` →
  `src/broker/messages.rs::tests::intent_display_with_empty_summary_renders_dash`
  (asserts the `Display` impl renders cleanly when `summary` is an
  empty-after-trim string — the validator rejects empty summary at
  `from_json`, but the `Display` impl is independent and should not panic
  on directly-constructed values).
- `Verification/feedback wording separability` →
  `src/skills.rs::tests::coordination_skill_verified_and_feedback_substrings_independent`
  (asserts the rendered coordination skill content contains BOTH
  `agent.verified` AND `agent.feedback` as independent substrings — neither
  is referenced solely via the other's surrounding prose).

#### Scenario: Pairwise-rejection test exists

- **WHEN** the rendered coordination skill content is inspected by `coordination_skill_rejects_pairwise_overcoordination`
- **THEN** the test SHALL assert the content contains the substring `pairwise` (or equivalent indicator) under a MUST NOT directive
- **AND** the test SHALL assert the content contains text instructing agents NOT to wait for go-ahead from peers when no conflict signal exists
- **AND** the test SHALL assert the content contains text instructing agents NOT to block on broker silence

#### Scenario: Intent Display edge test exists

- **WHEN** an `Intent` value constructed directly (bypassing `from_json`) with `summary = ""` is formatted via `Display`
- **THEN** the resulting string does not panic and matches the format `[<agent_id>] intent: <N> files for <ttl>s — `
- **AND** the trailing portion after the em-dash is the empty string

#### Scenario: Verified-feedback-separability test exists

- **WHEN** the rendered coordination skill content is inspected
- **THEN** the test asserts both substrings `agent.verified` and `agent.feedback` are present
- **AND** each is reachable from a heading describing its own message variant (not solely as a sub-bullet under the other)

### Requirement: Scenario-to-test mapping for learnings-mode

For the archived change `2026-05-13-learnings-mode`, the test suite SHALL
contain:

- `Default flush interval is 60 seconds` →
  `src/broker/learnings.rs::tests::default_flush_interval_is_60_seconds`
  (assert `LearningsConfig::default().flush_interval_seconds == 60`).
- `Aggregator does not start when supervisor is disabled` (E2E — no
  `session-learnings.md` produced) →
  `tests/e2e_learnings_aggregator_disabled.rs::aggregator_does_not_run_when_supervisor_disabled`
  (boot a broker with `[supervisor] enabled = false` and `[supervisor]
  learnings = true`, publish a sequence of messages, drop the handle, assert
  `.git-paw/session-learnings.md` does NOT exist).
- `Multi-session H2 append + all-five-categories round-trip` →
  `tests/e2e_learnings_multi_session.rs::multi_session_appends_h2_with_all_categories`
  (two sequential aggregator sessions in a `tempdir`; first session publishes
  events triggering all five categories; second session publishes a single
  event; assert file contains two `## Session Learnings — ...` H2s and the
  first session's content covers all five H3 categories).

The first-session-five-categories portion uses `serial_test::serial`
because the aggregator writes a fixed filename (`session-learnings.md`)
and parallel tests in the same `tempdir` would interleave writes.

#### Scenario: Default-interval test exists

- **WHEN** `default_flush_interval_is_60_seconds` is run
- **THEN** the test asserts `LearningsConfig::default().flush_interval_seconds == 60`

#### Scenario: Aggregator-disabled E2E test exists

- **GIVEN** a broker booted with `[supervisor] enabled = false` and `[supervisor] learnings = true`
- **WHEN** the broker processes events and is shut down
- **THEN** `.git-paw/session-learnings.md` SHALL NOT exist on disk

#### Scenario: Multi-session H2 + five-categories test exists

- **GIVEN** two sequential aggregator sessions in the same `tempdir`
- **WHEN** the second session flushes
- **THEN** `.git-paw/session-learnings.md` contains two distinct `## Session Learnings — \d{4}-...Z` H2 headings
- **AND** the first session's content includes H3 headings for `Conflict events`, `Where agents got stuck`, `Recovery cycles`, and `Permission patterns`
- **AND** the first session's content includes content covering the fifth category (the stuck-or-unresolved entries from the in-flight detector)

### Requirement: Scenario-to-test mapping for conflict-detection

The test suite SHALL contain the following test for the archived change `2026-05-13-conflict-detection`:

- `Detector stops cleanly on broker stop` →
  `src/broker/conflict.rs::tests::detector_stops_cleanly_on_broker_stop`
  (spawn the detector task, drop the broker handle, assert the task
  completes within one poll interval via a `tokio::time::timeout` wrapper).

The `ConflictConfig partial-fields` scenario is intentionally deferred per
design.md D5 item 2 — it is a linear combination of the
defaults-when-absent and all-fields-populated tests already in the suite.

#### Scenario: Detector-clean-stop test exists

- **GIVEN** a running conflict-detector task wired to a broker handle
- **WHEN** the broker handle is dropped
- **THEN** `tokio::time::timeout(poll_interval + 100ms, detector_task)` SHALL resolve to `Ok(_)` (task completed before the timeout fired)
- **AND** no further auto-emitted messages SHALL be published after the drop

### Requirement: Scenario-to-test mapping for cross-format-spec-selection

The test suite SHALL contain the following tests for the archived change `2026-05-13-cross-format-spec-selection`:

- `User cancels picker via Ctrl+C` →
  `src/interactive.rs::tests::select_specs_cancel_returns_user_cancelled`
  (use `TrackingPrompter::cancel_on_specs()` builder; assert the caller
  returns `Err(PawError::UserCancelled)`).
- `User confirms with zero rows selected` →
  `src/interactive.rs::tests::select_specs_zero_selection_returns_user_cancelled`
  (use `TrackingPrompter::for_specs_empty()` builder; assert the caller
  returns `Err(PawError::UserCancelled)`).
- `Bare --specs on TTY proceeds to picker` →
  `tests/cli_specs_tty_proceeds_to_picker.rs::bare_specs_on_tty_invokes_picker`
  (use `assert_cmd` with a controlled `Prompter` injected via a test-only
  hook; assert `select_specs` is invoked exactly once and no TTY-required
  error is emitted).

The `Unknown spec rejection E2E` scenario is intentionally deferred per
design.md D5 item 3 — already covered by the name-resolution unit test.

#### Scenario: Picker-cancellation test exists

- **GIVEN** a `TrackingPrompter` configured to return `Err(PawError::UserCancelled)` from `select_specs`
- **WHEN** the caller's flow runs
- **THEN** the caller SHALL return `Err(PawError::UserCancelled)`

#### Scenario: Zero-selection-cancellation test exists

- **GIVEN** a `TrackingPrompter` configured to return `Ok(vec![])` from `select_specs` (or, equivalently, the `select_specs` impl that maps zero-selection to `UserCancelled` internally)
- **WHEN** the caller's flow runs
- **THEN** the caller SHALL return `Err(PawError::UserCancelled)`

#### Scenario: Bare --specs TTY-picker test exists

- **WHEN** `git paw start --specs` runs under a controlled TTY context with a stubbed picker
- **THEN** the stubbed `select_specs` SHALL be invoked exactly once
- **AND** no error containing the substring "requires" or "interactive terminal" SHALL be emitted

### Requirement: Scenario-to-test mapping for v040-hardening

For the archived change `2026-05-13-v040-hardening`, the test suite SHALL
contain:

- `Question creates supervisor inbox when absent` →
  `src/broker/delivery.rs::tests::question_creates_supervisor_inbox_when_absent`
  (assert that publishing an `agent.question` when no `supervisor` inbox
  exists creates the inbox and routes the message there).

The `Whitespace-only question rejection` scenario is intentionally deferred
per design.md D5 item 4 — already covered by the existing validation test
for `payload.question` empty-or-whitespace rejection.

#### Scenario: Auto-create-supervisor-inbox test exists

- **GIVEN** a broker state containing an inbox for `feat-x` but no inbox for `supervisor`
- **WHEN** `publish_message` is called with an `agent.question` from `feat-x`
- **THEN** the test asserts a new inbox is created for `supervisor`
- **AND** the test asserts the question message is present in the new `supervisor` inbox
- **AND** subsequent `poll_messages` for `supervisor` returns the question

### Requirement: Scenario-to-test mapping for governance-config

For the archived change `2026-05-13-governance-config`, the test suite SHALL
contain:

- `GovernanceConfig has no gates field` →
  `src/config.rs::tests::governance_config_rejects_gates_field`
  (negative-assertion: deserialise a TOML string containing
  `[governance.gates] dod = true` and assert deserialisation either errors
  with an unknown-field error OR (if serde is configured to ignore unknown
  fields) returns a `GovernanceConfig` value whose serialised form does
  NOT contain a `gates` key).

#### Scenario: No-gates-field test exists

- **WHEN** a TOML string containing `[governance] dod = "docs/dod.md"\n[governance.gates]\ndod = true` is deserialised into `PawConfig`
- **THEN** the resulting `config.governance` value SHALL NOT expose a `gates` field on any public method or serialised output
- **AND** round-tripping the loaded config back to TOML SHALL NOT include a `[governance.gates]` section

### Requirement: Scenario-to-test mapping for governance-context

The test suite SHALL contain the following test for the archived change `2026-05-13-governance-context`:

- `Supervisor skill specifies the ordering` →
  `src/skills.rs::tests::supervisor_skill_governance_after_spec_audit_before_verified`
  (assert the rendered supervisor skill content positions the
  Governance-verification section text AFTER the Spec-Audit-Procedure
  section text AND BEFORE the `agent.verified`-publish step text, by
  comparing the substring byte-offsets in the rendered output).

#### Scenario: Governance-ordering test exists

- **WHEN** the rendered supervisor skill content is inspected
- **THEN** the byte offset of the substring `Governance verification` SHALL be greater than the byte offset of the substring `Spec Audit Procedure`
- **AND** the byte offset of `Governance verification` SHALL be less than the byte offset of the next subsequent `agent.verified` substring describing the publish step

### Requirement: Scenario-to-test mapping for spec-kit-format

For the archived change `2026-05-13-spec-kit-format`, the test suite SHALL
contain:

- `Unrecognised lines do not error` →
  `src/specs/speckit/parser.rs::tests::unrecognised_lines_are_ignored`
  (parse a `tasks.md` containing free-form commentary between task lines;
  assert the parser succeeds and the commentary is not associated with any
  task).
- `Boot prompt omits Implementation Plan when plan.md is missing` →
  `src/specs/speckit/backend.rs::tests::boot_prompt_omits_plan_section_when_plan_missing`
  (assert the assembled `SpecEntry.prompt` does NOT contain the
  `Implementation Plan` heading when `plan.md` is absent).
- `Boot prompt includes checklists when present` →
  `src/specs/speckit/backend.rs::tests::boot_prompt_includes_checklists_section_when_present`
  (assert the assembled prompt contains the `Validation Criteria` heading
  and the content of each checklist file when `<feature>/checklists/` is
  populated).
- `Single-[P] boot prompt contains one task description` →
  `src/specs/speckit/backend.rs::tests::single_p_boot_prompt_contains_one_task_description`
  (assert a `[P]`-entry's prompt contains its T-id and description but no
  sequential-execution instruction).
- `Explicit config in TOML wins over auto-detect` →
  `src/specs/speckit/scanning.rs::tests::explicit_config_wins_over_auto_detection`
  (set up a tempdir with `.specify/specs/` AND `[specs] type = "markdown"`
  in config; assert the Markdown backend is selected; assert the SpecKit
  backend is NOT invoked).
- `Coordination skill — Spec Kit consolidated worktree behaviour
  agent.done timing` →
  `src/skills.rs::tests::coordination_skill_consolidated_agent_done_timing`
  (assert the rendered coordination skill content contains text instructing
  the agent to publish `agent.done` only after all listed tasks show `- [x]`
  in the consolidated-worktree section).

The `Tasks attach to the preceding phase heading`, `Branch slug contains
only safe characters`, and `Coordination skill mentions tasks.md writeback`
scenarios are intentionally deferred per design.md D5 item 5 — they have
existing tests.

#### Scenario: Unrecognised-lines-ignored test exists

- **GIVEN** a `tasks.md` string mixing task lines and free-form commentary
- **WHEN** the parser runs on the string
- **THEN** the parse succeeds (no error)
- **AND** the resulting task list contains only the recognised task lines

#### Scenario: Plan-absent-prompt test exists

- **GIVEN** a `tempdir` feature directory with `spec.md` and `tasks.md` but no `plan.md`
- **WHEN** `SpecKitBackend` assembles the prompt for that feature
- **THEN** the assembled prompt string does NOT contain the substring `Implementation Plan`

#### Scenario: Checklists-included-when-present test exists

- **GIVEN** a `tempdir` feature directory with `checklists/auth-checklist.md` and `checklists/data-checklist.md`
- **WHEN** the prompt is assembled
- **THEN** the prompt contains the substring `Validation Criteria`
- **AND** the prompt contains the file content of both checklists

#### Scenario: Single-[P]-body test exists

- **GIVEN** a `[P]` task `T009` with description `"Add login form"`
- **WHEN** the prompt is assembled for that `SpecEntry`
- **THEN** the prompt contains the substring `T009`
- **AND** the prompt contains the substring `Add login form`
- **AND** the prompt does NOT contain a sequential-execution instruction

#### Scenario: Explicit-config-precedence test exists

- **GIVEN** a `tempdir` containing both `.specify/specs/` and `.git-paw/config.toml` with `[specs] type = "markdown"`
- **WHEN** `git_paw::specs::scan(...)` is invoked
- **THEN** the test asserts the Markdown backend was used
- **AND** the test asserts the SpecKit backend was NOT invoked

#### Scenario: Consolidated-agent-done-timing test exists

- **WHEN** the rendered coordination skill content is inspected
- **THEN** the content contains substring text instructing the agent to publish `agent.done` only after every task in the consolidated worktree's list shows `- [x]` in `tasks.md`

### Requirement: Scenario-to-test mapping for supervisor-as-pane

For the archived change `2026-05-13-supervisor-as-pane`, the test suite SHALL
contain:

- `Broker enabled in bare-start mode adds dashboard as pane 0` →
  `src/tmux.rs::tests::bare_start_with_broker_places_dashboard_at_pane_0`
  (assert the argv contract puts the dashboard at index 0).
- `Broker disabled produces no dashboard pane` →
  `src/tmux.rs::tests::broker_disabled_produces_no_dashboard_pane`
  (assert the argv contract for a broker-disabled launch contains no
  `__dashboard` subcommand).
- `Dashboard pane title` →
  `src/tmux.rs::tests::dashboard_pane_has_title_dashboard`
  (assert the argv contract sets the dashboard pane's title to `"dashboard"`).
- `Stop kills tmux and broker shuts down` →
  `tests/e2e_supervisor_stop.rs::stop_kills_tmux_and_shuts_down_broker`
  (boot a session in detached mode, run `git paw stop`, assert the broker
  port is free within 5 seconds AND the tmux session is killed).
- `Stop in supervisor mode also terminates auto-approve` →
  `tests/e2e_supervisor_stop.rs::stop_in_supervisor_mode_terminates_auto_approve`
  (boot a supervisor-mode session with `[supervisor.auto_approve] enabled =
  true`, run `git paw stop`, assert no further auto-approve activity is
  recorded in `broker.log` after the stop timestamp).
- `Supervisor auto-start launches all panes including supervisor pane` →
  `tests/e2e_supervisor_launch.rs::auto_start_launches_supervisor_and_agent_panes`
  (assert the resulting tmux session has the expected pane count and the
  pane indices match the spec's pane-0 = supervisor / pane-1 = dashboard /
  pane-2..N+1 = agents convention).
- `Top row is split 50/50 between supervisor and dashboard` →
  `src/tmux.rs::tests::supervisor_top_row_split_50_50`
  (assert the argv contract contains `split-window -h -p 50` or equivalent
  for the supervisor/dashboard top-row split).
- `cmd_supervisor returns immediately with attach hint` →
  `tests/e2e_supervisor_returns.rs::cmd_supervisor_returns_immediately_with_attach_hint`
  (assert_cmd run with a 10-second deadline; assert exit code 0; assert
  stdout contains `"Supervisor session 'paw-"` and `"tmux attach -t"`).
- `cmd_supervisor does NOT call the Rust merge loop` →
  `tests/source_audit.rs::cmd_supervisor_does_not_reference_run_merge_loop`
  (static grep of `src/main.rs` between `fn cmd_supervisor` and its closing
  brace; assert the substring `run_merge_loop` does NOT appear).
- `Supervisor pane receives a boot block` →
  `src/main.rs::tests::supervisor_pane_prompt_starts_with_boot_block`
  (assert the constructed supervisor-pane prompt begins with the boot-block
  substring `BRANCH_ID=supervisor`).
- `Supervisor registers itself on startup` →
  `tests/e2e_supervisor_launch.rs::supervisor_self_registers_on_startup`
  (assert that after the launch sequence runs, an `agent.status` message
  with `agent_id = "supervisor"`, `status = "working"`, and `message =
  "Supervisor booting"` is present in the broker).

The `Auto-approve thread runs inside dashboard subprocess` scenario is
intentionally deferred per design.md D5 item 6 — replaced with a source-grep
negative assertion in `tests/source_audit.rs::cmd_supervisor_does_not_spawn_auto_approve_thread`.

#### Scenario: Pane-0-dashboard bare-start test exists

- **WHEN** the tmux-argv helper builds the argv for a 3-branch bare-start with broker enabled
- **THEN** the resulting argv places `git paw __dashboard` at pane index 0
- **AND** places the coding-agent CLIs at pane indices 1, 2, 3

#### Scenario: No-dashboard-when-broker-disabled test exists

- **WHEN** the tmux-argv helper builds the argv for a 3-branch launch with broker disabled
- **THEN** the resulting argv contains no `__dashboard` subcommand
- **AND** the pane count is exactly 3

#### Scenario: Dashboard-pane-title test exists

- **WHEN** the tmux-argv helper builds any broker-enabled launch
- **THEN** the resulting argv sets the dashboard pane's title to `"dashboard"`

#### Scenario: Stop-kills-tmux-and-broker test exists

- **GIVEN** an active session in detached mode with broker enabled on port P
- **WHEN** `git paw stop` is invoked and completes
- **THEN** the test asserts port P is free (a fresh bind on the same port succeeds) within 5 seconds
- **AND** the tmux session no longer exists

#### Scenario: Stop-terminates-auto-approve test exists

- **GIVEN** an active supervisor-mode session with auto-approve enabled
- **WHEN** `git paw stop` is invoked and the timestamp T_stop is recorded
- **THEN** no `agent.status` messages tagged `auto_approved` SHALL appear in `broker.log` with timestamps after T_stop

#### Scenario: Auto-start-launches-all-panes test exists

- **GIVEN** a supervisor-mode launch with two spec branches
- **WHEN** `cmd_supervisor` runs to completion
- **THEN** the test asserts the resulting tmux session has 5 panes
- **AND** pane 0 is the supervisor pane
- **AND** pane 1 is the dashboard
- **AND** panes 2 and 3 are coding-agent panes

#### Scenario: 50/50-top-row-split test exists

- **WHEN** the tmux-argv helper builds the supervisor-mode top row
- **THEN** the resulting argv contains an `-h -p 50` (or equivalent) split between pane 0 and pane 1

#### Scenario: cmd_supervisor-returns-immediately test exists

- **WHEN** `git paw start --supervisor --branches a,b` is invoked via `assert_cmd` with a 10-second deadline
- **THEN** the process exits with status 0 inside the deadline
- **AND** stdout contains the substring `Supervisor session 'paw-`
- **AND** stdout contains the substring `tmux attach -t`

#### Scenario: cmd_supervisor-no-merge-loop test exists

- **WHEN** the body of `cmd_supervisor` in `src/main.rs` is statically inspected
- **THEN** the substring `run_merge_loop` does NOT appear between the function's opening and closing braces

#### Scenario: Supervisor-pane-receives-boot-block test exists

- **WHEN** the constructed prompt for pane 0 (the supervisor pane) is inspected
- **THEN** it begins with the boot-block substring including `BRANCH_ID=supervisor`
- **AND** is followed by the "Begin observing" framing message

#### Scenario: Supervisor-self-registration test exists

- **GIVEN** a supervisor-mode launch
- **WHEN** the launch sequence reaches the self-registration step
- **THEN** an `agent.status` message is published with `agent_id = "supervisor"`, `status = "working"`, and `message = "Supervisor booting"`

### Requirement: Deferred scenarios

The following scenarios SHALL be considered intentionally deferred from this change per design.md D5. They SHALL NOT be flagged as new coverage gaps by future audits as long as their deferral rationale remains valid. If their backing test or design rationale changes, the deferral SHALL be reopened in a follow-up change.

| Scenario | Deferral reason |
|---|---|
| `TTY launch attaches as before` (from-specs-launch-fixes) | Requires live-pty harness; non-TTY path is tested and exercises the same flow. |
| `ConflictConfig partial fields` (conflict-detection) | Linear combination of defaults + all-fields tests. |
| `Unknown spec name is rejected with candidate list` E2E (cross-format) | Already covered by name-resolution unit test at lower layer. |
| `Whitespace-only question rejection` (v040-hardening) | Already covered by existing `payload.question` empty-or-whitespace validation test. |
| `Tasks attach to preceding phase heading` (spec-kit-format) | Already covered by existing parser test. |
| `Branch slug contains only safe characters` (spec-kit-format) | Already covered by existing slug-set test. |
| `Coordination skill mentions tasks.md writeback` (spec-kit-format) | Already covered by existing skill-content test. |
| `Auto-approve thread runs inside dashboard subprocess` (supervisor-as-pane) | Replaced with source-grep negative assertion in `tests/source_audit.rs`; live process-introspection deferred to follow-up. |

#### Scenario: Deferred scenarios are documented with rationale

- **WHEN** the deferred-scenarios table in this spec is inspected
- **THEN** every row names the scenario, its source spec, and the rationale for deferral
- **AND** no deferred scenario lacks a corresponding rationale entry

