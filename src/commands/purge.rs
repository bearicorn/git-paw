//! `git paw purge` — tear down the session's tmux session, worktrees and
//! state, and the shared per-worktree teardown (`detach_worktree`) that
//! `git paw remove` and stale-receipt invalidation reuse. Branches git-paw
//! created are deleted only when merged; anything else is kept.
//!
//! The confirmation is the caller's: [`preview`] reports what a purge would
//! keep (and prints the unmerged-branch warning), the caller asks, and
//! [`purge`] acts on the decision carried in [`PurgeOptions`].

use std::path::{Path, PathBuf};

use crate::agents;
use crate::config;
use crate::error::PawError;
use crate::git;
use crate::session::{self, Session, WorktreeEntry};
use crate::tmux;

use super::{CommandContext, Output};

/// Options for [`purge`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PurgeOptions {
    /// `--force`: purge without a confirmation. Branches with unmerged
    /// commits are still kept (`purge-unmerged-branch-safety`).
    pub force: bool,
    /// The caller's confirmation, when it asked for one. Without `force` or
    /// this, the purge is cancelled.
    pub confirmed: bool,
}

/// What a purge would keep, reported before the caller asks for confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurgePreview {
    /// The session a purge would remove.
    pub session_name: String,
    /// The branches it would keep because they carry unmerged commits (or
    /// their merge state cannot be determined).
    pub retained: Vec<RetainedBranch>,
    /// The confirmation question for this purge.
    pub prompt: String,
}

/// What became of a branch git-paw created, once its worktree was removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchFate {
    /// The branch was merged and has been deleted.
    Deleted {
        /// The branch.
        branch: String,
        /// Its tip before deletion (short SHA, or `unknown`).
        tip: String,
    },
    /// The branch was kept.
    Retained {
        /// The branch.
        branch: String,
        /// Its tip (short SHA, or `unknown`).
        tip: String,
        /// Why it was kept.
        reason: String,
    },
}

/// What [`purge`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PurgeOutcome {
    /// The repository has no session.
    NoSession,
    /// Neither forced nor confirmed: nothing was touched.
    Cancelled,
    /// The session, its worktrees and its state were removed.
    Purged {
        /// The session.
        session_name: String,
        /// Every branch git-paw created, deleted or retained with its tip.
        branches: Vec<BranchFate>,
    },
}

/// Options for [`purge_stale`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PurgeStaleOptions {
    /// Restrict the sweep to this repository; `None` sweeps every repository
    /// on the machine (`--all-repos`).
    pub repo_root: Option<PathBuf>,
}

/// What [`purge_stale`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PurgeStaleOutcome {
    /// The stale sessions that were purged.
    pub purged: Vec<String>,
}

/// The confirmation question for a purge that keeps `retained`.
fn confirmation_prompt(retained: &[RetainedBranch]) -> String {
    if retained.is_empty() {
        "This will remove the tmux session, all worktrees, and session state. Continue?"
    } else {
        "Worktrees (including any uncommitted changes) and session state will be removed; \
         the branches listed above are kept. Continue?"
    }
    .to_string()
}

/// Reports what purging the repository's session would keep, writing the
/// unmerged-branch warning to `out` — the step before the caller asks for
/// confirmation. Touches nothing.
///
/// Returns `None` (after saying so) when the repository has no session.
///
/// # Errors
///
/// Returns an error when the session state cannot be read.
pub fn preview(
    ctx: &CommandContext,
    out: &mut dyn Output,
) -> Result<Option<PurgePreview>, PawError> {
    let Some(existing) = session::find_session_for_repo(ctx.repo_root())? else {
        out.line("No session to purge for this repo.");
        return Ok(None);
    };
    let retained = warn_unmerged(ctx.repo_root(), &existing, out);
    Ok(Some(PurgePreview {
        session_name: existing.session_name,
        prompt: confirmation_prompt(&retained),
        retained,
    }))
}

/// Removes everything: tmux session, worktrees, and state — when
/// `opts.force` or `opts.confirmed` says so; otherwise reports the purge
/// cancelled and touches nothing.
///
/// The unmerged-branch warning is [`preview`]'s; call it first when the
/// caller asks for confirmation.
///
/// # Errors
///
/// Returns an error when the session state cannot be read or deleted, or tmux
/// cannot kill the session.
pub fn purge(
    ctx: &CommandContext,
    opts: PurgeOptions,
    out: &mut dyn Output,
) -> Result<PurgeOutcome, PawError> {
    let repo_root = ctx.repo_root();
    let Some(existing) = session::find_session_for_repo(repo_root)? else {
        out.line("No session to purge for this repo.");
        return Ok(PurgeOutcome::NoSession);
    };
    if !(opts.force || opts.confirmed) {
        out.line("Purge cancelled.");
        return Ok(PurgeOutcome::Cancelled);
    }
    let sessions_dir = session::session_state_dir()?;
    let branches = teardown(
        repo_root,
        &sessions_dir,
        &existing,
        &mut kill_live_tmux,
        out,
    )?;
    out.line(&format!("Purged session '{}'.", existing.session_name));
    Ok(PurgeOutcome::Purged {
        session_name: existing.session_name,
        branches,
    })
}

/// Kills the tmux session `name` when it is alive.
fn kill_live_tmux(name: &str) -> Result<(), PawError> {
    if tmux::is_session_alive(name)? {
        tmux::kill_session(name)?;
    }
    Ok(())
}

/// Probes the existing receipt for staleness and, when stale (the receipt
/// claims active but the tmux session is gone), invalidates it — removing the
/// recorded worktrees and the receipt equivalent to `purge --force` — and emits
/// a notice naming the entry (design D5).
///
/// Like every other teardown path it goes through `detach_worktree`, so a
/// branch is deleted only when git-paw created it and it is already contained
/// in the default branch or the branch it was created from; anything else is
/// kept.
///
/// Returns `true` when invalidation fired; the caller SHALL then proceed with
/// a fresh launch as if no prior session existed. Returns `false` for a live
/// (`active` + alive), paused, stopped, or indeterminate receipt, leaving the
/// caller's normal reattach/recover decision intact.
pub(crate) fn invalidate_if_stale(
    repo_root: &Path,
    existing: &Session,
    out: &mut dyn Output,
) -> Result<bool, PawError> {
    let liveness = tmux::session_liveness(&existing.session_name);
    if session::DisplayStatus::from_receipt(&existing.status, liveness)
        != session::DisplayStatus::Stale
    {
        return Ok(false);
    }

    let when = existing
        .created_at_iso8601()
        .map(|t| format!(", last seen {t}"))
        .unwrap_or_default();
    out.warn(&format!(
        "notice: removed stale session receipt\n  ({}{}, tmux session no longer exists)",
        existing.session_name, when
    ));

    let sessions_dir = session::session_state_dir()?;
    let mut confirm = |_: &str| -> Result<bool, PawError> { Ok(true) };
    purge_with_prompt(
        repo_root,
        &sessions_dir,
        existing,
        true,
        &mut confirm,
        &mut kill_live_tmux,
        out,
    )?;
    Ok(true)
}

