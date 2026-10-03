Each numbered section is one commit and must leave `just check` green. A test that a section
breaks is deleted or rewritten in that same section; section 9 only adds the remaining
cross-module tests and checks traceability. Write each section's new or rewritten tests first
and watch them fail before changing the code. Keep `assets/scripts/*.sh` apostrophe-free inside
`$(cat <<'PY' …)` heredocs, and run `bash -n` after every script edit.

## 1. Split prompt detection out of `auto_approve.rs` (pure move, no behaviour change)

- [ ] 1.1 Record the baseline: `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast > target/cr-baseline.log 2>&1; echo $?`, and note the pass count
- [ ] 1.2 Move the detection items listed in design D4 (live-prompt gate constants and functions, `anchor_on_trailing_region`, input-box and rule-line parsing, `strip_decoration`, option-line and confirmation-boundary helpers, `extract_command_slice`, `file_prompt_regex`, `extract_path_from_file_prompt`) from `src/supervisor/auto_approve.rs` into `src/supervisor/permission_prompt.rs`, keeping each item's visibility and doc comments; update the module doc (`//!`) to "permission-prompt detection"
- [ ] 1.3 Move those items' unit tests with them, including `static_regexes_compile`; leave the classifier half and its tests in `auto_approve.rs`, importing what it still needs from `permission_prompt`
- [ ] 1.4 Update every importer: `drive.rs`, `approval_gate.rs`, `manual_approvals.rs`, `poll.rs`, `main.rs`, the doc links in `src/config/prompt_profile.rs`, and the tests under `tests/` that import moved items (`rtk grep "auto_approve::" src tests`)
- [ ] 1.5 Verify: the same pass count as 1.1, `tests/sweep_sh_live_gate_parity.rs` and `tests/cli_unavailable_markers_profile_parity.rs` green, and the diff is moves plus import paths only

## 2. `sweep.sh approve` answers once with option 1; remove `__classify`

- [ ] 2.1 Tests first (`tests/sweep_sh_approve_options.rs`). Delete `three_option_allowlisted_verb_takes_broad_grant` and `three_option_arbitrary_code_prompt_sends_one_time_yes`. Add:
  - a 3-option prompt sends `1` then `Enter`, never `2` (*approve answers once on a three-option prompt*);
  - a `git push --force origin main` prompt sends `1` + `Enter`, and a recording `git` shim on `PATH` sees no `paw __classify` (*approve does not judge the command*);
  - an option-1 line containing the broad-grant marker sends nothing and prints the reason (*approve refuses an option 1 that reads as a durable grant*);
  - a 2-option prompt sends no `Down`.

  Keep `cleared_prompt_sends_zero_keystrokes`
