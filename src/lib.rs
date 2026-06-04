//! git-paw — Parallel AI Worktrees.
//!
//! Orchestrates multiple AI coding CLI sessions across git worktrees
//! from a single terminal using tmux.

/// Default boot-prompt settle delay (ms) before the submit `Enter`, used
/// for any CLI without a `[clis.<name>].submit_delay_ms` override.
///
/// At launch git-paw injects the boot block, waits this long for a
/// paste-aware CLI to settle the (often large) paste, then sends `Enter`
/// separately. A same-call trailing `Enter` does not reliably submit a
/// large paste on some CLIs (W15-1, 2026-05-31 dogfood); the split +
/// settle does. The value is intentionally CLI-agnostic — per-CLI tuning
/// lives in config (`[clis.<name>].submit_delay_ms`), not a hardcoded
/// CLI-name table.
pub const DEFAULT_SUBMIT_DELAY_MS: u64 = 1500;

pub mod agents;
pub mod broker;
pub mod cli;
pub mod config;
pub mod coordination;
pub mod dashboard;
pub mod detect;
pub mod dirs;
pub mod error;
pub mod git;
pub mod init;
pub mod interactive;
pub mod lock;
pub mod logging;
pub mod opsx;
pub mod replay;
pub mod session;
pub mod skills;
pub mod specs;
pub mod supervisor;
pub mod tmux;
