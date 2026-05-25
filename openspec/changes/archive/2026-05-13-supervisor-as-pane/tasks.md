## 1. Layout calculation helpers

- [x] 1.1 Add a constant `SUPERVISOR_MAX_AGENTS: usize = 25` near the top of `src/main.rs` (or in a dedicated module like `src/supervisor/layout.rs`).
- [x] 1.2 Add a constant `SUPERVISOR_AGENTS_PER_ROW: usize = 5`.
- [x] 1.3 Add a constant `SUPERVISOR_PANE_OFFSET: usize = 2` (supervisor at 0, dashboard at 1, agents start at 2). Use this everywhere a pane offset is needed in supervisor mode.
- [x] 1.4 Implement pure helper `pub fn supervisor_layout(agent_count: usize) -> Result<SupervisorLayout, PawError>` returning a struct with: `agent_rows: usize`, `total_rows: usize`, `top_row_pct: u8`, `agent_row_pct: u8`. The function returns an error when `agent_count > SUPERVISOR_MAX_AGENTS` with the actionable "split into multiple sessions" message.
- [x] 1.5 Pure-function tests for `supervisor_layout`: 1, 5, 6, 10, 11, 15, 16, 20, 21, 25, 26 agents. Verify the proportions match the table (60/40, 40/30, 28/24, 28/18, 28/14.4) and 26 returns an error.

## 2. tmux layout builder

- [x] 2.1 In `src/tmux.rs` (or `src/supervisor/tmux_layout.rs`), implement `pub fn build_supervisor_session(...) -> TmuxSession` that constructs the layout per design D2:
  - new-session -d (detached)
  - split-window -h on pane 0 to create pane 1 (dashboard) at 50% of top row
  - split-window -v from pane 0 to create the agent-row sub-region with the top-row height percentage
  - For each agent row: split-window -v as needed; split-window -h as needed up to `agents_per_row`
  - Final pass: `tmux resize-pane -y <pct>` to enforce exact heights
- [x] 2.2 The builder SHALL produce panes in row-major order (left-to-right, top-to-bottom for agents).
- [x] 2.3 Unit-test the builder via the existing `tmux::TmuxSessionBuilder` test pattern (record the emitted commands; verify `split-window` count and `resize-pane` count match expected for given agent count).
- [x] 2.4 Verify pane indices for fixture agent counts: 5 agents → indices 0..6; 10 agents → indices 0..11; 20 agents → indices 0..21.

## 3. cmd_supervisor restructure

- [x] 3.1 Remove the foreground supervisor-CLI launch block in `src/main.rs::cmd_supervisor` (the `Command::new(supervisor_cli).status()` call at ~line 870 + its return-status handling).
- [x] 3.2 Remove the `run_merge_loop` call and surrounding result-handling code from `cmd_supervisor`.
- [x] 3.3 Add the hard-cap check (using `SUPERVISOR_MAX_AGENTS`) immediately after branch resolution; on overflow, return a `PawError::ConfigError` with the actionable message including requested count, max, and `--branches <subset>` hint.
- [x] 3.4 Restructure the tmux session-build step to use the new layout builder from task 2.1. Pane 0 spec = supervisor (Claude in repo_root with rendered supervisor.md as AGENTS.md, with approval flags); pane 1 spec = dashboard; panes 2..N+1 = coding agents.
- [x] 3.5 Update the boot-prompt injection loop to ALSO inject for pane 0 (supervisor pane). The supervisor pane's prompt is the boot block (with `BRANCH_ID = supervisor`) followed by a "Begin observing" framing message.
- [x] 3.6 Keep the supervisor self-registration HTTP POST (`agent.status` from `agent_id = "supervisor"`); it now happens before `cmd_supervisor` returns rather than before the foreground CLI starts.
- [x] 3.7 At the end of `cmd_supervisor`, print the launch-success message including `tmux attach -t paw-<project>` hint and return `Ok(())`. No blocking on any process.
- [x] 3.8 Delete the `run_merge_loop` function definition, `MergeResult`, `MergeResults`, and any private helpers that only the merge loop used. Run a grep audit for orphaned imports / dead code.
- [x] 3.9 Update the existing `attach_or_print_hint(&tmux_session.name)` call from `from-specs-launch-fixes` — `cmd_supervisor` no longer needs it (the change always returns without attaching). Remove that call from cmd_supervisor; keep it in cmd_start and cmd_start_from_specs.

## 4. Auto-approve thread relocation

