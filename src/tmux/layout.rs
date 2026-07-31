//! Supervisor-grid pane geometry and live row rebalancing (design D4, G3).
//!
//! Pure per-pane column-width computation plus the live `resize-pane` pass
//! that makes each agent row equal-width.

use std::process::Command;

use crate::error::PawError;

/// Compute the per-pane column-width resize targets for the equal-width row
/// rebalance (design D4, G3), given the live `window_width` and total
/// `agent_count`.
///
/// Agents are spliced into a row by successive `split-window -h`, and each
/// `-h` split halves the *current* pane — so a raw 3-agent row renders
/// 50/25/25, not equal thirds (the v0.8.0 G3 failure). For each agent row
/// (up to [`crate::supervisor::layout::SUPERVISOR_AGENTS_PER_ROW`] panes) this
/// resizes every pane but the last to `(window_width - separators) /
/// panes_in_row` columns — an equal share after accounting for the
/// one-column pane separators — and lets the last pane absorb the rounding
/// remainder, leaving the row equal-width within a one-column tolerance.
/// Returns `(pane_index, columns)` pairs for the panes to resize (the last
/// pane of each row is omitted).
///
/// Pure (no IO) so the row arithmetic is unit-testable; the live application
/// is [`rebalance_agent_rows`].
#[must_use]
pub fn agent_row_widths(window_width: usize, agent_count: usize) -> Vec<(usize, usize)> {
    use crate::supervisor::layout::{SUPERVISOR_AGENTS_PER_ROW, SUPERVISOR_PANE_OFFSET};

    let mut targets = Vec::new();
    if window_width == 0 {
        return targets;
    }
    let mut row_first_agent = 0;
    while row_first_agent < agent_count {
        let panes_in_row = (agent_count - row_first_agent).min(SUPERVISOR_AGENTS_PER_ROW);
        if panes_in_row > 1 {
            let separators = panes_in_row - 1;
            let per = window_width.saturating_sub(separators) / panes_in_row;
            for j in 0..(panes_in_row - 1) {
                targets.push((SUPERVISOR_PANE_OFFSET + row_first_agent + j, per));
            }
        }
        row_first_agent += panes_in_row;
    }
    targets
}

/// Query the live window width (columns) of `session_name`'s window 0.
///
/// Returns `Ok(None)` when the session/window is gone so callers degrade to a
/// no-op rather than failing.
fn query_window_width(session_name: &str) -> Result<Option<usize>, PawError> {
    let target = format!("{session_name}:0");
    let output = Command::new("tmux")
        .args(["display-message", "-p", "-t", &target, "#{window_width}"])
        .output()
        .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().parse().ok())
}

/// Rebalance every agent row to equal width on the live window (design D4, G3).
///
/// Queries the live window width, then resizes each row's panes (all but the
/// last, which absorbs the remainder) to an equal column share via
/// `tmux resize-pane -x <cols>`, so a row of agents renders equal-width within
/// a one-column tolerance instead of the 50/25/25 raw `-h` splits produce. A
/// no-op for 0 or 1 agents, or when the window is gone. Setting a pane's `-x`
/// width only moves its boundary with the horizontal neighbour in the same
/// row, so this never disturbs the top-row supervisor/dashboard 50/50 split
/// nor the per-row vertical heights.
///
/// Must run AFTER the splits (start/add) or the kill + height re-tile (remove)
/// settle so pane indices are contiguous (panes 2..N+1). No agent row exceeds
/// [`crate::supervisor::layout::SUPERVISOR_AGENTS_PER_ROW`] (5) by
/// construction, bounding the smallest equal-width target to ~20% per pane
/// (design D5). Best-effort per resize; one pane's tmux failure does not abort
/// the rest.
pub fn rebalance_agent_rows(session_name: &str, agent_count: usize) -> Result<(), PawError> {
    let Some(window_width) = query_window_width(session_name)? else {
        return Ok(());
    };
    for (pane_idx, cols) in agent_row_widths(window_width, agent_count) {
        let target = format!("{session_name}:0.{pane_idx}");
        let cols_str = cols.to_string();
        let _ = Command::new("tmux")
            .args(["resize-pane", "-t", &target, "-x", &cols_str])
            .status();
    }
    Ok(())
}
