## Context

tmux uses a Unix-domain socket to coordinate the server, every client, and every session. By default the socket lives at `$TMUX_TMPDIR/tmux-<uid>/default` (falling back to `/tmp` when `TMUX_TMPDIR` is unset). Every `tmux` invocation against the same socket joins the same tmux server process. When that server crashes, **every** session on that socket dies — the live `paw-git-paw` session, the test sessions, all of them.

Today every git-paw integration test that calls `tmux new-session` or runs `git paw start` (which itself calls `tmux new-session` inside `src/tmux.rs::TmuxCommand::execute()`) inherits the parent process's environment, which by default points at the user's shared default socket. During a full `cargo test --quiet` we create and tear down roughly fifteen real tmux sessions on that shared socket while the live supervisor session is also using it. The RCA in MILESTONE drift 35 concludes the most likely failure mode is a tmux server crash under load (consistent with upstream `tmux/tmux#3014` and `#3367`), not a misbehaving test that calls `kill-server`.

The cheapest, most-reliable fix is **per-test socket isolation** via `TMUX_TMPDIR`: tmux itself honours that env var as the directory in which it creates `tmux-<uid>/default`. If every test process sets `TMUX_TMPDIR` to a fresh `TempDir` before spawning tmux, every test session lives on its own socket and is unreachable from the live session's server. A server crash on the test socket cannot kill the live session.

`std::process::Command::new("tmux")` inherits the parent env by default, so `src/tmux.rs` already propagates `TMUX_TMPDIR` to its tmux children without any code change. The only change is in the test harness: every `Command` that will (directly or transitively) spawn tmux must have `TMUX_TMPDIR` set, and the env var must point at a test-owned tempdir whose lifetime exceeds the tmux server's. A shared helper plus a collision-guard makes the invariant easy to enforce.

## Goals / Non-Goals

**Goals:**

- Running `cargo test --quiet` (full suite, default parallelism) while a `paw-git-paw` session is live SHALL NOT touch the user's default tmux socket and SHALL NOT kill the live session.
- Every test that calls `tmux new-session` or runs `git paw start` SHALL use a dedicated, test-owned tmux socket directory.
- If the user accidentally starts a `cargo test` run with a `paw-*` session still on the default socket, the test run SHALL fail fast with a clear error message before any test spawns tmux.
- The HOME/XDG leak in `tests/e2e_tests.rs::broker_session_full_lifecycle` SHALL be plugged so the test no longer writes session JSON into the user's real sessions directory.
- The fix is mechanical and discoverable: a single helper that every tmux-spawning test wraps its `Command` with.

**Non-Goals:**

- Production code changes. `src/tmux.rs` and the rest of `src/` SHALL NOT change.
- Investigating or fixing the upstream tmux crash. Isolation makes the crash unobservable for our purposes; the crash itself is out of scope.
- Refactoring the test suite to share a single tmux server across tests. Per-test sockets are simpler and preserve test independence.
- Sandboxing the test suite from the user's `~` more broadly (only the HOME/XDG leak in `broker_session_full_lifecycle` is in scope; other tests already isolate HOME correctly per `tests/cli_tests.rs:278-279`).
- Adding a `cargo test`-wide pre-flight binary or build script. The collision-guard runs inside the existing `setup_test_repo()` so it triggers at the natural place rather than at build time.

## Decisions

### D1. `TMUX_TMPDIR` per-test override, not a code-side socket flag

**Choice:** Set `TMUX_TMPDIR` to a fresh `TempDir` on every `std::process::Command` that will spawn tmux (directly or transitively via `git paw start`). The child tmux process reads `TMUX_TMPDIR` natively and creates its socket under that directory. Different tests get different sockets; the user's live socket is never touched.

**Why:**

