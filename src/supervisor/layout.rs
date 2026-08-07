//! Pane-layout calculation for supervisor-mode tmux sessions.
//!
//! v0.5.0 supervisor mode arranges panes as:
//!
//! - Pane 0: supervisor agent (50% width of top row)
//! - Pane 1: dashboard (50% width of top row)
//! - Panes 2..N+1: coding agents, row-major, up to [`SUPERVISOR_AGENTS_PER_ROW`]
//!   columns per row
//!
//! Vertically, the top row always takes half the session and the agent rows
//! share the other half evenly. See
//! `openspec/changes/supervisor-as-pane/specs/tmux-orchestration/spec.md`.

use crate::error::PawError;

/// Maximum agents per supervisor session for v0.5.0. Above this, the launch
/// is rejected with an actionable "split into multiple sessions" error.
/// Configurable extension deferred to v1.0.0 (issue #17).
pub const SUPERVISOR_MAX_AGENTS: usize = 25;

/// Agents per agent-grid row for v0.5.0. Hard-coded; configurable in v1.0.0.
pub const SUPERVISOR_AGENTS_PER_ROW: usize = 5;

/// Offset applied to agent-pane indices in supervisor mode: supervisor at 0,
/// dashboard at 1, so the first coding agent lands at pane 2.
pub const SUPERVISOR_PANE_OFFSET: usize = 2;

/// Computed layout parameters for a supervisor-mode tmux session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SupervisorLayout {
    /// Number of horizontal rows holding coding agents (excludes the top row).
    pub agent_rows: usize,
    /// Total tmux rows = `agent_rows + 1` (1 top row + agent rows).
    pub total_rows: usize,
    /// Height percentage allocated to the top row (supervisor + dashboard).
    /// Always 50: the top row is half the session whatever the agent count.
    pub top_row_pct: u8,
    /// Height percentage allocated to each agent row: `50 / agent_rows`, the
    /// even split of the half the top row leaves. `f32` because the split is
    /// fractional for some row counts (3 agent rows land on 16.67%).
    pub agent_row_pct: f32,
}

/// Compute the layout for a supervisor session with `agent_count` coding agents.
///
/// The top row is always 50% of the session height; the agent rows split the
/// remaining 50% evenly. Adding agents shrinks the agent rows, never the top row.
///
/// Returns [`PawError::ConfigError`] when `agent_count > SUPERVISOR_MAX_AGENTS`.
pub fn supervisor_layout(agent_count: usize) -> Result<SupervisorLayout, PawError> {
    if agent_count > SUPERVISOR_MAX_AGENTS {
        return Err(PawError::ConfigError(format!(
            "{agent_count} agents requested; maximum is {SUPERVISOR_MAX_AGENTS} per session.\n\
             \n\
             Split into multiple sessions:\n  \
             git paw start --branches <subset>\n\
             \n\
             (Configurable max_agents is planned for v1.0.0 — see milestone.)"
        )));
    }

    let agent_rows = agent_count.div_ceil(SUPERVISOR_AGENTS_PER_ROW).max(1);
    let total_rows = agent_rows + 1;

    // `agent_rows` is capped at SUPERVISOR_MAX_AGENTS / SUPERVISOR_AGENTS_PER_ROW,
    // so the widening cast is exact.
    #[allow(clippy::cast_precision_loss)]
    let agent_row_pct = 50.0_f32 / agent_rows as f32;

    Ok(SupervisorLayout {
        agent_rows,
        total_rows,
        top_row_pct: 50,
        agent_row_pct,
    })
}

