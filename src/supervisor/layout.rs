//! Pane-layout calculation for supervisor-mode tmux sessions.
//!
//! v0.5.0 supervisor mode arranges panes as:
//!
//! - Pane 0: supervisor agent (50% width of top row)
//! - Pane 1: dashboard (50% width of top row)
//! - Panes 2..N+1: coding agents, row-major, up to [`SUPERVISOR_AGENTS_PER_ROW`]
//!   columns per row
//!
//! Vertical proportions vary with the total number of rows. See
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
    pub top_row_pct: u8,
    /// Height percentage allocated to each agent row. `f32` because the
    /// 21–25-agent bucket lands on 14.4%.
    pub agent_row_pct: f32,
}

/// Compute the layout for a supervisor session with `agent_count` coding agents.
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

    let (top_row_pct, agent_row_pct) = match total_rows {
        2 => (60u8, 40.0_f32),
        3 => (40u8, 30.0_f32),
        4 => (28u8, 24.0_f32),
        5 => (28u8, 18.0_f32),
        6 => (28u8, 14.4_f32),
        _ => unreachable!("agent_count > SUPERVISOR_MAX_AGENTS is rejected above"),
    };

    Ok(SupervisorLayout {
        agent_rows,
        total_rows,
        top_row_pct,
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

    fn assert_layout(
        agent_count: usize,
        expected_rows: usize,
        expected_top: u8,
        expected_agent: f32,
    ) {
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
            layout.top_row_pct, expected_top,
            "top_row_pct for {agent_count}"
        );
        assert!(
            (layout.agent_row_pct - expected_agent).abs() < 0.01,
            "agent_row_pct for {agent_count}: expected {expected_agent}, got {}",
            layout.agent_row_pct
        );
    }

    #[test]
    fn layout_for_1_agent() {
        assert_layout(1, 1, 60, 40.0);
    }

    #[test]
    fn layout_for_5_agents() {
        assert_layout(5, 1, 60, 40.0);
    }

    #[test]
    fn layout_for_6_agents() {
        assert_layout(6, 2, 40, 30.0);
    }

    #[test]
    fn layout_for_10_agents() {
        assert_layout(10, 2, 40, 30.0);
    }

    #[test]
    fn layout_for_11_agents() {
        assert_layout(11, 3, 28, 24.0);
    }

    #[test]
    fn layout_for_15_agents() {
        assert_layout(15, 3, 28, 24.0);
    }

    #[test]
    fn layout_for_16_agents() {
        assert_layout(16, 4, 28, 18.0);
    }

    #[test]
    fn layout_for_20_agents() {
        assert_layout(20, 4, 28, 18.0);
    }

    #[test]
    fn layout_for_21_agents() {
        assert_layout(21, 5, 28, 14.4);
    }

    #[test]
    fn layout_for_25_agents() {
        assert_layout(25, 5, 28, 14.4);
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

    #[test]
    fn constants_have_expected_values() {
        assert_eq!(SUPERVISOR_MAX_AGENTS, 25);
        assert_eq!(SUPERVISOR_AGENTS_PER_ROW, 5);
        assert_eq!(SUPERVISOR_PANE_OFFSET, 2);
    }
}
