//! Runtime operations against live tmux sessions and panes.
//!
//! Installation and liveness probes, session-name resolution, attach/detach,
//! pane and session teardown, and JSON-to-pane reconciliation.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::PawError;

/// Maximum number of session name collision retries.
const MAX_COLLISION_RETRIES: u32 = 10;

/// Check that tmux is installed on PATH.
///
/// Returns `Ok(())` if found, or `Err(PawError::TmuxNotInstalled)` with
/// install instructions if missing.
pub fn ensure_tmux_installed() -> Result<(), PawError> {
    which::which("tmux").map_err(|_| PawError::TmuxNotInstalled)?;
    Ok(())
}

/// Check whether a tmux session with the given name is currently alive.
pub fn is_session_alive(name: &str) -> Result<bool, PawError> {
    let status = Command::new("tmux")
        .args(["has-session", "-t", name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;

    Ok(status.success())
}

/// Outcome of a session-liveness probe (design D3 of `session-bugfixes`).
///
/// Distinguishes a genuinely-absent tmux session (`Stale`) from a probe that
/// could not be run at all (`Indeterminate`, e.g. the `tmux` binary is
/// missing). Receipt-staleness detection SHALL NOT report `🔴 stale` on an
/// `Indeterminate` probe — a missing tmux binary is not evidence the session
/// died.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionLiveness {
    /// `tmux has-session` returned exit 0 — the session exists.
    Alive,
    /// `tmux has-session` ran and returned non-zero — the session is gone.
    Stale,
    /// The probe could not be run (tmux binary absent/unreachable). The
    /// caller SHALL preserve the receipt's current state.
    Indeterminate,
}

/// Pure mapping from a probe's raw outcome to a [`SessionLiveness`].
///
/// `spawned` is whether the `tmux has-session` process started at all;
/// `success` is its exit-status success (only meaningful when `spawned`).
/// Extracted so each branch is unit-testable without a real tmux server.
pub(crate) fn classify_liveness(spawned: bool, success: bool) -> SessionLiveness {
    match (spawned, success) {
        (false, _) => SessionLiveness::Indeterminate,
        (true, true) => SessionLiveness::Alive,
        (true, false) => SessionLiveness::Stale,
    }
}

/// Probe a tmux session's liveness via a single `tmux has-session` call.
///
/// This is the cheap staleness check used by `status`, `start`, and
/// `purge --stale` (spec: "Liveness probe is cheap"). It runs exactly one
/// `tmux has-session -t <name>` invocation and never probes the broker or
/// agent processes.
pub fn session_liveness(name: &str) -> SessionLiveness {
    let spawn = Command::new("tmux")
        .args(["has-session", "-t", name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    match spawn {
        Ok(status) => classify_liveness(true, status.success()),
        Err(_) => classify_liveness(false, false),
    }
}

/// Resolve a unique session name, handling collisions with existing sessions.
///
/// Starts with `paw-<project_name>` and appends `-2`, `-3`, etc. if the name
/// is already taken by another session.
pub fn resolve_session_name(project_name: &str) -> Result<String, PawError> {
    let base = format!("paw-{project_name}");

    if !is_session_alive(&base)? {
        return Ok(base);
    }

    for suffix in 2..=MAX_COLLISION_RETRIES + 1 {
        let candidate = format!("{base}-{suffix}");
        if !is_session_alive(&candidate)? {
            return Ok(candidate);
        }
    }

    Err(PawError::TmuxError(format!(
        "too many session name collisions for '{base}'"
    )))
}

/// Attach the current terminal to the named tmux session.
///
/// This replaces the current process's stdio. Returns an error if the
/// session does not exist or tmux fails.
pub fn attach(name: &str) -> Result<(), PawError> {
    let status = Command::new("tmux")
        .args(["attach-session", "-t", name])
        .status()
        .map_err(|e| PawError::TmuxError(format!("failed to attach to tmux session: {e}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(PawError::TmuxError(format!(
            "failed to attach to session '{name}'"
        )))
    }
}

/// Detach all clients attached to the named tmux session.
///
/// Wraps `tmux detach-client -s <session>`. Idempotent: returns `Ok(())`
/// if the command succeeds OR if tmux reports the session has no
/// clients attached (the typical no-op error path on already-detached
/// sessions). Leaves the tmux server, the session, and every pane
/// process untouched.
pub fn detach_client(session_name: &str) -> Result<(), PawError> {
    let output = Command::new("tmux")
        .args(["detach-client", "-s", session_name])
        .output()
        .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;

    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    // "no clients attached" is the idempotent no-op case.
    if stderr.contains("no clients") || stderr.contains("no current client") {
        return Ok(());
    }
    Err(PawError::TmuxError(
        String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    ))
}

/// Kill a single pane within a session by `(session, pane_index)`.
///
/// Wraps `tmux kill-pane -t <session>:0.<index>`. Returns `Ok(())` if
/// the pane was killed OR if tmux reports the pane does not exist
/// (idempotent no-op on missing panes). Used by the pause flow to take
/// down the dashboard pane (which owns the broker subprocess) without
/// killing the rest of the session.
pub fn kill_pane(session_name: &str, pane_index: u32) -> Result<(), PawError> {
    let target = format!("{session_name}:0.{pane_index}");
    let output = Command::new("tmux")
        .args(["kill-pane", "-t", &target])
        .output()
        .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;

    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    // Pane-doesn't-exist is the idempotent no-op case.
    if stderr.contains("can't find pane")
        || stderr.contains("no such pane")
        || stderr.contains("pane not found")
    {
        return Ok(());
    }
    Err(PawError::TmuxError(
        String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    ))
}

/// Kill the named tmux session.
pub fn kill_session(name: &str) -> Result<(), PawError> {
    let output = Command::new("tmux")
        .args(["kill-session", "-t", name])
        .output()
        .map_err(|e| PawError::TmuxError(format!("failed to kill tmux session: {e}")))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(PawError::TmuxError(stderr.trim().to_owned()))
    }
}

/// Kill a single pane addressed by its tmux pane id (`%N`), regardless of the
/// process running in it (a CLI, a bare shell, or anything else).
///
/// Used by the `remove` path (design D2, G2a): the target pane is resolved
/// from the removed branch's worktree via [`resolve_pane_id_for_worktree`] so
/// the kill never depends on the JSON-position arithmetic that broke under an
/// orphan pane. A pane that has already gone is treated as an idempotent no-op,
/// matching [`kill_pane`]'s missing-pane tolerance.
pub fn kill_pane_by_id(pane_id: &str) -> Result<(), PawError> {
    let output = Command::new("tmux")
        .args(["kill-pane", "-t", pane_id])
        .output()
        .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;

    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    if stderr.contains("can't find pane")
        || stderr.contains("no such pane")
        || stderr.contains("pane not found")
    {
        return Ok(());
    }
    Err(PawError::TmuxError(
        String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    ))
}

/// List every live pane in `session_name`'s window as
/// `(pane_id, pane_current_path)` pairs via
/// `tmux list-panes -F '#{pane_id} #{pane_current_path}'`.
///
/// Returns an empty vector (not an error) when the session/server is gone, so
/// callers on a torn-down session degrade to "no live panes" rather than
/// failing. Only a genuine tmux execution failure surfaces as an error.
pub fn list_panes_with_paths(session_name: &str) -> Result<Vec<(String, String)>, PawError> {
    let output = Command::new("tmux")
        .args([
            "list-panes",
            "-t",
            session_name,
            "-F",
            "#{pane_id} #{pane_current_path}",
        ])
        .output()
        .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
        if stderr.contains("can't find")
            || stderr.contains("no such")
            || stderr.contains("no server running")
        {
            return Ok(Vec::new());
        }
        return Err(PawError::TmuxError(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut panes = Vec::new();
    for line in text.lines() {
        if let Some((id, path)) = line.split_once(' ') {
            panes.push((id.to_string(), path.to_string()));
        }
    }
    Ok(panes)
}

/// Resolve the tmux pane id (`%N`) whose `pane_current_path` matches
/// `worktree_path` in `session_name` (design D2, G2a).
///
/// Both the queried `pane_current_path` and `worktree_path` are canonicalised
/// before comparison so a symlinked temp root (e.g. macOS `/var` →
/// `/private/var`) does not defeat the match. Returns `Ok(None)` when no live
/// pane maps to the worktree (the agent's pane is already gone), so the caller
/// can treat removal as an idempotent no-op.
pub fn resolve_pane_id_for_worktree(
    session_name: &str,
    worktree_path: &Path,
) -> Result<Option<String>, PawError> {
    let want = canonical_or_self(worktree_path);
    for (pane_id, path) in list_panes_with_paths(session_name)? {
        if canonical_or_self(Path::new(&path)) == want {
            return Ok(Some(pane_id));
        }
    }
    Ok(None)
}

/// Canonicalise `path`, falling back to the path itself when it cannot be
/// resolved (e.g. a temp dir already removed). Keeps comparisons total.
fn canonical_or_self(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Pure JSON↔tmux reconciliation (design D3, G2b): given the session-JSON
/// agents as `(branch, worktree_path)` pairs and the set of live pane
/// `pane_current_path` values, return the branches whose worktree maps to no
/// live pane.
///
/// Split from [`reconcile_agents_to_panes`] so the divergence logic is unit
/// testable without a live tmux server. Paths are canonicalised on both sides
/// (with a self fallback) so the comparison is symlink-tolerant.
#[must_use]
pub fn agents_without_live_pane(
    agents: &[(String, PathBuf)],
    live_pane_paths: &[PathBuf],
) -> Vec<String> {
    let live: Vec<PathBuf> = live_pane_paths
        .iter()
        .map(|p| canonical_or_self(p))
        .collect();
    agents
        .iter()
        .filter(|(_, wt)| {
            let want = canonical_or_self(wt);
            !live.contains(&want)
        })
        .map(|(branch, _)| branch.clone())
        .collect()
}

/// Reconcile the session-JSON agents against the live tmux panes in
/// `session_name` and report any agent (by branch) whose worktree maps to no
/// live pane (the v0.8.0 G2 desync). Returns an empty vector when every agent
/// maps to a pane.
///
/// Surfaced (not auto-repaired) on the `add` path so a dropped/orphaned pane is
/// visible and recoverable rather than silent.
pub fn reconcile_agents_to_panes(
    session_name: &str,
    agents: &[(String, PathBuf)],
) -> Result<Vec<String>, PawError> {
    let live: Vec<PathBuf> = list_panes_with_paths(session_name)?
        .into_iter()
        .map(|(_, path)| PathBuf::from(path))
        .collect();
    Ok(agents_without_live_pane(agents, &live))
}
