## Why

During the v0.5.0 dogfood pass, running `cargo test --quiet` against the local checkout **twice** killed the live `paw-git-paw` supervisor session (broker on port 9119, dashboard pane, ten agent panes). The tmux server itself died: every pane vanished, `tmux ls` returned `no server running on /private/tmp/tmux-501/default`, and the supervisor had to relaunch the session from `.git-paw/sessions/paw-git-paw.json`. The bug is destructive and reproducible.

Static-analysis RCA (MILESTONE drift item 35, 2026-05-12) rules out a single offending test:

- Every `kill-session` in `src/` and `tests/` is uniquely prefixed (`paw-unit-test-*`, `paw-e2e-*`, `paw-aa-<tag>-<nanos>`, `paw-test-<tag>-<pid>-<n>`, `paw-rcv-<tag>-<pid>-<n>`, `paw-bootblk-<tag>-<pid>-<n>`, plus the `paw-repo` produced by `setup_test_repo()`'s `repo/` basename). None can collide with `paw-git-paw` by name.
- No `kill-server`, `pkill tmux`, `killall tmux`, `kill-window`, or `kill-pane` invocations exist anywhere in `src/` or `tests/`. The only `detach-client` (`tests/e2e_tests.rs:706`) is scoped to its own test session.
- A full `cargo test --quiet` creates and tears down ~15 real tmux sessions on the user's default socket (`/private/tmp/tmux-<uid>/default`): four multi-pane sessions in `e2e_tests`, three in `auto_approve_integration`, plus the full `git paw start` flows in `prompt_inbox_integration`, `boot_block_integration`, and `recover_integration`. Running concurrently with eleven live panes on the same socket is heavy load.

The **most likely cause** is a tmux server crash under load on the shared default socket (consistent with upstream tmux issues like `tmux/tmux#3014` and `#3367` — `select-layout tiled` recalculations, rapid `send-keys` floods, signal storms on macOS). It is not a malicious test; it is a tmux stability issue triggered by sharing a socket between the live session and the entire test suite.

The RCA also surfaced a **separate isolation bug**: `tests/e2e_tests.rs::broker_session_full_lifecycle` (line 836) does **not** override `HOME` or `XDG_DATA_HOME` before running `git paw start`. It writes `paw-repo.json` and broker logs into the user's real `~/Library/Application Support/git-paw/sessions/`, then runs `git paw stop` and `git paw purge --force` against that real directory. `find_session_for_repo` iterates every JSON file in that directory on each scan — including the live `paw-git-paw.json`. It dispatches by `repo_path` equality so it does not act on the live session, but the leak is real and pollutes the dev machine's session state.

This change is **test-infrastructure only**. No production code changes. No `src/` files modified. The goal is to make `cargo test --quiet` safe to run with a live `git-paw` session attached, and to plug the HOME/XDG leak in the one test that has it.

## What Changes

**New helper `tests/helpers/mod.rs::tmux_test_env()`** that returns a tuple `(TempDir, Vec<(&'static str, OsString)>)` (or equivalent): a freshly-created tempdir to scope the lifetime of the override socket directory, plus the env-var pairs (`TMUX_TMPDIR` set to the tempdir path, and `TMUX` removed) for any `std::process::Command` that will spawn tmux directly or transitively. The tempdir is dropped by the test, which removes the socket directory and any leftover socket files.

**Audit and update every test that spawns or talks to tmux** to apply `tmux_test_env()` to its `Command` builders. The five files that invoke `git paw start` or call `tmux new-session` directly are:

- `tests/e2e_tests.rs` — every `cmd()` builder that runs `git paw start` and every direct `tmux` invocation (capture-pane, list-panes, kill-session, detach-client).
- `tests/auto_approve_integration.rs` — direct `tmux new-session` + `git paw start` flows.
- `tests/boot_block_integration.rs` — `git paw start` flows.
- `tests/prompt_inbox_integration.rs` — direct `tmux new-session` + `git paw start` flows.
- `tests/recover_integration.rs` — `git paw start` flows (already overrides HOME, but not the tmux socket).

`src/tmux.rs::TmuxCommand::execute()` invokes `std::process::Command::new("tmux")` without explicitly clearing the env, so the child tmux process inherits `TMUX_TMPDIR` from the test process. No `src/tmux.rs` changes are needed.

**New helper `tests/helpers/mod.rs::guard_against_live_session()`** that fails the test run fast if a `paw-*` session exists on the user's default tmux socket (i.e. the socket the test would have used before this change). The check is cheap (`tmux ls 2>/dev/null | grep '^paw-'`) and is wired into `setup_test_repo()` so every integration test that depends on a tmpdir-backed repo also gets the guard for free. Failure messaging tells the user to either kill the live session or run targeted `cargo test --test <name>` invocations.

**Fix the HOME/XDG leak in `tests/e2e_tests.rs::broker_session_full_lifecycle`** (line ~874, the `cmd().current_dir(tr.path()).args(["start", ...])` call): adopt the existing pattern from `tests/cli_tests.rs:278-279` — `.env("HOME", fake_home.path())` plus `.env_remove("XDG_DATA_HOME")` — so the test writes session JSON and broker logs into a `TempDir`-rooted directory instead of the user's real `~/Library/Application Support/git-paw/sessions/`. Audit every other `cmd()` builder in `e2e_tests.rs` for the same leak; apply the same fix where missing. The same pattern SHALL be applied to every other test file that runs a `git paw` subcommand against the binary if the test currently relies on the user's real sessions directory by accident.

**No `src/` changes.** Production tmux invocation already honours `TMUX_TMPDIR` because `Command::new("tmux")` inherits parent env by default. Production behaviour is unaffected because users never set `TMUX_TMPDIR` themselves; only the test harness sets it.

**Not in scope (deferred):**

- Rewriting the test suite to share a single multi-test tmux server. The per-test socket-dir model is simpler and preserves test independence.
- Replacing every test's `kill-session` with `kill-server` on the test socket. The collision-guard plus per-test `TMUX_TMPDIR` already makes the live session unreachable; an explicit `kill-server` is redundant and would break per-test isolation if two tests in the same file shared a socket.
- A native-Rust tmux client. `std::process::Command` invocations are the project standard.
- Investigating the upstream tmux crash itself. Even if the crash is a tmux bug, isolating the test socket makes the bug unobservable for our purposes.

## Capabilities

### New Capabilities

- `test-isolation` — captures the test-harness invariants that protect the user's live tmux server and session state from the test suite. This capability is observable only through how the test files are structured (which env vars are set on `Command` builders, which guards run before tests spawn tmux); it has no runtime surface in the shipped binary. It is still spec-worthy because the invariants are non-obvious and easy to break by accident (any new test that runs `git paw start` or `tmux new-session` without the helper re-introduces the destructive behaviour).

### Modified Capabilities

*(none — this is a new capability)*

## Impact

**Code:**

- `src/` — **no changes**.

**Tests:**

- `tests/helpers/mod.rs` — adds `tmux_test_env()` and `guard_against_live_session()` helpers. `setup_test_repo()` calls the guard.
- `tests/e2e_tests.rs` — every `cmd()` builder and every direct `tmux` `Command` invocation applies `tmux_test_env()`. The `broker_session_full_lifecycle` `cmd()` chain gains `.env("HOME", fake_home.path())` and `.env_remove("XDG_DATA_HOME")`.
- `tests/auto_approve_integration.rs`, `tests/boot_block_integration.rs`, `tests/prompt_inbox_integration.rs`, `tests/recover_integration.rs` — every `Command` that spawns tmux applies `tmux_test_env()`.

**Docs:**

- `--help`: unchanged (no CLI surface change).
- README: unchanged.
- mdBook: unchanged (this is a test-internal invariant).
- Rustdoc on `tests/helpers/mod.rs::tmux_test_env` and `guard_against_live_session`: new doc comments describing the contract.
- `CONTRIBUTING.md`: a short paragraph under "Testing" naming the helpers and explaining why every new tmux-spawning test must call them. (If `CONTRIBUTING.md` does not exist, add the paragraph to the relevant section of `AGENTS.md` instead.)

**Backward compatibility:**

- Production behaviour byte-identical to v0.5. No `src/` changes; users do not set `TMUX_TMPDIR` themselves; the env var is read by tmux natively when set, ignored otherwise.
- Targeted `cargo test --test <name>` invocations continue to work exactly as before — each test file sets up its own socket dir.

**Mismatches resolved:**

- MILESTONE drift item 35 (cargo test kills the live paw-git-paw tmux session) — resolved by the per-test `TMUX_TMPDIR` socket and the collision-guard.
- The HOME/XDG leak in `broker_session_full_lifecycle` (sub-item of drift 35) — resolved by adopting the `tests/cli_tests.rs:278-279` pattern.