- [ ] 2.2 `assets/scripts/sweep.sh` `cmd_approve`: after the pane-0 refusal, the claim and the fresh capture, gate on `is_live_capture`. Find the option-1 line with `LIVE_PROMPT_OPTION_REGEX`. Refuse when it is missing or matches `BROAD_GRANT_MARKER_REGEX` (new, from `discover_prompt_profile_field broad_grant_marker` with an apostrophe-free fallback such as `ask again`). Then send `1`, `sleep 0.05`, `Enter`, and print `approved pane N (option 1)`
- [ ] 2.3 Delete `run_classifier`, `cmd_classify` and the `classify)` dispatch. Update the header subcommand list, `usage`, and the claim-section comment (the helper and the drive loop's nudges, not "three processes")
- [ ] 2.4 Remove `Command::Classify` (`src/cli.rs`) and its `main.rs` dispatch and `cmd_classify`. Delete the `cli.rs` `__classify` parse tests and its entries in the hidden-subcommand lists. Add a test that `git paw __classify` fails as an unrecognised subcommand and that `sweep.sh classify` exits 2 as unknown (*No classifier subcommand exists*)
- [ ] 2.5 Delete `tests/classify_subcommand.rs` and `tests/sweep_sh_classify.rs`. Update the expected output strings in `tests/sweep_sh_approve.rs` and `tests/sweep_sh_approve_claim.rs`; the pane-0 and claim behaviour tests stay
- [ ] 2.6 Reword the `assets/scripts/broker.sh` and `assets/scripts/docs-fetch.sh` header comments: invoked at a stable path so an exact-path permission rule can name it, not "the classifier recognises this path". Run `bash -n` on all three scripts
- [ ] 2.7 `.git-paw/scripts/` is gitignored; refresh the repo-root copy with `git paw init` when dogfooding (nothing to commit for it)

## 3. The drive loop escalates every live prompt and approves none

- [ ] 3.1 Tests first (`src/supervisor/drive.rs`): with a `RecordingDispatcher`, a live prompt for `ls -la` on a coding pane gives `PaneSweepOutcome::Escalated` with verdict `permission-prompt` and command `ls -la`, and zero dispatched keys (*A command formerly classified safe is escalated, not approved*). The same holds for a pane-0 prompt (agent `supervisor`; *A live prompt on the supervisor pane is escalated, not answered*) and for a file-edit prompt with no command header (command = target path; *A file-edit prompt is escalated by its target path*)
- [ ] 3.2 Add a table test over the proposal's false-SAFE captures (the `ls` + `rm -rf /important # [y/N]` block, `git reset $'--hard' HEAD~1`, `find . $'-delete'`, `export GIT_SSH_COMMAND=id; git fetch`, `awk 'BEGIN{system("id")}'`, `cat ~/.ssh/id_rsa`, `git stash`, `git paw purge`). Each must escalate with zero keys (*Command text cannot steer the outcome*)
- [ ] 3.3 In `sweep_pane`, after the live-prompt check, run dedup (`approval_dedup_key`), then build `Escalation { verdict: "permission-prompt", command: dedup_shape(&capture) }`, then call `record_escalation`, then record the `tooling_friction` learning. Leave the cli-unavailable branch and `submit_buffered_input` untouched
- [ ] 3.4 Delete `PromptVerdict`, `classify_prompt`, `prompt_command_slice`, `first_whitelist_match`, `send_approval`, `AlertSink::log_approval` and `BrokerAlertSink::log_approval`, and the `approval_keystrokes` import. Delete `DriveConfig` / `DriveRunOptions` `whitelist`, `approve_worktree_writes`, `protected_paths`, and their derivation in `src/commands/supervisor.rs::drive_unattended_loop`
- [ ] 3.5 Reword `Escalation::question`, `Escalation::handoff_text` and `friction_learning_body` for `permission-prompt` per design D5: needs a human decision; relay to the human; run `sweep.sh approve` only on the human's instruction; no "could not auto-approve". Add a unit test for *The permission-prompt hand-off asks for a relay, not a decision*
- [ ] 3.6 Rewrite or delete the `drive.rs` unit tests that assert a `Safe` verdict, a dispatched option digit, or `log_approval` (`rtk grep "PromptVerdict\|log_approval\|option_index\|classify_prompt" src/supervisor/drive.rs`). Keep the nudge, buffered-input, cli-unavailable and dedup tests green
- [ ] 3.7 `tests/unattended_drive_e2e.rs`:
  - Turn `unattended_escalates_danger_without_blocking_other_agent` into `unattended_escalates_live_prompt_without_blocking_other_agent`. Agent `feat/alpha`'s dummy reprints a live prompt for `ls -la` and appends any stdin bytes it receives to a marker file. Agent `feat/beta` publishes `done`.
  - Assert: the `feat/alpha` stdin marker file is empty (no keys); the broker log holds an `agent.question` naming `feat-alpha` and `ls -la`; `feat-beta` reaches `done`; the summary reads `escalated-for-review` and lists `[permission-prompt]` with `ls -la`; exit 0.
  - Rewrite `unattended_auto_approves_safe_prompt_then_completes` and `..._with_trailing_task_region` into escalation tests (the trailing-region capture is still detected and escalated, with no keys).
  - Rewrite `unattended_approves_supervisor_pane_zero_safe_prompt` into "a pane-0 prompt is escalated as `supervisor`, with no keys to pane 0".
- [ ] 3.8 Update the selftest e2e if it relied on the dummy prompt being approved (*Drive loop is exercised end-to-end without a real LLM*: escalate, no keys, completion, summary)

## 4. Delete the attended dashboard auto-approve thread

- [ ] 4.1 Move `AgentStatusRow` and `fetch_status_over_http` from `src/supervisor/poll.rs` into `src/supervisor/drive.rs` (still `pub`); update `tests/unattended_drive_e2e.rs`'s import
- [ ] 4.2 Delete `spawn_auto_approve_thread` and its wiring struct (`src/commands/supervisor.rs`), the spawn block and `dashboard_should_auto_approve` in `src/main.rs`, and the `UNATTENDED_ENV` / `GIT_PAW_UNATTENDED` producer and consumer
- [ ] 4.3 Delete `src/supervisor/poll.rs` and `src/supervisor/stall.rs`. Move `KeyDispatcher` / `TmuxKeyDispatcher` to a new `src/supervisor/keys.rs` (module doc, `///` docs) and delete `src/supervisor/approve.rs`. Delete `approval_gate::approval_send_gate` and its tests; keep `live_prompt_in_tail` and `approval_dedup_key`. Update `src/supervisor/mod.rs`
- [ ] 4.4 Collapse `PermissionType`, `classify_capture*` and `detect_permission_prompt*` into `has_approval_marker(captured, profile) -> bool` (plus a capture-and-scan wrapper only if a caller remains). Rewrite the unit tests for *Approval prompt detected in working agent* and *Detection carries no command class*
- [ ] 4.5 Delete `tests/auto_approve_integration.rs`, `main.rs`'s `dashboard_auto_approve_disabled_under_unattended`, and the `poll.rs` / `approve.rs` / `stall.rs` unit tests
- [ ] 4.6 Rewrite `tests/e2e_supervisor_stop.rs::stop_in_supervisor_mode_terminates_auto_approve`: with `[supervisor.auto_approve] enabled = true` declared, stop still tears the session down cleanly, and no `auto_approved` status appears in the broker log at any point
- [ ] 4.7 Rewrite `tests/source_audit.rs::cmd_supervisor_does_not_spawn_auto_approve_thread` to assert that no `spawn_auto_approve_thread`, `auto_approve_pane` or `tick_from_status` symbol exists under `src/`
- [ ] 4.8 Add an e2e for *An attended session runs no auto-approver*: an attended supervisor session (no `--unattended`) with `[supervisor.auto_approve] enabled = true` and `stall_threshold_seconds = 5`, whose agent dummy reprints a live prompt and records stdin. After more than three former stall periods the stdin record is empty

## 5. Drive-loop escalation log (absorbs `drive-loop-manual-decision-log`) and the approvals hint

- [ ] 5.1 Tests first (`drive.rs`): add a `RecordingManualDecisions` sink and a `NoopManualDecisions` helper. Check:
  - one live-prompt escalation produces exactly one sink call with that agent id and capture;
  - two sweeps of the same prompt within the dedup window produce one call;
  - a `cli-unavailable` escalation produces zero calls, and so do a `nudge-wedged` escalation and a correction-loop signal (*Non-prompt escalations are not logged*)
- [ ] 5.2 Cross-module test: a `RecorderManualDecisionSink` around a real `ManualDecisionRecorder` writing to a tempdir log, driven by one escalating sweep. Expect one JSONL line with `timestamp`, `agent_id`, `pattern` and `first_seen: true` (*A drive-loop escalation appends a JSONL line*)
- [ ] 5.3 The same with `manual_approvals_log = false` (`learnings = true`, no broker URL): no file is written and no learning is returned (*The opt-out applies to the drive loop*)
- [ ] 5.4 Add `pub trait ManualDecisionSink` and `DriveDeps.manual_decisions: &'a mut dyn ManualDecisionSink` (with docs), and add the field to every `DriveDeps { .. }` literal (`rtk grep "DriveDeps {" src`). In `sweep_pane`'s escalation arm, call `record_forwarded` after `dedup.should_emit` and before `record_escalation`
- [ ] 5.5 Add a production `RecorderManualDecisionSink { recorder, broker_url }` that records and then publishes a returned learning via `publish_to_broker_http`, warning on stderr on failure. Add `DriveRunOptions.manual_decisions`. `drive_unattended_loop` builds the recorder from the supervisor config (opt-out default on, `learnings`, `cli`), per design D6
- [ ] 5.6 Extend the 3.7 e2e: after the run, `.git-paw/sessions/<session>.manual-approvals.jsonl` holds exactly one `feat-alpha` / `ls -la` line
- [ ] 5.7 `src/supervisor/manual_approvals.rs`: rename `Suggestion::BundledPresetCandidate` to `Suggestion::UserAllowlist`, rendered `user allowlist`. Update its tests and `tests/approvals_integration.rs` (*Generic command suggested for the user allowlist*)

## 6. Config deprecation and the `git paw init` default

- [ ] 6.1 Tests first (`src/config/tests.rs`):
  - a pure `deprecated_table_warning` names both tables when both are declared, only the declared one otherwise, and returns `None` when neither is;
  - both tables with every field set load and round-trip, values preserved (*An old configuration with both tables loads with one warning*, *Declared tables round-trip*);
  - a `[supervisor]` config without the tables saves without a `[supervisor.common_dev_allowlist]` table (*An undeclared table is not written on save*);
  - `stall_threshold_seconds = 0` prints no clamp warning;
  - a pre-v0.5 config with no supervisor tables loads
- [ ] 6.2 E2E (`assert_cmd`): a repo whose `.git-paw/config.toml` declares both tables runs a config-loading subcommand. Its stderr contains exactly one deprecation line naming both tables; a repo without them gets none (*An old configuration with both tables loads with one warning*, *A configuration without the tables produces no warning*)
- [ ] 6.3 Make `SupervisorConfig::common_dev_allowlist` an `Option<CommonDevAllowlistConfig>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`. Mark both structs' docs and fields deprecated (no effect). Delete `AutoApproveConfig::{effective_whitelist, resolved, approve_worktree_writes}` and the clamp; keep `ApprovalLevelPreset` as a parse type
- [ ] 6.4 Print the warning from `load_config_file` once per file per process (design D7)
- [ ] 6.5 Remove the `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` stanzas, and the comment that refers to them, from the default template in `src/config/mod.rs`. Update the template-coverage and "all commented / multi-stack" tests. Add *Template omits the deprecated auto-approval tables* (no table, and no warning on load)
- [ ] 6.6 Delete the `src/config/tests.rs` whitelist / `is_safe_command` tests and any `effective_whitelist` tests
- [ ] 6.7 Remove the two deprecated tables from this repository's tracked `.git-paw/config.toml`. The working tree has unrelated uncommitted edits in that file, so stage only these hunks

## 7. Bundled skill doctrine and the boot directive

- [ ] 7.1 Tests first: rewrite `tests/tiered_permission_model_skill_content.rs` for *Supervisor skill — permission decisions belong to the agent CLI and the human*. The ladder is CLI permission system → relay to human → explicit `sweep.sh approve`; the skill says the approve answers once and never "don't ask again"; it contains no `__classify`, no dev-allowlist enumeration, and no "absorb routine approvals"
- [ ] 7.2 In the `src/skills.rs` tests:
  - rewrite `supervisor_skill_launch_sweep_escalates_unknown_via_agent_question` to *Supervisor skill relays permission prompts to the human*;
  - rewrite `supervisor_skill_launch_sweep_complements_auto_approve_thread` to *Supervisor skill does not rely on a background approver*;
  - delete `supervisor_skill_launch_sweep_references_down_enter_keystroke`;
  - keep `supervisor_skill_documents_proactive_launch_sweep` and `..._lists_four_pane_categories`;
  - update the `drive_loop_directive_*` tests to *Unattended supervisor boot context announces the drive loop* and *No boot context tells the supervisor to approve by pattern*
- [ ] 7.3 `assets/agent-skills/supervisor.md`:
  - rewrite the drive-loop directive region (the loop escalates every prompt to you; relay it to the human; approve only on their word);
  - rewrite the attended note;
  - rewrite §1.5 (launch-time sweep: permission prompt → relay);
  - rewrite §2 (continuous sweep: no absorption, no poll-thread fallback);
  - replace "The tiered permission model" and the §1.5 safe-command policy with the new ladder;
  - delete the "Absorb routine approvals" rule and "Auto-approve permission prompts";
  - reword the protected-path wording in the out-of-worktree violation section, the classifier-path wording near `broker.sh`, and the `tooling_friction` example;
  - remove every `{{DEV_ALLOWLIST_PRESET}}` placeholder, keeping the `<!-- allowlist-prose -->` span around the CLI-specific paste-buffer marker
- [ ] 7.4 `assets/agent-skills/coordination.md`: rewrite the "classifier backstop" paragraph on config-path writes. Such writes prompt and reach a human; never rely on a prompt appearing
- [ ] 7.5 `src/skills.rs`: delete the `{{DEV_ALLOWLIST_PRESET}}` substitution, `render_dev_allowlist_preset` and its tests. Update the placeholder doc list, the `with_drive_loop_directive` doc comment, and the drive-loop region ground-truth fixture
- [ ] 7.6 `tests/lang_agnostic_skill_audit.rs`: drop the `DEV_ALLOWLIST_PRESET` references. Keep the sentinel-stripping test green against the remaining `allowlist-prose` span (*Sentinel-comment scope excludes an allowed span*). Add *No dev-allowlist placeholder remains*
- [ ] 7.7 Check `tests/skill_stuck_shapes_prose.rs`, `tests/orchestrator_judgment_calls_skill_content.rs`, `tests/qualitative_learnings_skill_content.rs` and `tests/supervisor_routing_skill_content.rs` against the new prose; fix any pin on the removed sections
- [ ] 7.8 Add tests for the skill parts of *Supervisor treats out-of-worktree writes as violations* (no classifier wording) and *Prompt-free operation is attributed to the agent CLI's permission system* (*Supervisor guidance credits the CLI's permission system*)

