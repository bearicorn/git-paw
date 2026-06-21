## Why

git-paw's session-management features (`git paw add`/`remove`, the full start → broker → stop lifecycle) can only be exercised end-to-end by booting a real tmux session with a real AI CLI — there is no way for CI or a developer to validate the orchestration plumbing without an LLM backend and a live terminal. The isolation recipe that makes this safe (private tmux socket, ephemeral broker port, throwaway repo, dummy CLI) has been battle-tested in dogfooding but lives only as ad-hoc shell incantations. This change packages it as a first-class `git paw selftest` subcommand so the lifecycle is verifiable anywhere.

Separately, the F8 root-cause investigation found that the in-session verify flakes ("address already in use") are NOT caused by live-session collision — they come from the test broker-port helper `24_000 + (process::id() % 200)`, which yields only 200 PID-keyed ports. N concurrent `cargo test` runs collide modulo 200 and produce false-negative verify failures. The fix is an OS-assigned ephemeral port.

## What Changes

- Add a `git paw selftest` subcommand that runs an isolated session lifecycle (start → roster check → stop/purge) against a throwaway repo and a dummy CLI, then reports pass/fail. No real LLM backend or interactive terminal is required.
- Add a bundled E2E-isolation harness implementing the dogfood recipe: strip `TMUX`/`TMUX_PANE` from the child environment, use a private tmux socket (`tmux -L <uniq>` / `TMUX_TMPDIR`), allocate an ephemeral broker port, create the throwaway repo under `.git-paw/tmp/`, and launch a dummy CLI (e.g. `cat`/`sh`) so no real agent process is spawned.
- This closes git-paw-add's deferred live-verification tasks: the add/remove roster transitions become observable through `git paw selftest` with no live session required.
- **F8 correction:** replace the PID-mod-200 broker-port selection in the test harness with an OS-assigned ephemeral port (`bind 127.0.0.1:0`, read back the assigned port), making port selection collision-proof at any concurrency.

## Capabilities

### New Capabilities
- `selftest`: the `git paw selftest` subcommand and the bundled E2E-isolation harness that exercises a full session lifecycle (start, add/remove roster transitions, stop) with a dummy CLI, a private tmux socket, an ephemeral broker port, and a throwaway repo — reporting pass/fail with no real LLM backend.

### Modified Capabilities
- `test-isolation`: add a requirement mandating that broker-port selection in the test/selftest harness use an OS-assigned ephemeral port (`bind 127.0.0.1:0`) rather than the PID-mod-200 scheme, so concurrent runs never collide.

## Impact

- New subcommand in `src/cli.rs` (`Command::Selftest`) and dispatch in `src/main.rs`.
- New harness module (e.g. `src/selftest.rs`) implementing the isolation recipe, reusing the env-stripping and private-socket patterns already proven in `tests/helpers/mod.rs` and `tests/e2e_supervisor_stop.rs`.
- Test broker-port helper(s) across `tests/` switch from `N + (process::id() % M)` to ephemeral binding; the canonical helper already in `tests/e2e_supervisor_stop.rs::pick_broker_port` becomes the shared pattern.
- `--help` text, README CLI table, and the mdBook user guide gain the `selftest` command.
- No new third-party dependencies; uses `std::net::TcpListener`, `std::process::Command`, and existing tmux/broker/session modules.
