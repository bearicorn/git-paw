## 1. Dispatcher reorder (D5)

- [x] 1.1 In `src/main.rs:55-79`, move the supervisor-mode resolution chain (`resolve_supervisor_mode_from_cwd(supervisor, dry_run)?`) to BEFORE the `if from_specs { return cmd_start_from_specs(...) }` short-circuit.
- [x] 1.2 When `supervisor_enabled` is `true` AND `from_specs` is `true`, route to `cmd_supervisor` and pass `branches_flag = None` to trigger its existing `scan_specs(...)` fallback.
- [x] 1.3 When `supervisor_enabled` is `true` AND `from_specs` is `false`, route to `cmd_supervisor` with `branches_flag.as_deref()` (the existing v0.4 supervisor path).
- [x] 1.4 When `supervisor_enabled` is `false` AND `from_specs` is `true`, route to `cmd_start_from_specs` (existing behaviour).
- [x] 1.5 When neither is true, route to `cmd_start` (existing behaviour).
- [x] 1.6 Verify the existing `cmd_supervisor` body is unchanged. Its spec-scanning fallback at `src/main.rs:586-604` already handles `branches_flag = None` correctly.

## 2. Boot-block injection in `cmd_start_from_specs` (D4)

- [x] 2.1 In `src/main.rs::cmd_start_from_specs`, after `let mut tmux_session = builder.build()?;` and AFTER `tmux_session.execute()?;` (the existing line at ~1155), and BEFORE the `tmux::attach(...)` call (line ~1178), insert a broker-aware injection block.
- [x] 2.2 Mirror the existing `cmd_start:389-398` injection block:
  ```rust
  if broker_config.enabled {
      let pane_offset = usize::from(broker_config.enabled);
      for (idx, (branch, _)) in mappings.iter().enumerate() {
          let pane_idx = idx + pane_offset;
          let boot_block = git_paw::skills::build_boot_block(branch, &broker_config.url());
          let args = git_paw::tmux::build_boot_inject_args(
              &tmux_session.name,
              pane_idx,
              &boot_block,
          );
          let _ = std::process::Command::new("tmux").args(&args).status();
      }
  }
  ```
- [x] 2.3 Confirm the `mappings` variable name matches the local in `cmd_start_from_specs` (or adjust to match — the iterable that gives `(branch, cli)` tuples for spec mappings).
- [x] 2.4 Do NOT add spec-content / task-prompt injection here — that's deferred to a separate D1 change. Boot block only.
- [x] 2.5 Verify the failure of any individual `tmux send-keys` does not abort the launch (best-effort pattern; matches existing `cmd_start` behaviour with `let _ = ...`).

## 3. Non-TTY launch handling (D2)

- [x] 3.1 Add `use std::io::IsTerminal;` at the top of `src/main.rs` (or import locally where used).
- [x] 3.2 Define a small helper `fn is_interactive_stdin() -> bool { std::io::stdin().is_terminal() }` for clarity and easy mocking in tests.
- [x] 3.3 In `cmd_start` (the bare path), wrap the final `tmux::attach(&tmux_session.name)` call:
  ```rust
  if is_interactive_stdin() {
      tmux::attach(&tmux_session.name)
  } else {
      println!("Session '{}' started in detached mode.", tmux_session.name);
      println!("Attach with:  tmux attach -t {}", tmux_session.name);
      Ok(())
  }
  ```
- [x] 3.4 In `cmd_start_from_specs`, apply the same wrapping around its final `tmux::attach(...)` call.
- [x] 3.5 In `cmd_supervisor`, apply the same wrapping. Specifically, gate BOTH the implicit attach AND the foreground supervisor-CLI launch (`Command::new(supervisor_cli).status()` at `src/main.rs:870`) on `is_interactive_stdin()`. On non-TTY, skip the supervisor-CLI launch and add a hint:
  ```
  Session '<name>' started in detached mode.
  Supervisor agent NOT started — supervisor mode requires an interactive terminal.
  Attach with:  tmux attach -t <name>
  Run the supervisor manually from a real terminal:  cd <repo>; <supervisor_cli>
  ```
- [x] 3.6 Ensure the auto-approve poll thread (`spawn_auto_approve_thread`, `src/main.rs:849-868`) is also handled — if the supervisor CLI is skipped, the poll thread shouldn't run either (or should be configured to no-op). Stop the thread immediately if it was spawned conditionally.

## 4. Dispatcher tests