## 8. Delete the classifier remainder

- [ ] 8.1 Delete `src/supervisor/auto_approve.rs` (what is left after section 1) and `src/supervisor/dev_allowlist.rs`, and remove them from `src/supervisor/mod.rs`. Delete `ProtectedPaths` and everything listed under "delete" in design D4
- [ ] 8.2 Delete `tests/prompt_profile_security_boundary.rs` and `tests/memory_isolation_integration.rs`. Add tests for *A profile cannot widen the live-prompt window*, *A profile cannot make approve select another option* (a `sweep.sh` run with an overridden `option_line_pattern` / `broad_grant_marker` sends only `1` + `Enter` or nothing) and *The pane-0 refusal ignores profile content*
- [ ] 8.3 `src/config/cli.rs`: the `settings_path` doc says parse-only and drops the `ProtectedPaths` link. Keep or add the round-trip test (*Settings path still parses and round-trips*). `src/config/prompt_profile.rs`: point the field docs at `permission_prompt` and note that `broad_grant_marker` feeds the `sweep.sh approve` option-1 guard
- [ ] 8.4 `src/broker/learnings.rs`: delete the `auto_approved` arm of `observe()`, `record_auto_approve`, `extract_command_class`, the permission counter and threshold, and their tests. Keep the rendering of `permission_pattern` `agent.learning` records (*v0.5.0 sections unchanged*). Update `tests/learnings_mode_integration.rs`, `tests/e2e_qualitative_learnings.rs` and `tests/e2e_learnings_multi_session.rs` so they no longer feed `auto_approved` statuses
- [ ] 8.5 Retired-symbol gate: `rtk grep "__classify\|classify_prompt\|effective_whitelist\|PromptVerdict\|is_safe_command\|DEV_ALLOWLIST_PRESET\|dev_allowlist\|ProtectedPaths\|spawn_auto_approve_thread\|auto_approve_pane\|select_option_index\|PermissionType\|GIT_PAW_UNATTENDED" src tests assets`. The only matches allowed are the deliberate negative assertions in tests

