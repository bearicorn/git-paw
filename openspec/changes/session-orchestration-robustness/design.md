# Design — session-orchestration-robustness

## Context

The v0.8.0 dogfood (2026-06-24) ran a 6-agent supervisor session and hit
three orchestration-robustness defects that, together, made the session
unusable and left the persisted state corrupt. All three are in the
launch/add/remove/layout machinery in `src/tmux.rs`, the `add`/`remove`
command paths in `src/main.rs`, and the session bookkeeping in
`src/session.rs`:

- **G1 — CLI launch race.** The start flow waits a fixed `~2s`
  (`std::thread::sleep(Duration::from_secs(2))` in `cmd_start`,
  src/main.rs ~1358) and then injects the boot block via
  `submit_prompt_to_pane`. One of six panes never started its CLI, so the
  pane was still a bare shell when the timer fired. The multi-line boot
  block (prompt, paste-handling notes, `/opsx:apply …`) was typed into the
  *shell* line-by-line and errored. Intermittent — the other five panes
  launched within the 2s window.

- **G2 — add/remove pane integrity + JSON↔tmux desync.** Recovery via
  `git paw remove` + `git paw add` failed two ways. (a) `cmd_remove`
  (src/main.rs ~2536) computes `pane_idx = agent_pane_offset + pos` from
  the agent's *position in the session JSON* and calls
  `tmux::kill_pane(session, idx)`. With an orphan pane present (the failed
  G1 shell pane) the live grid had diverged from the JSON, so the
  index-derived target was wrong — it did not kill the failed pane and the
  subsequent re-tile dropped a *different* agent's pane. (b) That agent
  then existed in the session JSON and broker roster with no live tmux
  pane, and there was no command to detect or repair the desync.

- **G3 — unequal agent-row widths.** Agents are spliced into a row by
  successive `tmux split-window -h` (src/tmux.rs ~1052 in
  `build_supervisor_session`, and ~1146 in `build_add_agent_commands`).
  Each `-h` split halves the *current* pane, so a 3-agent row renders
  **50/25/25**, not equal thirds. `select-layout` is intentionally avoided
  for the whole window (it would scramble the pane-index ordering the rest
  of the system relies on), and no per-row width rebalance is done. The
  `layout_for(N)` vertical math (`src/supervisor/layout.rs`) is correct;
  the bug is purely in how the horizontal splits are *applied*. Reproduced
  cleanly with 3 agents and no orphan pane involved.

### Why current tests missed it

`layout_for(N)` is unit-tested (`src/supervisor/layout.rs::tests`), but
the tests assert the computed *plan* — row count and vertical height
percentages via `assert_layout(...)` — in isolation. They never apply the
layout to a live tmux window and measure actual pane geometry, and TUI
draw loops / terminal I/O are coverage-exempt ("tested manually"). No test
drives a real session through a failed launch, an add, or a remove and
asserts pane integrity + geometry. So this change MUST add the regression
tests that would have caught it: real-session pane-integrity assertions
and `layout_for`/applied-layout horizontal equal-width + minimum-width
assertions at higher N.

## Goals

- G1: gate boot-block injection on observed CLI readiness; relaunch the
  CLI if the pane is still a bare shell after a bounded timeout.
- G2a: `remove` kills the target pane by resolved pane id (via
  `pane_current_path` mapping) regardless of the running process.
- G2b: `add`/`remove` re-tile preserves every other agent's pane; add a
  JSON↔tmux reconciliation that detects/surfaces an agent with no live
  pane.
- G3: rebalance each agent row to equal width after splits, without
  disturbing the top-row supervisor/dashboard 50/50 or the per-row
  vertical heights; keep a sane minimum pane width at high N.
- Add regression tests that would have caught G2 and G3: real-session
  pane-integrity tests and applied-layout width assertions, reusing the
  E2E-isolation harness conventions already in `tests/`.

## Non-Goals

- No change to `layout_for(N)`'s vertical math or the agent-row count
  buckets — the height table is correct and stays as-is.
- No configurable `max_agents` (still 25, deferred to v1.0.0) and no
  change to `SUPERVISOR_AGENTS_PER_ROW` (stays 5).
- No config or wire-format changes; this is pure robustness. Existing
  sessions load and behave identically except they no longer
  lose/orphan panes.
- No automatic *repair* of a detected desync beyond surfacing it on the
  `add` path — a full self-healing reconcile command is out of scope (the
  reconciliation reports the divergence; manual recovery stays via
  `remove`/`add`).
- No move to `select-layout tiled` for the whole window (it scrambles
  pane indices the rest of the system depends on).

## Decisions

### D1 — Launch-readiness gate (G1)

Replace the blind fixed sleep before boot-block injection with a
readiness poll. After a pane's CLI command is sent, poll the pane via
`tmux capture-pane -p` (the same mechanism `src/supervisor/poll.rs`'s
`PaneInspector` already uses) on a short interval, looking for a
CLI-readiness marker that distinguishes the CLI's interactive UI from a
bare shell prompt, up to a bounded timeout. Only inject the boot block
once the marker is observed.

If the timeout elapses with the pane still a bare shell, relaunch the CLI
command into the pane (clear the input line with `C-u` first, as the
existing launch path already does) and poll again, up to a small relaunch
budget. The gate is conservative: a CLI whose UI matches no known marker
falls back to injecting after the budget — never worse than today's
fixed-sleep behaviour for an unrecognised CLI.

