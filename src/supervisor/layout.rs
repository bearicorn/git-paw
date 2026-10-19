//! Pane-layout calculation for supervisor-mode tmux sessions.
//!
//! The geometry now lives in [`crate::surface`], the core layout module every
//! surface-role decision reads; this module re-exports it at the old paths for
//! one release (`supervisor-pane-removal` drops the re-exports along with the
//! supervisor pane).
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

pub use crate::surface::{
    SUPERVISOR_AGENTS_PER_ROW, SUPERVISOR_MAX_AGENTS, SUPERVISOR_PANE_OFFSET, SupervisorLayout,
    layout_for, supervisor_layout,
};