- **Native tmux behaviour.** `TMUX_TMPDIR` is the documented tmux mechanism for redirecting the socket directory (`man tmux`, "ENVIRONMENT"). It needs zero changes to `src/tmux.rs` and zero changes to how production invokes tmux.
- **Per-test isolation by construction.** Each `TempDir` is unique and is dropped at the end of the test (or at the end of the test binary's process, whichever comes first), which automatically removes leftover sockets. Tests in the same file that share a `TempDir` share a tmux server, which is desirable for tests that exercise multi-pane / multi-session flows.
- **Zero production risk.** Production code does not set `TMUX_TMPDIR`. Production users do not set `TMUX_TMPDIR`. The env var is invisible to users who don't read this design doc.
- **Inheritance is automatic.** `std::process::Command::new("tmux")` inherits the parent env unless `.env_clear()` is called. `src/tmux.rs::TmuxCommand::execute` does not call `.env_clear()`, so any `TMUX_TMPDIR` set on the test process's `Command` builder propagates to every tmux subprocess it spawns, including the tmux subprocesses spawned by `git paw start` inside `cmd().args(["start", ...]).output()`.

**Alternatives considered:**

- **A1. Change every test's `kill-session` to `kill-server` on the test socket.** *Rejected* — only kills the per-test server *after* the test finishes; does nothing to prevent the shared-socket crash during the test run.

- **A2. Pass `-L <socket-name>` explicitly to every `tmux` invocation.** *Rejected* — `-L` selects a named socket on the **default** socket directory. Two test processes that pick the same name still collide, and we'd need to either generate unique names per process and per test (more complex than `TMUX_TMPDIR`) or thread the name through to `src/tmux.rs` (production code change, which violates the test-only scope).

- **A3. Run the entire test suite with `RUST_TEST_THREADS=1` while sessions are live.** *Rejected* — slows the test suite ~10x, requires every contributor to remember to set the env var, and doesn't actually prevent the crash (single-threaded test execution still creates ~15 sessions on the shared socket sequentially, which is still load that triggered the crash in the RCA reproducer at least once).

- **A4. Modify `src/tmux.rs` to always use `-L paw-tests` in `cfg(test)` builds.** *Rejected* — `cfg(test)` doesn't propagate to integration tests in `tests/`, which link against the release crate. Even if it did, baking test-specific socket selection into production code is exactly the kind of test-leak we want to avoid.

**Cost:** Every existing tmux-spawning test grows two extra `Command` builder calls (`.env(...)`, optionally `.env_remove(...)`). The collision-guard plus the helper make the pattern grep-able and easy to enforce on new tests.

### D2. Centralise the env injection via `tests/helpers/mod.rs::tmux_test_env()`

**Choice:** Add a shared helper that returns the `(TempDir, env-pairs)` tuple. Tests apply the env pairs to every `Command` builder that spawns tmux, and keep the `TempDir` alive for the duration of the test. The exact signature is left to the implementer, but the shape is something like:

```rust
pub struct TmuxTestEnv {
    socket_dir: TempDir,
}

impl TmuxTestEnv {
    pub fn new() -> Self { /* create TempDir under std::env::temp_dir() */ }
    pub fn apply(&self, cmd: &mut Command) -> &mut Command {
        cmd.env("TMUX_TMPDIR", self.socket_dir.path())
           .env_remove("TMUX") // detach from any outer tmux invocation
    }
    pub fn socket_dir(&self) -> &Path { self.socket_dir.path() }
}
```

**Why:**

- **One audit, one helper.** Future contributors who add a new tmux-spawning test discover the helper via the existing helpers module, copy the existing two-line pattern, and are protected automatically.
- **`TMUX` removal as well as `TMUX_TMPDIR` set.** When `cargo test` is launched from inside a tmux session, the parent process has `TMUX=<socket>,<pid>,<window>` in its environment. Children that see `TMUX` set behave as "inside-tmux clients" and can sometimes attach to the parent's server when they were meant to start a new one. Removing `TMUX` before each tmux spawn forces the child to behave as a standalone outer client.
- **`TempDir` lifetime matches the test.** The struct owns the `TempDir`, so dropping the struct removes the socket dir. If the test panics, `TempDir`'s `Drop` impl still runs and cleans up.

**Alternatives considered:**

- **A1. A free function `tmux_test_env() -> Vec<(&'static str, OsString)>`.** *Rejected* — every test caller would need to hold a `TempDir` alongside the env vec to keep the path alive. The struct couples the two and prevents the foot-gun.
- **A2. A `lazy_static!` global socket dir for the whole test process.** *Rejected* — defeats per-test isolation; if two tests in the same binary clobber each other's sessions on the shared global socket, we have re-created a smaller version of the original bug.

### D3. Collision-guard placement: `setup_test_repo()`, fail-on-collision

**Choice:** Add `guard_against_live_session()` to `tests/helpers/mod.rs`. Call it as the first line of `setup_test_repo()`. The guard executes `tmux ls 2>/dev/null` against the user's **default** socket (i.e. *without* applying `tmux_test_env()`) and inspects the output for any line starting with `paw-`. If a `paw-*` session is found, the guard **panics** with a clear message naming the offending session(s) and recommending either killing the live session or running targeted `cargo test --test <name>` invocations.

**Why:**

- **`setup_test_repo()` is the right place.** Every integration test that creates a real repo (which is every test that also spawns tmux) calls it. One placement covers every test.
- **Fail-fast, not skip.** If we silently `return` from a test that detects a live session, contributors lose coverage without noticing. A panic gives the maintainer a chance to make a deliberate choice: either kill the live session and re-run, or use `cargo test --test <name>` for targeted runs. Lost coverage from a silent skip would have masked the v0.4 dogfood regressions that surfaced drift item 24 (config-test isolation) and drift item 35 (this one).
- **Cheap check.** `tmux ls` against the default socket is one process spawn and returns instantly. The cost is paid once per test, and only `setup_test_repo()`-using tests pay it.
- **Defence in depth.** Even with `TMUX_TMPDIR` per-test isolation, the guard catches scenarios where a contributor accidentally writes a new test that forgets the helper. The guard says "you have a live `paw-*` session; until you make this test socket-isolated, we won't let it run."

**Alternatives considered:**

- **A1. Run the guard from a `#[ctor]`-style global init.** *Rejected* — adds a dev-dependency (`ctor`) just for one assertion. The `setup_test_repo()` placement covers every relevant test without new deps.
- **A2. Make the guard a runtime warning, not a panic.** *Rejected* — see "fail-fast, not skip" above; a warning is too easy to ignore in CI logs and gives no signal that the test was actually unsafe to run.
- **A3. Check for `paw-git-paw` literally, not any `paw-*`.** *Rejected* — too narrow. Contributors may dogfood with project-named sessions like `paw-other-project`; the guard should protect every git-paw session on the host, not just one.

### D4. HOME/XDG leak fix in `broker_session_full_lifecycle`

**Choice:** Adopt the existing pattern from `tests/cli_tests.rs:278-279`:

```rust
let fake_home = TempDir::new().expect("home tempdir");
let _start_output = cmd()
    .current_dir(tr.path())
    .env("HOME", fake_home.path())
    .env_remove("XDG_DATA_HOME")
    .args(["start", ...])
    .output()
    .expect("run start command");
```

Apply the same pattern to every other `cmd()` builder in the same test (the `stop` call, the `purge --force` call) so the test operates entirely against `fake_home`'s sessions directory. Then audit the rest of `tests/e2e_tests.rs` for any `cmd()` that runs a `git paw` subcommand without the same env overrides.

**Why:**

- **Pattern already exists and is tested.** `tests/cli_tests.rs:278-279` proves the pattern works for `purge --force`; the same pattern resolves the leak for `start`, `stop`, and any other subcommand that reads or writes the sessions directory.
- **No new helper needed.** The two lines are short and obvious enough that they don't need to be wrapped (and wrapping them adds friction for tests that intentionally want to test against the real HOME, of which there should be none).
- **Resolves drift 35's sub-item explicitly.** The drift item names this fix candidate by file and line; D4 implements it directly.

**Alternatives considered:**

- **A1. Add a `cmd_with_isolated_home() -> Command` helper.** *Rejected* — the two-line idiom is already idiomatic in `cli_tests.rs` and well-understood by contributors. A helper would be discoverable but no shorter. Defer until a third file picks up the pattern.
- **A2. Have `src/main.rs` read a `GIT_PAW_SESSIONS_DIR` env var.** *Rejected* — this is the env-var override pattern that `config-test-isolation` deliberately *did not* pick (see `openspec/changes/config-test-isolation/design.md::D1`). Use the HOME/XDG mechanism that already exists.

### D5. Backward compatibility: production behaviour unchanged

**Choice:** No `src/` changes. Production users do not set `TMUX_TMPDIR`. Production `git paw` invocations continue to talk to the user's default tmux socket. The change is purely about which tmux server the **test suite** uses.

**Why:**

- The crash that motivated this change is a property of the *test suite* sharing a socket with the live session. Fixing the test suite is sufficient; the live session continues to use the default socket as before.
- Users who already set `TMUX_TMPDIR` for their own reasons (e.g. running git-paw inside a custom tmux config) continue to have `git paw` honour their setting, because `Command::new("tmux")` inherits parent env.

## Risks / Trade-offs

**R1. CI runners that already isolate tmux per job.** If GitHub Actions / equivalent already creates a fresh `/tmp` per job, the per-test `TMUX_TMPDIR` is a no-op on top of an existing isolation layer. That's fine — it's still correct and still cheap.

**R2. Read-only `/tmp` in some sandboxed test environments.** `TempDir::new()` defaults to `std::env::temp_dir()`. If that location is read-only, the helper panics on construction with a clear "permission denied" message. This is the same failure mode as today (tests already use `TempDir` for HOME and repo isolation), so no new risk.

**R3. Contributors who add new tmux-spawning tests and forget the helper.** The collision-guard in `setup_test_repo()` catches the worst case (test run against a live session). Tests that don't go through `setup_test_repo()` bypass the guard; for those, the only defence is code review. Mitigation: the helpers module's Rustdoc names the invariant explicitly, and `CONTRIBUTING.md` (or `AGENTS.md`) gains a paragraph saying "every test that spawns tmux MUST apply `tmux_test_env()`".

**R4. Tests that intentionally exercise multi-session interaction on a shared socket.** None exist today; if one is added later, it can construct a single `TmuxTestEnv` and pass it to every relevant `Command`. The struct supports that case by design (one tempdir, many `Command::apply` calls).

## Open Questions

**Q1. Should we also remove `TMUX_PANE` from the child env?** The `TMUX` env var is the load-bearing one for "am I already inside tmux?" detection; `TMUX_PANE` is a sibling that some scripts read. Removing it costs nothing and avoids surprises. *Tentative answer: yes, remove `TMUX_PANE` alongside `TMUX` in `tmux_test_env::apply`.* Confirmed during implementation; revisit if any test relies on the inherited `TMUX_PANE`.

**Q2. Should the collision-guard check for `paw-*` or for the *exact* session name a live dogfood session would have (`paw-git-paw` only)?** Drift 35 reproduces with `paw-git-paw` specifically, but D3's "any `paw-*`" rationale generalises. *Tentative answer: any `paw-*`*. Confirmed during implementation; tighten to a configurable allowlist if the broader check produces false positives.

**Q3. Should the guard be opt-out (e.g. via env var `GIT_PAW_ALLOW_LIVE_SESSION=1`)?** A contributor running a single targeted test with `cargo test --test foo bar` may legitimately want to keep their live session. *Tentative answer: yes, honour `GIT_PAW_ALLOW_LIVE_SESSION=1` as an escape hatch in the guard, but default to fail.* The escape hatch is mentioned in the panic message so the maintainer discovers it on first failure.