/// Purges only stale sessions (receipt claims active but the tmux session is
/// gone), scoped to `opts.repo_root` when set, else to every repository on
/// the machine. Live sessions are always left untouched.
///
/// Stale is defined exactly as [`session::DisplayStatus::Stale`]: an `active`
/// receipt whose `tmux has-session` probe returns
/// [`tmux::SessionLiveness::Stale`]. Stopped receipts (intentionally stopped)
/// and sessions on a host with no tmux binary (Indeterminate probe) are NOT
/// touched — the fail-safe guarantee: a receipt is purged only when the probe
/// positively confirms staleness, never on a live or indeterminate liveness
/// read. Reports "nothing to purge" when no stale receipt is in scope. A
/// stale entry is orphaned, so nothing is confirmed.
///
/// # Errors
///
/// Returns an error when the session state cannot be read or deleted.
pub fn purge_stale(
    opts: PurgeStaleOptions,
    out: &mut dyn Output,
) -> Result<PurgeStaleOutcome, PawError> {
    let sessions_dir = session::session_state_dir()?;
    let all = session::load_all_sessions_in(&sessions_dir)?;

    let PurgeStaleOptions { repo_root } = opts;
    let stale: Vec<session::Session> = all
        .into_iter()
        .filter(|s| {
            if let Some(root) = &repo_root
                && !session::session_belongs_to_repo(s, root)
            {
                return false;
            }
            let liveness = tmux::session_liveness(&s.session_name);
            session::DisplayStatus::from_receipt(&s.status, liveness)
                == session::DisplayStatus::Stale
        })
        .collect();

    let mut outcome = PurgeStaleOutcome::default();
    if stale.is_empty() {
        out.line("No stale sessions to purge.");
        return Ok(outcome);
    }

    let mut confirm = |_: &str| -> Result<bool, PawError> { Ok(true) };
    for session_entry in &stale {
        // force = true: stale entries are orphaned, nothing to confirm.
        let result = purge_with_prompt(
            &session_entry.repo_path,
            &sessions_dir,
            session_entry,
            true,
            &mut confirm,
            &mut kill_live_tmux,
            out,
        )?;
        if result == PromptResult::Purged {
            out.line(&format!(
                "Purged stale session '{}'.",
                session_entry.session_name
            ));
            outcome.purged.push(session_entry.session_name.clone());
        }
    }
    Ok(outcome)
}

/// Tears down a single agent's worktree: runs the consumer's `on_remove` hook,
/// removes the worktree directory (with the per-worktree
/// `Removing worktree ...` / `...done (Xs)` progress markers) and, when git-paw
/// created the branch, deletes it afterwards.
///
/// Extracted from `purge`'s per-worktree loop (design D6, task 1.3) so
/// `remove` performs byte-identical removal for a single agent that
/// `git paw purge` performs for every agent. Both teardown paths therefore run
/// the hook by construction — `on_remove` lives here rather than being repeated
/// at each command handler. Best-effort throughout: a failed hook, a failed
/// worktree-remove, or a failed branch-delete is surfaced as a `warning:` on
/// `stderr` and does not abort — a resource git-paw could not drop must never
/// cost the operator a stranded worktree.
///
/// `on_remove` is the configured `[worktree.hooks] on_remove` command, or `None`
/// when the consumer declared no hook (in which case nothing is run).
///
/// Returns the fate of the branch when git-paw created it, `None` otherwise.
pub(crate) fn detach_worktree(
    repo_root: &Path,
    entry: &WorktreeEntry,
    on_remove: Option<&str>,
    out: &mut dyn Output,
) -> Option<BranchFate> {
    // Before the directory is deleted, so the hook can still read the checkout.
    if let Some(command) = on_remove
        && let Some(warning) =
            crate::worktree_provision::run_remove_hook(command, &entry.branch, &entry.worktree_path)
    {
        out.warn(&format!("warning: {warning}"));
    }

    out.warn(&format!(
        "Removing worktree {}...",
        entry.worktree_path.display()
    ));
    let started = std::time::Instant::now();
    let result = git::remove_worktree(repo_root, &entry.worktree_path);
    let elapsed_secs = started.elapsed().as_secs_f64();
    match result {
        Ok(()) => {
            out.warn(&format!("  ...done ({elapsed_secs:.1}s)"));
        }
        Err(e) => {
            out.warn(&format!(
                "warning: failed to remove worktree '{}' after {:.1}s: {e}",
                entry.worktree_path.display(),
                elapsed_secs
            ));
        }
    }

    entry
        .branch_created
        .then(|| retire_created_branch(repo_root, entry, out))
}

/// The start point to record on a worktree entry git-paw just created a branch
/// for — the main checkout's current branch, which is what
/// `git worktree add -b <branch> <path>` branches from.
///
/// `None` when the branch already existed (there is no git-paw-chosen start
/// point to record) or when `HEAD` is detached. Teardown treats a missing value
/// as "compare against the default branch only", which is the pre-existing
/// behaviour, so an unrecorded entry is never less safe.
pub(crate) fn created_branch_base(repo_root: &Path, branch_created: bool) -> Option<String> {
    if !branch_created {
        return None;
    }
    git::current_branch(repo_root).ok()
}

/// Where a branch's commits stand relative to the branches it could have
/// landed in.
pub(crate) enum MergeState {
    /// Contained in the default branch, or in the branch it was created from.
    Merged,
    /// Carries commits that are in neither. `count` is `None` when the
    /// comparison succeeded but counting the commits did not.
    Unmerged {
        count: Option<usize>,
        compared_against: String,
    },
    /// No comparison could be made; the wrapped text says why.
    Undeterminable(String),
}

