//! `OpenSpec` (`opsx`) integration support.
//!
//! Houses the role-gating guard (`opsx-role-gating` capability): a
//! post-commit watcher hook that detects archive-activity commits by
//! non-supervisor agents and reacts per the configured
//! [`crate::config::RoleGatingMode`]. The capability is scoped to the
//! `OpenSpec` spec engine — under Spec Kit / Markdown engines the guard is
//! inert and the forbidden-command skill sections are omitted.

pub mod role_guard;

pub use role_guard::{
    AgentAttribution, Classification, CommitDiff, RoleGatingContext, SUPERVISOR_AGENT_ID,
    classify_commit, resolve_agent_id,
};