/// Pure grid-geometry function of agent count, named per the add/remove
/// design (D1). The v0.5.0 layout builder ([`supervisor_layout`]) is already a
/// pure function of `agent_count`; `layout_for` is the canonical name the
/// `add-branch` / `remove-branch` specs use to make explicit that the same
/// geometry is recomputed for `N → N+1` (add) and `N → N−1` (remove)
/// re-tiling, not just the initial start-time layout.
///
/// Returns [`PawError::ConfigError`] when `agent_count > SUPERVISOR_MAX_AGENTS`
/// — the same "split into multiple sessions" error `git paw start` surfaces.
pub fn layout_for(agent_count: usize) -> Result<SupervisorLayout, PawError> {
    supervisor_layout(agent_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervisor_layout_covers_each_agent_count_bucket() {
        // One row per agent count currently covered:
        // agent_count -> (agent_rows, agent_row_pct). Each row also asserts
        // total_rows == agent_rows + 1 and a top row of 50%. Rows straddle every
        // bucket boundary (lower + upper edge of each row-count tier).
        for (agent_count, expected_rows, expected_agent) in [
            (1, 1, 50.0_f32),
            (5, 1, 50.0),
            (6, 2, 25.0),
            (10, 2, 25.0),
            (11, 3, 50.0 / 3.0),
            (15, 3, 50.0 / 3.0),
            (16, 4, 12.5),
            (20, 4, 12.5),
            (21, 5, 10.0),
            (25, 5, 10.0),
        ] {
            let layout = supervisor_layout(agent_count).expect("layout should compute");
            assert_eq!(
                layout.agent_rows, expected_rows,
                "agent_rows for {agent_count}"
            );
            assert_eq!(
                layout.total_rows,
                expected_rows + 1,
                "total_rows for {agent_count}"
            );
            assert_eq!(
                layout.top_row_pct, 50,
                "top_row_pct for {agent_count} is half the session"
            );
            assert!(
                (layout.agent_row_pct - expected_agent).abs() < 0.01,
                "agent_row_pct for {agent_count}: expected {expected_agent}, got {}",
                layout.agent_row_pct
            );
        }
    }

    #[test]
    fn top_row_stays_half_the_session_as_agents_grow() {
        // Growing the agent count must shrink the agent rows, never the top row.
        let small = supervisor_layout(2).expect("small layout should compute");
        let large = supervisor_layout(25).expect("large layout should compute");

        assert_eq!(small.top_row_pct, large.top_row_pct, "top row is unchanged");
        assert_eq!(small.top_row_pct, 50, "top row is half the session");
        assert!(
            large.agent_row_pct < small.agent_row_pct,
            "only the per-agent-row share shrinks: {} vs {}",
            large.agent_row_pct,
            small.agent_row_pct
        );
    }

    #[test]
    fn agent_rows_evenly_split_the_lower_half() {
        // Whatever the row count, the agent rows sum back to the 50% the top
        // row leaves.
        for agent_count in 1..=SUPERVISOR_MAX_AGENTS {
            let layout = supervisor_layout(agent_count).expect("layout should compute");
            #[allow(clippy::cast_precision_loss)]
            let total = layout.agent_row_pct * layout.agent_rows as f32;
            assert!(
                (total - 50.0).abs() < 0.01,
                "agent rows for {agent_count} should sum to 50%, got {total}"
            );
        }
    }

    #[test]
    fn layout_rejects_26_agents() {
        let err = supervisor_layout(26).expect_err("26 agents should be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("26 agents requested"),
            "error mentions count: {msg}"
        );
        assert!(msg.contains("maximum is 25"), "error mentions max: {msg}");
        assert!(
            msg.contains("--branches"),
            "error suggests --branches workaround: {msg}"
        );
    }

    #[test]
    fn layout_rejects_far_above_cap() {
        let err = supervisor_layout(100).expect_err("100 agents should be rejected");
        assert!(err.to_string().contains("100 agents requested"));
    }

    #[test]
    fn layout_for_matches_supervisor_layout_across_the_range() {
        // layout_for is the D1-named alias; it must be identical to
        // supervisor_layout for every valid count and reject the same way.
        for n in 1..=SUPERVISOR_MAX_AGENTS {
            assert_eq!(
                layout_for(n).expect("layout_for should compute"),
                supervisor_layout(n).expect("supervisor_layout should compute"),
                "layout_for({n}) should match supervisor_layout({n})"
            );
        }
        assert!(
            layout_for(SUPERVISOR_MAX_AGENTS + 1).is_err(),
            "layout_for should reject above the cap like supervisor_layout"
        );
    }
}