/// Compares `entry`'s branch against the default branch and, when one was
/// recorded, the branch it was created from.
///
/// Both comparisons matter because `git worktree add -b` takes no start point:
/// a branch git-paw created starts at whatever the main checkout had checked
/// out. Launched from an integration branch, an agent branch that has been
/// merged *into that integration branch* is finished work, but it is not yet an
/// ancestor of the default branch — comparing against the default alone would
/// keep it and report the entire integration backlog as its "unmerged" count.
/// Containment in **either** branch therefore means merged, and the count is
/// taken against the recorded base when there is one, so the number describes
/// the branch's own commits.
///
/// Fails closed: when every comparison errors the result is
/// [`MergeState::Undeterminable`], which callers treat as "retain".
pub(crate) fn branch_merge_state(
    repo_root: &Path,
    entry: &WorktreeEntry,
    default_branch: Option<&str>,
) -> MergeState {
    // The recorded base first: it is the more specific question, and its label
    // is the one a count should be reported against.
    let mut targets: Vec<&str> = Vec::new();
    if let Some(base) = entry.base_branch.as_deref() {
        targets.push(base);
    }
    if let Some(default) = default_branch
        && !targets.contains(&default)
    {
        targets.push(default);
    }

    let mut first_comparable: Option<&str> = None;
    let mut failures = Vec::new();
    for target in targets {
        match git::branch_is_merged_into(repo_root, &entry.branch, target) {
            Ok(true) => return MergeState::Merged,
            Ok(false) => first_comparable.get_or_insert(target),
            Err(e) => {
                failures.push(format!("{target}: {e}"));
                continue;
            }
        };
    }

    // At least one target answered "not contained" — report against the first
    // one, which is the recorded base when there is one. If none answered at
    // all, nothing is known and the branch must be kept.
    let Some(compared_against) = first_comparable else {
        return MergeState::Undeterminable(if failures.is_empty() {
            "the default branch could not be resolved and no base branch was recorded".to_string()
        } else {
            failures.join("; ")
        });
    };

    MergeState::Unmerged {
        count: git::unmerged_commit_count(repo_root, &entry.branch, compared_against).ok(),
        compared_against: compared_against.to_string(),
    }
}

/// `-d` when git would accept the safe delete for `branch`, `-D` otherwise.
///
/// `git branch -d` refuses unless the branch is contained in the checked-out
/// branch, so suggesting it blindly hands the operator a command that fails.
/// Suggesting `-D` blindly is worse: it is the irreversible form, and an
/// operator who pastes it loses the branch even in the cases where git would
/// have vetoed the delete. The check is the same containment question git
/// itself asks.
pub(crate) fn discard_flag(repo_root: &Path, branch: &str) -> &'static str {
    let safe = git::current_branch(repo_root)
        .ok()
        .and_then(|head| git::branch_is_merged_into(repo_root, branch, &head).ok())
        .unwrap_or(false);
    if safe { "-d" } else { "-D" }
}

/// Decides the fate of a branch git-paw created, once its worktree is gone.
///
/// `git branch -D` is the only irreversible operation in the teardown: it drops
/// the branch reflog with the ref, and the worktree whose `HEAD` reflog was the
/// other recovery path has just been removed. So the deletion is gated on the
/// read-only check in [`branch_merge_state`] — the branch is deleted only when
/// its tip is an ancestor of the default branch or of the branch it was created
/// from.
///
/// The gate fails **closed**: a branch carrying commits in neither, a branch
/// whose comparison errors, and a branch in a repository whose default branch
/// cannot be resolved are all retained. Every outcome is printed with the SHA
/// that matters — `Deleted branch <name> (was <sha>)` on deletion, and on
/// retention the tip plus how to resume (`git worktree add`) or discard
/// (`git branch -d` / `-D`) the branch by hand. The returned fate carries the
/// same tip; a failed delete is reported as retained.
pub(crate) fn retire_created_branch(
    repo_root: &Path,
    entry: &WorktreeEntry,
    out: &mut dyn Output,
) -> BranchFate {
    let tip =
        git::branch_tip_short(repo_root, &entry.branch).unwrap_or_else(|_| "unknown".to_string());

    let default_branch = git::default_branch(repo_root).ok();
    let retention_reason = match branch_merge_state(repo_root, entry, default_branch.as_deref()) {
        MergeState::Merged => None,
        MergeState::Unmerged {
            count: Some(count),
            compared_against,
        } => Some(format!("{count} commit(s) not in {compared_against}")),
        MergeState::Unmerged {
            count: None,
            compared_against,
        } => Some(format!(
            "commits not in {compared_against}; count unavailable"
        )),
        MergeState::Undeterminable(why) => {
            Some(format!("merge state could not be determined: {why}"))
        }
    };

    if let Some(reason) = retention_reason {
        out.warn(&format!("Kept branch {} at {tip} ({reason})", entry.branch));
        out.warn(&format!(
            "  resume it with: git worktree add '{}' {}",
            entry.worktree_path.display(),
            entry.branch
        ));
        out.warn(&format!(
            "  discard it with: git branch {} {}",
            discard_flag(repo_root, &entry.branch),
            entry.branch
        ));
        return BranchFate::Retained {
            branch: entry.branch.clone(),
            tip,
            reason,
        };
    }

    match git::delete_branch(repo_root, &entry.branch) {
        Ok(()) => {
            out.warn(&format!("Deleted branch {} (was {tip})", entry.branch));
            BranchFate::Deleted {
                branch: entry.branch.clone(),
                tip,
            }
        }
        Err(e) => {
            out.warn(&format!(
                "warning: failed to delete branch '{}': {e}",
                entry.branch
            ));
            BranchFate::Retained {
                branch: entry.branch.clone(),
                tip,
                reason: format!("failed to delete: {e}"),
            }
        }
    }
}

/// Whether [`purge_with_prompt`] tore the session down.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PromptResult {
    Purged,
    Cancelled,
}

/// The purge sequence with its confirmation as a callback: the
/// unmerged-commits warning ([`warn_unmerged`]), then `confirm` unless `force`
/// is set, then [`teardown`]. Stale-receipt invalidation and `purge --stale`
/// run it with a confirmation that always answers yes.
pub(crate) fn purge_with_prompt(
    repo_root: &Path,
    sessions_dir: &Path,
    session: &Session,
    force: bool,
    confirm: &mut dyn FnMut(&str) -> Result<bool, PawError>,
    kill_tmux: &mut dyn FnMut(&str) -> Result<(), PawError>,
    out: &mut dyn Output,
) -> Result<PromptResult, PawError> {
    let unmerged = warn_unmerged(repo_root, session, out);
    if !force && !confirm(&confirmation_prompt(&unmerged))? {
        return Ok(PromptResult::Cancelled);
    }
    teardown(repo_root, sessions_dir, session, kill_tmux, out)?;
    Ok(PromptResult::Purged)
}

