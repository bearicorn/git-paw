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
- **Equal-width agent panes within a row (G3) — ROOT CAUSE CONFIRMED.** Agents are added to a row by successive `split-window -h` (each split halves the *current* pane), and `select-layout` is intentionally avoided — so a 3-agent row renders **50/25/25**, not equal thirds (reproduced cleanly with 3 agents, no orphan pane involved). Fix: after creating a row's panes, rebalance them to equal width — apply `select-layout even-horizontal` scoped to the agent panes, or `resize-pane` each to `100/agents_in_row`%, WITHOUT disturbing the explicit top-row (supervisor/dashboard 50/50) proportions. Also assert a sane minimum pane width at higher N. The `layout_for(N)` vertical math is already correct; the bug is purely in how the horizontal splits are *applied* in `tmux.rs`.

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
