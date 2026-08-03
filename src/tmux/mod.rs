//! Tmux session and pane orchestration.
//!
//! Checks tmux availability, creates sessions, splits panes, sends commands,
//! applies layouts, and manages attach/reattach. Uses a builder pattern for
//! testability and dry-run support.

mod command;
mod layout;
mod readiness;
mod session;

#[cfg(test)]
mod tests;

pub use command::{
    PaneSpec, TmuxCommand, TmuxSession, TmuxSessionBuilder, build_add_agent_commands,
    build_boot_inject_args, build_remove_retile_commands, build_supervisor_session,
    build_supervisor_submit_argv_pair,
};
pub use layout::{agent_row_widths, rebalance_agent_rows};
pub use readiness::{
    CLI_READY_MARKERS, GateOutcome, PaneReadiness, ReadinessBudget, classify_pane_readiness,
    gate_pane_for_injection,
};
pub use session::{
    SessionLiveness, agents_without_live_pane, attach, detach_client, ensure_tmux_installed,
    is_session_alive, kill_pane, kill_pane_by_id, kill_session, list_panes_with_paths,
    reconcile_agents_to_panes, resolve_pane_id_for_worktree, resolve_session_name,
    session_liveness,
};

// Private helpers exercised only by the in-crate test module (widened from
// module-private to `pub(crate)` so `tests.rs` can reach them via `super::*`).
#[cfg(test)]
pub(crate) use layout::rebalance_agent_rows_with;
#[cfg(test)]
pub(crate) use readiness::{gate_pane_generic, relaunch_cli_into_pane};
#[cfg(test)]
pub(crate) use session::{
    attach_with, classify_liveness, detach_client_with, is_session_alive_with,
    kill_pane_by_id_with, kill_pane_with, kill_session_with, list_panes_with_paths_with,
    reconcile_agents_to_panes_with, resolve_pane_id_for_worktree_with, resolve_session_name_with,
    session_liveness_with,
};