/// Writes the unmerged-commits warning for `session`'s branches to `out` —
/// before any confirmation is asked — and returns the branches a purge keeps.
pub(crate) fn warn_unmerged(
    repo_root: &Path,
    session: &Session,
    out: &mut dyn Output,
) -> Vec<RetainedBranch> {
    // The single default-branch resolver (`git-operations`), not a purge-local
    // lookup: a purge-local fallback to a literal `main` made every comparison
    // fail in a `master`-default repository without `origin/HEAD`, and the
    // branches were deleted without a warning. When the resolver itself fails,
    // nothing is comparable, so every branch is reported as undeterminable and
    // `retire_created_branch` keeps all of them.
    let default_branch = git::default_branch(repo_root);
    if let Err(ref e) = default_branch {
        out.warn(&format!(
            "Warning: the default branch could not be resolved ({e}) — no branch will be deleted."
        ));
    }
    let unmerged = collect_unmerged_branches(repo_root, session, default_branch.as_deref().ok());

    if !unmerged.is_empty() {
        out.warn(&format!(
            "Warning: {} branch(es) have unmerged commits and will be kept:",
            unmerged.len()
        ));
        for retained in &unmerged {
            match (retained.unmerged_count, &retained.compared_against) {
                (Some(count), Some(target)) => {
                    out.warn(&format!(
                        "  {} ({}): {count} commit(s) not in {target}",
                        retained.branch, retained.tip
                    ));
                }
                (None, Some(target)) => {
                    out.warn(&format!(
                        "  {} ({}): commits not in {target}; count unavailable",
                        retained.branch, retained.tip
                    ));
                }
                (_, None) => {
                    out.warn(&format!(
                        "  {} ({}): merge state could not be determined",
                        retained.branch, retained.tip
                    ));
                }
            }
        }
        out.warn(
            "Their worktrees (including any uncommitted changes) and the session state \
             will be removed; the branches themselves are kept.",
        );
        // The whole warning block is written before this returns, so it lands
        // on the terminal before the caller's confirmation prompt reads stdin
        // (Bug C in v0-5-0-audit-cleanup: a warning still draining could race
        // the prompt's read and swallow the `y` + Enter).
    }
    unmerged
}

/// Tears the session down — tmux, worktrees, branches git-paw created (merged
/// ones only), the session file and the discovery file — and returns each
/// created branch's fate.
pub(crate) fn teardown(
    repo_root: &Path,
    sessions_dir: &Path,
    session: &Session,
    kill_tmux: &mut dyn FnMut(&str) -> Result<(), PawError>,
    out: &mut dyn Output,
) -> Result<Vec<BranchFate>, PawError> {
    kill_tmux(&session.session_name)?;

    // Per-worktree teardown (`on_remove` hook + worktree-remove + branch
    // cleanup), delegated to the shared `detach_worktree` helper so `remove`
    // performs the exact same per-worktree removal `purge` does (design D6, task
    // 1.3). The helper emits the per-worktree begin / `...done (Xs)` progress
    // markers Bug D in v0-5-0-audit-cleanup added. The hook is resolved once
    // rather than per entry — purge tears down every agent, and re-reading the
    // config in the loop would buy nothing. A config that cannot be read leaves
    // it unset, so an unreadable config never blocks a purge.
    let worktree_runtime = config::load_config(repo_root, None)
        .unwrap_or_default()
        .worktree_runtime();
    let on_remove = worktree_runtime.on_remove_hook();
    let mut fates = Vec::new();
    for entry in &session.worktrees {
        fates.extend(detach_worktree(repo_root, entry, on_remove, out));
    }

    if let Some(ref log_path) = session.broker_log_path {
        let _ = std::fs::remove_file(log_path);
    }

    // Bug E (v0-5-0-audit-cleanup §9c) — strip the supervisor-pane boot
    // block from AGENTS.md so it does not accumulate across sessions.
    // Idempotent: missing block / missing AGENTS.md is a no-op.
    if let Err(e) = agents::remove_session_boot_block(repo_root) {
        out.warn(&format!(
            "warning: failed to clean session boot block from AGENTS.md: {e}"
        ));
    }

    // Deleting the session file releases every worktree's runtime port block
    // along with its entry — the slots live on the entries and
    // `worktree_provision::allocate_slot` reads the live roster, so the next
    // session starts from slot 0 again.
    session::delete_session_in(&session.session_name, sessions_dir)?;

    // Remove the per-repo discovery file sweep.sh reads (capability
    // `session-json-location`). Idempotent — a missing file is not an error.
    if let Err(e) = session::remove_repo_session_file(repo_root, &session.session_name) {
        out.warn(&format!(
            "warning: failed to remove per-repo session discovery file: {e}"
        ));
    }

    Ok(fates)
}

/// A session branch that purge will report as unmerged, and therefore keep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedBranch {
    /// The branch name as recorded in the session file.
    pub branch: String,
    /// Short SHA of the branch tip, or `"unknown"` if it could not be read.
    pub tip: String,
    /// The branch the count is against — the recorded base when the entry has
    /// one, else the default branch. `None` when no comparison could be made.
    pub compared_against: Option<String>,
    /// `Some(n)` — `n` commits are not in `compared_against`; `None` — the
    /// count could not be taken.
    pub unmerged_count: Option<usize>,
}

