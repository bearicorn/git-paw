//! Supervisor-side helpers.
//!
//! The submodules here keep the supervisor's pane handling — permission-prompt
//! detection, keystroke dispatch, the per-pane claim, the unattended drive
//! loop, its headless orchestrator hand-off and the manual-decision log — out
//! of `main.rs` and testable in isolation.
//!
//! None of them approves a permission prompt. git-paw makes no safety
//! decision about an agent's command; the agent CLI's own permission system
//! or the human decides, and the only approval path is an explicit
//! `sweep.sh approve`.

pub mod approval_gate;
pub mod branch_refresh;
pub mod claim;
pub mod drive;
pub mod escalation;
pub mod keys;
pub mod loop_binding;
pub mod manual_approvals;
pub mod permission_prompt;