## 9. Test sweep and traceability

- [ ] 9.1 Every scenario in `specs/*/spec.md` maps to at least one test. Write the map in the PR description, naming the tests from sections 2–8. Add any missing test now, including:
  - *A prompt nobody answers does not end the run* (a unit test of `drive_loop` with a fake clock: a pane on a permanent prompt does not exit before the heartbeat);
  - *Escalation of a live prompt reuses agent.question* and *No permission-prompt message variant exists*;
  - *A prompted boot publish is escalated, not answered*
- [ ] 9.2 Confirm every deletion in proposal Impact → Tests is done and that no test still asserts an `auto_approved` status, a `Down` keystroke, or option `2`
- [ ] 9.3 `tests/security_posture_docs.rs`: rewrite for the new control list (*Disclaimer is present and reachable*, *Controls are not overstated*, *No classifier is presented as a control*)
- [ ] 9.4 `cli.rs` help tests: `start --help` describes `--unattended` as driving the wave and escalating prompts, and does not say it auto-approves (*--unattended appears in help output*)

## 10. Docs

- [ ] 10.1 `--help` (`src/cli.rs`):
  - remove the `start` example comment "auto-approve safe prompts via [supervisor.auto_approve]";
  - rewrite `--unattended`'s `help` and long help (escalates every prompt; prompt-free runs need the CLI's own allow rules, or `full-auto` in a sandbox);
  - rewrite `approvals`' long help (prompts the drive loop escalated; the hint names the CLI's project or user allowlist)
