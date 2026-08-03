//! `git paw remove <branch>` — detach a single agent from a running
//! supervisor-mode session. Extracted verbatim from `main.rs`
//! (code-analysis-refactor R2c).
//!
//! `detach_worktree` (shared with the purge cluster, relocated in R2b) and
//! `write_repo_discovery_file` remain in `main.rs` and are referenced through
//! the crate root.

use git_paw::agents;
use git_paw::error::PawError;
use git_paw::git;
use git_paw::session::{self, SessionMode};
use git_paw::tmux;

use super::helpers::{agent_pane_offset, bare_mode_unsupported};
use crate::{detach_worktree, write_repo_discovery_file};

/// `git paw remove <branch>` — detach a single agent from a running session
/// (capability `remove-branch`).
#[allow(clippy::too_many_lines)]
pub(crate) fn cmd_remove(branch: &str, keep_worktree: bool, force: bool) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    // 5.2 Refuse `git paw remove supervisor` with a pointer to `git paw stop`.
    if branch == "supervisor" {
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

    if existing.mode == SessionMode::Bare {
        return Err(bare_mode_unsupported(&existing.session_name, "remove"));
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
    let _lock = git_paw::lock::SessionLock::acquire(&repo_root)?;

    let offset = agent_pane_offset(&existing);
    let session_alive = tmux::is_session_alive(&existing.session_name).unwrap_or(false);

    // 5.5 Kill the target tmux pane by RESOLVED pane id, not a JSON-position
    // index (design D2, G2a): map the removed branch's worktree to its live
    // pane via `pane_current_path` and kill that pane id, regardless of the
    // process running in it (a bare shell from a failed launch, a CLI, or
    // anything else). This never targets a different agent's pane even when a
    // stale orphan pane has shifted the grid (the v0.8.0 G2 failure), and is
    // an idempotent no-op when no pane maps (the pane is already gone).
    if session_alive
        && let Some(pane_id) =
            tmux::resolve_pane_id_for_worktree(&existing.session_name, &target.worktree_path)?
    {
        tmux::kill_pane_by_id(&pane_id)?;
    }

    // 5.6 Recompute layout_for(N-1) and re-apply so the grid re-flows.
    let remaining = existing.worktrees.len() - 1;
    if session_alive && remaining > 0 {
        let layout = git_paw::supervisor::layout::layout_for(remaining)?;
        tmux::build_remove_retile_commands(&existing.session_name, remaining, layout).execute()?;
        // Rebalance the re-flowed agent rows to equal width on the live window
        // (design D4, G3): tmux renumbered the survivors after the kill, so the
        // contiguous rows are resized to even columns for the new count.
        if let Err(e) = tmux::rebalance_agent_rows(&existing.session_name, remaining) {
            eprintln!("warning: could not rebalance agent-row widths: {e}");
        }
    }

    // 5.7 Delegate to detach_worktree for removal, unless --keep-worktree.
    if keep_worktree {
        println!(
            "Keeping worktree on disk: {}",
            target.worktree_path.display()
        );
    } else {
        detach_worktree(&repo_root, &target, &mut std::io::stderr());
    }

    // 5.8 Drop the branch/pane entry from the session JSON.
    let mut updated = existing.clone();
    updated.worktrees.remove(pos);
    session::save_session(&updated)?;
    write_repo_discovery_file(
        &repo_root,
        &updated.session_name,
        &updated.worktrees,
        offset,
    );

    println!(
        "Removed '{branch}' from session '{}'.",
        updated.session_name
    );
    Ok(())
}
