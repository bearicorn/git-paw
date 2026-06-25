## Why

The v0.8.0 dogfood (2026-06-24) exposed three orchestration-robustness gaps that made a 6-agent session unusable and left the session state corrupt:

- **G1 — CLI launch race.** One of six agent panes never started its CLI; the pane stayed a bare shell, so the entire injected boot block (prompt, paste-handling notes, `/opsx:apply …`) was typed *into the shell* line-by-line and errored. Intermittent (the other five launched fine).
- **G2 — add/remove pane integrity.** Recovering via `git paw remove` + `git paw add` (a) did NOT kill the failed agent's pane (it was a shell, not the expected CLI process), leaving an orphan pane that skewed the grid, and (b) the re-tile **dropped a *different* agent's pane** — that agent ended with no tmux pane while still present in the session JSON + broker roster (JSON↔tmux desync, no clean recovery command).
- **G3 — grid layout at higher N.** With 6 agents the `layout_for(N)` grid produced panes too thin to use, with unequal row widths.

## Why the current tests missed this

`layout_for(N)` IS unit-tested, but the tests assert the computed *plan* — row count + vertical height percentages (`assert_layout(6, 2, 40, 30.0)`) — in isolation. They never apply the layout to a live tmux window and measure actual pane geometry, and TUI draw loops / terminal I/O are explicitly coverage-exempt ("tested manually"). The thin/unequal panes were not a `layout_for` bug at all — they were G2 (an orphan pane + a dropped pane making the live window diverge from the plan). No test drives a real session through a failed launch or add/remove and asserts pane integrity + geometry. So this change must **add the regression tests that would have caught it** (see the F8 `git paw selftest`/E2E-isolation harness): real-session pane-integrity assertions, plus `layout_for` horizontal equal-width / minimum-usable-width assertions at high N.

## What Changes

- **Launch-readiness check (G1).** Before injecting the boot block into a pane, verify the CLI process actually started (poll for the CLI's prompt / a readiness marker, with a short timeout); if the pane is still a bare shell, retry/relaunch instead of dumping the boot block into the shell.
- **Add/remove pane integrity (G2).** `git paw remove` SHALL kill its pane by pane-id regardless of the running process (shell or CLI). `git paw add`/`remove` re-tiling SHALL preserve every *other* agent's pane (no agent loses its pane to a re-tile). Add a reconciliation that detects session-JSON agents with no live pane and surfaces/repairs the desync.
- **Layout application is broken in BOTH dimensions (G3) — needs a rework, not a patch.** The grid is built with naive successive `split-window` and `select-layout` is *intentionally avoided*, so `layout_for(N)`'s computed proportions are **never actually applied**. Two confirmed symptoms, live: (a) **widths** — a row of 3 agents renders **50/25/25** (each `split-window -h` halves the current pane), not equal thirds; (b) **heights on add** — `git paw add` (3→4 agents) split a new bottom row but did NOT redistribute row heights, leaving the new panes at **height = 1** (just the title bar, unusable) while the existing rows kept their height. So the re-tile changes pane *count* without re-applying *geometry*. Proof the target is achievable: `tmux select-layout tiled` produces a perfectly even grid (all ~145×23) on the same window. **Fix = rework layout application:** compute the full target geometry (rows × cols with their height %/width %) from `layout_for(N)` and apply it deterministically — rebalancing BOTH row heights AND column widths — on the initial build AND on every `add`/`remove` re-tile (drop the "avoid select-layout / successive-halving" approach; either drive `select-layout`/`resize-pane` to the computed geometry, or special-case the supervisor/dashboard top row and tile the rest). Assert a minimum usable pane size at higher N. The `layout_for(N)` math is correct; the entire bug is that nothing applies it.

## Capabilities

### New Capabilities
<!-- None. -->

### Modified Capabilities
- `tmux-orchestration`: launch-readiness gate before boot-block injection; `layout_for(N)` equal-width rows + minimum pane width at higher N.
- `add-branch`: `git paw add` re-tile preserves all existing agents' panes.
- `remove-branch`: `git paw remove` kills its pane by pane-id regardless of the running process; no orphan panes; no collateral pane loss.

## Impact

- Affected code: `src/tmux.rs` (launch sequencing + readiness check, `layout_for`), `src/session.rs` (add/remove pane bookkeeping + JSON↔tmux reconciliation), the `add`/`remove` command paths.
- Tests: launch-readiness ret/relaunch behaviour; remove kills a shell-occupied pane; add/remove preserves other panes; `layout_for(N)` width assertions for N up to the 25 cap.
- Docs: tmux/session-management docs note the readiness check + layout behaviour at higher N.
- Backward compatible: pure robustness — no config/wire changes; existing sessions behave the same except they no longer lose/orphan panes.
