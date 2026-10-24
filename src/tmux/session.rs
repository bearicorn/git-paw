//! Runtime operations against live tmux sessions and panes.
//!
//! Installation and liveness probes, session-name resolution, attach/detach,
//! pane and session teardown, and JSON-to-pane reconciliation.
//!
//! Every tmux invocation here goes through the [`CommandRunner`] seam. Each
//! public entry point keeps its original signature and delegates to a
//! `*_with(runner, …)` sibling wired to [`RealCommandRunner`], so callers are
//! untouched while the argv and the success/failure branches become assertable
//! without a live tmux server (`code-analysis-refactor` D2). The real runner
//! performs exactly what the previous inline `Command::new("tmux")` calls did.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::command_runner::{CommandRunner, RealCommandRunner};
use crate::error::PawError;
use crate::surface::resolve_agent;

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
    is_session_alive_with(&RealCommandRunner, name)
}

/// [`is_session_alive`] against an injected runner.
///
/// Captures the probe's output rather than discarding it via `Stdio::null()` as
/// the previous inline call did — indistinguishable to the user either way,
/// since neither reaches the terminal and only the exit status is inspected.
pub(crate) fn is_session_alive_with(
    runner: &dyn CommandRunner,
    name: &str,
) -> Result<bool, PawError> {
    let output = runner
        .run("tmux", &["has-session", "-t", name])
        .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;

    Ok(output.success)
}

/// Outcome of a session-liveness probe (design D3 of `session-bugfixes`).
///
/// Defined on the terminal port ([`crate::terminal::SessionLiveness`]); the
/// tmux probes below answer it from `tmux has-session` (`Alive` / `Stale`, or
/// `Indeterminate` when tmux cannot be run) and [`session_liveness_for`]'s
/// per-pane check (`AgentsExited`).
pub use crate::terminal::SessionLiveness;

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
    session_liveness_with(&RealCommandRunner, name)
}

/// [`session_liveness`] against an injected runner.
pub(crate) fn session_liveness_with(runner: &dyn CommandRunner, name: &str) -> SessionLiveness {
    match runner.run("tmux", &["has-session", "-t", name]) {
        Ok(output) => classify_liveness(true, output.success),
        Err(_) => classify_liveness(false, false),
    }
}

/// Foreground-shell process names — a pane whose `#{pane_current_command}`
/// is one of these has reverted to an interactive prompt (its launched CLI
/// process exited), not a still-running agent. Used by
/// [`has_live_agent_pane_with`] (GP-03c) and [`is_shell_command`].
const SHELL_COMMAND_NAMES: &[&str] = &["bash", "zsh", "sh", "dash", "fish", "ksh", "csh", "tcsh"];

/// Whether `command` — a pane's `#{pane_current_command}` — is a foreground
/// shell rather than a running agent CLI.
///
/// A pane that has reverted to a shell looks *idle* to every capture-based
/// predicate: it shows no input box, no permission prompt and no
/// mid-response marker, because none of those are a shell's. Anything that
/// decides whether to type into a pane therefore has to ask what the pane is
/// RUNNING as well as what it looks like — otherwise free text intended for
/// a CLI composer is handed to a command interpreter.
///
/// Shares [`SHELL_COMMAND_NAMES`] with the liveness probe so the two cannot
/// disagree about what counts as a shell.
#[must_use]
pub fn is_shell_command(command: &str) -> bool {
    SHELL_COMMAND_NAMES.contains(&command.trim())
}

/// Grace period after a session's `created_at` during which
/// [`session_liveness_for`] treats the session as alive purely from tmux
/// session existence, without yet requiring a live agent-pane process — the
/// CLI launched via `send-keys` needs a moment to start (GP-03c's "tolerate
/// the launch window" requirement).
const AGENT_LAUNCH_GRACE_PERIOD: Duration = Duration::from_secs(30);

