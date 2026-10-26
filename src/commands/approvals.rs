//! `git paw approvals` — reports the manually-approved command patterns for a
//! session.

use std::path::Path;

use crate::error::PawError;
use crate::git;
use crate::session;

use super::{CommandContext, Output};

/// Options for [`approvals`]: the `git paw approvals` flags.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApprovalsOptions {
    /// `--session`: the session to report on, else the repository's.
    pub session: Option<String>,
    /// `--limit`: report at most this many patterns.
    pub limit: Option<usize>,
    /// `--json`: render JSON instead of the text table.
    pub json: bool,
}

/// What [`approvals`] reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalsOutcome {
    /// The session reported on.
    pub session_name: String,
    /// How many patterns were reported.
    pub patterns: usize,
}

/// Resolves which session's manual-approval log to read.
///
/// `--session` wins when present. Otherwise the active session for the current
/// repo is used. Unlike `replay`, a missing log file is not an error (the
/// session simply recorded no manual approvals), so this only needs to name a
/// session — it does not validate that a log exists.
fn resolve_approvals_session(
    repo_root: &Path,
    session_flag: Option<&str>,
) -> Result<String, PawError> {
    if let Some(name) = session_flag {
        return Ok(name.to_string());
    }
    match session::find_session_for_repo(repo_root)? {
        Some(s) => Ok(s.session_name),
        None => Err(PawError::SessionError(
            "no active session for this repo; pass --session <NAME> to target one".to_string(),
        )),
    }
}

/// Reports the manually-approved command patterns for a session.
///
/// Reads the per-session manual-approval JSONL log, aggregates by pattern,
/// applies the promotion-target heuristic, sorts by descending count, and
/// renders either a text table (default) or JSON (`--json`). An empty/missing
/// log produces an empty result without error.
///
/// # Errors
///
/// Returns an error when no session can be named or the log cannot be read.
pub fn approvals(
    ctx: &CommandContext,
    opts: ApprovalsOptions,
    out: &mut dyn Output,
) -> Result<ApprovalsOutcome, PawError> {
    use crate::supervisor::manual_approvals::{self, AggregatedApproval, Suggestion};

    let ApprovalsOptions {
        session: session_flag,
        limit,
        json,
    } = opts;
    let session_flag = session_flag.as_deref();
    let repo_root = ctx.repo_root().to_path_buf();
    let session_name = resolve_approvals_session(&repo_root, session_flag)?;
    let project_name = git::project_name(&repo_root);

    let log_path = manual_approvals::log_path(&repo_root, &session_name);
    let mut rows = manual_approvals::aggregate(&log_path)
        .map_err(|e| PawError::SessionError(format!("failed to read manual-approvals log: {e}")))?;
    if let Some(n) = limit {
        rows.truncate(n);
    }

    // Pair each pattern with its promotion-target suggestion. Branch/worktree
    // context is per-agent and not retained by aggregation, so the report
    // leans on the project name plus the `./`-token rule.
    let classified: Vec<(AggregatedApproval, Suggestion)> = rows
        .into_iter()
        .map(|r| {
            let s = manual_approvals::suggest_target(&r.pattern, &project_name, "", None);
            (r, s)
        })
        .collect();
    let outcome = ApprovalsOutcome {
        session_name: session_name.clone(),
        patterns: classified.len(),
    };

    if json {
        let approvals: Vec<serde_json::Value> = classified
            .iter()
            .map(|(r, s)| {
                serde_json::json!({
                    "pattern": r.pattern,
                    "count": r.count,
                    "suggested_target": s.json_value(),
                    "first_seen": r.first_seen,
                    "last_seen": r.last_seen,
                })
            })
            .collect();
        let body = serde_json::json!({
            "session": session_name,
            "approvals": approvals,
        });
        out.line(
            &serde_json::to_string_pretty(&body)
                .map_err(|e| PawError::SessionError(format!("failed to serialize JSON: {e}")))?,
        );
        return Ok(outcome);
    }

    if classified.is_empty() {
        out.line(&format!(
            "no manual approvals recorded for session '{session_name}'"
        ));
        return Ok(outcome);
    }

    // Text table: PATTERN / COUNT / SUGGEST, columns sized to content.
    let pattern_w = classified
        .iter()
        .map(|(r, _)| r.pattern.len())
        .max()
        .unwrap_or(0)
        .max("PATTERN".len());
    let count_w = classified
        .iter()
        .map(|(r, _)| r.count.to_string().len())
        .max()
        .unwrap_or(0)
        .max("COUNT".len());

    out.line(&format!(
        "{:<pattern_w$}  {:>count_w$}  SUGGEST",
        "PATTERN", "COUNT"
    ));
    for (r, s) in &classified {
        out.line(&format!(
            "{:<pattern_w$}  {:>count_w$}  {}",
            r.pattern,
            r.count,
            s.label()
        ));
    }
    Ok(outcome)
}