/// The session's branches that are contained in neither the default branch nor
/// the branch they were created from.
///
/// `default_branch` is `None` when the resolver itself failed; an entry with no
/// recorded base is then uncomparable and is reported as such. A comparison
/// that errors is likewise reported rather than skipped: the old
/// `continue`-on-failure silently classified an uncomparable branch as merged,
/// which is the direction that loses work.
pub(crate) fn collect_unmerged_branches(
    repo_root: &Path,
    session: &Session,
    default_branch: Option<&str>,
) -> Vec<RetainedBranch> {
    let mut out = Vec::new();
    for entry in &session.worktrees {
        if Some(entry.branch.as_str()) == default_branch {
            continue;
        }
        let (compared_against, unmerged_count) =
            match branch_merge_state(repo_root, entry, default_branch) {
                MergeState::Merged => continue,
                MergeState::Unmerged {
                    count,
                    compared_against,
                } => (Some(compared_against), count),
                MergeState::Undeterminable(_) => (None, None),
            };
        let tip = git::branch_tip_short(repo_root, &entry.branch)
            .unwrap_or_else(|_| "unknown".to_string());
        out.push(RetainedBranch {
            branch: entry.branch.clone(),
            tip,
            compared_against,
            unmerged_count,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::CaptureOutput;

    /// Everything written to the sink's warning stream, one line each.
    fn warn_text(sink: &CaptureOutput) -> String {
        sink.warnings.iter().fold(String::new(), |mut text, line| {
            text.push_str(line);
            text.push('\n');
            text
        })
    }
    use crate::session::SessionStatus;
    use serial_test::serial;
    use std::path::PathBuf;
    use std::process::Command as StdCommand;
    use std::time::UNIX_EPOCH;
    use tempfile::TempDir;

    // --- purge UX ---

    /// Repo sandbox with a committed `main`, one feature worktree, and a
    /// session file persisted into a sibling sessions dir. Everything is under
    /// a single `TempDir` so drop cleans up.
    struct PurgeSandbox {
        _sandbox: TempDir,
        repo: PathBuf,
        sessions_dir: PathBuf,
        session: Session,
    }

    fn git(dir: &Path, args: &[&str]) {
        let out = StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git spawn");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// The tip SHA of `branch`, or `None` when the branch does not exist.
    fn branch_tip(repo: &Path, branch: &str) -> Option<String> {
        let out = StdCommand::new("git")
            .current_dir(repo)
            .args(["rev-parse", "--verify", "--quiet", branch])
            .output()
            .expect("git spawn");
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn setup_purge_sandbox(with_unmerged_commit: bool) -> PurgeSandbox {
        setup_purge_sandbox_on("main", with_unmerged_commit)
    }

    /// `setup_purge_sandbox` with an explicit default-branch name, so the
    /// `master`-default / no-`origin/HEAD` path can be exercised.
    fn setup_purge_sandbox_on(default_branch: &str, with_unmerged_commit: bool) -> PurgeSandbox {
        let sandbox = TempDir::new().expect("tempdir");
        let repo = sandbox.path().join("repo");
        std::fs::create_dir(&repo).expect("mkdir repo");
        let sessions_dir = sandbox.path().join("sessions");

        git(&repo, &["init", "-q", "-b", default_branch]);
        git(&repo, &["config", "user.email", "test@test.com"]);
        git(&repo, &["config", "user.name", "Test"]);
        std::fs::write(repo.join("README.md"), "# test").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "initial"]);

        let wt = git::create_worktree(
            &repo,
            "feature/test",
            false,
            crate::config::WorktreePlacement::Sibling,
        )
        .expect("create worktree");
        if with_unmerged_commit {
            std::fs::write(wt.path.join("new.txt"), "hello").unwrap();
            git(&wt.path, &["add", "."]);
            git(&wt.path, &["commit", "-q", "-m", "feature work"]);
        }

        let session = Session {
            session_name: "paw-repo".to_string(),
            repo_path: repo.clone(),
            project_name: "repo".to_string(),
            created_at: UNIX_EPOCH,
            status: SessionStatus::Stopped,
            worktrees: vec![WorktreeEntry {
                branch: "feature/test".to_string(),
                worktree_path: wt.path.clone(),
                cli: "claude".to_string(),
                branch_created: wt.branch_created,
                base_branch: created_branch_base(&repo, wt.branch_created),
                pending_boot_prompt: None,
                pending_entry_point: None,
                runtime_slot: None,
                cli_session_id: None,
                spec_id: None,
            }],
            broker_port: None,
            broker_bind: None,
            broker_log_path: None,
            mode: crate::session::SessionMode::Bare,
            dashboard_pane: None,
            layout: None,
            broker_authority: None,
        };
        session::save_session_in(&session, &sessions_dir).expect("save session");

        PurgeSandbox {
            _sandbox: sandbox,
            repo,
            sessions_dir,
            session,
        }
    }

    /// A sandbox whose main checkout sits on an integration branch that is
    /// ahead of the default branch, with one agent worktree branched off it —
    /// the shape `git paw start` produces when the operator launches a wave
    /// from an integration branch rather than from `main`.
    ///
    /// When `merged_into_integration` the agent's commit is merged back into
    /// the integration branch before the session struct is built.
    fn setup_integration_sandbox(merged_into_integration: bool) -> PurgeSandbox {
        let sandbox = TempDir::new().expect("tempdir");
        let repo = sandbox.path().join("repo");
        std::fs::create_dir(&repo).expect("mkdir repo");
        let sessions_dir = sandbox.path().join("sessions");

        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["config", "user.email", "test@test.com"]);
        git(&repo, &["config", "user.name", "Test"]);
        std::fs::write(repo.join("README.md"), "# test").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "initial"]);

        // The integration branch runs ahead of main by two commits — exactly
        // the backlog a default-branch-only comparison would miscount as the
        // agent's own work.
        git(&repo, &["checkout", "-q", "-b", "integration"]);
        for n in 1..=2 {
            std::fs::write(repo.join(format!("integration-{n}.txt")), "x").unwrap();
            git(&repo, &["add", "."]);
            git(&repo, &["commit", "-q", "-m", &format!("integration {n}")]);
        }

        let wt = git::create_worktree(
            &repo,
            "feature/test",
            false,
            crate::config::WorktreePlacement::Sibling,
        )
        .expect("create worktree");
        std::fs::write(wt.path.join("agent.txt"), "agent work").unwrap();
        git(&wt.path, &["add", "."]);
        git(&wt.path, &["commit", "-q", "-m", "agent work"]);

        if merged_into_integration {
            git(
                &repo,
                &["merge", "-q", "--no-ff", "-m", "merge", "feature/test"],
            );
        }

        let session = Session {
            session_name: "paw-repo".to_string(),
            repo_path: repo.clone(),
            project_name: "repo".to_string(),
            created_at: UNIX_EPOCH,
            status: SessionStatus::Stopped,
            worktrees: vec![WorktreeEntry {
                branch: "feature/test".to_string(),
                worktree_path: wt.path.clone(),
                cli: "claude".to_string(),
                branch_created: wt.branch_created,
                base_branch: created_branch_base(&repo, wt.branch_created),
                pending_boot_prompt: None,
                pending_entry_point: None,
                runtime_slot: None,
                cli_session_id: None,
                spec_id: None,
            }],
            broker_port: None,
            broker_bind: None,
            broker_log_path: None,
            mode: crate::session::SessionMode::Bare,
            dashboard_pane: None,
            layout: None,
            broker_authority: None,
        };
        session::save_session_in(&session, &sessions_dir).expect("save session");

        PurgeSandbox {
            _sandbox: sandbox,
            repo,
            sessions_dir,
            session,
        }
    }

    // --- shared teardown: branch retention (`git-add-remove-branch`) ---

    /// Spec `git-add-remove-branch` — "A same-named tag never stands in for the
    /// branch". `rev-parse` tries `refs/tags/<name>` before `refs/heads/<name>`,
    /// so an unqualified comparison would answer on the tag's history: the tag
    /// here sits on merged history while the branch carries unmerged work.
    #[test]
    #[serial]
    fn detach_worktree_is_not_fooled_by_a_tag_sharing_the_branch_name() {
        let sb = setup_purge_sandbox(true);
        // A tag with the branch's exact name, pointing at fully merged history.
        git(&sb.repo, &["tag", "feature/test", "main"]);
        let entry = &sb.session.worktrees[0];
        let branch_tip_before = branch_tip(&sb.repo, "refs/heads/feature/test")
            .expect("branch ref resolves before teardown");
        let tag_tip = branch_tip(&sb.repo, "refs/tags/feature/test").expect("tag ref resolves");
        assert_ne!(
            branch_tip_before, tag_tip,
            "the fixture must put the tag on different history than the branch"
        );

        let mut stderr = CaptureOutput::default();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = warn_text(&stderr);
        assert_eq!(
            branch_tip(&sb.repo, "refs/heads/feature/test").as_deref(),
            Some(branch_tip_before.as_str()),
            "the unmerged branch must survive despite the merged same-named tag: {err_text:?}"
        );
        assert!(
            err_text.contains("Kept branch feature/test at")
                && err_text.contains("1 commit(s) not in"),
            "the comparison must run on refs/heads, not the tag: {err_text:?}"
        );
        assert!(
            err_text.contains(&branch_tip_before[..7]),
            "the reported tip must be the branch's, not the tag's: {err_text:?}"
        );
    }

    /// Spec `git-add-remove-branch` — "A branch merged into the branch it was
    /// created from is deleted". Launched from an integration branch, work that
    /// has landed in that branch is finished even though it is not yet in the
    /// default branch.
    #[test]
    #[serial]
    fn detach_worktree_deletes_a_branch_merged_into_its_recorded_base() {
        let sb = setup_integration_sandbox(true);
        let entry = &sb.session.worktrees[0];
        assert_eq!(
            entry.base_branch.as_deref(),
            Some("integration"),
            "the session must record the branch git-paw branched from"
        );

        let mut stderr = CaptureOutput::default();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = warn_text(&stderr);
        assert!(
            branch_tip(&sb.repo, "feature/test").is_none(),
            "a branch merged into its base should be deleted; stderr: {err_text:?}"
        );
        assert!(
            err_text.contains("Deleted branch feature/test (was "),
            "deletion must print the branch and its SHA: {err_text:?}"
        );
    }

    /// Spec `git-add-remove-branch` — "An unmerged branch is counted against
    /// the branch it was created from". The integration branch is two commits
    /// ahead of `main`; the agent added one. Counting against `main` would
    /// report three and describe the integration backlog as the agent's work.
    #[test]
    #[serial]
    fn detach_worktree_counts_an_unmerged_branch_against_its_recorded_base() {
        let sb = setup_integration_sandbox(false);
        let entry = &sb.session.worktrees[0];
        let tip_before = branch_tip(&sb.repo, "feature/test").expect("branch exists");

        let mut stderr = CaptureOutput::default();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = warn_text(&stderr);
        assert_eq!(
            branch_tip(&sb.repo, "feature/test").as_deref(),
            Some(tip_before.as_str()),
            "an unmerged branch must survive: {err_text:?}"
        );
        assert!(
            err_text.contains("1 commit(s) not in integration"),
            "the count must be against the recorded base, not the default branch: {err_text:?}"
        );
        assert!(
            !err_text.contains("3 commit(s)"),
            "the integration backlog must not be counted as the agent's work: {err_text:?}"
        );
    }

    /// Spec `git-add-remove-branch` — the retention hint quotes the worktree
    /// path (a path with a space would otherwise produce a broken command) and
    /// offers `git branch -d` when git would accept the safe delete.
    #[test]
    #[serial]
    fn retention_hint_quotes_the_path_and_prefers_the_safe_delete_when_it_would_work() {
        // HEAD is `integration`, which contains the branch — so `-d` succeeds —
        // but the entry records no base, so teardown still compares against
        // `main` only and keeps it (the pre-`base_branch` session shape).
        let mut sb = setup_integration_sandbox(true);
        sb.session.worktrees[0].base_branch = None;
        let entry = &sb.session.worktrees[0];
        let worktree_path = entry.worktree_path.display().to_string();

        let mut stderr = CaptureOutput::default();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = warn_text(&stderr);
        assert!(
            err_text.contains(&format!("git worktree add '{worktree_path}' feature/test")),
            "the resume hint must quote the worktree path: {err_text:?}"
        );
        assert!(
            err_text.contains("git branch -d feature/test"),
            "`-d` must be offered when git would accept it: {err_text:?}"
        );
    }

    /// The `-D` fallback: nothing contains the branch, so the safe delete would
    /// be refused and the hint must say so.
    #[test]
    #[serial]
    fn retention_hint_falls_back_to_force_delete_when_the_safe_delete_would_fail() {
        let sb = setup_purge_sandbox(true);
        let entry = &sb.session.worktrees[0];

        let mut stderr = CaptureOutput::default();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = warn_text(&stderr);
        assert!(
            err_text.contains("git branch -D feature/test"),
            "`-D` is the only form that would work here: {err_text:?}"
        );
    }

    /// Spec `git-add-remove-branch` — "remove keeps a created branch with
    /// unmerged commits".
    #[test]
    #[serial]
    fn detach_worktree_keeps_a_created_branch_with_unmerged_commits() {
        let sb = setup_purge_sandbox(true);
        let entry = &sb.session.worktrees[0];
        let tip_before = branch_tip(&sb.repo, &entry.branch).expect("branch exists");
        let worktree_path = entry.worktree_path.clone();

        let mut stderr = CaptureOutput::default();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = warn_text(&stderr);
        assert!(!worktree_path.exists(), "worktree should be removed");
        assert_eq!(
            branch_tip(&sb.repo, &entry.branch).as_deref(),
            Some(tip_before.as_str()),
            "the branch must survive at the same tip"
        );
        assert!(
            err_text.contains("Kept branch feature/test at ")
                && err_text.contains("1 commit(s) not in main"),
            "retention must name the branch, its tip, and the unmerged count: {err_text:?}"
        );
        assert!(
            err_text.contains(&format!(
                "git worktree add '{}' feature/test",
                worktree_path.display()
            )),
            "retention must print the resume hint: {err_text:?}"
        );
        assert!(
            err_text.contains("git branch -D feature/test"),
            "retention must print the discard hint: {err_text:?}"
        );
    }

    /// Spec `git-add-remove-branch` — "remove deletes a created branch that is
    /// fully merged".
    #[test]
    #[serial]
    fn detach_worktree_deletes_a_created_branch_that_is_fully_merged() {
        let sb = setup_purge_sandbox(false);
        let entry = &sb.session.worktrees[0];
        let tip_before = branch_tip(&sb.repo, &entry.branch).expect("branch exists");
        let short_tip = &tip_before[..7];

        let mut stderr = CaptureOutput::default();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = warn_text(&stderr);
        assert!(
            branch_tip(&sb.repo, &entry.branch).is_none(),
            "a fully merged created branch should be deleted"
        );
        assert!(
            err_text.contains(&format!("Deleted branch feature/test (was {short_tip}")),
            "deletion must print the branch and the SHA it was at: {err_text:?}"
        );
    }

    /// Spec `git-add-remove-branch` — "A pre-existing branch is never deleted".
    #[test]
    #[serial]
    fn detach_worktree_leaves_a_pre_existing_branch_untouched() {
        let mut sb = setup_purge_sandbox(false);
        sb.session.worktrees[0].branch_created = false;
        let entry = &sb.session.worktrees[0];
        let tip_before = branch_tip(&sb.repo, &entry.branch).expect("branch exists");

        let mut stderr = CaptureOutput::default();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = warn_text(&stderr);
        assert_eq!(
            branch_tip(&sb.repo, &entry.branch).as_deref(),
            Some(tip_before.as_str()),
            "a branch git-paw did not create must remain even when merged"
        );
        assert!(
            !err_text.contains("Deleted branch"),
            "no deletion should be reported: {err_text:?}"
        );
    }

    /// Spec `git-add-remove-branch` — "A failed merge comparison retains the
    /// branch". The session entry names a branch that is not in the repository,
    /// so `git merge-base` cannot compare it.
    #[test]
    #[serial]
    fn detach_worktree_retains_a_branch_whose_merge_state_is_undeterminable() {
        let mut sb = setup_purge_sandbox(true);
        sb.session.worktrees[0].branch = "feature/ghost".to_string();
        let entry = &sb.session.worktrees[0];

        let mut stderr = CaptureOutput::default();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = warn_text(&stderr);
        assert!(
            err_text.contains("Kept branch feature/ghost")
                && err_text.contains("merge state could not be determined"),
            "an uncomparable branch must be retained with the reason stated: {err_text:?}"
        );
        assert!(
            !err_text.contains("Deleted branch"),
            "nothing may be deleted on a failed comparison: {err_text:?}"
        );
        assert!(
            branch_tip(&sb.repo, "feature/test").is_some(),
            "the real branch must be untouched"
        );
    }

    #[test]
    #[serial]
    fn purge_no_unmerged_commits_runs_without_warning() {
        let sb = setup_purge_sandbox(false);
        let worktree_path = sb.session.worktrees[0].worktree_path.clone();
        let short_tip =
            branch_tip(&sb.repo, "feature/test").expect("branch exists")[..7].to_string();

        let mut confirm_calls = 0;
        let mut confirm = |_: &str| -> Result<bool, PawError> {
            confirm_calls += 1;
            Ok(true)
        };
        let mut stderr = CaptureOutput::default();

        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            false,
            &mut confirm,
            &mut |_: &str| Ok(()),
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PromptResult::Purged);
        assert_eq!(confirm_calls, 1);
        let err_text = warn_text(&stderr);
        assert!(
            !err_text.contains("unmerged"),
            "stderr should not mention unmerged: {err_text:?}"
        );
        assert!(!worktree_path.exists(), "worktree should be removed");
        assert!(
            !sb.sessions_dir.join("paw-repo.json").exists(),
            "session file should be removed"
        );
        assert!(
            branch_tip(&sb.repo, "feature/test").is_none(),
            "a fully merged created branch should be deleted"
        );
        assert!(
            err_text.contains(&format!("Deleted branch feature/test (was {short_tip}")),
            "each deletion should print the SHA the branch was at: {err_text:?}"
        );
    }

    #[test]
    #[serial]
    fn purge_with_unmerged_commits_emits_warning_to_stderr() {
        let sb = setup_purge_sandbox(true);
        let tip_before = branch_tip(&sb.repo, "feature/test").expect("branch exists before purge");

        let mut confirm = |_: &str| -> Result<bool, PawError> { Ok(true) };
        let mut stderr = CaptureOutput::default();

        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            false,
            &mut confirm,
            &mut |_: &str| Ok(()),
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PromptResult::Purged);
        let err_text = warn_text(&stderr);
        assert!(
            err_text.contains("Warning:") && err_text.contains("unmerged commits"),
            "stderr should contain unmerged warning: {err_text:?}"
        );
        assert!(
            err_text.contains("feature/test"),
            "stderr should name the branch: {err_text:?}"
        );
        assert!(
            err_text.contains("will be kept"),
            "the warning must say the branches are kept, not lost: {err_text:?}"
        );
        assert!(
            err_text.contains("1 commit(s) not in main"),
            "the warning must report the unmerged count: {err_text:?}"
        );
        assert_eq!(
            branch_tip(&sb.repo, "feature/test").as_deref(),
            Some(tip_before.as_str()),
            "an unmerged branch must survive the purge at the same tip"
        );
    }

    /// Spec `supervisor-launch` — "Confirmed purge also retains unmerged
    /// branches".
    #[test]
    #[serial]
    fn purge_confirmed_by_the_user_still_retains_an_unmerged_branch() {
        let sb = setup_purge_sandbox(true);
        let tip_before = branch_tip(&sb.repo, "feature/test").expect("branch exists before purge");
        let worktree_path = sb.session.worktrees[0].worktree_path.clone();

        let mut confirm = |_: &str| -> Result<bool, PawError> { Ok(true) };
        let mut stderr = CaptureOutput::default();

        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            false,
            &mut confirm,
            &mut |_: &str| Ok(()),
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PromptResult::Purged);
        assert!(!worktree_path.exists(), "worktree should still be removed");
        assert_eq!(
            branch_tip(&sb.repo, "feature/test").as_deref(),
            Some(tip_before.as_str()),
            "answering Y must not delete an unmerged branch"
        );
    }

    /// Spec `supervisor-launch` — "An undeterminable merge state is treated as
    /// unmerged". The session names a branch that no longer exists, so every
    /// ref comparison against it errors.
    #[test]
    #[serial]
    fn purge_reports_an_undeterminable_merge_state_and_deletes_nothing() {
        let mut sb = setup_purge_sandbox(true);
        let tip_before = branch_tip(&sb.repo, "feature/test").expect("branch exists before purge");
        sb.session.worktrees[0].branch = "feature/ghost".to_string();

        let mut stderr = CaptureOutput::default();
        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            true, // force
            &mut |_: &str| Ok(true),
            &mut |_: &str| Ok(()),
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PromptResult::Purged);
        let err_text = warn_text(&stderr);
        assert!(
            err_text.contains("merge state could not be determined"),
            "an uncomparable branch must be reported, not silently treated as merged: {err_text:?}"
        );
        assert_eq!(
            branch_tip(&sb.repo, "feature/test").as_deref(),
            Some(tip_before.as_str()),
            "no branch may be deleted when the comparison cannot be made"
        );
    }

    /// Spec `supervisor-launch` — "Purge in a master-default repository without
    /// origin/HEAD compares against master". The purge-local resolver used to
    /// fall back to a literal `main` here, so the comparison failed and the
    /// branch was deleted without a warning.
    #[test]
    #[serial]
    fn purge_in_a_master_default_repo_without_origin_head_reports_the_unmerged_commit() {
        let sb = setup_purge_sandbox_on("master", true);
        let tip_before = branch_tip(&sb.repo, "feature/test").expect("branch exists before purge");

        let mut stderr = CaptureOutput::default();
        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            true, // force
            &mut |_: &str| Ok(true),
            &mut |_: &str| Ok(()),
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PromptResult::Purged);
        let err_text = warn_text(&stderr);
        assert!(
            err_text.contains("1 commit(s) not in master"),
            "the comparison must run against master, not a literal main: {err_text:?}"
        );
        assert_eq!(
            branch_tip(&sb.repo, "feature/test").as_deref(),
            Some(tip_before.as_str()),
            "the unmerged branch must survive"
        );
    }

    #[test]
    #[serial]
    fn purge_force_skips_confirm_but_still_warns() {
        let sb = setup_purge_sandbox(true);
        let tip_before = branch_tip(&sb.repo, "feature/test").expect("branch exists before purge");

        let mut confirm_calls = 0;
        let mut confirm = |_: &str| -> Result<bool, PawError> {
            confirm_calls += 1;
            Ok(false)
        };
        let mut stderr = CaptureOutput::default();

        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            true, // force
            &mut confirm,
            &mut |_: &str| Ok(()),
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PromptResult::Purged);
        assert_eq!(confirm_calls, 0, "force should not invoke confirm");
        let err_text = warn_text(&stderr);
        assert!(
            err_text.contains("Warning:") && err_text.contains("unmerged commits"),
            "force mode should still warn: {err_text:?}"
        );
        assert!(
            err_text.contains("will be kept"),
            "--force must not imply the branches are deleted: {err_text:?}"
        );
        assert_eq!(
            branch_tip(&sb.repo, "feature/test").as_deref(),
            Some(tip_before.as_str()),
            "--force must still retain a branch with unmerged commits"
        );
        assert!(
            err_text.contains("git branch -D feature/test"),
            "the retention must print how to discard the branch by hand: {err_text:?}"
        );
    }

    #[test]
    #[serial]
    fn purge_cancelled_leaves_worktree_in_place() {
        let sb = setup_purge_sandbox(true);
        let worktree_path = sb.session.worktrees[0].worktree_path.clone();

        let mut confirm = |_: &str| -> Result<bool, PawError> { Ok(false) };
        let mut tmux_calls = 0;
        let mut kill_tmux = |_: &str| -> Result<(), PawError> {
            tmux_calls += 1;
            Ok(())
        };
        let mut stderr = CaptureOutput::default();

        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            false,
            &mut confirm,
            &mut kill_tmux,
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PromptResult::Cancelled);
        assert_eq!(tmux_calls, 0, "tmux must not be killed on cancel");
        assert!(worktree_path.exists(), "worktree must remain on cancel");
        assert!(
            sb.sessions_dir.join("paw-repo.json").exists(),
            "session file must remain on cancel"
        );
    }

    /// Recorder for the order of sink writes and confirm calls. Used by the
    /// Bug C regression test to assert the warning lands BEFORE the prompt
    /// fires.
    struct OrderedSink {
        events: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    impl Output for OrderedSink {
        fn line(&mut self, text: &str) {
            self.events.borrow_mut().push(format!("line:{text}"));
        }

        fn warn(&mut self, text: &str) {
            self.events.borrow_mut().push(format!("warn:{text}"));
        }
    }

    // Bug D — purge --force "freeze". Per-worktree progress markers
    // must surround each `git::remove_worktree` call so the user can
    // tell the command is making progress on a large worktree. The
    // assertion is on the stderr contents after a real successful
    // purge_with_prompt run. v0-5-0-audit-cleanup tasks 9b.1–9b.5.
    #[test]
    #[serial]
    fn purge_emits_per_worktree_progress_messages() {
        let sb = setup_purge_sandbox(false);
        let worktree_path_display = sb.session.worktrees[0].worktree_path.display().to_string();

        let mut confirm = |_: &str| -> Result<bool, PawError> { Ok(true) };
        let mut stderr = CaptureOutput::default();

        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            true, // force — bypass confirm to isolate the progress assertion
            &mut confirm,
            &mut |_: &str| Ok(()),
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PromptResult::Purged);
        let err_text = warn_text(&stderr);
        assert!(
            err_text.contains(&format!("Removing worktree {worktree_path_display}")),
            "stderr should announce each worktree removal by path; got:\n{err_text}"
        );
        assert!(
            err_text.contains("...done ("),
            "stderr should emit a `...done (Xs)` marker per worktree once removal completes; got:\n{err_text}"
        );
    }

    // Bug C — purge confirmation prompt regression. When unmerged-
    // commit warnings are written, the whole block must reach the sink
    // before the prompt reads stdin. Otherwise the warning races against
    // the prompt and dialoguer can mis-classify the `y` keystroke as
    // cancellation. v0-5-0-audit-cleanup tasks
    // 9a.1–9a.4 (the piped-stdin integration test described in 9a.4 is
    // impractical because dialoguer probes the TTY and behaves
    // differently under pipes; the behavioural assertion below covers
    // the same regression shape without a TTY dependency).
    #[test]
    #[serial]
    fn purge_with_unmerged_commits_warns_before_confirm() {
        let sb = setup_purge_sandbox(true);

        let events: std::rc::Rc<std::cell::RefCell<Vec<String>>> =
            std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let events_for_confirm = std::rc::Rc::clone(&events);

        let mut confirm = |_: &str| -> Result<bool, PawError> {
            events_for_confirm.borrow_mut().push("confirm".to_string());
            Ok(true)
        };
        let mut sink = OrderedSink {
            events: std::rc::Rc::clone(&events),
        };

        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            false,
            &mut confirm,
            &mut |_: &str| Ok(()),
            &mut sink,
        )
        .unwrap();

        assert_eq!(outcome, PromptResult::Purged);
        let recorded = events.borrow().clone();
        let confirm_idx = recorded
            .iter()
            .position(|s| s == "confirm")
            .expect("confirm must have been called");
        // The whole unmerged-commits warning block — down to its closing
        // "branches themselves are kept" line — must be written before the
        // prompt fires.
        let block_end = recorded
            .iter()
            .position(|s| s.contains("the branches themselves are kept"))
            .expect("the unmerged-commits warning block was written");
        assert!(
            block_end < confirm_idx,
            "the warning block must land before the prompt; events: {recorded:?}"
        );
        assert!(
            recorded[..confirm_idx]
                .iter()
                .any(|s| s.starts_with("warn:Warning: 1 branch(es)")),
            "the warning header must precede the prompt; events: {recorded:?}"
        );
    }
}