- [x] 4.1 Unit test: parse `start --from-specs --supervisor` and assert the dispatcher target is `cmd_supervisor` (use a flag-tracking mock or refactor the dispatcher to a testable pure function).
- [x] 4.2 Unit test: parse `start --from-specs` (no supervisor flag, no `[supervisor]` config) and assert the dispatcher target is `cmd_start_from_specs`.
- [x] 4.3 Unit test: parse `start --from-specs` with a config containing `[supervisor] enabled = true` and assert the dispatcher target is `cmd_supervisor`.
- [x] 4.4 Unit test: parse `start --from-specs --no-supervisor` and assert the dispatcher target is `cmd_start_from_specs`.
- [x] 4.5 Unit test: parse bare `start` and assert the dispatcher target is `cmd_start`.
- [x] 4.6 Integration test (using `assert_cmd` in `--dry-run` mode): `start --from-specs --supervisor --dry-run` produces the supervisor-mode dry-run header (`Supervisor: ...`, `Agent CLI: ...`, `Approval: ...`) rather than the from-specs dry-run header (`Session: ...`).

## 5. Boot-injection tests

- [x] 5.1 Integration test (using `assert_cmd` and a mocked tmux that records `send-keys` calls): launch bare `--from-specs` with `[broker] enabled = true` and a fixture spec; assert that exactly one `send-keys` call is made per spec pane with the expected boot-block content.
- [x] 5.2 Integration test: same fixture but `[broker] enabled = false`; assert that NO `send-keys` calls are made for boot-block injection.
- [x] 5.3 Unit test: `build_boot_inject_args` produces the same argv shape used in `cmd_start_from_specs` and `cmd_start` (already exists; just ensure new call sites pass through it consistently).
- [x] 5.4 Pane-offset test: with broker enabled and 3 spec mappings, assert pane targets are `<session>:0.1`, `<session>:0.2`, `<session>:0.3` (dashboard at `0.0`).

## 6. Non-TTY handling tests

- [x] 6.1 Integration test: invoke `start --branches feat/x` with stdin redirected from `/dev/null` (or use `assert_cmd`'s stdin-control); assert exit code 0, stdout contains "Session" and "Attach with: tmux attach", AND no "failed to attach" error.
- [x] 6.2 Integration test: same as above with `--from-specs`; assert the same outcome.
- [x] 6.3 Integration test: same as above with `--from-specs --supervisor`; assert exit code 0, the attach hint AND the additional supervisor-mode hint, AND that the supervisor CLI is NOT spawned.
- [x] 6.4 Sanity test: with a real TTY (or mocked-as-TTY), the legacy `tmux::attach` call is still made.

## 7. Documentation

- [x] 7.1 Update `docs/src/user-guide/start.md` (or wherever the existing `start` flow is documented) with: the new dispatch ordering for `--from-specs --supervisor`; the boot-block-in-spec-mode behaviour; the non-TTY behaviour and the manual-attach hint.
- [x] 7.2 Add a short troubleshooting entry: "If you see 'Session started in detached mode' instead of attaching, your terminal is not interactive — `tmux attach -t <session>` to take over."
- [x] 7.3 `mdbook build docs/` succeeds.

## 8. Release notes

- [x] 8.1 Release notes: announce the dispatcher fix (`--from-specs --supervisor` now actually engages supervisor mode); the boot-block parity for bare `--from-specs` (broker mode); the non-TTY launch handling.
- [x] 8.2 Loud call-out for users who relied on the silently-broken `--from-specs --supervisor` (it was running spec-mode-only despite the flag): from this release, supervisor mode actually engages — verify your `.git-paw/config.toml` `[supervisor]` section is set up correctly OR pass `--no-supervisor` if you want the spec-only behaviour.

## 9. Quality gates

- [x] 9.1 `just check` — fmt, clippy, all tests green.
- [x] 9.2 `just deny` — supply chain clean.
- [x] 9.3 No new `unwrap()` / `expect()` in non-test code.
- [x] 9.4 `mdbook build docs/` succeeds.
- [x] 9.5 `openspec validate from-specs-launch-fixes` passes.
- [x] 9.6 Manual smoke test: from a real interactive terminal, run `git paw start --from-specs --supervisor --dry-run` against a repo with pending specs and verify the dry-run output shows `Supervisor: <cli>`, `Agent CLI: <cli>`, branch list per spec — i.e. the supervisor-mode dry-run shape, NOT the from-specs dry-run shape.
- [x] 9.7 Manual smoke test: from a non-TTY context (e.g. `git paw start < /dev/null` or piped output), the launch exits cleanly with the attach-hint instead of the "failed to attach" error.
