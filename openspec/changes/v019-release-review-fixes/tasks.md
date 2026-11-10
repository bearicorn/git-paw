## 1. Regression fixes (R1–R3)

- [x] 1.1 `src/init.rs`: add `refresh_root_scripts` and use it from `run_init`; unit tests `refresh_root_scripts_replaces_a_stale_script` and `refresh_root_scripts_leaves_current_scripts_untouched`
- [x] 1.2 `src/commands/supervisor.rs`: `write_orchestrator_context` calls `refresh_root_scripts` after `export_skills`; e2e `tests/e2e_supervisor_launch.rs::a_supervised_launch_refreshes_a_stale_root_helper_script`
- [x] 1.3 `docs/src/user-guide/supervisor.md`: upgrade note — a supervised launch refreshes `.git-paw/scripts/`
- [x] 1.4 `src/commands/supervisor.rs`: the unattended existing-session branch prints `selection_ignored_notice` when a selection is given and the session records agents; assert the notice in `tests/unattended_drive_e2e.rs::unattended_rebinds_a_loop_to_a_live_session_after_heartbeat`
- [x] 1.5 `src/commands/start.rs`: shared `ADD_AGENTS_CLI_HINT` ending the selection notice and the empty-session refusal; update `docs/src/user-guide/session-lifecycle.md`
- [x] 1.6 `src/mcp/tools/session.rs`: `mcp_wording` strips both CLI hints from `start_session` warnings and its `SessionError`; unit test `start_session_wording_drops_the_cli_hints`

## 2. Audit fixes (A1–A12)

- [x] 2.1 Edit the `core-project-conventions` Purpose: "README Features section" → "README Highlights section" (A5)
- [x] 2.2 `docs/src/configuration/README.md` `prompt_profile` row: menu fields, defaults, empty list fails the load (A2)
- [x] 2.3 `docs/src/user-guide/cli-prompt-profiles.md`: empty-menu-list rule in the validation list and the "Breaking since v0.19" note (A3)
- [x] 2.4 `tests/readme_features.rs`: comments cite the current requirement title (A7)
- [x] 2.5 `src/doctor_tests.rs`: `no_worker_cli_configured_raises_no_cost_cap_warning` (A9)
- [x] 2.6 `tests/source_audit.rs`: `.contains(` / `contains(&` in `DISPATCH_FORMS` with positive controls; fix any real finding (A11)
- [x] 2.7 `tests/helpers/source_scan.rs`: blank lines inside `/* */` block comments; test `lines_inside_a_block_comment_are_not_audited` (A11)
- [x] 2.8 `src/commands/start.rs`: merge the two stacked comment blocks above `LaunchWarnings` (A12)

## 3. Bounded broker calls (H1)

- [x] 3.1 `assets/scripts/_paw_common.sh`: `PAW_BROKER_CURL_BOUNDS` and the `GIT_PAW_BROKER_URL` override; every broker `curl` in `_paw_common.sh`, `broker.sh` and `sweep.sh` passes the bounds; tests `learn_against_a_broker_that_never_answers_returns_within_the_bound`, `every_broker_curl_in_the_helpers_passes_the_bounds`, `learn_publishes_to_the_broker_url_override`
- [x] 3.2 `src/supervisor/drive.rs`: `SweepLearningSink` kills a run past `LEARNING_TIMEOUT` and passes the session's broker URL; test `a_hung_learning_is_dropped_at_the_bound`; `unattended_rebind_uses_the_recorded_broker_endpoint` passes with real root scripts
- [x] 3.4 Override accepted only as a loopback `http://host:port`; test `a_non_loopback_broker_url_override_is_ignored`
- [x] 3.5 `tests/helpers/script_cmd.rs` clears `GIT_PAW_BROKER_URL` for every script-running test, `TmuxTestEnv::apply` too; test `a_script_test_ignores_an_inherited_broker_url`; sink test `a_fast_learning_runs_with_the_session_broker_url`
- [x] 3.3 Docs: name `GIT_PAW_BROKER_URL` and the call bounds where the helpers are documented

## 4. Quality gates

- [ ] 4.1 `cargo fmt`, then `just lint`
- [ ] 4.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast` for the touched suites, checking the real exit code
- [ ] 4.3 `mdbook build docs/`
- [ ] 4.4 `openspec validate v019-release-review-fixes --strict`
