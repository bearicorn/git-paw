## 1. Shared test infrastructure (seam first, no test rewritten yet)

- [ ] 1.1 Add `helpers::wait_until(what, condition)` with one `HANG_CEILING` constant and an injectable clock; on the ceiling, panic with the description, the elapsed time and caller-attached diagnostics
- [ ] 1.2 Unit tests: returns as soon as the condition holds; names the condition on hang (both instant via the injected clock)
- [ ] 1.3 Add `helpers::StubBroker`: blocking accept on its own thread, reads headers then exactly `Content-Length` body bytes, records bodies, replies at once, stops only on drop
- [ ] 1.4 Unit tests: records a request made late in the fixture's life (drop → refused only then); records a body sent in two writes whole
- [ ] 1.5 Add both `PATH` forms to `tests/helpers/mod.rs`: the pinned-binary form (prepend the binary-under-test directory) and the strip-`git-paw` form for fallback tests. Write both fresh: `sweep_sh_classify.rs`, which carried a copy, is deleted by v0.17.0's `classifier-retirement`
- [ ] 1.6 Extend `TmuxTestEnv::apply`/`apply_assert`/`apply_to_process` to pin the pane shell to `/bin/sh` (verify tmux takes `default-shell` from the starting client's `SHELL`; otherwise set `default-shell` on the private server after it starts); unit test `isolated_pane_runs_the_pinned_shell`
- [ ] 1.7 Extend `TmuxTestEnv::drop` to record every pane's process group, `kill-server`, then `wait_until` the groups are empty; kill and report survivors at the ceiling; unit tests `dropping_the_env_reaps_pane_processes` and `reaping_reports_a_process_it_had_to_kill` (short ceiling override)
- [ ] 1.8 Set every assert_cmd `.timeout()` the suite uses to `HANG_CEILING`, and report a signal-killed run as "hung" in the helper that inspects output

## 2. Standing convention tests

- [ ] 2.1 `tests/tmux_isolation_conventions.rs::every_tmux_spawn_is_socket_isolated`: scan `tests/` for `Command::new("tmux")` and binary invocations of `start`/`pause`/`stop`/`add`/`remove`/`attach`; accept `tmux_test_env`, an explicit `-L`/`-S` socket, or a documented-exception comment; name the file and construction on failure. Share the scanner with `test-session-state-isolation`'s HOME guard
- [ ] 2.2 `tests/script_binary_conventions.rs::every_script_test_pins_git_paw`: flag any test that runs `sweep.sh` (or another script resolving `git paw`) without the pinned or stripped `PATH`
- [ ] 2.3 Verify each guard fails when one conforming file is reverted, then passes once restored

## 3. Bring non-conforming files into line

- [ ] 3.1 `tests/pause_e2e.rs`: apply `tmux_test_env` to every tmux and binary invocation (all three tests)
- [ ] 3.2 `tests/agent_pane_liveness_e2e.rs`, `tests/stop_confirmation_test.rs`: same
- [ ] 3.3 `tests/default_branch_fallback_integration.rs`, `tests/rebase_integration.rs`: same (HOME is handled by `test-session-state-isolation`)
- [ ] 3.4 Every `sweep_sh_*` file without a pinned `PATH` (`sweep_sh_stuck_detection.rs`, `sweep_sh_status_publish.rs`, `sweep_sh_session_discovery.rs`, `sweep_sh_conventions.rs`, `sweep_sh_tmux_fallback.rs` as applicable): pin the binary under test
- [ ] 3.5 Replace the per-file stub recorders in `sweep_sh_stuck_detection.rs`, `sweep_sh_status_publish.rs`, `sweep_sh_session_discovery.rs`, `broker_sh_session_discovery.rs` with `StubBroker`

## 4. Rewrite the named tests onto the rules

- [ ] 4.1 `pause_detaches_and_stops_broker`: assert `GET /status` answers before `pause`; read the broker host pid from its surface; `wait_until` that pid exits; then assert one connect is refused. Delete `wait_until_port_free` and the 200 ms probe
- [ ] 4.2 `sweep_sh_stuck_detection`: assert the first sweep printed `published` before each dedup assertion; add `expired_dedup_entry_republishes` with a seeded dedup entry older than the window; keep `blocked_on_supervisor_past_window_publishes` and `permission_prompt_with_stale_heartbeat_publishes_stuck` on the same precondition pattern
- [ ] 4.3 `provisioned_files_are_present_before_the_agent_cli_starts`: drop the 45 s budget for `wait_until` on the recorded listing; attach the probe pane's capture as the hang diagnostic
- [ ] 4.4 `cmd_supervisor_returns_immediately_with_attach_hint`: remove the 10 s timeout as the assertion; assert exit 0 and the hint with stdin not a TTY; a hang is reported by the shared ceiling (against the stdout v0.18.0 `supervisor-pane-removal` gave it)
- [ ] 4.5 Grep `tests/` for remaining hand-rolled `Instant::now() + …` deadline loops used as assertions and migrate them to `wait_until`

## 5. Coverage and CI

- [ ] 5.1 `justfile` `coverage`: `cargo llvm-cov --no-report` then `cargo llvm-cov report --html --output-dir docs/book/coverage`; verify the profile-file pattern is per process and how a malformed profile is reported; document the recovery if cargo-llvm-cov exposes no failure-mode option
- [ ] 5.2 Rollout check: `just coverage` live (with `GIT_PAW_ALLOW_LIVE_SESSION=1`) vs cold on one commit — merge succeeds and line coverage matches; truncate one profile between phases and confirm the output names it
- [ ] 5.3 `.github/workflows/ci.yml`: remove `continue-on-error` from `smoke-headless`
- [ ] 5.4 Correct the `justfile` `verify` comment once 2.1 passes (the suite is then actually socket-isolated)

## 6. Verification

- [ ] 6.1 Rollout check: capture `tmux ls` on the default socket before and after a full `cargo test` next to a live session; no test session appears, the live session is unchanged
- [ ] 6.2 Rollout check: `just check` cold (no tmux server, `TMUX` unset, no `git-paw` on `PATH`) and the full suite live give the same result for every test
- [ ] 6.3 Rollout check: run the four named tests 20× under three concurrent full-suite runs; zero failures
- [ ] 6.4 Rollout check: run the `sweep_sh_*` files with a decoy `git-paw` first on the ambient `PATH`; results match CI

## 7. Docs

- [ ] 7.1 `CONTRIBUTING.md` and `docs/src/contributing.md`: the test-writing rules (socket helper, `wait_until`, pinned script binary, `StubBroker`, no wall-clock assertions) and the new coverage recipe
- [ ] 7.2 No `--help`, README or configuration-reference change (no user-facing surface)

## 8. Gates

- [ ] 8.1 `cargo fmt`, then `just lint`
- [ ] 8.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code, not piped output
- [ ] 8.3 `just check` cold; `just deny`; `mdbook build docs/`
- [ ] 8.4 `openspec validate test-suite-determinism --strict`