- [x] 4.1 Move `spawn_auto_approve_thread` invocation from `cmd_supervisor` to the dashboard's `__dashboard` subcommand entry point (`src/main.rs::cmd_dashboard` or wherever the `__dashboard` subcommand routes to).
- [x] 4.2 Pass the necessary state (broker URL, supervisor config's auto_approve section, pane_map) to the dashboard subprocess. Use environment variables or a session-state file (the session is already saved before `__dashboard` starts; `__dashboard` can read it).
- [x] 4.3 Update the pane_map computation: agents start at index 2 (`SUPERVISOR_PANE_OFFSET`) in supervisor mode, not 1. The map must reflect this.
- [x] 4.4 Bare-mode (no supervisor) keeps the existing pane_offset logic (dashboard at 0, agents at 1) — only supervisor mode shifts.
- [x] 4.5 Test: in supervisor mode, the auto-approve thread is alive in the `__dashboard` process (verifiable via `ps`, or by killing the dashboard pane and observing that subsequent permission prompts no longer auto-fire).
- [x] 4.6 Test: in non-supervisor broker mode, the auto-approve thread continues to NOT spawn from cmd_supervisor (it never did). Existing behaviour unchanged.

## 5. Supervisor self-registration timing

- [x] 5.1 Verify the existing `publish_to_broker_http(broker_url, build_status_message("supervisor", "working", Some("Supervisor booting")))` call in `cmd_supervisor` is preserved.
- [x] 5.2 Move it (if necessary) to a position AFTER `tmux_session.execute()` but BEFORE the function returns. The order: build session → execute → save state → sleep 2s → send-keys for all panes → publish supervisor self-register → print attach hint → return.
- [x] 5.3 Test: after `cmd_supervisor` returns, the broker's `/status` endpoint shows `agent_id = "supervisor"` registered with status `"working"` and message `"Supervisor booting"`.

## 6. Recovery flow

- [x] 6.1 Update `recover_session` in `src/main.rs` to detect supervisor mode (via the saved `Session`'s mode marker, or by checking that the session has a supervisor config in the live config file).
- [x] 6.2 In supervisor mode recovery: rebuild the session using the new layout (per task 2.1). Don't preserve the v0.4 single-row layout.
- [x] 6.3 In non-supervisor recovery: existing recovery logic unchanged.
- [x] 6.4 If a v0.4-saved session is encountered (no supervisor mode marker, but supervisor config present in current `.git-paw/config.toml`), warn that the session was created with v0.4 layout and is being restarted with v0.5 layout. Do NOT block the recovery.
- [x] 6.5 Test: stop a supervisor session via `git paw stop`, then re-launch via `git paw start --supervisor`. Verify the recovered session has the new layout (supervisor at pane 0, dashboard at pane 1, agents at 2+).

## 7. Embedded supervisor.md skill update

- [x] 7.1 In `assets/agent-skills/supervisor.md`, add a new section `### When the user types in your pane` covering the three cases (status question, directive, judgment-call ask) per the `agent-skills` spec delta. Include `curl` examples for the directive case (publishing `agent.feedback`) and the escalation case (publishing `agent.question`).
- [x] 7.2 Add a new section `### Merge orchestration` covering the topological-order computation, per-branch merge + test loop, regression handling, cycle handling, and final-summary publish per the `agent-skills` spec delta. Include explicit shell snippets for `git checkout main`, `git merge --ff-only feat/<branch>`, the `<test_command>` reference, and the `git reset --hard` revert path.
- [x] 7.3 Verify both new sections are included in `skills::resolve("supervisor")` output (the rendered template).
- [x] 7.4 Mirror skill updates into `docs/src/user-guide/supervisor.md` (or the user-guide chapter for supervisor mode) so docs reflect the new sections.

## 8. Skill-content tests

- [x] 8.1 Test: supervisor skill contains the substring `When the user types in your pane` (or equivalent heading).
- [x] 8.2 Test: supervisor skill mentions `agent.feedback` in the directive context.
- [x] 8.3 Test: supervisor skill mentions `agent.question` in the judgment-call context.
- [x] 8.4 Test: supervisor skill states the autonomous loop continues alongside user input.
- [x] 8.5 Test: supervisor skill contains the substring `Merge orchestration` (or equivalent heading).
- [x] 8.6 Test: supervisor skill mentions `git merge --ff-only`.
- [x] 8.7 Test: supervisor skill mentions `git reset --hard` for regression revert.
- [x] 8.8 Test: supervisor skill mentions `agent.question` for cycle handling in merge orchestration.
- [x] 8.9 Test: supervisor skill instructs publishing a final `agent.status` summary after merge orchestration.

## 9. Layout / pane-index tests

- [x] 9.1 Test: 5-agent supervisor session — pane 0 = supervisor, pane 1 = dashboard, panes 2-6 = agents in single row, top row 60% / agent row 40%.
- [x] 9.2 Test: 10-agent — agents in 2 rows of 5, top row 40% / each agent row 30%.
- [x] 9.3 Test: 11-agent — 3 agent rows (5 + 5 + 1), top row 28% / each agent row 24%.
- [x] 9.4 Test: 20-agent — 4 agent rows of 5, top row 28% / each agent row 18%.
- [x] 9.5 Test: 25-agent — 5 agent rows of 5, top row 28% / each agent row 14.4%.
- [x] 9.6 Test: 26-agent attempted launch returns `PawError::ConfigError` with the cap message; no tmux session created.
- [x] 9.7 Test: pane indices for 7-agent session — pane 2 is the first agent (top-left of grid), pane 6 is the fifth agent (top-right of first agent row), pane 7 is the sixth agent (start of second agent row).

## 10. cmd_supervisor integration tests

- [x] 10.1 Integration test: dry-run with supervisor config + 3 spec branches → output contains `Supervisor:`, `Agent CLI:`, `Approval:`, and lists 3 agent branches; does NOT contain "session plan (from specs):" header (which would be the from-specs-only output).
- [x] 10.2 Integration test: real launch (against a real tmux server in CI) of 3-agent supervisor session — verify pane count = 5 (supervisor, dashboard, 3 agents), supervisor pane has Claude or echo CLI command, dashboard pane runs `git-paw __dashboard`, exit code 0, attach-hint printed.
- [x] 10.3 Integration test: cmd_supervisor returns within ~5 seconds (no foreground-CLI block); compare to v0.4 behaviour where it blocked indefinitely.
- [x] 10.4 Integration test: after cmd_supervisor returns, the broker's `/status` endpoint shows the `supervisor` agent registered.
- [x] 10.5 Integration test: from a non-TTY context (`assert_cmd::output()`), supervisor launch succeeds without erroring on "open terminal failed" (was the symptom in dogfood D2).

## 11. Hard-cap test

- [x] 11.1 Integration test: configure 26 spec branches; run `git paw start --from-specs --supervisor`; verify exit code is non-zero, stderr contains "26 agents requested", "maximum is 25", and "split into multiple sessions"; no tmux session is created.

## 12. Stop / Recovery tests

- [x] 12.1 Test: `git paw stop` on a supervisor session — broker port freed, broker.log final-flushed, dashboard pane killed (which terminates auto-approve thread).
- [x] 12.2 Test: `git paw start --supervisor` after a stop — recovery rebuilds with new layout (supervisor at pane 0, dashboard at pane 1).
- [x] 12.3 Test: v0.4-style saved session (manually constructed without supervisor-mode marker) → recovery prints the v0.4-to-v0.5 layout warning and rebuilds with v0.5 layout.

## 13. Documentation

- [x] 13.1 Update `docs/src/user-guide/supervisor.md` (or wherever supervisor mode is documented) with the new model: supervisor is a pane in tmux; user runs `tmux attach -t paw-<project>` to interact; merge orchestration is now skill-driven.
- [x] 13.2 Update the layout description with the proportions table and pane-index map.
- [x] 13.3 Document the 25-agent hard cap and the "split into multiple sessions" workaround.
- [x] 13.4 Document the auto-approve relocation (now in dashboard subprocess; dies when dashboard pane is killed).
- [x] 13.5 Document the merge-orchestration regression: v0.4's auto-merge-after-supervisor-CLI-exit is gone in this change; the supervisor agent now performs merges per its skill (or the user merges manually).
- [x] 13.6 Add or update the user-guide section "When the user types in your pane" if a separate page exists, OR cross-reference the embedded skill section.
- [x] 13.7 `mdbook build docs/` succeeds.

## 14. Release notes

- [x] 14.1 Loud release-notes call-out: supervisor mode UX changes from "your terminal is the supervisor" to "the supervisor is a pane in tmux." Existing scripts that relied on `cmd_supervisor` blocking on the supervisor CLI's exit need to update.
- [x] 14.2 Release-notes call-out: auto-merge after supervisor-CLI exit is removed in v0.5.0; the supervisor agent itself merges via skill instructions (or merge manually).
- [x] 14.3 Release-notes call-out: 25-agent hard cap; configurable layout (`max_agents` + `agents_per_row`) lands in v1.0.0.
- [x] 14.4 Release-notes call-out: non-TTY launches now work end-to-end for supervisor mode (the launch is always detached).

## 15. Quality gates

- [x] 15.1 `just check` — fmt, clippy, all tests green.
- [x] 15.2 `just deny` — supply chain clean.
- [x] 15.3 No new `unwrap()` / `expect()` in non-test code.
- [x] 15.4 `mdbook build docs/` succeeds.
- [x] 15.5 `openspec validate supervisor-as-pane` passes.
- [x] 15.6 Manual smoke test: from a real interactive terminal, run `git paw start --from-specs --supervisor`. Verify the tmux session opens (or the attach hint prints if launching from non-TTY); attach with `tmux attach -t paw-<project>`. Confirm: pane 0 is supervisor (Claude prompt visible), pane 1 is dashboard, panes 2+ are agents. Type a status question into the supervisor pane; the supervisor responds conversationally using its skill's "When the user types in your pane" guidance.
- [x] 15.7 Manual smoke test: confirm the `cmd_supervisor` flow returns control to the user's terminal immediately (within ~5 seconds), printing the attach hint, with no foreground-CLI block.
- [x] 15.8 Grep audit: no remaining references to `run_merge_loop`, `MergeResult`, or `MergeResults` in the codebase outside of CHANGELOG / archived spec content.
- [x] 15.9 Grep audit: every `pane_offset = 1` (or hardcoded `1`) reference that was for the dashboard-at-pane-0 model has been audited; supervisor mode uses `SUPERVISOR_PANE_OFFSET = 2`.