/// Session-liveness probe that additionally requires at least one live
/// agent-pane CLI process once past the launch grace period (GP-03c).
///
/// `tmux has-session` alone is insufficient: git-paw launches every pane as a
/// shell and `send-keys`s the CLI command into it, so a pane whose CLI
/// process exits reverts to its underlying interactive shell rather than
/// closing — the tmux session (kept up by the supervisor/dashboard panes)
/// and every agent pane keep existing even after every agent's CLI has died
/// (the full-auto boot-exit failure mode, where the first-run
/// bypass-permissions dialog defaults to "No, exit" and every pane reverts to
/// a bare shell within seconds). Checking each agent pane's CURRENT
/// foreground command is what distinguishes a genuinely running agent from
/// one that silently reverted to a shell prompt.
///
/// `agent_pane_offset` is the pane index of the first coding-agent pane (see
/// `agent_pane_offset` in `commands::helpers`) — panes before it (supervisor,
/// dashboard) are never consulted. A session that is otherwise alive but
/// within [`AGENT_LAUNCH_GRACE_PERIOD`] of `created_at` is reported alive
/// regardless, so a just-launched session (whose agent CLI has not yet had
/// time to start) never flaps to not-alive.
///
/// The exited-agent case returns [`SessionLiveness::AgentsExited`], not
/// [`SessionLiveness::Stale`]: the tmux session is still there, so callers that
/// render staleness ("the tmux session no longer exists", `purge --stale`) must
/// not treat it as an absent session. `Stale` from this probe still means only
/// that `tmux has-session` failed.
#[must_use]
pub fn session_liveness_for(
    name: &str,
    created_at: SystemTime,
    agent_pane_offset: usize,
) -> SessionLiveness {
    session_liveness_for_with(
        &RealCommandRunner,
        name,
        created_at,
        agent_pane_offset,
        SystemTime::now(),
    )
}

/// [`session_liveness_for`] against an injected runner and clock.
pub(crate) fn session_liveness_for_with(
    runner: &dyn CommandRunner,
    name: &str,
    created_at: SystemTime,
    agent_pane_offset: usize,
    now: SystemTime,
) -> SessionLiveness {
    let base = session_liveness_with(runner, name);
    if base != SessionLiveness::Alive {
        return base;
    }
    if now
        .duration_since(created_at)
        .is_ok_and(|age| age < AGENT_LAUNCH_GRACE_PERIOD)
    {
        return SessionLiveness::Alive;
    }
    if has_live_agent_pane_with(runner, name, agent_pane_offset) {
        SessionLiveness::Alive
    } else {
        SessionLiveness::AgentsExited
    }
}

/// `@paw_role` values that mark a pane as infrastructure rather than a coding
/// agent. Every other role git-paw sets is an agent pane's branch name.
const NON_AGENT_PANE_ROLES: &[&str] = &["dashboard", "supervisor"];

