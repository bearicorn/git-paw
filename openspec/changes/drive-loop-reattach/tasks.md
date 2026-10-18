## 1. Drive-loop binding

- [ ] 1.1 Add `src/supervisor/loop_binding.rs` (modelled on `claim.rs`): acquire per the design D4 table (create, replace a stale / other-instance / unreadable binding, refuse a fresh binding of the same instance; a supersede mode for recovery), refresh by mtime, release on drop only while it still names this loop; the refusal names the session, the holder's pid and the binding file. Unit tests (tempfile, backdated mtime): `a_binding_not_refreshed_within_the_window_lapses`, `a_binding_for_another_instance_does_not_block`, `recovery_supersedes_a_leftover_binding`, a fresh same-instance binding refuses
- [ ] 1.2 `FileSessionInstanceGuard` also requires the binding to name this loop and refreshes it; `run_drive_loop` holds the binding for the whole run. Unit test `lost_binding_ends_the_loop_before_it_acts` (no sweep, no keystroke, outcome `session-torn-down`); `instance_torn_down_stops_the_loop_before_the_next_sweep` passes unchanged
- [ ] 1.3 A fresh `--unattended` launch takes the binding after the session is saved and before the loop; `git paw stop` removes the binding after recording `Stopped`

## 2. Re-attach on a live session

- [ ] 2.1 In `cmd_supervisor`'s existing-session branch, when `unattended`: refuse a bare session before any change; take the binding (supersede on Stopped); Active → run the loop; Paused → revive without attaching, then run the loop; Stopped → recover without attaching, then run the loop; print the session name and `git paw attach`. Without `unattended` the three branches are unchanged
- [ ] 2.2 The drive-loop entry takes its roster from the live session, its broker URL from the recorded `broker_bind` / `broker_port`, and its instance token from the receipt read back from disk

## 3. Tests

- [ ] 3.1 E2E in `tests/unattended_drive_e2e.rs` (socket- and HOME-isolated, short `GIT_PAW_DRIVE_HEARTBEAT_MS`, condition-based waits): `unattended_rebinds_a_loop_to_a_live_session_after_heartbeat` (stdin null; same command re-run; pane count and titles unchanged, no worktree created, no `-2` session, binds without waiting), `second_unattended_loop_on_a_live_session_is_refused` (non-zero, names session and pid, first loop keeps running), `unattended_rebind_uses_the_recorded_broker_endpoint`, `unattended_refuses_a_bare_session` (session record, panes and worktrees unchanged)
- [ ] 3.2 E2E: `unattended_revives_a_paused_session_then_drives`, `unattended_recovers_a_stopped_session_then_drives`, `tests/e2e_supervisor_stop.rs::stop_revokes_the_drive_loop_binding`; confirm `tests/supervisor_reattach_integration.rs` passes unchanged

## 4. Docs

- [ ] 4.1 `docs/src/user-guide/supervisor.md`: what to run after a heartbeat exit or an interrupted loop (re-run the same `git paw start --unattended`), what it does per session status, one drive loop per session (the refusal and how to clear it), `stop` revokes the binding, bare sessions are refused, deleting `.git-paw/tmp/` ends a running loop, and the upgrade note (stop a loop started by an older binary before re-attaching); update "Session-instance lifecycle"
- [ ] 4.2 `start` help (`--unattended` help string and `long_about`), `docs/src/cli-reference.md` (`start --unattended` on a live session), README unattended paragraph (one clause)

## 5. Gates

- [ ] 5.1 `cargo fmt`, `just lint`, `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast` (verify the real exit code), `just deny`, `mdbook build docs/`, `openspec validate drive-loop-reattach --strict`
- [ ] 5.2 Security and safety review (`security-and-safety-review`): no new process, tmux command or keystroke path; the re-bound loop approves nothing; the binding path is the repository root plus a fixed name, its contents are parsed as data and its pid is never signalled; a superseded or revoked loop exits before acting