- [ ] 10.2 `README.md`:
  - the `start --unattended` bullet and the autonomous-orchestrator bullet (no "risky/unknown"; prompts are relayed);
  - delete the "Auto-approval policy" Features bullet (*README Features section does not advertise auto-approval*);
  - the supervisor / dev-allowlist paragraph;
  - the security-disclaimer list
- [ ] 10.3 `docs/src/user-guide/supervisor.md`:
  - rewrite `## Unattended mode` (bullets, the "not a second classifier" note, the dashboard-thread note, the pump/brain table rows for prompts);
  - rewrite "`auto` resolves to a deterministic permission mode";
  - replace `## Common dev-command allowlist`, `## The tiered permission model` and `## Auto-approve classification` with one `## Agent permissions` section (*Supervisor user-guide explains agent permissions*), keeping "Run dev commands bare" and the re-confirm, pane-0 and claim content as the `sweep.sh approve` description;
  - reword `## Out-of-worktree write violations`;
  - retarget `## Manual approvals` (the drive loop records every escalated prompt; attended sessions record none);
  - fix the polyglot-skills paragraph
- [ ] 10.4 `docs/src/configuration/README.md`:
  - the `settings_path` text becomes parse-only;
  - `manual_approvals_log` applies to `--unattended` runs;
  - the approval-flag table drops "flow through the classifier / drive loop";
  - the "Auto-approve safe permission prompts" and `common_dev_allowlist` reference becomes a deprecation subsection (*Configuration reference marks the auto-approval tables deprecated*)
