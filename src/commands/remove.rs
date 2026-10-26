//! `git paw remove <branch>` — detach a single agent from a running
//! grid session (bare or supervised), tearing its worktree down through the
//! same `detach_worktree` purge uses.

use crate::agents;
use crate::error::PawError;
use crate::git;
use crate::session;
use crate::surface::SessionLayout;
use crate::terminal::tmux::TmuxBackend;
use crate::terminal::{GridGeometry, TerminalBackend};
use crate::tmux;

use super::helpers::{agent_pane_offset, legacy_tiled_layout_unsupported};
use super::launch::write_repo_discovery_file;
use super::purge::BranchFate;
use super::purge::detach_worktree;
use super::{CommandContext, Output};

/// Options for [`remove`]: the `git paw remove` arguments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoveOptions {
    /// The agent's branch.
    pub branch: String,
    /// `--keep-worktree`: detach the pane, leave the worktree on disk.
    pub keep_worktree: bool,
    /// `--force`: remove despite uncommitted changes.
    pub force: bool,
}

/// What [`remove`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveOutcome {
    /// The session the agent left.
    pub session_name: String,
    /// The branch removed.
    pub branch: String,
    /// The fate of the branch when git-paw created it and the worktree was
    /// removed; `None` with `--keep-worktree` or for a pre-existing branch.
    pub branch_fate: Option<BranchFate>,
}

