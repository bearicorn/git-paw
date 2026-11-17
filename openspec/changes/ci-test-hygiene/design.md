## Context

File:line references are as of `a450f46` (main at v0.20.0 plus the v0.21.0 spec commits).

**MSRV.** `Cargo.toml:5` declares `rust-version = "1.96"`, and `README.md:13`, `CONTRIBUTING.md:7`,
`docs/src/contributing.md:7` and `AGENTS.md:590` repeat it. AGENTS.md also says the MSRV must
stay at or below the cargo-dist release runner's rustc. `rust-toolchain.toml:2` pins
`channel = "stable"`, and every job in `.github/workflows/ci.yml` installs
`dtolnay/rust-toolchain@stable` (`ci.yml:43`, `:70`, `:94`, `:141`, `:159`, `:186`). Nothing
compiles on 1.96. `CHANGELOG.md:254` records that the MSRV was already lowered once, from 1.97 to
1.96, after the release build disagreed with it.

**Flaky timing tests.**
- `tests/e2e_supervisor_approval_flags.rs:221` `coding_agents_resolve_flags_from_agent_approval`
  launches a supervised session and polls each agent pane for a sentinel flag through
  `capture_pane_until` (`:75`), whose deadline is a fixed 10 s (`:77`). Under load, two agent CLIs
  booting plus the supervisor can take longer than that.
- `tests/e2e_supervisor_stop.rs:429` `stop_revokes_the_drive_loop_binding` is `#[serial]`, spawns
  an unattended drive loop (`:450`), and waits up to 60 s for the binding file (`:455`) before
  stopping and recovering.
- `tests/agent_pane_liveness_e2e.rs:409` (DV-18) builds `DeadPanes` (`:137`). It runs
  `tmux new-session -d` with no command (`:140-145`), so each pane runs the user's login shell,
  and doctor must then report "agent panes exited". The likely cause, not yet confirmed: a pane
  whose interactive shell is still starting can show a non-shell command for a moment (an
  oh-my-zsh prompt runs `git`), and doctor reads that as a live agent.

**Leaked processes (DV-21).** `TmuxTestEnv::new` (`tests/helpers/mod.rs:81-84`) creates an
anonymous `TempDir`. Its `Drop` (`:143-165`) runs `tmux kill-server` on the private socket.
`Drop` does not run when the test process is killed: a `cargo test` timeout, Ctrl-C, or a
nextest/CI kill. The private tmux server is a daemon, so it survives its test, and its panes keep a
live parent. `src/dashboard.rs:55-76` (`orphaned`, `getppid() == 1`) therefore never fires for
the pane's `__dashboard`, and the broker it hosts lives on. Directly spawned children leak the same
way: `tests/broker_host_e2e.rs:109-124` (`spawn_host`), `tests/e2e_supervisor_stop.rs:323` and
`tests/unattended_drive_e2e.rs:395`/`:430` (`spawn_unattended`, `_detached`).

**Stand-in race.** c286737 fixed `tests/cmd_golden_output.rs:140-162` and
`tests/command_library.rs:103-128`. Before the fix, their fixtures read liveness while the pane
still showed a shell, and `pause` reported the session already stopped. Each got its own copy of
a 10 s `#{pane_current_command} == "sleep"` poll. Other sites:
- `tests/agent_pane_liveness_e2e.rs:314-324` has a third copy (a `contains("sleep")` check on
  `list-panes`).
- `tests/pause_e2e.rs:160` types `sleep 3600` with `send-keys` and never waits.
- `tests/broker_host_lifecycle_e2e.rs:935` and `tests/sweep_sh_broker_window.rs:123` start
  `sleep 3600` panes.
- Six files each define their own `wait_until`: `agent_pane_liveness_e2e.rs:64`,
  `e2e_supervisor_stop.rs:362`, `broker_host_lifecycle_e2e.rs:244`, `broker_host_e2e.rs:160`,
  `broker_verdict_authority_e2e.rs:227` and `unattended_drive_e2e.rs:730`.

## Goals / Non-Goals

**Goals:**
- CI fails when the code no longer builds on the declared MSRV.
- Known flakes are written down once, with an exit bar.
- No test run leaves a `git-paw` process behind for the next one.
- One implementation of "wait until the pane runs its stand-in".

**Non-Goals:**
- Running the full test suite on the MSRV (`check` catches API and syntax use; the test job keeps
  stable).
- Retrying flaky tests automatically (`nextest --retries`). A retry hides the flake that the
  register exists to keep visible.
- Changing production orphan handling. `orphaned()` is correct for production. The leak exists
  because the test harness keeps a tmux server alive.
- Fixing the three flakes in this change. The register and the shared wait are the tools. Each fix
  lands as its own commit against its register entry.

## Decisions

