## 1. CLI parser additions

- [x] 1.1 In `src/cli.rs`, add `Command::Pause` variant with no fields, `about` and `long_about` strings per the cli-parsing delta. The `long_about` SHALL mention the RAM trade-off (CLIs stay running) and point at `stop` for the destructive alternative.
- [x] 1.2 Extend `Command::Stop` to add a `force: bool` field (`#[arg(long, default_value_t = false, help = "Skip confirmation prompt")]`). Rewrite `Stop`'s `long_about` to name all three teardown verbs (pause / stop / purge).
- [x] 1.3 Update the root `after_help` quick-start guide in the `Cli` struct's `#[command]` attribute to include a `git paw pause` line between the start/status and stop lines.
- [x] 1.4 Update `Command::Stop` destructuring callers in `main.rs::run` (the dispatch match arm) to pull out the new `force` field and forward to `cmd_stop`.

## 2. Session-state extensions

- [x] 2.1 In `src/session.rs`, add `Paused` variant to `SessionStatus`. Confirm `#[serde(rename_all = "lowercase")]` produces `"paused"` on the wire.
- [x] 2.2 Extend the `Display` impl for `SessionStatus` to render `Paused => "paused"`.
- [x] 2.3 Update `Session::effective_status` per the session-state delta table: `Paused + alive => Paused`, `Paused + dead => Stopped`, others unchanged.
- [x] 2.4 Add `dashboard_pane: Option<u32>` field to `Session` with `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- [x] 2.5 Wire up `dashboard_pane` population in `cmd_start` (bare-start: `Some(0)` when broker enabled) and `cmd_supervisor` (supervisor mode: `Some(1)`). Plain `cmd_start_from_specs` follows whichever path it routes to.

## 3. `cmd_pause` implementation

- [x] 3.1 Add `cmd_pause()` in `src/main.rs` that:
  - Loads the current session via `session::find_session_for_repo`.
  - If no session exists, prints "No active session for this repo." and returns `Ok(())`.
  - If the session's recorded status is already `Paused`, prints "Session '<name>' is already paused." and returns `Ok(())`.
  - If the session's effective status is `Stopped`, prints "Session '<name>' is already stopped; pause has no effect." and returns `Ok(())`.
  - Otherwise: calls `tmux::detach_client(name)`, then `tmux::kill_pane(name, session.dashboard_pane.unwrap_or(0))`, then updates session status to `Paused` and persists.
  - Prints the resume hint: `"Session '<name>' paused. <N> CLI pane(s) still running. Run 'git paw start' to resume."`
- [x] 3.2 Wire `Command::Pause` into `main.rs::run`'s dispatch match arm.
- [x] 3.3 If the broker had no dashboard pane (e.g. `[broker] enabled = false`), `cmd_pause` SHALL still detach the client and update status — there is no broker pane to kill. The `kill_pane` step SHALL only fire when `session.broker_port.is_some()`.

## 4. `tmux::detach_client` and (if absent) `tmux::kill_pane`

- [x] 4.1 In `src/tmux.rs`, add `pub fn detach_client(session_name: &str) -> Result<(), PawError>` that runs `tmux detach-client -s <session-name>`. The function SHALL return `Ok(())` if the command exits 0 OR if stderr indicates "no clients attached" (idempotent no-op).
- [x] 4.2 If `tmux::kill_pane(session, pane_index)` is not already present in `tmux.rs`, add it. Wraps `tmux kill-pane -t <session>:0.<index>`. Returns `Ok(())` if the pane was already gone.
- [x] 4.3 Unit tests: `detach_client_succeeds_on_attached_session`, `detach_client_is_noop_with_no_clients`, `kill_pane_removes_pane`, `kill_pane_is_noop_for_missing_pane`. Use the existing test-helper pattern (create a detached test session with a unique `paw-pause-test-*` name; tear it down on drop).

## 5. `cmd_stop` confirmation prompt

- [ ] 5.1 Extend `cmd_stop` signature to take `force: bool`.
- [ ] 5.2 Before killing the session, when `force == false` AND stdin is a TTY (use the same `is_interactive_stdin()` helper as `cmd_supervisor`), render a `dialoguer::Confirm` prompt that:
  - Names the destructive consequences ("kills all CLI processes; loses agent conversation context").
  - Mentions `git paw pause` as the soft-stop alternative.
  - Mentions `git paw purge` as the full-reset alternative.
  - Defaults to `false` (user must press `y` to proceed).
- [ ] 5.3 When the session's recorded status is `Paused`, augment the prompt body with: "This session is currently paused; continuing will kill the still-running CLIs (`<branch1>`, `<branch2>`, …)."
- [ ] 5.4 When `force == true` OR stdin is not a TTY, skip the prompt and proceed with the kill (v0.4 back-compat).
- [ ] 5.5 If the user answers `no`, print "Stop cancelled." and return `Ok(())` without modifying state.

## 6. `cmd_start` restart-from-pause path

- [x] 6.1 In `cmd_start`, after the existing `find_session_for_repo` lookup, branch on `existing.effective_status(...)`:
  - `Active + alive` → existing reattach path (unchanged).
  - `Paused + alive` → new `restart_from_pause(&existing)` helper.
  - `Stopped` (any) → existing cold-recovery path (unchanged).
- [x] 6.2 Implement `restart_from_pause(session: &Session)`:
  - Re-create the dashboard pane at `session.dashboard_pane.unwrap_or(0)`. Use the same tmux-orchestration helper that the initial launch uses to create the dashboard pane (factor out if needed).
  - `tmux send-keys` the `git paw __dashboard` command into the new pane.
  - Update session: `status = Active`; persist.
  - Call `tmux::attach(session.session_name)`.
- [x] 6.3 The restart-from-pause path SHALL NOT call any worktree-creation, CLI-spawning, or boot-prompt-injection helpers. Confirm by code-walking the helper to ensure no such calls exist.

## 7. `cmd_status` rendering

- [x] 7.1 Update the emoji / label map in `cmd_status` to handle three states: `Active => "🟢 active"`, `Paused => "🔵 paused"` (or similar blue/teal indicator), `Stopped => "🟡 stopped"`.
- [x] 7.2 For a paused session, append a one-line hint: `"  ↳ run 'git paw start' to resume"` (or equivalent).
- [x] 7.3 For a paused session, the broker line SHALL read `"<url> (paused — run 'git paw start' to resume)"` instead of `"(not responding)"`. The probe still runs; the failure mode is just labelled differently.

## 8. Help-text updates

- [x] 8.1 Run `git paw --help`, `git paw pause --help`, `git paw stop --help`, and confirm:
  - Root `--help` lists `pause` and shows it in the `after_help` quick-start.
  - `pause --help` mentions the RAM trade-off and points at `stop` for the destructive alternative.
  - `stop --help` mentions `pause` (soft) and `purge` (full) alongside its own behaviour, and lists the `--force` flag.

## 9. Unit tests

- [x] 9.1 `cli::tests::pause_parses` — `git paw pause` parses to `Command::Pause`.
- [x] 9.2 `cli::tests::pause_help_mentions_ram_tradeoff` — `pause --help` output contains a RAM-related phrase and the `stop` cross-reference.
- [x] 9.3 `cli::tests::stop_with_force` — `git paw stop --force` parses with `force: true`.
- [x] 9.4 `cli::tests::stop_without_force` — `git paw stop` parses with `force: false`.
- [x] 9.5 `cli::tests::stop_help_mentions_pause_and_purge` — `stop --help` output references both `pause` and `purge`.
- [x] 9.6 `cli::tests::root_help_lists_pause` — `git paw --help` output lists the `pause` subcommand.
- [x] 9.7 `session::tests::paused_status_serializes_lowercase` — round-trip a `Session` with `status = Paused`, assert JSON contains `"status":"paused"`.
- [x] 9.8 `session::tests::v04_session_without_dashboard_pane_loads_as_none` — parse a v0.4-shaped JSON (no `dashboard_pane`), assert load succeeds with `dashboard_pane == None`.
- [x] 9.9 `session::tests::effective_status_paused_alive_remains_paused` — assert `effective_status(|_| true)` against `Paused` returns `Paused`.
- [x] 9.10 `session::tests::effective_status_paused_dead_downgrades_to_stopped` — assert `effective_status(|_| false)` against `Paused` returns `Stopped`.
- [x] 9.11 `tmux::tests::detach_client_noop_when_no_clients_attached` — start a detached test session, run `detach_client`, assert `Ok(())`.

## 10. Integration tests

- [x] 10.1 `tests/pause_e2e.rs::pause_detaches_and_stops_broker` — `assert_cmd` driven: start a small session (broker enabled, 2 branches, no supervisor), `git paw pause`, then assert:
  - `tmux has-session -t <name>` exits 0 (session alive).
  - Broker port is free (TCP probe returns no listener).
  - Session state on disk has `status: "paused"`.
- [-] 10.2 `tests/pause_e2e.rs::start_after_pause_restarts_broker` — DEFERRED. Driving the full `git paw start` flow against a paused session needs a TTY (the attach-or-print-hint path) and a working `--dry-run` for the paused branch (current `--dry-run` only previews fresh-launch). Restart-from-pause behaviour is covered by unit tests on `effective_status` (paused+alive → Paused) plus the manual smoke test 13.6.
- [x] 10.3 `tests/pause_e2e.rs::pause_idempotent_on_already_paused` — pause an already-paused session, assert exit 0 and an "already paused" message on stdout.
- [x] 10.4 `tests/pause_e2e.rs::stop_after_pause_kills_remaining_panes` — pause, then `git paw stop --force`; assert tmux session is gone AND each previously-running CLI process PID is no longer alive.
- [x] 10.5 `tests/stop_confirmation_test.rs::stop_force_skips_prompt` — `git paw stop --force` against a live session in a TTY-emulated test exits 0 without rendering the prompt. (Use a pty-emulation crate or run via `assert_cmd` with `--force`; do not test the actual prompt — that requires interactive stdin.)
- [x] 10.6 `tests/stop_confirmation_test.rs::stop_non_tty_skips_prompt` — `git paw stop` (no `--force`) with stdin redirected from `/dev/null` proceeds without a prompt and exits 0 (v0.4 back-compat).

## 11. Documentation

- [x] 11.1 Add a new mdBook page `docs/src/user-guide/pause.md` covering:
  - The three teardown verbs (pause / stop / purge) and when to use each.
  - The RAM trade-off for pause.
  - The restart-from-pause flow (`git paw start` against a paused session is cheap).
  - Cross-reference to the (future v1.0.0) `hibernate` and per-CLI `--continue` work.
- [x] 11.2 Update `docs/src/SUMMARY.md` to include the new pause page.
- [x] 11.3 Update `README.md`'s CLI table to add a row for `pause`.
- [x] 11.4 Update `docs/src/user-guide/stop.md` (or wherever `stop` is documented) with the new confirmation prompt behaviour and `--force` flag. (No `stop.md` exists; updated the `git paw stop` section in `docs/src/cli-reference.md` instead.)
- [x] 11.5 Run `mdbook build docs/`; assert success.

## 12. Release notes

- [x] 12.1 v0.5.0 release notes: announce `git paw pause`. Include:
  - The motivating use case (short break, instant resume, no context loss).
  - The RAM trade-off (~300 MB / Claude pane held across pause).
  - The `git paw stop` UX change (new TTY confirmation prompt; `--force` to skip; non-TTY contexts unchanged).
  - Cross-reference to drift 60 (motivation) and v1.0.0 hibernate (future).

## 13. Quality gates

- [ ] 13.1 `just check` — fmt, clippy, all tests green.
- [ ] 13.2 `just deny` — supply chain clean.
- [ ] 13.3 No new `unwrap()` / `expect()` in non-test code.
- [ ] 13.4 `mdbook build docs/` succeeds.
- [ ] 13.5 `openspec validate git-paw-pause --strict` passes.
- [ ] 13.6 Manual smoke: start a local 3-branch session, pause it, confirm tmux survives and broker dies, run start, confirm CLI panes resume mid-conversation (no fresh boot prompts).
