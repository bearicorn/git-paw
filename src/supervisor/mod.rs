//! Supervisor-side helpers for the auto-approve patterns feature.
//!
//! See `openspec/changes/auto-approve-patterns/` for the spec contracts.
//! The submodules here keep auto-approval logic out of `main.rs` and
//! testable in isolation.

pub mod approve;
pub mod auto_approve;
pub mod curl_allowlist;
pub mod permission_prompt;
pub mod poll;
pub mod stall;