/// `git paw remove <branch>` — detach a single agent from a running session
/// (capability `remove-branch`).
///
/// # Errors
///
/// Returns an error when the branch is the supervisor or not an agent, the
/// worktree has uncommitted changes (without `force` / `keep_worktree`), or a
/// tmux or session-state operation fails.
#[allow(clippy::too_many_lines)]
pub fn remove(
    ctx: &CommandContext,
    opts: RemoveOptions,
    out: &mut dyn Output,
) -> Result<RemoveOutcome, PawError> {
    let RemoveOptions {
        branch,
        keep_worktree,
        force,
    } = opts;
    let branch = branch.as_str();
    let repo_root = ctx.repo_root().to_path_buf();

    // 5.2 Refuse `git paw remove supervisor` with a pointer to `git paw stop`.
    // Wire identity: this names the CLI argument the user typed, not the role
    // of a session surface.
    if branch == crate::surface::SUPERVISOR_AGENT_ID {
        return Err(PawError::SessionError(
            "refusing to remove the supervisor. To end the whole session, run `git paw stop` \
             (or `git paw purge` to also remove worktrees)."
                .to_string(),
        ));
    }

    // 5.1 Resolve the active session and locate the target branch.
    let Some(existing) = session::find_session_for_repo(&repo_root)? else {
        return Err(PawError::SessionError(
            "no active session for this repository.".to_string(),
        ));
    };

    // As in `add`: every grid session is hot-attachable, and only a live
    // session built with the earlier tiled layout is refused — before the
    // pane is killed, the worktree detached or the session JSON rewritten.
    let session_layout = SessionLayout::of(&existing);
    if session_layout.is_legacy_tiled() {
        return Err(legacy_tiled_layout_unsupported(
            &existing.session_name,
            "remove",
        ));
    }

    let Some(pos) = existing.worktrees.iter().position(|w| w.branch == branch) else {
        let live: Vec<&str> = existing
            .worktrees
            .iter()
            .map(|w| w.branch.as_str())
            .collect();
        return Err(PawError::SessionError(format!(
            "branch '{branch}' is not an agent in session '{}'. Live agents: {}.",
            existing.session_name,
            if live.is_empty() {
                "(none)".to_string()
            } else {
                live.join(", ")
            }
        )));
    };
    let target = existing.worktrees[pos].clone();

    // 5.3 Uncommitted-work safety check (D7) — unless --force or --keep-worktree.
    if !force && !keep_worktree {
        let dirty = git::uncommitted_files(&target.worktree_path).unwrap_or_default();
        // Filter out git-paw's own managed/injected files (the gitignored
        // sidecar and any residual managed `AGENTS.md` block). Only genuine
        // user work should block removal — a just-started worktree whose only
        // dirt is git-paw's injection is treated as clean (see
        // `agents::is_managed_path`). The refusal message lists only the
        // residual user files, so the user is never told to commit git-paw's
        // own bookkeeping.
        let residual: Vec<String> = dirty
            .into_iter()
            .filter(|f| !agents::is_managed_path(&target.worktree_path, f))
            .collect();
        if !residual.is_empty() {
            let list = residual
                .iter()
                .map(|f| format!("  {f}"))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(PawError::SessionError(format!(
                "worktree for '{branch}' has uncommitted changes:\n{list}\n\n\
                 Commit them first, or pass --force to remove anyway (the changes will be lost), \
                 or --keep-worktree to detach the pane and leave the worktree on disk."
            )));
        }
    }

    tmux::ensure_tmux_installed()?;

    // 5.4 Take the advisory lock for the mutate-the-session section.
    let _lock = crate::lock::SessionLock::acquire(&repo_root)?;

    let offset = agent_pane_offset(&existing);
    let backend = TmuxBackend::new();
    let session_alive = backend
        .session_alive(&existing.session_name)
        .unwrap_or(false);

    // 5.5 Kill the target tmux pane by RESOLVED pane id, not a JSON-position
    // index (design D2, G2a): map the removed branch's worktree to its live
    // pane via `pane_current_path` and kill that pane id, regardless of the
    // process running in it (a bare shell from a failed launch, a CLI, or
    // anything else). This never targets a different agent's pane even when a
    // stale orphan pane has shifted the grid (the v0.8.0 G2 failure), and is
    // an idempotent no-op when no pane maps (the pane is already gone).
    if session_alive
        && let Some(surface) =
            backend.surface_owned_by(&existing.session_name, &target.worktree_path)?
    {
        backend.kill(&surface)?;
    }

    // 5.6 Recompute layout_for(N-1) and re-apply so the grid re-flows.
    let remaining = existing.worktrees.len() - 1;
    if session_alive && remaining > 0 {
        let grid = GridGeometry {
            first_agent: offset,
            agent_count: remaining,
            has_top_row: SessionLayout::of(&existing).has_top_row(),
        };
        backend.reflow_rows(&existing.session_name, &grid)?;
        // Rebalance the re-flowed agent rows to equal width on the live window
        // (design D4, G3): tmux renumbered the survivors after the kill, so the
        // contiguous rows are resized to even columns for the new count.
        if let Err(e) = backend.layout(&existing.session_name, &grid) {
            out.warn(&format!(
                "warning: could not rebalance agent-row widths: {e}"
            ));
        }
    }

    // 5.7 Delegate to detach_worktree for removal, unless --keep-worktree. The
    // helper runs the consumer's `on_remove` hook before deleting the directory;
    // `--keep-worktree` skips both, since the worktree — and so whatever the
    // hook would tear down — is deliberately being left in place.
    let branch_fate = if keep_worktree {
        out.line(&format!(
            "Keeping worktree on disk: {}",
            target.worktree_path.display()
        ));
        None
    } else {
        let worktree_runtime = ctx.config_or_default().worktree_runtime();
        detach_worktree(&repo_root, &target, worktree_runtime.on_remove_hook(), out)
    };

    // 5.8 Drop the branch/pane entry from the session JSON. This also releases
    // the agent's runtime port block: the slot lives on the entry, and
    // `worktree_provision::allocate_slot` reads the live roster, so the next
    // `git paw add` reuses the freed slot instead of climbing past it. Nothing
    // else has to be cleaned up — do not start caching slots elsewhere, or the
    // free list silently stops working.
    let mut updated = existing.clone();
    updated.worktrees.remove(pos);
    session::save_session(&updated)?;
    write_repo_discovery_file(
        &repo_root,
        &updated.session_name,
        &updated.worktrees,
        offset,
        out,
    );

    out.line(&format!(
        "Removed '{branch}' from session '{}'.",
        updated.session_name
    ));
    Ok(RemoveOutcome {
        session_name: updated.session_name,
        branch: branch.to_string(),
        branch_fate,
    })
}
