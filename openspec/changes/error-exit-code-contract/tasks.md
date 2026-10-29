## 1. I/O context (compiler-driven, its own commit)

- [ ] 1.1 Replace `PawError::IoError(#[from] std::io::Error)` with a variant holding operation, path and source, plus a `PawError::io(op, path, err)` constructor; `Display` = `I/O: <op> <path>: <source>`
- [ ] 1.2 Fix every site the compiler flags (bare `?` on an `io::Result`, e.g. `worktree_provision.rs` copy/write/create_dir_all, `random.rs`) to pass the operation and path
- [ ] 1.3 Unit test `io_error_names_operation_and_path`

## 2. Exit-code contract

- [ ] 2.1 Add `PawError::WaveIncomplete(outcome label)`; `exit_code()` arms: `UserCancelled` and `NotAttempted` (v0.17.0) → 2, `WaveIncomplete` and `IntegrationRefused` (v0.21.0) → 3, everything else → 1
- [ ] 2.2 `run_drive_loop`: after printing the summary, return `WaveIncomplete` for `escalated-for-review`, `stuck`, `heartbeat`, `session-torn-down`; no `cli-unavailable` outcome exists after v0.18.0 (a coding agent's is an escalation, so `escalated-for-review`); add a pure `outcome_exit_status` used by both and its table test `outcome_exit_status_table`
- [ ] 2.3 `cmd_purge`: a declined confirmation returns `NotAttempted` stating that the confirmation was declined; a dialoguer failure because stdin is not a terminal returns `NotAttempted` naming `--force`. No new variant (check what `purge-unmerged-branch-safety` left in this path first)
- [ ] 2.4 Unit test `test_exit_code_contract_table` over every `PawError` shape; update `test_general_errors_exit_code`; keep v0.17.0's *NotAttempted exit code and message* test green

## 3. Actionable messages

- [ ] 3.1 Walk every `PawError` construction site (≈360; `SessionError` ≈126). Classify each as user-reachable or internal; for user-reachable ones lacking an object or a next step, fix the message in place (e.g. "could not determine XDG data directory" → name `HOME` / `XDG_DATA_HOME`; `CliNotFound` → point at `list-clis` / `add-cli`). Record the walk's result in the change
- [ ] 3.2 `tests/error_matrix_e2e.rs::every_user_reachable_failure_is_actionable`: one row per failure class (not a git repo, tmux missing, no CLI on `PATH`, `unknown-cli`, unknown preset, invalid config TOML, `provision-io`, purge without a terminal, `unknown-flag`); assert exit code and stable anchors in stderr; HOME- and socket-isolated per `core-ci-hygiene`
- [ ] 3.3 `tests/cli_tests.rs::test_purge_cancel_preserves_worktrees`: assert exit 2 and that stderr names `--force`
- [ ] 3.4 `tests/cli_prompt_matrix.rs::purge_declined_exits_two` (PTY harness): answer no, assert exit 2 and nothing removed

## 4. Unattended exit status tests

- [ ] 4.1 `unattended_heartbeat_exits_with_summary`: assert exit 3 and the unchanged summary
- [ ] 4.2 `unattended_reports_escalated_for_review_when_branch_unmerged`: assert exit 3
- [ ] 4.3 `unattended_reports_completed_when_branch_is_merged`: assert exit 0 (v0.17.0 rewrote `unattended_auto_approves_safe_prompt_then_completes` into an escalation test, so it is no longer the clean-completion case)
- [ ] 4.4 `escalated_prompt_exits_three_even_when_every_branch_merged`: a dummy agent shows a live prompt long enough to be escalated, then clears it, commits and publishes a terminal status with its branch merged; assert the summary reads `escalated-for-review` with the `permission-prompt` escalation, and exit 3
- [ ] 4.5 Update v0.17.0's `unattended_escalates_live_prompt_without_blocking_other_agent` (and the other escalation e2e tests) from exit 0 to exit 3

## 5. --from-specs deprecation warning

- [ ] 5.1 Register `--from-specs` in `src/deprecation.rs` (kind flag, since 0.24.0, replacement `--from-all-specs`, removed in 1.0.0); the existing post-parse `deprecation::scan_args` + `deprecation::warn` call in `main` (from v0.21.0 `supervisor-vocabulary-split`) then warns for it — do not add a second scan or warn function
- [ ] 5.2 Unit test `deprecation_scan_stops_at_terminator` gains the `--from-specs` rows (alias present, absent, after `--`) in the shared `scan_args` table
- [ ] 5.3 Replace `from_specs_alias_emits_no_deprecation_warning_on_stderr` with `from_specs_alias_emits_a_deprecation_warning` (warning present for the alias, absent for `--from-all-specs`, stdout and exit status equal)

## 6. Docs

- [ ] 6.1 `docs/src/cli-reference.md` Exit Codes: the 0/1/2/3 table with meanings (`NotAttempted` under 2; `WaveIncomplete` and `IntegrationRefused` under 3; an unattended run that escalated any permission prompt exits 3 even when its work completed); `docs/src/architecture.md` exit-code list; root `after_help` gains a one-line exit-code summary
- [ ] 6.2 `tests/cli_dx_docs.rs::exit_code_table_is_consistent_across_layers`: the three layers list the same four codes and the escalated-prompt rule
- [ ] 6.3 `docs/src/user-guide/supervisor.md`: unattended exit status per outcome, including exit 3 after any escalated permission prompt; `docs/src/user-guide/doctor.md`: cross-reference the contract (doctor unchanged)
- [ ] 6.4 Changelog: the commit carries a `!` breaking marker and a `BREAKING CHANGE:` footer for the `--unattended` (0 → 3) and declined-`purge` (0 → 2) status changes
- [ ] 6.5 No configuration-reference change

## 7. Gates

- [ ] 7.1 `cargo fmt`, then `just lint`
- [ ] 7.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code
- [ ] 7.3 `just deny`; `mdbook build docs/`
- [ ] 7.4 `openspec validate error-exit-code-contract --strict`