### D1 — `cargo +1.96 check --all-targets --locked` in its own job
A new `msrv` job installs `dtolnay/rust-toolchain@1.96` and runs
`cargo +1.96 check --all-targets --locked`. The `+1.96` override takes precedence over
`rust-toolchain.toml`'s `stable`, so the file stays as is for contributors. `--locked` uses the
committed `Cargo.lock`, so a dependency resolved to a release that needs a newer rustc fails here
rather than on a user's machine. Its Swatinem cache uses `shared-key: msrv` and the same
`save-if` rule as the other jobs (`ci.yml:51-55`), because a 1.96 `target/` cannot be shared with
stable's.

*Alternatives:*
- `cargo msrv verify`: an extra tool to install, for the same check.
- `cargo hack --rust-version`: also an extra tool. It is only worth it once there are feature
  combinations, and there are none.

### D2 — The drift test parses both files with no new dependency
`tests/ci_workflow_conventions.rs` reads `Cargo.toml` with the `toml` crate already in
`[dependencies]`. It finds the `msrv` job's toolchain line in `ci.yml` with a line scan for
`dtolnay/rust-toolchain@` under `msrv:`, plus the `cargo +<v>` token in its `run`. Both versions
must equal `rust-version`. The test reads YAML as text on purpose, so it needs no YAML crate,
which would be a new dependency and require an AGENTS.md approval.

### D3 — The flake register is a TOML file
`tests/known_flakes.toml` holds one `[[flake]]` table per test, with keys `test` (`file::fn`),
`symptom`, `first_seen` and `tracking` (a free-text note). `tests/flake_register.rs` parses it
with `toml` and checks that each `fn <name>(` exists in the named file. TOML rather than Markdown
so the check is exact. The exit bar (ten consecutive runs while the suite runs in parallel) is
recorded in the commit that removes the entry.

### D4 — Reaping by owner marker, never by process name
- `TmuxTestEnv::new` creates its directory with `tempfile::Builder::new().prefix("git-paw-tmux-")`
  and writes `owner.pid` (the test process's pid, plus its start time, so a reused pid is not
  mistaken for the owner).
- The new `helpers::spawn_paw_child` records `<pid>` and the owner marker under
  `$TMPDIR/git-paw-test-children/`.
- Before creating anything, both scan their prefix. For each entry whose owner is gone, they run
  `tmux -S <dir>/tmux-<uid>/default kill-server`, or send `SIGTERM` to the child's process group
  and then `SIGKILL` after 2 s, then remove the entry.
- The children are spawned with `process_group(0)`, as `src/command_runner.rs:267-269` already
  does for production one-shots, so one signal reaches any grandchild.
- Nothing is ever selected by `ps`/`pgrep` on the name `git-paw`. That would kill the developer's
  live dogfood session.

*Alternative:* make `__dashboard` and `__broker` exit when their tmux socket file disappears. This
is a production behaviour change made for the sake of tests, and it does not cover directly
spawned children. Rejected.

### D5 — `wait_for_pane_command(env, target, expected, timeout)`
The new helper in `tests/helpers/mod.rs` generalises the c286737 loop, with a 50 ms poll. On
timeout it panics with the pane target and the last command seen. The six private `wait_until`
copies move to one `helpers::wait_until(budget, cond)` in the same commit. The convention test
flags a file that has a `sleep <n>` pane stand-in but no call to `wait_for_pane_command`.
`broker_host_lifecycle_e2e.rs:935` and `sweep_sh_broker_window.rs:123` either adopt the helper
or carry the same documented-exception comment the HOME convention test already honours, if they
never read liveness.

## Risks / Trade-offs

- [The 1.96 toolchain download adds time to CI] → about 30 s uncached, and the job runs in
  parallel with the others.
- [A dev-dependency's own MSRV is above 1.96] → the job fails on day one. Pin that crate with a
  lockfile downgrade, or record it in AGENTS.md. Do not raise the MSRV past the release runner.
- [The reaper kills a slow but alive test's server] → it cannot. The owner check uses pid plus
  start time, and an owner that is alive is never reaped.
- [The register becomes a parking lot] → every entry carries `first_seen`. The pre-release
  checklist reviews entries older than one release.

## Migration Plan

1. Add `helpers::wait_until` and `wait_for_pane_command`, and move the existing copies onto them,
   with no behaviour change.
2. Add the owner marker, the reaper and `spawn_paw_child`, then move the spawn sites onto them.
3. Add the register with the three entries, plus its convention test.
4. Add the `msrv` job and the drift test.
5. Update CONTRIBUTING with the register and the exit bar.

Rollback is a revert. No production or persisted state is involved.

## Resolved Questions (2026-11-17)

- **Issue links in register entries:** optional. `tracking` stays free text.
- **DV-18's cause:** checked during implementation. If it is the shell's prompt commands,
  `DeadPanes` starts its panes without rc files before the entry is de-registered.
