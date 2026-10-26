//! `git paw replay` — resolves which captured session log to replay, or lists
//! the captured logs. Showing a log (through a pager, or raw to stdout) is
//! terminal work and belongs to the caller.

use std::path::PathBuf;

use crate::error::PawError;

use super::{CommandContext, Output};

/// Options for [`replay`]: the `git paw replay` arguments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplayOptions {
    /// The branch whose log to replay (required unless `list`).
    pub branch: Option<String>,
    /// `--list`: list the captured logs instead.
    pub list: bool,
    /// `--color`: keep the log's colors (the caller pages it).
    pub color: bool,
    /// `--session`: the session to look in, else the repository's latest.
    pub session: Option<String>,
}

/// What [`replay`] resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayOutcome {
    /// The captured logs were listed to the sink.
    Listed,
    /// The log to show; the caller writes it out.
    Log {
        /// The log file.
        path: PathBuf,
        /// Whether to keep its colors.
        color: bool,
    },
}

/// Resolves the captured session log to replay, or lists the captured logs
/// through `out`.
///
/// # Errors
///
/// Returns an error when no branch is given without `list`, or the session
/// or its log cannot be found.
pub fn replay(
    ctx: &CommandContext,
    opts: ReplayOptions,
    out: &mut dyn Output,
) -> Result<ReplayOutcome, PawError> {
    let repo_root = ctx.repo_root();
    if opts.list {
        crate::replay::display_list(repo_root, out)?;
        return Ok(ReplayOutcome::Listed);
    }

    // clap's `required_unless_present = "list"` rejects this first; the error
    // is defence in depth should that attribute ever drift.
    let branch = opts.branch.ok_or_else(|| {
        PawError::ReplayError("a branch is required unless --list is given".into())
    })?;
    let session_name = crate::replay::resolve_session(repo_root, opts.session.as_deref())?;
    let path = crate::replay::find_log(repo_root, &session_name, &branch)?;
    Ok(ReplayOutcome::Log {
        path,
        color: opts.color,
    })
}