The gate is shared by the start path (`cmd_start`) and the add path
(`cmd_add` / `build_add_agent_commands` caller) so an added agent gets the
same protection.

Rationale: the failure was a *race*, not a too-short sleep — bumping the
sleep would only paper over it. Observing readiness is the only robust
fix, and `capture-pane` is already a proven primitive in this codebase.

### D2 — Remove kills by resolved pane id, not JSON-position index (G2a)

`cmd_remove` SHALL resolve the removed branch's pane by mapping its
worktree path to a live pane via `pane_current_path` (`tmux list-panes
-F '#{pane_id} #{pane_current_path}'`), then kill that pane by its
`#{pane_id}`. This decouples the kill from the JSON-position arithmetic
that broke under an orphan pane, and works whether the pane runs a CLI or
a bare shell. If no live pane maps to the worktree (already gone), the
kill is a no-op and removal proceeds to JSON deregistration (idempotent,
matching `kill_pane`'s existing missing-pane tolerance).

### D3 — Re-tile preserves all other panes + reconciliation (G2b)

After splicing (add) or killing (remove) the single target pane and
re-applying the layout, the live window SHALL contain exactly one pane
per remaining session-JSON agent (plus supervisor + dashboard). A
reconciliation helper compares session-JSON agents against the live
`pane_current_path` set and reports any JSON agent with no live pane. On
the `add` path this runs post-re-tile and surfaces a desync so it is
visible/recoverable instead of silent. Surfacing (not auto-repair) is the
chosen scope; auto-repair is a non-goal.

### D4 — Per-row equal-width rebalance (G3)

After a row's agent panes are created by `split-window -h`, rebalance
that row to equal width. Two viable mechanisms, both acceptable per the
spec: (a) `select-layout even-horizontal` *scoped to the row's agent
panes* (must not reorder pane indices or touch the top row), or (b)
`resize-pane` each agent pane in the row to `100 / agents_in_row` percent
of the row width. The rebalance MUST NOT touch the top-row
supervisor/dashboard 50/50 split nor the per-row vertical heights from the
`layout_for` table (those stay enforced by the existing
`push_supervisor_resize_pass`). Applied uniformly by the start builder
(`build_supervisor_session`), the add builder
(`build_add_agent_commands`), and the remove re-tile
(`build_remove_retile_commands`), so any path yields equal-width rows.

Preference: a scoped `even-horizontal` per row is simplest and rounds
columns the way tmux already does for the user; `resize-pane` per pane is
the fallback if scoping `even-horizontal` to a row proves unreliable
across tmux versions. Either satisfies the equal-width spec scenarios.

### D5 — Minimum-width handling at high N

The minimum equal-width target is bounded by `SUPERVISOR_AGENTS_PER_ROW`
(5), giving a 20% floor per pane at the widest rows. No row exceeds 5
panes by construction (the grid wraps to a new row at the 6th agent), so
equal-width rebalancing never drives a pane below the 20% target. This is
asserted directly rather than introducing a new minimum-width knob.

### D6 — Regression tests (the tests that would have caught it)

- **Applied-layout width tests** (live tmux): build/apply a 3-agent
  supervisor layout, query real pane widths via `tmux list-panes -F
  '#{pane_width}'`, and assert the three agent panes are equal within a
  one-column tolerance (would have caught G3's 50/25/25). Also assert the
  top row stays ~50/50 and a 5-agent row is ~20% each.
- **Real-session pane-integrity tests** (live tmux): drive a session
  through `add` and `remove` and assert pane count == JSON-agent count
  (+supervisor+dashboard) and that the worktree→pane `pane_current_path`
  mapping is intact for every surviving agent (would have caught G2's
  dropped/orphaned pane). Include a remove-of-a-shell-occupied-pane case.
- **Reconciliation unit test**: a session JSON agent with no matching
  live pane is reported as divergent; a fully-mapped session reports none.
- Reuse the existing E2E-isolation conventions in `tests/`
  (`add_remove_e2e.rs`, `e2e_supervisor_launch.rs`): isolated tmux socket,
  serialized runs, real-repo tempdirs.

## Risks / Trade-offs

- **Readiness-marker brittleness.** A CLI whose interactive UI does not
  match the marker heuristic could never be classified ready. Mitigation:
  the conservative fallback (D1) injects after the budget, so an
  unrecognised CLI is no worse off than today's fixed sleep.
- **Slower happy-path launch.** Polling adds a few hundred ms vs the flat
  2s. Mitigation: poll on a short interval and stop as soon as the marker
  appears — typically faster than the old flat 2s for CLIs that start
  quickly.
- **`even-horizontal` scoping across tmux versions.** Scoping a layout to
  a row may behave differently on macOS tmux 3.6a vs Linux apt-tmux 3.4.
  Mitigation: the spec permits the `resize-pane`-per-pane fallback; the
  applied-layout width tests run against the real tmux on the host and
  catch a version that misbehaves.
- **Reconciliation false positives during a transient re-tile.** A pane
  may momentarily be missing mid-re-tile. Mitigation: reconciliation runs
  only after the re-tile settles, on the `add` path, against the final
  pane set.
- **Relaunch loop.** A genuinely broken CLI could be relaunched up to the
  budget. Mitigation: a small bounded attempt count, then fall back to
  injection and let the user/supervisor recover via `remove`/`add`.
