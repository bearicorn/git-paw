## Why

The suite's known flakes are not slow machines; they are tests whose outcome depends on things
the test does not control. Reading the code for each one:

- `pause_e2e` builds its sessions on the developer's **default** tmux socket (no
  `tmux_test_env`), never asserts that its broker came up before driving `pause`, and infers
  broker shutdown by polling a port with a 200 ms connect timeout. Its ceiling is already 30 s,
  not the 5 s the roadmap records.
- `sweep_sh_stuck_detection` runs `sweep.sh` with the developer's ambient `PATH`, so the
  prompt markers come from whatever `git-paw` happens to be installed (the case-sensitivity
  failure `live-prompt-tail-window` fixes is only reachable that way). Its stub broker shuts
  itself down 20 s after spawn and frames requests by "read until a short read". The dedup
  window is 300 s and cannot elapse between two sweeps; the dedup failure is a first publish
  that the 2 s client timeout or a dead stub turned into "failed", so no dedup entry was written.
- `worktree_env_provisioning`'s probe CLI is typed into the developer's own login shell, which
  reads the developer's dotfiles (the test overrides neither `HOME` nor the shell). It
  reproduced 3/3 in isolation under load.
- `cmd_supervisor_returns_immediately_with_attach_hint` uses assert_cmd's 10 s timeout as its
  assertion, so a slow but correct launch is killed and reported as a failure with empty
  output.
- llvm-cov profile merges break when an instrumented `git-paw` process is still running, or is
  killed mid-write, after its test returned.

Five test files still spawn tmux on the default socket, so `just verify`'s claim that "the suite
is socket-isolated" is false, and CI's cold-start job is `continue-on-error`, so a cold failure
never blocks a merge. v1.0.0 freezes on this suite; it has to be deterministic first.

## What Changes

- **Socket isolation becomes enforced, not audited.** A standing test fails the suite when a
  test file spawns tmux (directly or through `git paw`) without `tmux_test_env`. The five
  non-conforming files are fixed. The helper also pins the shell tmux starts in new panes, so
  a pane never runs the developer's shell configuration.
- **Waits are condition-based with one hang ceiling.** A shared `wait_until` helper returns as
  soon as the awaited condition holds and, at a single suite-wide ceiling, fails naming the
  condition with diagnostics. No assertion depends on a wall-clock budget. Shutdown is inferred
  from the owning process exiting, not from port probing.
- **Tests that run a bundled script pin the `git-paw` it resolves** to the binary under test
  (or strip it, for fallback tests), enforced by a standing test.
- **Stub servers live for their fixture** and frame requests by `Content-Length`.
- **Every process a test starts has exited before the test returns**, so no instrumented
  process can write a coverage profile during the merge. `just coverage` collects and reports
  in separate phases.
- **The suite passes cold.** CI's cold-start job becomes a blocking gate.
- The named tests are rewritten onto these rules. No production code changes.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `core-ci-hygiene`: *Tmux-spawning tests SHALL use a dedicated tmux socket* gains standing
  enforcement and pane-shell pinning; ADDED requirements for condition-based waits, pinned
  script binaries, fixture-scoped stub servers, process reaping and coverage collection, and a
  blocking cold-start gate.

## Impact

- **Tests:** `tests/helpers/mod.rs` (`TmuxTestEnv::apply*`, new `wait_until`, a pinned-binary
  `PATH` helper, a stub-broker helper, pane-process reaping in `TmuxTestEnv::drop`);
  `tests/pause_e2e.rs`, `tests/sweep_sh_stuck_detection.rs`, `tests/worktree_env_provisioning.rs`,
  `tests/e2e_supervisor_returns.rs`, `tests/agent_pane_liveness_e2e.rs`,
  `tests/stop_confirmation_test.rs`, `tests/default_branch_fallback_integration.rs`,
  `tests/rebase_integration.rs`, and every `sweep_sh_*` file without a pinned `PATH`; two new
  convention tests.
- **CI / tooling:** `.github/workflows/ci.yml` (`smoke-headless` loses `continue-on-error`),
  `justfile` (`coverage` split into collect + report; `verify` comment corrected).
- **Docs:** `CONTRIBUTING.md` / `docs/src/contributing.md` test-writing rules.
- **Production code:** none. **Enum-variant ripple:** none. **Dependencies:** none.
- **Cross-version dependencies:**
  - Builds on the released v0.17.0 `test-session-state-isolation` (HOME/XDG standing guard). It
    modified a different `core-ci-hygiene` requirement; the two convention tests should share
    one source scanner.
  - Built on the released v0.17.0 `stop-without-confirmation` (dropped `stop --force` from
    `pause_e2e.rs` and `stop_confirmation_test.rs`) and `live-prompt-tail-window` (fixed the
    case-sensitive marker match the stuck-detection tests tripped on).
  - v0.17 `classifier-retirement` MODIFIES *Tmux-spawning tests SHALL use a dedicated tmux socket*
    (its direct-tmux scenario names `tests/sweep_sh_approve.rs`, because it deletes
    `tests/auto_approve_integration.rs`). This delta starts from that text. The same change deletes
    `__classify` and `tests/sweep_sh_classify.rs`, so the pinned-binary helper is written fresh and
    the bundled scripts reach `git paw` only through `__prompt-profile`.
  - Built on the released v0.18.0 `broker-host-separation` (pause stops a `broker` service surface,
    which rewrote `pause_e2e`'s simulated layout) and `supervisor-pane-removal`, which kept
    `cmd_supervisor_returns_immediately_with_attach_hint` with new stdout. The test did not
    disappear.
  - v0.18.0 `terminal-backend-seam` changed how tests reach tmux; the standing isolation test
    must recognise the fake backend as isolated.
