## 1. Port additions (seam first, tmux unchanged)

- [ ] 1.1 Rebase over v0.18.0 `terminal-backend-seam` and `broker-host-separation` as archived; confirm the trait's method names and `SurfaceSpec` shape this change extends
- [ ] 1.2 `src/terminal/mod.rs`: `SurfaceActivity { Busy { last_output }, AwaitingInput, Failed(TurnExit), Interrupted, Unknown }`, `surfaces_are_interactive()`, `activity(surface)`, optional `TurnPlan { first, continuation }` on `SurfaceSpec`; doc comments on every item
- [ ] 1.3 `PawError::Unsupported { backend, operation }` (exit code 1, like every other error); `TmuxBackend` answers interactive / `Unknown`, ignores `TurnPlan`
- [ ] 1.4 `FakeBackend`: non-interactive mode, scripted activity, records `TurnPlan`
- [ ] 1.5 Tests: tmux argv fixtures identical with and without a `TurnPlan`; tmux activity issues no tmux command; fake scripts activity (scenarios "tmux answers interactive and unknown", "tmux ignores the plan", "The fake scripts activity")
- [ ] 1.6 Gate: `just check` green; existing tmux, drive-loop and e2e suites pass unchanged

## 2. Pure core of the headless surface

- [ ] 2.1 `src/terminal/headless/state.rs`: run-state layout from `SessionName` / `BranchSlug`; `surface.json`, `turns.jsonl` record types (internal, not a contract edge)
- [ ] 2.2 Pure `surface_activity(lock_held, last_turn, pending)` and liveness mapping; table test covering the six rows (scenario "Activity follows the turn lifecycle")
- [ ] 2.3 Pure draft / submit / clear transitions and the identical-pending-message dedup rule; table tests
- [ ] 2.4 Gate: `just check` green

## 3. Turn runner

- [ ] 3.1 Hidden `__headless-runner` subcommand (turn mode): `setsid` via a documented `unsafe extern "C"` declaration inside `pre_exec`; stdin `/dev/null`, output to `runner.log`; take the surface lock with `File::try_lock`, write the pid after locking
- [ ] 3.2 Turn loop: claim the oldest message by rename, run `sh -c 'exec <command>'` in the owner worktree with the session overlay env, write the message to stdin then close it, copy the shared stdout/stderr pipe to `output.log` (and the `stream_to` destination: append, create `0600`, refuse a symlink), append start/end records; first vs continuation command; release the lock, re-check the inbox, exit when empty
- [ ] 3.3 Service mode (`--service <name>`): run the command as a child, hold the service lock, forward nothing on `SIGHUP`, wait; stop the child when the run directory disappears (checked every 5 s)
- [ ] 3.4 Gate: `just check` green

## 4. HeadlessBackend adapter

- [ ] 4.1 `src/terminal/headless.rs`: `open_session` / `session_alive` / `close_session`, `set_session_env` (overlay only), `spawn_surface` (requires `TurnPlan`), `list_surfaces`, `label_surface`, `send_text` / `send_key` (Enter submits + ensures a runner with a bounded 5 s wait; `CtrlU` clears; other keys `Unsupported`), `capture_tail`, `stream_to`, `liveness`, `activity`
- [ ] 4.2 `kill` (SIGTERM to the group only while its lock is held, 5 s grace, SIGKILL, remove state) and `close_session`; `kill` via a documented `unsafe extern "C"` declaration
- [ ] 4.3 `spawn_service` / `stop_service` (`SIGHUP` to the group) / `service_alive`; `attach` (poll-tail every surface, label prefix, return on interrupt); `detach_clients` and `layout` no-ops
- [ ] 4.4 Owner-only permissions (`DirBuilderExt::mode(0o700)`, `OpenOptionsExt::mode(0o600)`) on every run-state path

## 5. Integration tests (stub CLI script, no tmux, `tempfile` data dir)

- [ ] 5.1 Spawn starts no process; first then continuation command; a spec without a plan is refused
- [ ] 5.2 Text+Enter is one message; `CtrlU` discards; a digit key is unsupported and changes nothing; a draft written by one process is submitted by another
- [ ] 5.3 Ordering: a message sent during a turn runs next; identical pending messages collapse; an interrupted turn is not replayed
- [ ] 5.4 Detachment: a turn survives the submitting process exiting and its pseudo-terminal closing (the PTY harness); stdin carries the message and no argument contains it; the stub reports no controlling terminal
- [ ] 5.5 Output: first-byte capture into the destination; later turns append; a symlinked destination is refused while the turn completes; `capture_tail` spans turns and is empty for a fresh surface
- [ ] 5.6 A stub exiting 1 reads failed and not alive; kill stops a busy stub within 10 s; close leaves no process and no run state; an idle session is alive
- [ ] 5.7 Attach prints labelled output; interrupting attach leaves the turn running
- [ ] 5.8 Session overlay reaches the turn; a caller token never appears in any run-state file; run-state modes are `0700` / `0600` and nothing is written inside the repository
- [ ] 5.9 Services: the broker host started as a headless service still answers `/status` after its launcher and pseudo-terminal are gone; stopping it frees the port within 5 s with the accepted message in the log; deleting the run state stops it within 10 s

## 6. Guards and docs

- [ ] 6.1 Extend the source audit: `setsid`, process-group `kill` and the runner command line only in `src/terminal/headless*` and the runner handler; every `unsafe` there preceded by `SAFETY:`; the port vocabulary audit covers the new identifiers
- [ ] 6.2 `docs/src/architecture.md`: module list and responsibilities gain the headless adapter and the runner; `docs/src/architecture-direction.md` Terminal row and Ports table updated (second adapter exists); `mdbook build docs/` succeeds
- [ ] 6.3 Gate 5 review of the `unsafe` sites (`setsid`, `kill`) and the threat table in design.md (`security-and-safety-review`)
- [ ] 6.4 Every scenario maps to a named test; `just check` and `just deny` green; full suite green against the merge-base with `cargo test --no-fail-fast`; no `unwrap`/`expect` in non-test code; exit codes verified with `cmd; echo $?`, not through a pipe
