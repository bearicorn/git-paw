//! git-paw — Parallel AI Worktrees.
//!
//! Orchestrates multiple AI coding CLI sessions across git worktrees
//! from a single terminal using tmux.

pub mod agents;
pub mod broker;
pub mod cli;
pub mod config;
pub mod dashboard;
pub mod detect;
pub mod dirs;
pub mod error;
pub mod git;
pub mod init;
pub mod interactive;
pub mod logging;
pub mod merge_loop;
pub mod replay;
pub mod session;
pub mod skills;
pub mod specs;
pub mod summary;
pub mod supervisor;
pub mod tmux;