- [ ] 10.5 `docs/src/quick-start-supervisor.md`, `docs/src/cli-reference.md` (`--unattended`, `approvals`), `docs/src/user-guide/coordination.md` ("Prompt-free boot calls" becomes "the CLI's permission system decides"), `docs/src/user-guide/docs-fetch.md`, `docs/src/user-guide/agents-md.md`, `docs/src/user-guide/cli-prompt-profiles.md` (field consumers; `broad_grant_marker` feeds the approve guard), `docs/src/user-guide/learnings.md` (no `auto_approved` counter), `docs/src/user-guide/tiered-model-workflow.md`, `docs/src/user-guide/skill-templates.md` (drop `{{DEV_ALLOWLIST_PRESET}}`), `docs/src/supported-clis.md`
- [ ] 10.6 `docs/src/user-guide/security-posture.md`:
  - the control table lists the CLI permission system, the sandbox and remote control;
  - state that git-paw approves no prompt;
  - note that "don't ask again" grants written by earlier versions persist in the CLI's settings
- [ ] 10.7 Architecture: `docs/src/architecture.md` (module tree and table, without `auto_approve.rs`, `approve.rs`, `poll.rs`, `stall.rs` and `dev_allowlist.rs`, with `keys.rs`; the dashboard-process diagram without the approve thread) and `docs/src/architecture-direction.md` (drop the "command classifier (`git paw __classify`)" example)
- [ ] 10.8 Repo-local guidance, which is not exported: `AGENTS.md` Design Principles (the classifier example and "auto-approved execution power"; agents now run with the CLI's own permission mode) and `.agents/skills/{export-agnosticism,security-and-safety-review,code-standards,concept-decoupling,test-strategy}/SKILL.md` references to the classifier and `__classify`. Keep `tests/agent_skills_conform.rs` green
- [ ] 10.9 `docs/src/specifications/README.md` index blurbs for the capabilities whose Purpose changes (design D9)

## 11. Gates

- [ ] 11.1 `cargo fmt`, then `just lint > target/cr-lint.log 2>&1; echo $?` returns 0
- [ ] 11.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast > target/cr-test.log 2>&1; echo $?` returns 0. Never trust a piped status; re-run a known load flake in isolation before calling it a regression
- [ ] 11.3 `cargo test --bin git-paw > target/cr-bin.log 2>&1; echo $?` returns 0
- [ ] 11.4 `mdbook build docs/ > target/cr-mdbook.log 2>&1; echo $?` returns 0
- [ ] 11.5 `openspec validate classifier-retirement --strict; echo $?` returns 0
- [ ] 11.6 Re-run the 8.5 grep; the only matches are deliberate negative assertions
- [ ] 11.7 `just deny` returns 0 (no dependency change is expected)

## 12. Release notes

- [ ] 12.1 The change's commit carries `feat(supervisor)!:` with a `BREAKING CHANGE:` footer (git-cliff builds the changelog from it). The footer lists four things: `--unattended` no longer auto-approves; `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` have no effect and warn; `git paw __classify` and `sweep.sh classify` are removed; `git paw approvals --json` now reports `suggested_target = "user allowlist"`

Note for the release notes (required): 0.16.0's classifier chose "Yes, and don't ask again" for
prompts it judged safe. The agent CLI persisted those choices as durable grants. For Claude Code
they live in the repository-root `.claude/settings.local.json`, which every worktree and the
user's own sessions share. This repository's copy has 281 entries, including broad ones such as
`Bash(git-paw purge:*)`. Retiring the classifier stops new grants but removes none. The release
notes must tell users to review that file (and any user-level equivalent) and delete grants they
did not intend. git-paw never edits or deletes it.

Note for the archiver (supervisor, at archive time; not an implementation task): rewrite the
`## Purpose` paragraphs listed in design D9, then re-run `tests/spec_purpose_backfilled.rs` and
`mdbook build docs/`. Delete `openspec/changes/drive-loop-manual-decision-log/` and the four
dropped changes before archiving this one.
