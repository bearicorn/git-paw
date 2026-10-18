## 1. Grid geometry for every session

- [ ] 1.1 Grid geometry takes "has top row" as an input; no-dashboard variant (agents from 0, rows share 100%); `build_add_agent_commands`, `build_remove_retile_commands`, `rebalance_agent_rows` take the first-agent index from `SessionLayout`; unit tests for 1/5/6/10/25 agents with and without a dashboard
- [ ] 1.2 `TmuxSessionBuilder` bare build (`cmd_start`, `launch_spec_session`) uses the grid builder; no `select-layout tiled` / `main-horizontal` for agents; tests for the "Every pane-less session uses the agent grid" scenarios (bare, spec-driven, broker-less)
- [ ] 1.3 Recovery (`commands/recover.rs`) rebuilds bare sessions as grids and re-saves the layout; test "Recovery rebuilds a grid"

## 2. Layout record

- [ ] 2.1 Add the optional `Session` layout field (`serde(default, skip_serializing_if)`), written by every start / recovery / restart; `SessionLayout` reads it (absent + bare = tiled, absent + supervised = grid); round-trip and earlier-file load tests

## 3. Hot-attach on every grid session (GP-49)

- [ ] 3.1 Replace `commands/helpers.rs::bare_mode_unsupported` with a legacy-tiled refusal in `cmd_add` / `cmd_remove`, checked before any mutation; message names the tiled layout, tells the user to stop and start again, cites no version; replace the `bare_mode_unsupported` unit tests (`commands/helpers.rs`) that pinned the REMOVED *Bare-session add and remove refusal states the limitation plainly*
- [ ] 3.2 `tests/add_remove_e2e.rs`: add to / remove from a bare grid session; add to a broker-less grid session; add to a pre-relocation supervised session (agents from 2); legacy tiled refusal leaves worktrees, panes and session JSON untouched
- [ ] 3.3 Re-tiling requirement tests re-indexed (agents from 1; dashboard only when present)

## 4. Passive discovery and the live roster

- [ ] 4.1 `drive_loop` re-reads the coding-agent roster from the session state each sweep (keep the previous roster on a read failure); completion and summary use the last-read roster
- [ ] 4.2 Tests: a live prompt on an agent added mid-run is escalated by the running loop (a `permission-prompt` naming that agent, no answering keystroke); a removed agent gets no further keystrokes and is absent from the summary; an unreadable roster keeps the previous one; add/remove send no keystroke to any other surface; the discovery file lists/omits the agent

## 5. Readiness gate on the last ungated path (DS-5)

- [ ] 5.1 Gate `restart_from_pause`'s held-prompt submit (`commands/start.rs`, around `:973`) through `gate_pane_or_fail_on_dialog` per pending pane; a dialog-stuck pane keeps its held prompt and is reported on stderr while the rest of the session restarts (bare `start` at `:406` and `launch_spec_session` at `:823` are already gated by v0.17.0)
- [ ] 5.2 Tests: restart keeps the held prompt for a dialog-stuck pane and names it on stderr (scenario "Restart from pause gates a held prompt", in `tests/pause_e2e.rs`); map "Bare start is gated" and "Spec-driven start stuck on a first-run dialog fails loudly" to the existing `bare_launch_fails_loudly_on_stuck_dialog` / `spec_driven_launch_fails_loudly_on_stuck_dialog` (`tests/start_spec_launch_parity_integration.rs`), adding a bare-shell pre-fill test only if none pins it; unrecognised CLI still falls back to fixed-budget injection

## 6. Docs and gates

- [ ] 6.1 `docs/src/user-guide/session-lifecycle.md` (add/remove on every session; the tiled-layout refusal and its fix), `docs/src/architecture.md` (one layout), `user-guide/dashboard.md` (grid below the dashboard), `cli-reference.md` + `add`/`remove` `--help` (no "supervisor-mode only"), README CLI table; `mdbook build docs/` succeeds
- [ ] 6.2 Every scenario maps to a named test; `just check` + `just deny` green; full suite green vs merge-base (`cargo test --no-fail-fast`)
