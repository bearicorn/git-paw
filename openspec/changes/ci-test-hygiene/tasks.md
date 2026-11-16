## 1. Shared waits (no behaviour change)

- [ ] 1.1 `tests/helpers/mod.rs`: add `wait_until(budget, cond)` and `wait_for_pane_command(env, target, expected, timeout)` (50 ms poll; panic names the pane and the last command seen)
- [ ] 1.2 Replace the private `wait_until` copies in `agent_pane_liveness_e2e.rs`, `e2e_supervisor_stop.rs`, `broker_host_lifecycle_e2e.rs`, `broker_host_e2e.rs`, `broker_verdict_authority_e2e.rs`, `unattended_drive_e2e.rs`
- [ ] 1.3 Move the c286737 loops in `cmd_golden_output.rs` and `command_library.rs`, and the `list-panes` check in `agent_pane_liveness_e2e.rs::set_up_paused_layout`, onto `wait_for_pane_command`
- [ ] 1.4 `pause_e2e.rs`: wait for `sleep` after the `send-keys` that starts the stand-in
- [ ] 1.5 `broker_host_lifecycle_e2e.rs:935`, `sweep_sh_broker_window.rs:123`: adopt the helper, or add a documented-exception comment if the test never reads liveness
- [ ] 1.6 Convention test `tests/ci_workflow_conventions.rs::stand_in_fixtures_use_the_shared_pane_wait`

## 2. Reaping leaked processes

- [ ] 2.1 `TmuxTestEnv::new`: `git-paw-tmux-` prefix and an `owner.pid` marker (pid + start time)
- [ ] 2.2 `helpers::spawn_paw_child`: `process_group(0)`, record the child under `$TMPDIR/git-paw-test-children/`, and kill the group on drop
- [ ] 2.3 Reaper run by both constructors: kill servers and child groups whose owner is gone, then remove their entries; never select by process name
- [ ] 2.4 Move `broker_host_e2e.rs::spawn_host`, both `spawn_unattended` copies, and the MCP server fixtures onto `spawn_paw_child`
- [ ] 2.5 Tests: `a_dead_owners_tmux_server_is_reaped`, `a_dead_owners_child_is_reaped`, `a_live_owners_tmux_server_is_left_alone` (each simulates the dead owner with a short-lived helper process)
- [ ] 2.6 Maintainer check: after `just smoke` killed mid-run and run again, `pgrep -fl 'git-paw __(dashboard|broker)'` lists nothing from either run (record the output in the PR)

## 3. Flake register

- [ ] 3.1 `tests/known_flakes.toml` with the three entries (DV-18 included), each with symptom and first-seen date
- [ ] 3.2 `tests/flake_register.rs::every_registered_test_exists`
- [ ] 3.3 `CONTRIBUTING.md` and `docs/src/contributing.md`: the register, what to do when a registered test fails, and the exit bar (condition waits only, plus ten consecutive runs while the suite runs in parallel)

## 4. MSRV job

- [ ] 4.1 `.github/workflows/ci.yml`: `msrv` job (`dtolnay/rust-toolchain@1.96`, `shared-key: msrv`, `cargo +1.96 check --all-targets --locked`)
- [ ] 4.2 `tests/ci_workflow_conventions.rs::the_msrv_job_checks_all_targets_on_the_declared_rust_version`
- [ ] 4.3 Prove the job fails on a post-1.96 API with a throwaway commit (not merged); note the run in the PR
- [ ] 4.4 Run `actionlint` on the workflow

## 5. Gates

- [ ] 5.1 `cargo fmt`; `just check`; `just smoke-all`
