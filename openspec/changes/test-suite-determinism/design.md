## Context

The flakes on the roadmap were each filed as "load-sensitive, passes isolated". Reading the tests
shows a narrower cause in every case: the outcome depends on state the test does not own.

| Test | What it depends on that it does not control |
|---|---|
| `pause_e2e::pause_detaches_and_stops_broker` | Builds its session on the **default** tmux socket (no `tmux_test_env`; under `GIT_PAW_ALLOW_LIVE_SESSION=1` that is the developer's live server). Waits up to 5 s for the broker to bind but never asserts it did. Infers shutdown from `wait_until_port_free`, whose probe is a 200 ms `connect_timeout` — a slow accept reads as "free". The ceiling is already 30 s (selftest-harness §3.4), and the dashboard-orphan cause was fixed in v0.10.0. |
| `sweep_sh_stuck_detection::repeated_detection_is_deduped` (and siblings `blocked_on_supervisor_past_window_publishes`, `permission_prompt_with_stale_heartbeat_publishes_stuck`) | `sweep.sh` sources `discover_prompt_profile_field` → `git paw __prompt-profile` from the ambient `PATH`, so the markers come from whatever `git-paw` is installed (none in CI). `STUCK_DEDUP_WINDOW_SECONDS=300` cannot elapse between two back-to-back sweeps; the dedup entry is written only after a publish that `urlopen(timeout=2)` judges successful, and the fixture's recorder stops accepting 20 s after it is spawned. A first publish that times out or hits a dead stub leaves no entry, so the second sweep publishes again. |
| `worktree_env_provisioning::provisioned_files_are_present_before_the_agent_cli_starts` | The probe CLI is typed (`send-keys`) into the pane's login shell — the developer's `$SHELL` reading the developer's dotfiles, because the test overrides neither `HOME` nor the shell. Startup cost and any rc-file prompt are unbounded. Reproduced 3/3 in isolation under load. |
| `e2e_supervisor_returns::cmd_supervisor_returns_immediately_with_attach_hint` | `.timeout(10 s)` is the assertion. A launch that is slow but correct is killed; the failure shows empty stdout and stderr. The property under test is "does not block on attach", which a non-TTY stdin already makes observable: an attach would fail fast, not hang. |
| llvm-cov profile merge | An instrumented `git-paw` (typically `__dashboard` in a pane) still running, or killed mid-write, when `cargo llvm-cov` merges. Tests on the default socket leave panes on a server that outlives the test binary; `TmuxTestEnv::drop` kills the server but does not wait for pane processes to exit. |

Five files spawn tmux on the default socket: `pause_e2e.rs`, `agent_pane_liveness_e2e.rs`,
`stop_confirmation_test.rs`, and — through `git paw start` — `default_branch_fallback_integration.rs`
and `rebase_integration.rs`. Four files carry their own stub broker with a self-imposed 15–20 s
lifetime (`sweep_sh_stuck_detection.rs`, `sweep_sh_status_publish.rs`,
`sweep_sh_session_discovery.rs`, `broker_sh_session_discovery.rs`). CI's `smoke-headless` job is
`continue-on-error: true`.

## Goals / Non-Goals

**Goals:**

- Each named test passes or fails for a reason it controls, on a quiet machine, under load, next to
  a live session, and cold.
- The rules are enforced by the suite, so the next test file cannot regress them silently.
- A hang is reported as a hang, naming what was awaited.

**Non-Goals:**

- Changing production behaviour. `sweep.sh`'s 2 s publish timeout is left alone: it is an exported
  script surface and v0.24.0 is a feature freeze (see Risks).
- Raising timeouts. The only ceiling left is a hang guard.
- Removing `guard_against_live_session` or `GIT_PAW_ALLOW_LIVE_SESSION`. With every test isolated
  they become belt-and-braces, which is fine.

## Decisions

### D1 — Standing socket-isolation test, mirroring the HOME guard

`test-session-state-isolation` (v0.17) replaces a one-time HOME audit with a convention test. The
same failure mode applies to tmux: the original audit passed, then five files arrived without the
helper. Extend the same scanner: a file that spawns `tmux` or invokes the binary with a
tmux-reaching subcommand must apply `tmux_test_env` (or an explicit `-L`/`-S` socket, as
`sweep_sh_tmux_fallback.rs` does), or carry a documented-exception comment. Since v0.18.0's
`terminal-backend-seam`, a test using the fake backend counts as isolated.

*Alternative rejected:* setting `TMUX_TMPDIR` suite-wide via `.cargo/config.toml` `[env]`. It also
applies to `cargo run`, which would move a developer's dogfood session onto a hidden socket, and it
cannot unset `TMUX`, which tmux consults before `TMUX_TMPDIR`.

### D2 — Pin the pane shell in the isolation helper

`TmuxTestEnv::apply*` sets `SHELL=/bin/sh` on the child. The test-owned server is started by that
child, so its default shell is `/bin/sh`; with HOME already a tempdir (v0.17 rule) no per-user
startup file runs. Production is unaffected — the helper is test-only. *To verify:* that tmux
derives `default-shell` from the starting client's `SHELL` on every supported tmux version; if not,
the helper sets `default-shell` on the private server right after it starts.

### D3 — One `wait_until` with one ceiling

`helpers::wait_until(what: &str, condition) -> T` polls the condition and returns as soon as it
holds. At `HANG_CEILING` (one constant, 120 s) it panics with `what`, the elapsed time, and any
diagnostics the caller attached (a pane capture, a process listing). The polling loop takes an
injectable clock so its own unit tests run instantly. assert_cmd `.timeout()` stays only as the hang
guard, set to the same ceiling, and a run killed by it is reported as "hung", never as an
assertion failure.

### D4 — Infer shutdown from process exit

For pause: read the broker host's pid from the session's service surface before driving `pause`
(the `broker` window since v0.18.0; the dashboard pane for a session started before the host split), wait for that process
to exit, then assert one connect is refused. A listening socket is closed when its process exits,
so the check is causal rather than sampled. The precondition is asserted too: `GET /status` answers
before `pause` runs.

### D5 — Pin the binary bundled scripts resolve

A helper prepends the directory of the binary under test to the child's `PATH`; fallback tests
strip every `git-paw` from `PATH`. Both forms are new in `tests/helpers/mod.rs`: the copy that
`sweep_sh_classify.rs` carried is deleted with that file by v0.17.0's `classifier-retirement`, so
nothing is moved. A standing test flags any test that runs `sweep.sh` (or any script that resolves
`git paw`) without one of the two.

### D6 — One stub broker helper

Replace the four copies with `helpers::StubBroker`: blocking accept on its own thread, reads headers
then exactly `Content-Length` body bytes, records the body, replies at once, and stops only when
dropped. Tests assert the sweep's own outcome line (`published` / `deduped` / `publish failed`) as
a precondition before asserting on recorded bodies, so a transport failure is reported as one.
Window-expiry cases pin time through seeded state files (the existing `seed_progress` pattern,
extended to the dedup file), not through elapsed wall-clock time.

### D7 — Reap pane processes before the isolation helper is dropped

`TmuxTestEnv::drop` lists every pane's process group on its private server, sends `kill-server`,
and waits (D3) until those groups are empty. A process still alive at the ceiling is killed and
reported by name. No instrumented process can then outlive its test and write a profile during the
merge.

### D8 — `just coverage` collects, then reports

`cargo llvm-cov --no-report` runs the suite; `cargo llvm-cov report` merges afterwards. Profiles are
written per process, so one bad file cannot corrupt another, and a malformed profile is named in the
output. *To verify:* the profile-file pattern cargo-llvm-cov sets, and whether it exposes
`llvm-profdata merge`'s failure mode; if it does not, the recipe documents how to locate and
discard a malformed profile instead.

### D9 — Cold-start CI job blocks merges

Remove `continue-on-error` from `smoke-headless` once the rewrites land. `just check` must pass with
no tmux server, no `TMUX`, no broker and no `git-paw` on `PATH`; that is exactly the CI job's
environment.

## Risks / Trade-offs

- [The source scanners are heuristic] → Prefer false positives (cheap to annotate) over false
  negatives (a test on the live socket); recognise each file's own `cmd()` helper pattern.
- [`sweep.sh` still uses a 2 s publish timeout] → With a stub that accepts on a dedicated thread
  and replies at once the budget is ample; the explicit `published` precondition makes a residual
  timeout visible as a transport failure, not a dedup bug. An environment override for the timeout
  was considered and rejected: it adds a surface to an exported script during the freeze.
- [Pinning `/bin/sh` hides shell-specific bugs] → Production panes run the user's shell; a
  shell-compatibility matrix is a separate concern and out of scope.
- [Reaping lengthens teardown] → It waits only for processes that are already exiting.

## Open Questions

- D2 and D8 each carry one "to verify" fact about a third-party tool (tmux `default-shell`,
  cargo-llvm-cov's merge options).

## Release-candidate gate (cycle process — not this change's tasks)

Recorded here because it is process, not product behaviour, and needs no requirement. It runs once,
last in the v0.24.0 cycle, on the release-candidate commit, after every finding change has merged:

1. Spec audit — every scenario in `openspec/specs/` maps to a test; no SHALL left unimplemented
   or contradicted (`spec-organization`).
2. Docs — `--help`, README, mdBook and the configuration reference agree; `mdbook build docs/`
   passes (`doc-completeness`).
3. Security and safety review of the whole exported surface — bundled skills, scripts, the init
   config, the `git paw setup-cli` grant table, injected `AGENTS.md`, hooks (`security-and-safety-review`,
   `export-agnosticism`).
4. Coverage ≥ 80 % from a clean collection (D8).
5. `cargo deny check` and `cargo audit` clean.
6. MSRV (`rust-version`) at or below the rustc every cargo-dist release runner installs — read the
   runner rustc from the last release workflow run, not from local stable.
7. `just check` green cold (D9) and the `upgrade-compat-matrix` rows green for every release.
8. Release-candidate dogfood: one full cycle driven by an external orchestrator session (the v0.18
   relocation) on a repository other than git-paw, with workers on at least two CLIs that v0.23.0
   rates first-class. Findings are filed as v0.24.0 changes; the gate re-runs after they merge.
