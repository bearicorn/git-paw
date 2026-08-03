//! Command handlers, extracted from `main.rs`.
//!
//! `main`/`run` keep only dispatch + argument wiring; each subcommand handler
//! and its leaf helpers live under this module. Modules are added wave by wave
//! (code-analysis-refactor R2) — see that change's `tasks.md` for the split
//! plan. Handlers take plain arguments and return `Result<(), PawError>`, and
//! the tmux/git orchestration ones route through the [`git_paw::command_runner`]
//! seam so their argv is unit-testable.

pub mod add;
pub mod approvals;
pub mod clis;
pub mod helpers;
pub mod pause;
pub mod recover;
pub mod remove;
pub mod replay;
pub mod start;
pub mod status;
pub mod stop;