/// Returns `true` when at least one coding-agent pane has a foreground process
/// other than a bare shell (GP-03c).
///
/// Each pane is classified **on its own**:
///
/// - a pane whose pane-scoped `@paw_role` option (set at launch to the pane's
///   branch, see `push_pane_title`) is one of [`NON_AGENT_PANE_ROLES`] is
///   infrastructure, never an agent;
/// - any other non-empty role is an agent pane;
/// - a pane with no role at all falls back to the positional rule
///   (`pane_index >= agent_pane_offset`).
///
/// Roles are preferred over pane indices because `pane_index` is
/// **positional**: killing a pane renumbers every pane after it. `git paw
/// pause` kills the dashboard pane, so a paused session's surviving agent
/// slides down into the dashboard's old index — and a purely positional probe
/// would then look past it and wrongly conclude that every agent had exited,
/// which makes `git paw start` tear down a session whose agents are alive.
///
/// Classifying per pane rather than switching modes for the whole session
/// keeps a **mixed** session correct: panes added before and after a
/// `[layout].border_affordances` change can disagree about whether they carry
/// a role, and a role-less agent pane must still be able to prove the session
/// alive. Both rules err toward "alive", which is the safe direction: the cost
/// of a false "exited" is a torn-down session.
pub(crate) fn has_live_agent_pane_with(
    runner: &dyn CommandRunner,
    session_name: &str,
    agent_pane_offset: usize,
) -> bool {
    let Ok(output) = runner.run(
        "tmux",
        &[
            "list-panes",
            "-t",
            session_name,
            "-F",
            // Role last: a branch name may contain the separator, a pane index
            // and a command name cannot, so the role takes the remainder.
            "#{pane_index}|#{pane_current_command}|#{@paw_role}",
        ],
    ) else {
        return false;
    };
    if !output.success {
        return false;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().any(|line| {
        let mut parts = line.splitn(3, '|');
        let Some(Ok(idx)) = parts.next().map(|p| p.trim().parse::<usize>()) else {
            return false;
        };
        let Some(command) = parts.next().map(str::trim) else {
            return false;
        };
        let role = parts.next().unwrap_or_default().trim();

        let is_agent_pane = if role.is_empty() {
            idx >= agent_pane_offset
        } else {
            !NON_AGENT_PANE_ROLES.contains(&role)
        };
        is_agent_pane && !SHELL_COMMAND_NAMES.contains(&command)
    })
}

/// Resolve a unique session name, handling collisions with existing sessions.
///
/// Starts with the sanitised `paw-<project_name>` base
/// ([`SessionName::from_project`](crate::domain::SessionName::from_project))
/// and appends `-2`, `-3`, etc. to that base if the name is already taken by
/// another session.
pub fn resolve_session_name(project_name: &str) -> Result<String, PawError> {
    resolve_session_name_with(&RealCommandRunner, project_name)
}

/// [`resolve_session_name`] against an injected runner, so the collision-retry
/// walk and its exhaustion error are testable without occupying real sessions.
pub(crate) fn resolve_session_name_with(
    runner: &dyn CommandRunner,
    project_name: &str,
) -> Result<String, PawError> {
    let base = crate::domain::SessionName::from_project(project_name);

    if !is_session_alive_with(runner, base.as_str())? {
        return Ok(base.into_string());
    }

    for suffix in 2..=MAX_COLLISION_RETRIES + 1 {
        let candidate = base.with_collision_suffix(suffix);
        if !is_session_alive_with(runner, candidate.as_str())? {
            return Ok(candidate.into_string());
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
    attach_with(&RealCommandRunner, name)
}

/// [`attach`] against an injected runner.
///
/// Uses [`CommandRunner::run_inheriting_stdio`] — `tmux attach-session` takes
/// over this process's terminal, so its stdio MUST be inherited. Capturing it
/// would leave the user with a hung, screen-less attach.
pub(crate) fn attach_with(runner: &dyn CommandRunner, name: &str) -> Result<(), PawError> {
    let status = runner
        .run_inheriting_stdio("tmux", &["attach-session", "-t", name])
        .map_err(|e| PawError::TmuxError(format!("failed to attach to tmux session: {e}")))?;

    if status.success {
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
    detach_client_with(&RealCommandRunner, session_name)
}

/// [`detach_client`] against an injected runner.
pub(crate) fn detach_client_with(
    runner: &dyn CommandRunner,
    session_name: &str,
) -> Result<(), PawError> {
    let output = runner
        .run("tmux", &["detach-client", "-s", session_name])
        .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;

    if output.success {
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
    kill_pane_with(&RealCommandRunner, session_name, pane_index)
}

/// [`kill_pane`] against an injected runner.
pub(crate) fn kill_pane_with(
    runner: &dyn CommandRunner,
    session_name: &str,
    pane_index: u32,
) -> Result<(), PawError> {
    let target = format!("{session_name}:0.{pane_index}");
    let output = runner
        .run("tmux", &["kill-pane", "-t", &target])
        .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;

    if output.success {
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
    kill_session_with(&RealCommandRunner, name)
}

/// [`kill_session`] against an injected runner.
pub(crate) fn kill_session_with(runner: &dyn CommandRunner, name: &str) -> Result<(), PawError> {
    let output = runner
        .run("tmux", &["kill-session", "-t", name])
        .map_err(|e| PawError::TmuxError(format!("failed to kill tmux session: {e}")))?;

    if output.success {
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
    kill_pane_by_id_with(&RealCommandRunner, pane_id)
}

/// [`kill_pane_by_id`] against an injected runner.
pub(crate) fn kill_pane_by_id_with(
    runner: &dyn CommandRunner,
    pane_id: &str,
) -> Result<(), PawError> {
    let output = runner
        .run("tmux", &["kill-pane", "-t", pane_id])
        .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;

    if output.success {
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
    list_panes_with_paths_with(&RealCommandRunner, session_name)
}

/// [`list_panes_with_paths`] against an injected runner, so the `-F` output
/// parse and the session-gone degradation are testable without a live server.
pub(crate) fn list_panes_with_paths_with(
    runner: &dyn CommandRunner,
    session_name: &str,
) -> Result<Vec<(String, String)>, PawError> {
    let output = runner
        .run(
            "tmux",
            &[
                "list-panes",
                "-t",
                session_name,
                "-F",
                "#{pane_id} #{pane_current_path}",
            ],
        )
        .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;

    if !output.success {
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

/// List every live pane in `session_name`'s first window as
/// `(pane_index, pane_current_path)` pairs via
/// `tmux list-panes -F '#{pane_index} #{pane_current_path}'`.
///
/// The index-valued sibling of [`list_panes_with_paths`]: a pane id (`%N`) is
/// stable across a layout change, which is exactly why it is the right key for
/// targeting a pane — but the per-repo discovery file records the pane INDEX,
/// so a caller rebuilding that file needs indices.
///
/// Returns an empty vector (not an error) when the session/server is gone, and
/// skips any line whose index does not parse, so a caller degrades rather than
/// failing on one odd pane.
pub fn list_panes_with_indices(session_name: &str) -> Result<Vec<(usize, String)>, PawError> {
    list_panes_with_indices_with(&RealCommandRunner, session_name)
}

/// [`list_panes_with_indices`] against an injected runner.
pub(crate) fn list_panes_with_indices_with(
    runner: &dyn CommandRunner,
    session_name: &str,
) -> Result<Vec<(usize, String)>, PawError> {
    let output = runner
        .run(
            "tmux",
            &[
                "list-panes",
                "-t",
                &format!("{session_name}:0"),
                "-F",
                "#{pane_index} #{pane_current_path}",
            ],
        )
        .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;

    if !output.success {
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
        let Some((idx, path)) = line.trim_end().split_once(' ') else {
            continue;
        };
        let Ok(idx) = idx.trim().parse::<usize>() else {
            continue;
        };
        let path = path.trim();
        if path.is_empty() {
            continue;
        }
        panes.push((idx, path.to_string()));
    }
    Ok(panes)
}

/// Resolve the tmux pane id (`%N`) whose `pane_current_path` matches
/// `worktree_path` in `session_name` (design D2, G2a).
///
/// The comparison is the core owner rule ([`resolve_agent`]): both the queried
/// `pane_current_path` and `worktree_path` are canonicalised so a symlinked temp root (e.g. macOS `/var` →
/// `/private/var`) does not defeat the match. Returns `Ok(None)` when no live
/// pane maps to the worktree (the agent's pane is already gone), so the caller
/// can treat removal as an idempotent no-op.
pub fn resolve_pane_id_for_worktree(
    session_name: &str,
    worktree_path: &Path,
) -> Result<Option<String>, PawError> {
    resolve_pane_id_for_worktree_with(&RealCommandRunner, session_name, worktree_path)
}

/// [`resolve_pane_id_for_worktree`] against an injected runner.
pub(crate) fn resolve_pane_id_for_worktree_with(
    runner: &dyn CommandRunner,
    session_name: &str,
    worktree_path: &Path,
) -> Result<Option<String>, PawError> {
    let roster = [("", worktree_path)];
    for (pane_id, path) in list_panes_with_paths_with(runner, session_name)? {
        if resolve_agent(Path::new(&path), roster).is_some() {
            return Ok(Some(pane_id));
        }
    }
    Ok(None)
}

/// Pure JSON↔tmux reconciliation (design D3, G2b): given the session-JSON
/// agents as `(branch, worktree_path)` pairs and the set of live pane
/// `pane_current_path` values, return the branches whose worktree maps to no
/// live pane.
///
/// Split from [`reconcile_agents_to_panes`] so the divergence logic is unit
/// testable without a live tmux server. Each comparison is the core owner rule
/// ([`resolve_agent`]: exact canonical-path equality, symlink-tolerant).
#[must_use]
pub fn agents_without_live_pane(
    agents: &[(String, PathBuf)],
    live_pane_paths: &[PathBuf],
) -> Vec<String> {
    agents
        .iter()
        .filter(|(branch, wt)| {
            let roster = [(branch.as_str(), wt.as_path())];
            !live_pane_paths
                .iter()
                .any(|live| resolve_agent(live, roster).is_some())
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
    reconcile_agents_to_panes_with(&RealCommandRunner, session_name, agents)
}

/// [`reconcile_agents_to_panes`] against an injected runner.
pub(crate) fn reconcile_agents_to_panes_with(
    runner: &dyn CommandRunner,
    session_name: &str,
    agents: &[(String, PathBuf)],
) -> Result<Vec<String>, PawError> {
    let live: Vec<PathBuf> = list_panes_with_paths_with(runner, session_name)?
        .into_iter()
        .map(|(_, path)| PathBuf::from(path))
        .collect();
    Ok(agents_without_live_pane(agents, &live))
}
