## 1. Reproduce the three defects

- [x] 1.1 Add an E2E test launching `--specs` without supervisor mode against a stub CLI, asserting the agent pane's prompt is SUBMITTED (no residual unsubmitted input line) — SHALL fail before the fix
- [x] 1.2 Extend it to assert the submitted prompt contains the task prompt naming the assignment and the backend's apply invocation — SHALL fail before the fix
- [x] 1.3 Add a test asserting the worker pane command carries the resolved approval flags for a config with non-empty `Auto` flags — SHALL fail before the fix
- [x] 1.4 Add the guard test for bare `git paw start`: the boot block SHALL still be pre-filled WITHOUT an `Enter`, so the human-driven flow is provably untouched (satisfied by the pre-existing, unchanged `boot_inject_args_use_literal_flag_with_no_enter` / `manual_mode_boot_block_lands_in_agent_pane` in `tests/boot_block_integration.rs` — see the rationale in `tests/start_spec_launch_parity_integration.rs`'s task-1.4 section for why a new live-pane probe is not meaningful for this specific content)

## 2. Submit the spec-driven boot prompt

- [x] 2.1 Replace the bare `build_boot_inject_args` call in `cmd_start_with_specs` with the existing `submit_prompt_to_pane` seam (literal send, per-CLI settle delay, separate `Enter`)
- [x] 2.2 Resolve the settle delay per CLI via `resolve_submit_delay_ms`, as the supervisor path does
- [x] 2.3 Confirm no CLI name is branched on anywhere in the new path (`cli-resolution` requires the mechanism stay CLI-agnostic)

## 3. Inject the task prompt

- [x] 3.1 Make `build_task_prompt` reachable from `commands/start.rs` with the smallest possible move or re-export; do NOT start the v0.18.0 library-surface refactor here
- [x] 3.2 Build each agent's prompt as boot block + task prompt, byte-identical to what the supervisor path produces for the same spec
- [x] 3.3 Inject and submit the task prompt even when `[broker] enabled = false`, where no boot block is injected
- [x] 3.4 Add a test asserting the spec-driven prompt matches the supervisor path's prompt for the same spec entry

## 4. Apply approval flags on every launch path

- [x] 4.1 Call `config::resolve_approval_flags` in `cmd_start_with_specs` and use the result when building `PaneSpec.cli_command`
- [x] 4.2 Do the same in bare `cmd_start`
- [x] 4.3 Assert an empty resolved flag string yields the bare CLI name with no trailing separator
- [x] 4.4 Grep for any remaining construction of a worker pane command from a bare CLI name and fix or justify each

## 5. Spec traceability

- [x] 5.1 Verify every scenario in `specs/boot-block/spec.md` maps to at least one test
- [x] 5.2 Verify every scenario in `specs/supervisor-config/spec.md` maps to at least one test
- [x] 5.3 Confirm the modified `Manual mode boot block pre-fill` no longer contradicts `cli-resolution`'s split-send requirement — read both together and state the boundary in the commit body

## 6. Docs and gates

- [x] 6.1 Update the user-guide section describing the from-specs / manual launch flows to state which paths submit and which pre-fill
- [x] 6.2 Note in the docs that non-supervisor workers now receive the configured approval flags, since this changes effective permission behaviour for existing configs
- [x] 6.3 `cargo fmt`, then `just lint`
- [x] 6.4 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code rather than piped output (full suite ran to completion; only the 3 pre-existing `sweep_sh_stuck_detection` failures, confirmed by the orchestrator as unrelated and already fixed on `live-prompt-tail-window` — everything else green)
- [x] 6.5 `mdbook build docs/`
- [x] 6.6 `openspec validate start-spec-launch-parity --strict`

## 7. Amendment — per-repo discovery file (folded in 2026-10-18)

- [x] 7.1 `cmd_start_with_specs` SHALL call `write_repo_discovery_file`, as the bare `cmd_start`, `cmd_supervisor`, `add` and `remove` paths already do; it is the only launch path that does not, which is why `.git-paw/sessions/` stayed empty through both dogfood waves and `sweep.sh` could not find the session from the repo root
- [x] 7.2 Add a test asserting `.git-paw/sessions/paw-<project>.json` exists, with the agent list, after a spec-driven launch without supervisor mode

## 8. Amendment — stale docs in this change's area (folded in 2026-10-18, from a docs sweep)

- [x] 8.1 `docs/src/cli-reference.md` "Approval flag resolution" reads as if flags apply to every pane; after this change they do — make the text match the new behaviour (all launch paths), not the old one (the section actually lives in `docs/src/configuration/README.md` under `## Supervisor` — `cli-reference.md` has no such heading; fixed there)
- [x] 8.2 README unattended text claims `--force` "only matters for spec-mode launches"; correct it to what `--force` actually does after this change (it only matters for a spec-mode launch WITHOUT supervisor — `cmd_supervisor` never reads `force` and performs no uncommitted-spec check, so the README's own `git paw start --supervisor --force` example was actively misleading; removed it)
- [x] 8.3 `docs/src/` spec-driven-launch chapter (~line 300) says the boot block carries "broker URL, curl-publish patterns" and that the broker is on by default in supervisor mode; correct both against the code (broker.sh helper, not raw curl; `BrokerConfig::default()` is `enabled: false` unconditionally — supervisor mode never forces it on)

## 9. Gate review (b387ff7) — safety: launch-readiness gate on the start-time path

- [x] 9.1 `launch_spec_session` (spec-driven path) SHALL call `gate_pane_or_fail_on_dialog` for each pane before `submit_prompt_to_pane`, reusing the existing seam (`tmux-orchestration`'s Launch-readiness gate), so a blind `Enter` cannot accept a first-run trust/bypass dialog
- [x] 9.2 `cmd_start`'s bare pre-fill path SHALL also gate each pane before `build_boot_inject_args`, failing the launch loudly on `DialogStuck` the same way the supervisor and (now) spec-driven paths do
- [x] 9.3 Add an e2e test proving injection is withheld until the pane reports a readiness marker (not injected immediately after `execute()`) — `spec_driven_launch_withholds_injection_until_pane_is_ready`
- [x] 9.4 Add an e2e test proving a pane stuck on an unanswered first-run dialog fails the launch loudly (non-zero exit, no prompt injected) on both the spec-driven and bare paths — `spec_driven_launch_fails_loudly_on_stuck_dialog`, `bare_launch_fails_loudly_on_stuck_dialog`; both verified to fail against the pre-gate code

## 10. Gate review (b387ff7) — approval flags on the recovery launch path

- [x] 10.1 `cli_command_for_recovery` in `src/commands/recover.rs` SHALL apply resolved approval flags (it currently launches the bare `entry.cli` plus only resume args), per the ADDED `supervisor-config` requirement ("every launch path that spawns a worker CLI")
- [x] 10.2 Add a test asserting a recovered pane's command carries the resolved approval flags

## 11. Gate review (b387ff7) — retarget stale tests, drop the tautological one

- [x] 11.1 `tests/from_specs_launch_fixes_integration.rs` (~:261-334) asserts the spec path's pane command via `build_boot_inject_args`, which `cmd_start_with_specs` no longer calls — retarget those assertions at the real launch path (live tmux pane / probe script), not the now-unused pure helper
- [x] 11.2 Add a test for "Boot block is injected per agent pane in spec-mode-with-broker" with 3 real specs — panes 1, 2, 3 each get their own boot+task prompt — `spec_driven_launch_injects_per_pane_for_multiple_specs` (verifies a bijection rather than assuming discovery order matches input order)
- [x] 11.3 Add a test for "Pane offset accounts for dashboard" with N > 1 panes against the real launch path — covered by the same test (dashboard pane 0, agents at 1..=3, none mixed)
- [x] 11.4 Add a test for "Boot-block injection failure is non-fatal" (a simulated non-zero `tmux send-keys` SHALL NOT abort the launch) — `spec_driven_launch_survives_a_send_keys_failure`; manually verified the pane is genuinely gone (`tmux list-panes` confirms) and `tmux` logs `can't find pane: 1` while the launch still exits 0
- [x] 11.5 Replace the tautological `combine_boot_and_task_prompt_matches_supervisor_path_composition` unit test (it only proves the helper equals itself) with one that exercises the real spec-driven launch path, or drop it (dropped — `spec_driven_launch_submits_boot_prompt_with_task_prompt` already exercises the real path)

## 12. Gate review (b387ff7) — doc corrections

- [x] 12.1 `docs/src/user-guide/spec-driven-launch.md` (~:328, my own new text) claims no boot block is injected on any path when the broker is disabled — false for supervisor mode, where `attach_agent` builds the boot block unconditionally regardless of `broker_config.enabled`; correct the claim to the non-supervisor path only
- [x] 12.2 `docs/src/*/coordination.md` (~:142-143) still says manual broker mode only pre-fills and omits the spec-driven submit behaviour this change added; update it
- [x] 12.3 `docs/src/architecture.md` (~:389) says the readiness gate guards `git paw start`; make that true (task 9) and state it precisely

## 13. Gate review (b387ff7) — nits

- [x] 13.1 Use `find_free_port` instead of the fixed port `19219` (and `19229`) in `tests/start_spec_launch_parity_integration.rs`
- [x] 13.2 Poll the pane instead of a single `capture_pane` call in the approval-flag tests (`spec_driven_launch_applies_resolved_approval_flags`, `bare_launch_applies_resolved_approval_flags`)
- [x] 13.3 Fix the module doc's "three defects" framing and the stale "uses `cat`" comment in `tests/start_spec_launch_parity_integration.rs` (the submission tests use a logging probe script, not `cat`, after the revision earlier in this change)
- [x] 13.4 Update `proposal.md` to mention the `session-state` capability (per-repo discovery file) folded in as task 7
- [x] 13.5 README: state that bare `git paw start` now applies the resolved approval flags too (e.g. Claude's `acceptEdits` under the default `auto` level), not just supervisor-mode panes

## 14. Re-review (299d97b) — gate must run AFTER session state is saved

- [x] 14.1 `launch_spec_session` and `cmd_start` both gate-and-inject BEFORE `session::save_session` / `write_repo_discovery_file` — the opposite order from the supervisor path (`commands/supervisor.rs` saves ~:806/810, then gates ~:846/860). A stuck dialog on either start path today leaves a live tmux session + worktrees with no saved session state and no discovery file: `git paw stop`/`purge` cannot find the session, and a retry launches a new `paw-<project>-2` session instead of reattaching. Move both loops' session save + discovery-file write to before the gate-and-inject step, matching the supervisor path's order.
- [x] 14.2 Update `spec_driven_launch_fails_loudly_on_stuck_dialog` and `bare_launch_fails_loudly_on_stuck_dialog` to also assert the session JSON and the per-repo discovery file exist after the failed launch. Verified both assertions fail against the pre-reorder code (exact panic: "must still save session state for ...").
- [x] 14.3 `docs/src/architecture.md`'s readiness-gate paragraph says the gate guards "every pane a launch creates" — recovery and a broker-less bare start spawn panes without ever calling the gate (harmless: there is nothing to inject in either case). Reword to "every pane that receives an injected prompt".
- [x] 14.4 Make `spec_driven_launch_withholds_injection_until_pane_is_ready` robust to full-suite load: poll / use a longer window rather than a fixed 300ms early-capture delay. Rewritten to measure elapsed wall-clock time from this test's own detection of the session to when (if) the anchor first appears, asserting a *relative* gap (≥ half the stub's delay) rather than a fixed absolute sleep.
