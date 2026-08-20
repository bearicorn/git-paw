//! Git operations.
//!
//! Validates git repositories, lists branches, creates and removes worktrees,
//! and derives worktree directory names from project and branch names.
//!
//! Every git invocation here goes through the [`CommandRunner`] seam. Each
//! public entry point keeps its original signature and delegates to a
//! `*_with(runner, …)` sibling wired to [`RealCommandRunner`], so callers are
//! untouched while the argv and the success/failure branches become assertable
//! without spawning a real `git` (`git-command-runner-seam` D1). The real runner
//! performs exactly what the previous inline `Command::new("git")` calls did.
//!
//! # Working directory
//!
//! Every call here targets a specific repository or worktree, which the inline
//! calls expressed as `Command::current_dir(<path>)`. [`CommandRunner`] models
//! only `(program, argv)`, so the same working directory is carried as a leading
//! `-C <path>` — git chdirs there before anything else, so the effective
//! behaviour (repo discovery, relative pathspecs, inherited env) is unchanged
//! (`git-command-runner-seam` D2). Paths enter argv via
//! [`Path::to_string_lossy`]; in practice every path git-paw handles is already
//! UTF-8, because the repository root it derives them from is itself decoded
//! lossily out of `git rev-parse --show-toplevel` in [`validate_repo`].

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::command_runner::{CommandRunner, RealCommandRunner};
use crate::config::WorktreePlacement;
use crate::domain::WorktreePath;
use crate::error::PawError;
use crate::specs::SpecEntry;

/// Validates that the given path is inside a git repository.
///
/// Returns the absolute path to the repository root.
pub fn validate_repo(path: &Path) -> Result<PathBuf, PawError> {
    validate_repo_with(&RealCommandRunner, path)
}

/// [`validate_repo`] against an injected runner.
pub(crate) fn validate_repo_with(
    runner: &dyn CommandRunner,
    path: &Path,
) -> Result<PathBuf, PawError> {
    let cwd = path.to_string_lossy();
    let output = runner
        .run("git", &["-C", &cwd, "rev-parse", "--show-toplevel"])
        .map_err(|e| PawError::BranchError(format!("failed to run git: {e}")))?;

    if !output.success {
        return Err(PawError::NotAGitRepo);
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
}

/// Lists all branches (local and remote), deduplicated, sorted, with remote
/// prefixes stripped.
///
/// Remote branches like `origin/main` are included as `main`. If a branch
/// exists both locally and remotely, only one entry appears. `HEAD` pointers
/// are excluded.
pub fn list_branches(repo_root: &Path) -> Result<Vec<String>, PawError> {
    list_branches_with(&RealCommandRunner, repo_root)
}

/// [`list_branches`] against an injected runner.
pub(crate) fn list_branches_with(
    runner: &dyn CommandRunner,
    repo_root: &Path,
) -> Result<Vec<String>, PawError> {
    let cwd = repo_root.to_string_lossy();
    let output = runner
        .run(
            "git",
            &["-C", &cwd, "branch", "-a", "--format=%(refname:short)"],
        )
        .map_err(|e| PawError::BranchError(format!("failed to run git branch: {e}")))?;

    if !output.success {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PawError::BranchError(format!(
            "git branch failed: {stderr}"
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let branches: BTreeSet<String> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.contains("HEAD"))
        .map(|line| {
            // Strip remote prefix (e.g., "origin/main" -> "main")
            let mut branch_name = line.trim().to_string();

            // Handle full ref format: refs/remotes/origin/branch -> branch
            if let Some(stripped) = branch_name.strip_prefix("refs/remotes/") {
                branch_name = stripped.to_string();
            }
            // Handle short format: origin/branch -> branch
            if let Some(stripped) = branch_name.strip_prefix("origin/") {
                branch_name = stripped.to_string();
            }

            branch_name
        })
        .collect();

    // Remove duplicates that can arise from local+remote branches with same name
    let mut unique: Vec<String> = branches.into_iter().collect();
    unique.sort();
    Ok(unique)
}

/// Derives a worktree directory name from project and branch names.
///
/// The format is: `<project>-<branch>` with non-alphanumeric characters replaced by `-`.
pub fn worktree_dir_name(project: &str, branch: &str) -> String {
    let project_safe: String = project
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let branch_safe: String = branch
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    format!("{project_safe}-{branch_safe}")
}

/// Derives the child-layout worktree slug from a branch name alone.
///
/// Replaces `/` with `-` and strips every character outside the safe set
/// of ASCII letters, digits, dot, dash, and underscore (`[A-Za-z0-9._-]`).
/// Unlike [`worktree_dir_name`] the project name is NOT prepended, because a
/// child worktree already lives under that project's `.git-paw/worktrees/`,
/// so the prefix would be redundant. Thus `feat/auth-flow` → `feat-auth-flow`
/// and `fix/issue#42` → `fix-issue42`.
#[must_use]
pub fn branch_slug(branch: &str) -> String {
    crate::domain::BranchSlug::for_branch(branch).into_string()
}

/// Returns the name of the default branch (usually "main" or "master").
pub fn default_branch(repo_root: &Path) -> Result<String, PawError> {
    default_branch_with(&RealCommandRunner, repo_root)
}

/// [`default_branch`] against an injected runner.
pub(crate) fn default_branch_with(
    runner: &dyn CommandRunner,
    repo_root: &Path,
) -> Result<String, PawError> {
    let cwd = repo_root.to_string_lossy();
    let output = runner
        .run(
            "git",
            &["-C", &cwd, "symbolic-ref", "refs/remotes/origin/HEAD"],
        )
        .map_err(|e| PawError::BranchError(format!("failed to run git symbolic-ref: {e}")))?;

    if !output.success {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PawError::BranchError(format!(
            "git symbolic-ref failed: {stderr}"
        )));
    }

    let ref_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if let Some(branch) = ref_name.strip_prefix("refs/remotes/origin/") {
        Ok(branch.to_string())
    } else {
        Err(PawError::BranchError(format!(
            "unexpected ref format: {ref_name}"
        )))
    }
}

/// Returns the short name of the current branch (e.g., "main", "feat/add-auth").
pub fn current_branch(repo_root: &Path) -> Result<String, PawError> {
    current_branch_with(&RealCommandRunner, repo_root)
}

/// [`current_branch`] against an injected runner.
pub(crate) fn current_branch_with(
    runner: &dyn CommandRunner,
    repo_root: &Path,
) -> Result<String, PawError> {
    let cwd = repo_root.to_string_lossy();
    let output = runner
        .run("git", &["-C", &cwd, "branch", "--show-current"])
        .map_err(|e| PawError::BranchError(format!("failed to run git branch: {e}")))?;

    if !output.success {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PawError::BranchError(format!(
            "git branch failed: {stderr}"
        )));
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() {
        return Err(PawError::BranchError(
            "not on any branch (detached HEAD)".to_string(),
        ));
    }
    Ok(branch)
}

/// Returns the name of the project (directory name of the git repository).
pub fn project_name(repo_root: &Path) -> String {
    repo_root
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("unknown")
        .to_string()
}

/// Result of creating a worktree, including whether the branch was newly created.
#[derive(Debug)]
pub struct WorktreeCreation {
    /// Path to the created worktree directory.
    pub path: PathBuf,
    /// Whether git-paw created the branch (true) or it already existed (false).
    pub branch_created: bool,
}

/// Returns the path of the worktree (main repo or any sibling worktree) that
/// currently has `branch` checked out, if any.
///
/// Private, and reached only from [`rebase_branch_onto_default_with`], which
/// already carries a runner — so unlike the public entry points there is no
/// no-runner wrapper to preserve.
fn find_worktree_for_branch_with(
    runner: &dyn CommandRunner,
    repo_root: &Path,
    branch: &str,
) -> Result<Option<PathBuf>, PawError> {
    let cwd = repo_root.to_string_lossy();
    let list = runner
        .run("git", &["-C", &cwd, "worktree", "list", "--porcelain"])
        .map_err(|e| PawError::WorktreeError(format!("failed to run git worktree list: {e}")))?;
    if !list.success {
        return Ok(None);
    }
    let listing = String::from_utf8_lossy(&list.stdout);
    let expected_branch_ref = format!("refs/heads/{branch}");
    let mut current_path: Option<PathBuf> = None;
    for line in listing.lines() {
        if let Some(rest) = line.strip_prefix("worktree ") {
            current_path = Some(PathBuf::from(rest));
        } else if let Some(rest) = line.strip_prefix("branch ")
            && rest == expected_branch_ref
            && let Some(p) = current_path.take()
        {
            return Ok(Some(p));
        }
    }
    Ok(None)
}

/// Rebases `branch` onto the repo's default branch.
///
/// Runs the rebase inside the worktree where `branch` is currently checked out
/// (the main repo or one of its sibling worktrees). If `branch` is not checked
/// out anywhere, the main repo's HEAD is switched to it for the rebase and
/// restored afterwards so the subsequent `git worktree add` call still works.
///
/// On rebase failure, runs `git rebase --abort` (best-effort), restores the
/// main repo's HEAD if it was switched, and returns a `WorktreeError`
/// containing git's stderr. The branch is left at its pre-rebase HEAD.
///
/// Private, and reached only from [`create_worktree_with`], which already
/// carries a runner — so unlike the public entry points there is no no-runner
/// wrapper to preserve.
fn rebase_branch_onto_default_with(
    runner: &dyn CommandRunner,
    repo_root: &Path,
    branch: &str,
) -> Result<(), PawError> {
    let default = default_branch_with(runner, repo_root)?;
    let repo_cwd = repo_root.to_string_lossy();

    let occupied_at = find_worktree_for_branch_with(runner, repo_root, branch)?;
    let (workdir, original_head): (PathBuf, Option<String>) = if let Some(wt) = occupied_at {
        (wt, None)
    } else {
        let original = runner
            .run("git", &["-C", &repo_cwd, "symbolic-ref", "--short", "HEAD"])
            .ok()
            .filter(|o| o.success)
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        (repo_root.to_path_buf(), original)
    };

    let workdir_cwd = workdir.to_string_lossy();
    let mut rebase_argv: Vec<&str> = vec!["-C", &workdir_cwd, "rebase", &default];
    if original_head.is_some() {
        rebase_argv.push(branch);
    }
    let output = runner
        .run("git", &rebase_argv)
        .map_err(|e| PawError::WorktreeError(format!("failed to run git rebase: {e}")))?;

    if !output.success {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let _ = runner.run("git", &["-C", &workdir_cwd, "rebase", "--abort"]);
        if let Some(orig) = &original_head
            && orig != branch
        {
            let _ = runner.run("git", &["-C", &repo_cwd, "checkout", orig]);
        }
        return Err(PawError::WorktreeError(format!(
            "rebase onto main failed: {stderr}"
        )));
    }

    if let Some(orig) = original_head
        && orig != branch
    {
        let _ = runner.run("git", &["-C", &repo_cwd, "checkout", &orig]);
    }

    Ok(())
}

/// Resolves the absolute worktree directory path for `branch` under the
/// given `placement`.
///
/// [`WorktreePlacement::Child`] resolves to
/// `<repo_root>/.git-paw/worktrees/<branch-slug>`, creating the
/// `.git-paw/worktrees/` parent if absent. [`WorktreePlacement::Sibling`]
/// resolves to `<repo_parent>/<project>-<branch-slug>` (the v0.7.0 layout)
/// and creates nothing. Factored out of [`create_worktree`] so the path
/// derivation stays readable and independently testable.
/// Computes the absolute worktree directory path for `branch` under `placement`
/// **without creating anything** — a pure derivation safe for previews such as
/// `git paw start --dry-run`.
///
/// [`WorktreePlacement::Child`] → `<repo_root>/.git-paw/worktrees/<branch-slug>`;
/// [`WorktreePlacement::Sibling`] → `<repo_parent>/<project>-<branch-slug>`.
pub fn worktree_path_for(
    repo_root: &Path,
    branch: &str,
    placement: WorktreePlacement,
) -> Result<PathBuf, PawError> {
    match placement {
        WorktreePlacement::Child => Ok(repo_root
            .join(".git-paw")
            .join("worktrees")
            .join(branch_slug(branch))),
        WorktreePlacement::Sibling => {
            let project = project_name(repo_root);
            let dir_name = worktree_dir_name(&project, branch);
            let parent = repo_root.parent().ok_or_else(|| {
                PawError::WorktreeError("cannot determine parent directory of repo".to_string())
            })?;
            Ok(parent.join(&dir_name))
        }
    }
}

/// A short, human-readable rendering of the worktree path for `--dry-run`
/// previews: relative to the repo root for a child worktree
/// (`.git-paw/worktrees/<slug>`), or `../<name>` for a sibling. Reflects the
/// configured [`WorktreePlacement`] rather than assuming the sibling layout.
pub fn worktree_display_path(
    repo_root: &Path,
    branch: &str,
    placement: WorktreePlacement,
) -> Result<String, PawError> {
    let path = worktree_path_for(repo_root, branch, placement)?;
    Ok(path.strip_prefix(repo_root).map_or_else(
        |_| {
            path.file_name().map_or_else(
                || path.display().to_string(),
                |n| format!("../{}", n.to_string_lossy()),
            )
        },
        |rel| rel.display().to_string(),
    ))
}

fn resolve_worktree_path(
    repo_root: &Path,
    branch: &str,
    placement: WorktreePlacement,
) -> Result<WorktreePath, PawError> {
    // A child worktree needs its `.git-paw/worktrees/` parent to exist before
    // `git worktree add`; the resolved path itself is the pure derivation.
    if placement == WorktreePlacement::Child {
        let worktrees_dir = repo_root.join(".git-paw").join("worktrees");
        std::fs::create_dir_all(&worktrees_dir).map_err(|e| {
            PawError::WorktreeError(format!(
                "failed to create '{}': {e}",
                worktrees_dir.display()
            ))
        })?;
    }
    Ok(WorktreePath::new(worktree_path_for(
        repo_root, branch, placement,
    )?))
}

/// Creates a git worktree for `branch`.
///
/// If the branch already exists, checks it out in a new worktree. If the
/// branch does not exist, creates it from HEAD with `git worktree add -b`.
/// Returns both the worktree path and whether the branch was newly created,
/// so the session can track which branches to delete on purge.
///
/// When `rebase_onto_main` is `true` and the target branch already exists in
/// the local repository, the branch is rebased onto `default_branch()` BEFORE
/// the existence check. The rebase resolves drift between supervisor work on
/// main and live agent branches (MILESTONE.md drift item 48: agents otherwise
/// commit on a stale baseline). On rebase conflict the function runs
/// `git rebase --abort` and returns `PawError::WorktreeError`; the branch is
/// left at its pre-rebase HEAD. When `rebase_onto_main` is `false` or the
/// branch does not yet exist locally, the rebase step is skipped.
///
/// `placement` selects where the worktree directory is created (see
/// [`WorktreePlacement`]). [`WorktreePlacement::Child`] resolves to
/// `<repo_root>/.git-paw/worktrees/<branch-slug>` (creating
/// `.git-paw/worktrees/` if absent); [`WorktreePlacement::Sibling`] resolves
/// to `<repo_parent>/<project>-<branch-slug>`, matching the v0.7.0 layout.
/// Only the resolved target path varies with placement; all other behaviour
/// is identical.
pub fn create_worktree(
    repo_root: &Path,
    branch: &str,
    rebase_onto_main: bool,
    placement: WorktreePlacement,
) -> Result<WorktreeCreation, PawError> {
    create_worktree_with(
        &RealCommandRunner,
        repo_root,
        branch,
        rebase_onto_main,
        placement,
    )
}

/// [`create_worktree`] against an injected runner.
pub(crate) fn create_worktree_with(
    runner: &dyn CommandRunner,
    repo_root: &Path,
    branch: &str,
    rebase_onto_main: bool,
    placement: WorktreePlacement,
) -> Result<WorktreeCreation, PawError> {
    let worktree_path = resolve_worktree_path(repo_root, branch, placement)?.into_path_buf();
    let cwd = repo_root.to_string_lossy();

    // Rebase agent branch onto the repo's default branch BEFORE the
    // idempotency check. Resolves MILESTONE.md drift item 48: the supervisor
    // advances main while agents are running, so on resume (or fresh launch
    // of an existing branch) the agent's worktree would otherwise be N
    // commits behind main and every subsequent commit chains from a stale
    // baseline. Order matters: rebasing before the idempotency check means a
    // surviving worktree's branch ref is updated transparently on resume.
    if rebase_onto_main {
        let branch_ref = format!("refs/heads/{branch}");
        let branch_exists = runner
            .run("git", &["-C", &cwd, "rev-parse", "--verify", &branch_ref])
            .is_ok_and(|o| o.success);
        if branch_exists {
            rebase_branch_onto_default_with(runner, repo_root, branch)?;
        }
    }

    // If a worktree already exists at this path AND is registered with git for
    // the same branch, treat it as a successful (idempotent) creation. This is
    // the resume / crash-recovery path — the worktree survived a previous
    // session and `git paw start` should reuse it instead of bailing on
    // "already exists".
    if worktree_path.exists() {
        // Canonicalize the expected path so symlink-resolved porcelain output
        // (e.g. macOS's `/private/var/folders/...` vs `/var/folders/...`)
        // compares equal to the path git-paw computed for the worktree.
        let expected_canonical = std::fs::canonicalize(&worktree_path).ok();
        let list = runner
            .run("git", &["-C", &cwd, "worktree", "list", "--porcelain"])
            .map_err(|e| {
                PawError::WorktreeError(format!("failed to run git worktree list: {e}"))
            })?;
        if list.success {
            let listing = String::from_utf8_lossy(&list.stdout);
            let expected_branch_ref = format!("refs/heads/{branch}");
            // Parse porcelain blocks separated by blank lines. Each block has
            // `worktree <path>` and `branch <ref>` lines.
            let mut current_path: Option<PathBuf> = None;
            for line in listing.lines() {
                if let Some(rest) = line.strip_prefix("worktree ") {
                    current_path = std::fs::canonicalize(PathBuf::from(rest)).ok();
                } else if let Some(rest) = line.strip_prefix("branch ") {
                    let path_matches = match (&current_path, &expected_canonical) {
                        (Some(p), Some(e)) => p == e,
                        _ => false,
                    };
                    if path_matches && rest == expected_branch_ref {
                        return Ok(WorktreeCreation {
                            path: worktree_path,
                            branch_created: false,
                        });
                    }
                }
            }
        }
        // Path exists but not as a git worktree for this branch — let the
        // `git worktree add` call below produce its usual error so the user
        // sees something actionable.
    }

    // Try with existing branch first.
    let target = worktree_path.to_string_lossy();
    let output = runner
        .run("git", &["-C", &cwd, "worktree", "add", &target, branch])
        .map_err(|e| PawError::WorktreeError(format!("failed to run git worktree add: {e}")))?;

    if output.success {
        return Ok(WorktreeCreation {
            path: worktree_path,
            branch_created: false,
        });
    }

    let stderr = String::from_utf8_lossy(&output.stderr);

    // If the branch doesn't exist, create it with -b.
    if stderr.contains("invalid reference") {
        let output = runner
            .run(
                "git",
                &["-C", &cwd, "worktree", "add", "-b", branch, &target],
            )
            .map_err(|e| {
                PawError::WorktreeError(format!("failed to run git worktree add -b: {e}"))
            })?;

        if output.success {
            return Ok(WorktreeCreation {
                path: worktree_path,
                branch_created: true,
            });
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PawError::WorktreeError(format!(
            "git worktree add -b failed for branch '{branch}': {stderr}"
        )));
    }

    Err(PawError::WorktreeError(format!(
        "git worktree add failed for branch '{branch}': {stderr}"
    )))
}

/// Removes the worktree at the given path.
///
/// The path should be the worktree directory path, not a branch name.
pub fn remove_worktree(repo_root: &Path, worktree_path: &Path) -> Result<(), PawError> {
    remove_worktree_with(&RealCommandRunner, repo_root, worktree_path)
}

/// [`remove_worktree`] against an injected runner.
///
/// `worktree_path` reaches argv via [`Path::to_string_lossy`], so a
/// (Unix-legal) non-UTF-8 path is passed to `git` with its invalid bytes
/// replaced rather than verbatim — the removal then fails cleanly instead of
/// panicking. git-paw never produces such a path itself: it derives every
/// worktree path from a repository root that [`validate_repo`] already decoded
/// lossily, plus an ASCII-safe [`branch_slug`].
pub(crate) fn remove_worktree_with(
    runner: &dyn CommandRunner,
    repo_root: &Path,
    worktree_path: &Path,
) -> Result<(), PawError> {
    // Always pass --force per `git-operations/spec.md`'s "SHALL force-remove a
    // worktree" requirement. `remove_worktree` is only called from purge,
    // which is destructive by nature: an agent that produced uncommitted or
    // untracked files in its worktree would otherwise trip "contains modified
    // or untracked files, use --force to delete it" and leak the worktree on
    // disk even though the user already typed `--force` at the CLI.
    let cwd = repo_root.to_string_lossy();
    let target = worktree_path.to_string_lossy();
    let output = runner
        .run(
            "git",
            &["-C", &cwd, "worktree", "remove", "--force", &target],
        )
        .map_err(|e| {
            PawError::WorktreeError(format!(
                "failed to remove worktree at {}: {e}",
                worktree_path.display()
            ))
        })?;

    if !output.success {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PawError::WorktreeError(format!(
            "git worktree remove failed for worktree at {}: {stderr}",
            worktree_path.display()
        )));
    }

    Ok(())
}

/// Prunes stale worktree registrations from the git worktree list.
///
/// This should be called before creating new worktrees to avoid conflicts.
pub fn prune_worktrees(repo_root: &Path) -> Result<(), PawError> {
    prune_worktrees_with(&RealCommandRunner, repo_root)
}

/// [`prune_worktrees`] against an injected runner.
pub(crate) fn prune_worktrees_with(
    runner: &dyn CommandRunner,
    repo_root: &Path,
) -> Result<(), PawError> {
    let cwd = repo_root.to_string_lossy();
    let output = runner
        .run("git", &["-C", &cwd, "worktree", "prune"])
        .map_err(|e| PawError::WorktreeError(format!("failed to prune worktrees: {e}")))?;

    if !output.success {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PawError::WorktreeError(format!(
            "git worktree prune failed: {stderr}"
        )));
    }

    Ok(())
}

/// Checks for uncommitted changes in spec directories or files.
///
/// Returns a list of spec IDs that have uncommitted changes (modified, added,
/// or untracked files). Uses `git status --porcelain` against the spec's path.
///
/// Supports both spec layouts:
/// - `OpenSpec`: `specs/<id>/` directory; the whole directory is probed.
/// - `Markdown`: `specs/<id>.md` file; the single file is probed.
///
/// If neither layout exists for a spec id, it is silently skipped.
pub fn check_uncommitted_specs(
    repo_root: &Path,
    specs: &[SpecEntry],
) -> Result<Vec<String>, PawError> {
    check_uncommitted_specs_with(&RealCommandRunner, repo_root, specs)
}

/// [`check_uncommitted_specs`] against an injected runner.
pub(crate) fn check_uncommitted_specs_with(
    runner: &dyn CommandRunner,
    repo_root: &Path,
    specs: &[SpecEntry],
) -> Result<Vec<String>, PawError> {
    let mut uncommitted_specs = Vec::new();

    let cwd = repo_root.to_string_lossy();
    let specs_dir = repo_root.join("specs");

    for spec in specs {
        let dir_path = specs_dir.join(&spec.id);
        let file_path = specs_dir.join(format!("{}.md", spec.id));

        let porcelain_target = if dir_path.is_dir() {
            format!("specs/{}", spec.id)
        } else if file_path.is_file() {
            format!("specs/{}.md", spec.id)
        } else {
            continue;
        };

        let output = runner
            .run(
                "git",
                &["-C", &cwd, "status", "--porcelain", "--", &porcelain_target],
            )
            .map_err(|e| {
                PawError::BranchError(format!(
                    "failed to run git status for spec {}: {e}",
                    spec.id
                ))
            })?;

        if !output.success {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(PawError::BranchError(format!(
                "git status failed for spec {}: {stderr}",
                spec.id
            )));
        }

        let status_output = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !status_output.is_empty() {
            uncommitted_specs.push(spec.id.clone());
        }
    }

    Ok(uncommitted_specs)
}

/// Returns the list of files with uncommitted changes in `worktree_root`.
///
/// Runs `git status --porcelain -z` inside the worktree and parses each
/// NUL-terminated record's path (the portion after the 3-character XY-status
/// prefix). NUL-delimiting — rather than newline-splitting — is what makes
/// this robust: with `-z`, git emits paths verbatim (never quoted) and never
/// wraps a record across delimiters, so a path that itself contains a newline,
/// or file content that would otherwise bleed across lines, always stays a
/// single record. Newline-splitting `git status --porcelain` (the prior
/// implementation) could mis-read such a fragment as a phantom "file" — which
/// surfaced as `git paw remove` refusing a clean just-started worktree over a
/// bogus dirty entry. Untracked, modified, staged, and renamed entries are all
/// included. An empty vec means the worktree is clean. Used by `git paw
/// remove`'s uncommitted-work safety check (design D7).
pub fn uncommitted_files(worktree_root: &Path) -> Result<Vec<String>, PawError> {
    uncommitted_files_with(&RealCommandRunner, worktree_root)
}

/// [`uncommitted_files`] against an injected runner.
pub(crate) fn uncommitted_files_with(
    runner: &dyn CommandRunner,
    worktree_root: &Path,
) -> Result<Vec<String>, PawError> {
    let cwd = worktree_root.to_string_lossy();
    let output = runner
        .run("git", &["-C", &cwd, "status", "--porcelain", "-z"])
        .map_err(|e| {
            PawError::WorktreeError(format!(
                "failed to run git status in {}: {e}",
                worktree_root.display()
            ))
        })?;

    if !output.success {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PawError::WorktreeError(format!(
            "git status failed in {}: {stderr}",
            worktree_root.display()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Porcelain v1 with `-z`: each entry is a NUL-terminated record `XY <path>`
    // (status in bytes 0..2, a space at byte 2, path from byte 3). A
    // rename/copy carries `R`/`C` in the index (X) column and is followed by a
    // SECOND NUL-terminated field holding the original path; report the new
    // path and consume that origin field. Splitting on NUL (not newlines) means
    // a path containing a newline stays one record, so content can never be
    // mistaken for a filename.
    let mut files = Vec::new();
    let mut records = stdout.split('\0');
    while let Some(rec) = records.next() {
        if rec.len() <= 3 {
            continue;
        }
        let is_rename_or_copy = matches!(rec.as_bytes()[0], b'R' | b'C');
        files.push(rec[3..].to_string());
        if is_rename_or_copy {
            // The origin path occupies the next NUL-delimited field.
            let _ = records.next();
        }
    }
    Ok(files)
}

/// Merges the specified branch into the current branch.
///
/// Returns `true` if the merge was successful, `false` if there were conflicts.
pub fn merge_branch(repo_root: &Path, branch: &str) -> Result<bool, PawError> {
    merge_branch_with(&RealCommandRunner, repo_root, branch)
}

/// [`merge_branch`] against an injected runner.
pub(crate) fn merge_branch_with(
    runner: &dyn CommandRunner,
    repo_root: &Path,
    branch: &str,
) -> Result<bool, PawError> {
    let cwd = repo_root.to_string_lossy();
    let output = runner
        .run(
            "git",
            &["-C", &cwd, "merge", "--no-ff", "--no-commit", branch],
        )
        .map_err(|e| {
            PawError::WorktreeError(format!("failed to run git merge for branch {branch}: {e}"))
        })?;

    if !output.success {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Check if this is a conflict (exit code 1) vs other error
        if output.code == Some(1) {
            return Ok(false);
        }
        return Err(PawError::WorktreeError(format!(
            "git merge failed for branch {branch}: {stderr}"
        )));
    }

    Ok(true)
}

/// Deletes a branch.
pub fn delete_branch(repo_root: &Path, branch: &str) -> Result<(), PawError> {
    delete_branch_with(&RealCommandRunner, repo_root, branch)
}

/// [`delete_branch`] against an injected runner.
pub(crate) fn delete_branch_with(
    runner: &dyn CommandRunner,
    repo_root: &Path,
    branch: &str,
) -> Result<(), PawError> {
    let cwd = repo_root.to_string_lossy();
    let output = runner
        .run("git", &["-C", &cwd, "branch", "-D", branch])
        .map_err(|e| PawError::BranchError(format!("failed to delete branch {branch}: {e}")))?;

    if !output.success {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PawError::BranchError(format!(
            "git branch -D failed for branch {branch}: {stderr}"
        )));
    }

    Ok(())
}

/// Excludes a file from git tracking by adding it to `info/exclude`.
///
/// This prevents the file from being tracked by git without modifying the
/// repository's `.gitignore` file, which is useful for worktree-specific
/// files that should not be committed. Idempotent: an entry already present is
/// not duplicated.
///
/// **Linked worktrees.** Git reads `info/exclude` from the *common* git
/// directory, never the per-worktree git directory, so for a linked worktree
/// (`<worktree>/.git` is a file pointing at `<common>/worktrees/<id>`) the
/// entry is written to the common dir's `info/exclude`. Writing it to the
/// per-worktree dir — as an earlier version did — has no effect: git silently
/// ignores it, so the path would remain stageable.
pub fn exclude_from_git(worktree_root: &Path, filename: &str) -> Result<(), PawError> {
    // Resolve the effective `info/exclude` path. For a normal repo this is
    // `<worktree>/.git/info/exclude`; for a linked worktree it is the common
    // dir's `info/exclude`.
    let dot_git = worktree_root.join(".git");
    let exclude_file = if dot_git.is_file() {
        let gitdir = std::fs::read_to_string(&dot_git)
            .ok()
            .and_then(|s| s.strip_prefix("gitdir: ").map(|s| s.trim().to_owned()))
            .unwrap_or_default();
        // A linked worktree's git dir is `<common>/worktrees/<id>`; strip the
        // last two components to reach the common dir.
        let worktree_git_dir = PathBuf::from(&gitdir);
        let common_dir = worktree_git_dir
            .parent()
            .and_then(Path::parent)
            .map_or_else(|| worktree_git_dir.clone(), Path::to_path_buf);
        common_dir.join("info").join("exclude")
    } else {
        dot_git.join("info").join("exclude")
    };

    // Read existing exclude patterns
    let existing = if exclude_file.exists() {
        std::fs::read_to_string(&exclude_file).unwrap_or_default()
    } else {
        String::new()
    };

    // Add the filename if not already present
    if !existing.lines().any(|line| line.trim() == filename) {
        let mut updated = existing;
        if !updated.ends_with('\n') && !updated.is_empty() {
            updated.push('\n');
        }
        updated.push_str(filename);
        updated.push('\n');

        // Create the `info/` directory if it doesn't exist.
        if let Some(parent) = exclude_file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                PawError::SessionError(format!("failed to create .git/info directory: {e}"))
            })?;
        }

        std::fs::write(&exclude_file, updated).map_err(|e| {
            PawError::SessionError(format!("failed to write to .git/info/exclude: {e}"))
        })?;
    }

    Ok(())
}

/// Marks a file as assume-unchanged in git's index.
///
/// This prevents `git add -A`, `git add .`, and `git commit -a` from
/// staging the file. Returns `Ok` even if the command fails, as this
/// is a belt-and-suspenders measure.
pub fn assume_unchanged(worktree_root: &Path, filename: &str) -> Result<(), PawError> {
    assume_unchanged_with(&RealCommandRunner, worktree_root, filename)
}

/// [`assume_unchanged`] against an injected runner.
// Always-`Ok` by design: the wrap mirrors the public sibling's `Result`
// contract, which callers already use with `?`, and this call is
// belt-and-suspenders so its failure is deliberately silent.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn assume_unchanged_with(
    runner: &dyn CommandRunner,
    worktree_root: &Path,
    filename: &str,
) -> Result<(), PawError> {
    // `CommandRunner::run` (captured) rather than `run_inheriting_stdio` so
    // git's "fatal: Unable to mark file" stderr (emitted when the file isn't
    // tracked) doesn't bleed through to the parent process. This is
    // belt-and-suspenders — failure is silent by design because
    // `exclude_from_git` is the primary protection for untracked AGENTS.md.
    let cwd = worktree_root.to_string_lossy();
    let _ = runner.run(
        "git",
        &["-C", &cwd, "update-index", "--assume-unchanged", filename],
    );
    Ok(())
}

/// Clears the assume-unchanged bit on a file in git's index.
///
/// Undoes a prior `git update-index --assume-unchanged`, so the file is
/// reported by `git status` and staged by `git add -A` again. Returns `Ok`
/// even if the command fails (e.g. the file is untracked, or no bit was set),
/// because this is a self-healing measure: worktrees created by an older
/// git-paw version may carry a stale assume-unchanged bit on `AGENTS.md`, and
/// clearing it on every start makes the upgrade transparent to the user.
pub fn no_assume_unchanged(worktree_root: &Path, filename: &str) -> Result<(), PawError> {
    no_assume_unchanged_with(&RealCommandRunner, worktree_root, filename)
}

/// [`no_assume_unchanged`] against an injected runner.
// Always-`Ok` by design — see [`assume_unchanged_with`].
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn no_assume_unchanged_with(
    runner: &dyn CommandRunner,
    worktree_root: &Path,
    filename: &str,
) -> Result<(), PawError> {
    // `CommandRunner::run` (captured) rather than `run_inheriting_stdio` so
    // git's stderr (emitted when the file isn't tracked) doesn't bleed through
    // to the parent process.
    let cwd = worktree_root.to_string_lossy();
    let _ = runner.run(
        "git",
        &[
            "-C",
            &cwd,
            "update-index",
            "--no-assume-unchanged",
            filename,
        ],
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use tempfile::TempDir;

    use crate::config::WorktreePlacement;
    use crate::error::PawError;
    use crate::git::{
        WorktreeCreation, branch_slug, create_worktree, worktree_display_path, worktree_path_for,
    };

    /// Sets up a temp repo with `origin/HEAD` pointing to `refs/heads/main`,
    /// an initial commit on `main`, and the `feat/example` branch at the same
    /// commit. The fixture is what `create_worktree` expects when called with
    /// `rebase_onto_main = true`.
    struct RebaseRepo {
        _sandbox: TempDir,
        repo: PathBuf,
    }

    impl RebaseRepo {
        fn path(&self) -> &Path {
            &self.repo
        }
    }

    fn run_git(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("run git command");
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn capture_git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("run git command");
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Builds a repo with `origin/main` tracking set up so `default_branch()`
    /// resolves cleanly. The repo is on `main` at one commit; `feat/example`
    /// is created at the same commit (caller advances either side as needed).
    fn setup_rebase_repo() -> RebaseRepo {
        let sandbox = TempDir::new().expect("tempdir");
        let bare = sandbox.path().join("bare.git");
        let repo = sandbox.path().join("repo");
        std::fs::create_dir_all(&bare).unwrap();

        run_git(&bare, &["init", "--bare", "-b", "main"]);

        // Clone the bare repo as a worktree-capable working repo.
        let status = Command::new("git")
            .args([
                "clone",
                bare.to_str().unwrap(),
                repo.to_str().unwrap(),
                "--origin",
                "origin",
            ])
            .status()
            .expect("git clone");
        assert!(status.success());

        run_git(&repo, &["config", "user.email", "test@test.com"]);
        run_git(&repo, &["config", "user.name", "Test"]);
        run_git(&repo, &["checkout", "-b", "main"]);
        std::fs::write(repo.join("a.txt"), "one\n").unwrap();
        run_git(&repo, &["add", "."]);
        run_git(&repo, &["commit", "-m", "init"]);
        run_git(&repo, &["push", "-u", "origin", "main"]);
        run_git(&bare, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        run_git(&repo, &["remote", "set-head", "origin", "main"]);
        run_git(&repo, &["branch", "feat/example"]);

        RebaseRepo {
            _sandbox: sandbox,
            repo,
        }
    }

    fn advance_main(repo: &Path, commits: usize) {
        for i in 0..commits {
            std::fs::write(repo.join(format!("main-{i}.txt")), format!("v{i}\n")).unwrap();
            run_git(repo, &["add", "."]);
            run_git(repo, &["commit", "-m", &format!("main commit {i}")]);
        }
    }

    fn head_sha(repo: &Path, branch: &str) -> String {
        capture_git(repo, &["rev-parse", branch])
    }

    #[test]
    fn create_worktree_rebases_branch_when_behind_main() {
        let r = setup_rebase_repo();
        advance_main(r.path(), 2);

        let result = create_worktree(r.path(), "feat/example", true, WorktreePlacement::Sibling)
            .expect("rebase succeeds");
        assert!(
            matches!(
                result,
                WorktreeCreation {
                    branch_created: false,
                    ..
                }
            ),
            "branch existed, branch_created must be false"
        );
        assert!(result.path.exists(), "worktree directory must be created");

        // feat/example contains main's commits → 0 commits in feat..main.
        let count = capture_git(r.path(), &["rev-list", "--count", "feat/example..main"]);
        assert_eq!(count, "0", "feat/example must include main's commits");
    }

    #[test]
    fn create_worktree_rebase_noop_when_branch_up_to_date() {
        let r = setup_rebase_repo();
        // Branch is already at main HEAD — rebase is a no-op.
        let before = head_sha(r.path(), "feat/example");
        let _result = create_worktree(r.path(), "feat/example", true, WorktreePlacement::Sibling)
            .expect("noop rebase succeeds");
        let after = head_sha(r.path(), "feat/example");
        assert_eq!(before, after, "noop rebase must not change HEAD");
    }

    #[test]
    fn create_worktree_rebase_conflict_aborts_and_errors() {
        let r = setup_rebase_repo();

        // Diverge: modify a.txt on feat/example, then modify the same line on
        // main with a different content. Rebase will conflict.
        run_git(r.path(), &["checkout", "feat/example"]);
        std::fs::write(r.path().join("a.txt"), "feat-version\n").unwrap();
        run_git(r.path(), &["add", "."]);
        run_git(r.path(), &["commit", "-m", "feat edit"]);
        run_git(r.path(), &["checkout", "main"]);
        std::fs::write(r.path().join("a.txt"), "main-version\n").unwrap();
        run_git(r.path(), &["add", "."]);
        run_git(r.path(), &["commit", "-m", "main edit"]);

        let pre = head_sha(r.path(), "feat/example");
        let result = create_worktree(r.path(), "feat/example", true, WorktreePlacement::Sibling);
        let err = result.expect_err("rebase must error on conflict");
        match err {
            PawError::WorktreeError(msg) => assert!(
                msg.contains("rebase onto main failed"),
                "expected 'rebase onto main failed' in error, got: {msg}"
            ),
            other => panic!("expected WorktreeError, got {other:?}"),
        }

        let post = head_sha(r.path(), "feat/example");
        assert_eq!(pre, post, "branch HEAD must be restored after abort");

        let git_dir = r.path().join(".git");
        assert!(
            !git_dir.join("rebase-merge").exists(),
            "rebase-merge dir must not survive abort"
        );
        assert!(
            !git_dir.join("rebase-apply").exists(),
            "rebase-apply dir must not survive abort"
        );
    }

    #[test]
    fn create_worktree_no_rebase_preserves_v0_5_behaviour() {
        let r = setup_rebase_repo();
        advance_main(r.path(), 2);

        let before = head_sha(r.path(), "feat/example");
        let result = create_worktree(r.path(), "feat/example", false, WorktreePlacement::Sibling)
            .expect("no-rebase path succeeds");
        let after = head_sha(r.path(), "feat/example");
        assert_eq!(before, after, "rebase_onto_main=false must not change HEAD");
        assert!(result.path.exists(), "worktree directory must be created");
    }

    #[test]
    fn create_worktree_new_branch_skips_rebase_regardless_of_flag() {
        let r = setup_rebase_repo();
        // feat/new does NOT exist locally.
        let result = create_worktree(r.path(), "feat/new", true, WorktreePlacement::Sibling)
            .expect("new-branch creation succeeds");
        assert!(
            matches!(
                result,
                WorktreeCreation {
                    branch_created: true,
                    ..
                }
            ),
            "new branch must report branch_created=true"
        );
        assert!(result.path.exists(), "worktree directory must be created");
    }

    #[cfg(unix)]
    #[test]
    fn remove_worktree_does_not_panic_on_non_utf8_path() {
        // Regression test for the previous `worktree_path.to_str().unwrap()`
        // panic at the call site in `remove_worktree`. A `PathBuf` built from
        // non-UTF-8 bytes (legal on Unix) must reach argv without ever
        // unwrapping to `&str` — today via `to_string_lossy()`, which replaces
        // the invalid bytes rather than panicking. The `git` invocation is
        // expected to fail (the path does not exist); the test asserts only
        // that we reach the failure path without panicking.
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        use std::path::PathBuf;

        use super::remove_worktree;

        let repo = tempfile::tempdir().expect("tempdir");

        // 0x66 0x80 0x66 — 0x80 is an invalid UTF-8 start byte.
        let non_utf8 = OsString::from_vec(vec![b'f', 0x80, b'f']);
        let worktree_path = PathBuf::from(non_utf8);

        // The call must return Err, not panic. `git worktree remove` will
        // fail because the path doesn't exist, but argv must be constructed
        // without unwrapping a non-UTF-8 path.
        let result = remove_worktree(repo.path(), &worktree_path);
        assert!(result.is_err(), "expected Err for non-existent worktree");
    }

    // --- worktree placement (worktree-embedded-placement) ---

    #[test]
    fn branch_slug_replaces_slash_with_dash() {
        assert_eq!(branch_slug("feat/auth-flow"), "feat-auth-flow");
        assert_eq!(branch_slug("a/b/c"), "a-b-c");
    }

    #[test]
    fn branch_slug_strips_unsafe_characters() {
        // `#` is outside [A-Za-z0-9._-] and is stripped (not replaced).
        assert_eq!(branch_slug("fix/issue#42"), "fix-issue42");
    }

    #[test]
    fn branch_slug_preserves_safe_punctuation() {
        // dot, underscore, and dash are all in the safe set.
        assert_eq!(branch_slug("release/v1.2_rc-3"), "release-v1.2_rc-3");
    }

    #[test]
    fn create_worktree_child_placement_creates_inside_repo() {
        let r = setup_rebase_repo();

        let result = create_worktree(r.path(), "feat/auth-flow", false, WorktreePlacement::Child)
            .expect("child worktree creation succeeds");

        let expected = r
            .path()
            .join(".git-paw")
            .join("worktrees")
            .join("feat-auth-flow");
        assert_eq!(result.path, expected, "child worktree path mismatch");
        assert!(result.path.exists(), "child worktree directory must exist");
        assert!(
            r.path().join(".git-paw").join("worktrees").is_dir(),
            ".git-paw/worktrees/ must be created"
        );
    }

    #[test]
    fn worktree_display_path_reflects_placement() {
        let r = setup_rebase_repo();
        // Child: rendered relative to the repo root — NOT as a `../` sibling
        // (the `--dry-run` bug this fixes: it hardcoded `../<dir>` regardless).
        let child = worktree_display_path(r.path(), "feat/auth-flow", WorktreePlacement::Child)
            .expect("child display path");
        assert_eq!(child, ".git-paw/worktrees/feat-auth-flow");
        assert!(
            !child.starts_with("../"),
            "a child worktree must not render as a sibling: {child}"
        );
        // Sibling: rendered as `../<project>-<slug>` beside the repo.
        let sibling = worktree_display_path(r.path(), "feat/auth-flow", WorktreePlacement::Sibling)
            .expect("sibling display path");
        assert!(
            sibling.starts_with("../") && sibling.ends_with("-feat-auth-flow"),
            "a sibling worktree renders beside the repo: {sibling}"
        );
    }

    #[test]
    fn worktree_path_for_creates_nothing() {
        let r = setup_rebase_repo();
        let path = worktree_path_for(r.path(), "feat/x", WorktreePlacement::Child)
            .expect("pure path derivation");
        // The preview derivation must not create the worktree (dry-run safety).
        assert!(
            !path.exists(),
            "worktree_path_for must not create anything: {}",
            path.display()
        );
    }

    #[test]
    fn create_worktree_child_slug_strips_unsafe_characters() {
        let r = setup_rebase_repo();

        let result = create_worktree(r.path(), "fix/issue#42", false, WorktreePlacement::Child)
            .expect("child worktree creation succeeds");

        assert!(
            result.path.ends_with(".git-paw/worktrees/fix-issue42"),
            "expected slug-derived child path, got {}",
            result.path.display()
        );
    }

    #[test]
    fn create_worktree_sibling_placement_creates_beside_repo() {
        let r = setup_rebase_repo();
        let project = super::project_name(r.path());

        let result = create_worktree(r.path(), "feature/test", false, WorktreePlacement::Sibling)
            .expect("sibling worktree creation succeeds");

        let expected = r
            .path()
            .parent()
            .unwrap()
            .join(format!("{project}-feature-test"));
        assert_eq!(result.path, expected, "sibling worktree path mismatch");
        assert!(
            result.path.exists(),
            "sibling worktree directory must exist"
        );
    }

    #[test]
    fn create_worktree_child_and_sibling_differ_for_same_branch() {
        // Sanity: the two placements resolve to different locations for the
        // same branch — child under the repo, sibling in the parent.
        let r = setup_rebase_repo();

        let child =
            create_worktree(r.path(), "feat/x", false, WorktreePlacement::Child).expect("child");
        let sibling = create_worktree(r.path(), "feat/y", false, WorktreePlacement::Sibling)
            .expect("sibling");

        assert!(
            child.path.starts_with(r.path()),
            "child must be inside repo"
        );
        assert!(
            !sibling.path.starts_with(r.path()),
            "sibling must be outside repo"
        );
    }

    /// Regression: a `git status --porcelain` entry whose path contains a
    /// newline must stay ONE record, not split into phantom "files." The old
    /// newline-splitting parser bled such content into bogus entries, which
    /// made `git paw remove` refuse a clean just-started worktree over a
    /// non-existent dirty file. `-z` (NUL-delimited) parsing fixes it.
    #[test]
    fn uncommitted_files_keeps_newline_bearing_path_as_one_record() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path();
        run_git(repo, &["init", "-b", "main"]);
        run_git(repo, &["config", "user.email", "test@test.com"]);
        run_git(repo, &["config", "user.name", "Test"]);
        std::fs::write(repo.join("tracked.txt"), "x\n").unwrap();
        run_git(repo, &["add", "."]);
        run_git(repo, &["commit", "-m", "init"]);

        // One untracked file whose NAME embeds a newline plus the exact boot
        // warning text that previously surfaced as a phantom `**WARNING:` path.
        let evil = repo.join("evil\n**WARNING: Do NOT publish");
        std::fs::write(&evil, "content\n").unwrap();

        let files = crate::git::uncommitted_files(repo).expect("uncommitted_files");
        assert_eq!(
            files.len(),
            1,
            "newline-bearing path must be one record, not split into phantoms: {files:?}"
        );
        assert!(
            files[0].contains("evil") && files[0].contains("**WARNING"),
            "the single record should carry the whole path: {files:?}"
        );
    }

    // --- CommandRunner seam (git-command-runner-seam) ---
    //
    // These exercise the `*_with(runner, …)` entry points against a
    // `FakeCommandRunner`: the argv each function hands to `git` is the
    // observable outbound contract of this module, and the scripted exit
    // status / stdout / stderr drives each success and failure branch. No real
    // `git` is spawned.

    mod seam {
        use std::path::Path;

        use tempfile::TempDir;

        use crate::command_runner::CommandOutput;
        use crate::command_runner::test_support::FakeCommandRunner;
        use crate::config::WorktreePlacement;
        use crate::error::PawError;
        use crate::specs::{SpecBackendKind, SpecEntry};

        /// A scripted git result: exit 0 with the given stdout.
        fn ok_with(stdout: &str) -> CommandOutput {
            CommandOutput {
                success: true,
                code: Some(0),
                stdout: stdout.as_bytes().to_vec(),
                stderr: Vec::new(),
            }
        }

        /// A scripted git result: exit `code` with the given stderr.
        fn fail_with(code: i32, stderr: &str) -> CommandOutput {
            CommandOutput {
                success: false,
                code: Some(code),
                stdout: Vec::new(),
                stderr: stderr.as_bytes().to_vec(),
            }
        }

        /// A runner that cannot spawn `git` at all.
        fn unspawnable() -> FakeCommandRunner {
            FakeCommandRunner::scripted(|_, _| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "git missing",
                ))
            })
        }

        /// The single `(program, argv)` the fake recorded.
        fn only_call(fake: &FakeCommandRunner) -> (String, Vec<String>) {
            let calls = fake.calls();
            assert_eq!(calls.len(), 1, "expected exactly one git invocation");
            calls.into_iter().next().unwrap()
        }

        /// Every recorded argv, in call order.
        fn argv_sequence(fake: &FakeCommandRunner) -> Vec<Vec<String>> {
            fake.calls().into_iter().map(|(_, argv)| argv).collect()
        }

        fn spec_entry(id: &str) -> SpecEntry {
            SpecEntry {
                id: id.to_string(),
                backend: SpecBackendKind::OpenSpec,
                branch: format!("feat/{id}"),
                cli: None,
                prompt: String::new(),
                owned_files: None,
            }
        }

        /// One row per single-shot entry point: the exact argv it builds,
        /// including the leading `-C <cwd>` that carries the working directory
        /// the inline calls set with `Command::current_dir`.
        #[test]
        #[allow(clippy::too_many_lines)] // one row per entry point — a data table
        fn each_entry_point_builds_its_exact_git_argv() {
            type Invoke = Box<dyn Fn(&FakeCommandRunner)>;
            let repo = Path::new("/repo");
            let cases: Vec<(&str, Vec<&str>, Invoke)> = vec![
                (
                    "validate_repo",
                    vec!["-C", "/repo", "rev-parse", "--show-toplevel"],
                    Box::new(|r| {
                        let _ = super::super::validate_repo_with(r, repo);
                    }),
                ),
                (
                    "list_branches",
                    vec!["-C", "/repo", "branch", "-a", "--format=%(refname:short)"],
                    Box::new(|r| {
                        let _ = super::super::list_branches_with(r, repo);
                    }),
                ),
                (
                    "default_branch",
                    vec!["-C", "/repo", "symbolic-ref", "refs/remotes/origin/HEAD"],
                    Box::new(|r| {
                        let _ = super::super::default_branch_with(r, repo);
                    }),
                ),
                (
                    "current_branch",
                    vec!["-C", "/repo", "branch", "--show-current"],
                    Box::new(|r| {
                        let _ = super::super::current_branch_with(r, repo);
                    }),
                ),
                (
                    "find_worktree_for_branch",
                    vec!["-C", "/repo", "worktree", "list", "--porcelain"],
                    Box::new(|r| {
                        let _ = super::super::find_worktree_for_branch_with(r, repo, "feat/x");
                    }),
                ),
                (
                    "remove_worktree",
                    vec![
                        "-C",
                        "/repo",
                        "worktree",
                        "remove",
                        "--force",
                        "/repo/.git-paw/worktrees/feat-x",
                    ],
                    Box::new(|r| {
                        let _ = super::super::remove_worktree_with(
                            r,
                            repo,
                            Path::new("/repo/.git-paw/worktrees/feat-x"),
                        );
                    }),
                ),
                (
                    "prune_worktrees",
                    vec!["-C", "/repo", "worktree", "prune"],
                    Box::new(|r| {
                        let _ = super::super::prune_worktrees_with(r, repo);
                    }),
                ),
                (
                    "uncommitted_files",
                    vec!["-C", "/repo", "status", "--porcelain", "-z"],
                    Box::new(|r| {
                        let _ = super::super::uncommitted_files_with(r, repo);
                    }),
                ),
                (
                    "merge_branch",
                    vec!["-C", "/repo", "merge", "--no-ff", "--no-commit", "feat/x"],
                    Box::new(|r| {
                        let _ = super::super::merge_branch_with(r, repo, "feat/x");
                    }),
                ),
                (
                    "delete_branch",
                    vec!["-C", "/repo", "branch", "-D", "feat/x"],
                    Box::new(|r| {
                        let _ = super::super::delete_branch_with(r, repo, "feat/x");
                    }),
                ),
                (
                    "assume_unchanged",
                    vec![
                        "-C",
                        "/repo",
                        "update-index",
                        "--assume-unchanged",
                        "AGENTS.md",
                    ],
                    Box::new(|r| {
                        let _ = super::super::assume_unchanged_with(r, repo, "AGENTS.md");
                    }),
                ),
                (
                    "no_assume_unchanged",
                    vec![
                        "-C",
                        "/repo",
                        "update-index",
                        "--no-assume-unchanged",
                        "AGENTS.md",
                    ],
                    Box::new(|r| {
                        let _ = super::super::no_assume_unchanged_with(r, repo, "AGENTS.md");
                    }),
                ),
            ];

            for (label, expected, invoke) in cases {
                let fake = FakeCommandRunner::succeeding("");
                invoke(&fake);
                let (program, argv) = only_call(&fake);
                assert_eq!(program, "git", "{label}: must invoke git");
                assert_eq!(argv, expected, "{label}: argv mismatch");
            }
        }

        #[test]
        fn validate_repo_reports_the_toplevel_and_maps_failure_to_not_a_git_repo() {
            let inside = FakeCommandRunner::succeeding("/repo/root\n");
            assert_eq!(
                super::super::validate_repo_with(&inside, Path::new("/repo/root/sub")).unwrap(),
                Path::new("/repo/root")
            );

            let outside = FakeCommandRunner::failing("fatal: not a git repository");
            assert!(
                matches!(
                    super::super::validate_repo_with(&outside, Path::new("/tmp")),
                    Err(PawError::NotAGitRepo)
                ),
                "a non-zero rev-parse means the path is not in a repo"
            );
        }

        #[test]
        fn list_branches_dedupes_strips_remote_prefixes_and_drops_head() {
            let fake = FakeCommandRunner::succeeding(
                "main\nfeat/x\norigin/main\norigin/feat/y\nrefs/remotes/origin/feat/z\norigin/HEAD\n\n",
            );
            assert_eq!(
                super::super::list_branches_with(&fake, Path::new("/repo")).unwrap(),
                ["feat/x", "feat/y", "feat/z", "main"],
                "remote prefixes strip, duplicates collapse, HEAD pointers drop"
            );

            let broken = FakeCommandRunner::failing("fatal: not a git repository");
            let err = super::super::list_branches_with(&broken, Path::new("/repo")).unwrap_err();
            assert!(
                matches!(&err, PawError::BranchError(m)
                    if m.contains("git branch failed") && m.contains("not a git repository")),
                "the failure must carry git's stderr: {err:?}"
            );
        }

        #[test]
        fn default_branch_strips_the_ref_prefix_and_rejects_an_unexpected_ref() {
            let fake = FakeCommandRunner::succeeding("refs/remotes/origin/trunk\n");
            assert_eq!(
                super::super::default_branch_with(&fake, Path::new("/repo")).unwrap(),
                "trunk"
            );

            let odd = FakeCommandRunner::succeeding("refs/heads/main\n");
            let err = super::super::default_branch_with(&odd, Path::new("/repo")).unwrap_err();
            assert!(
                matches!(&err, PawError::BranchError(m) if m.contains("unexpected ref format")),
                "a ref outside refs/remotes/origin/ is not a default branch: {err:?}"
            );

            let no_head = FakeCommandRunner::failing(
                "fatal: ref refs/remotes/origin/HEAD is not a symbolic ref",
            );
            let err = super::super::default_branch_with(&no_head, Path::new("/repo")).unwrap_err();
            assert!(
                matches!(&err, PawError::BranchError(m) if m.contains("git symbolic-ref failed")),
                "unexpected error: {err:?}"
            );
        }

        #[test]
        fn current_branch_trims_the_name_and_rejects_a_detached_head() {
            let on_branch = FakeCommandRunner::succeeding("feat/add-auth\n");
            assert_eq!(
                super::super::current_branch_with(&on_branch, Path::new("/repo")).unwrap(),
                "feat/add-auth"
            );

            // `git branch --show-current` succeeds with empty output on a
            // detached HEAD.
            let detached = FakeCommandRunner::succeeding("\n");
            let err = super::super::current_branch_with(&detached, Path::new("/repo")).unwrap_err();
            assert!(
                matches!(&err, PawError::BranchError(m) if m.contains("detached HEAD")),
                "empty output means detached HEAD: {err:?}"
            );
        }

        #[test]
        fn merge_branch_maps_exit_one_to_conflict_and_other_failures_to_an_error() {
            let clean = FakeCommandRunner::succeeding("");
            assert!(
                super::super::merge_branch_with(&clean, Path::new("/repo"), "feat/x").unwrap(),
                "a successful merge reports true"
            );

            let conflicted =
                FakeCommandRunner::scripted(|_, _| Ok(fail_with(1, "CONFLICT in a.txt")));
            assert!(
                !super::super::merge_branch_with(&conflicted, Path::new("/repo"), "feat/x")
                    .unwrap(),
                "exit 1 is a conflict, not an error"
            );

            let broken =
                FakeCommandRunner::scripted(|_, _| Ok(fail_with(128, "fatal: not something")));
            let err =
                super::super::merge_branch_with(&broken, Path::new("/repo"), "feat/x").unwrap_err();
            assert!(
                matches!(&err, PawError::WorktreeError(m)
                    if m.contains("git merge failed") && m.contains("not something")),
                "any exit other than 1 is a real failure: {err:?}"
            );
        }

        #[test]
        fn uncommitted_files_parses_nul_records_and_consumes_a_rename_origin() {
            // Records: modified, untracked, and a rename whose SECOND field is
            // the origin path — which must be consumed, not reported.
            let fake = FakeCommandRunner::succeeding(
                " M src/git.rs\0?? notes.txt\0R  new/name.rs\0old/name.rs\0",
            );
            assert_eq!(
                super::super::uncommitted_files_with(&fake, Path::new("/wt")).unwrap(),
                ["src/git.rs", "notes.txt", "new/name.rs"],
                "a rename reports the new path only"
            );

            let clean = FakeCommandRunner::succeeding("");
            assert!(
                super::super::uncommitted_files_with(&clean, Path::new("/wt"))
                    .unwrap()
                    .is_empty(),
                "no records means a clean worktree"
            );

            let broken = FakeCommandRunner::failing("fatal: not a git repository");
            let err = super::super::uncommitted_files_with(&broken, Path::new("/wt")).unwrap_err();
            assert!(
                matches!(&err, PawError::WorktreeError(m)
                    if m.contains("git status failed in /wt")),
                "the failure must name the worktree: {err:?}"
            );
        }

        #[test]
        fn worktree_and_branch_failures_surface_gits_stderr() {
            let fake = FakeCommandRunner::failing("fatal: refusing to do that");

            let err = super::super::remove_worktree_with(
                &fake,
                Path::new("/repo"),
                Path::new("/wt/feat-x"),
            )
            .unwrap_err();
            assert!(
                matches!(&err, PawError::WorktreeError(m)
                    if m.contains("/wt/feat-x") && m.contains("refusing to do that")),
                "remove must name the worktree and carry stderr: {err:?}"
            );

            let err = super::super::prune_worktrees_with(&fake, Path::new("/repo")).unwrap_err();
            assert!(
                matches!(&err, PawError::WorktreeError(m)
                    if m.contains("git worktree prune failed") && m.contains("refusing to do that")),
                "unexpected error: {err:?}"
            );

            let err =
                super::super::delete_branch_with(&fake, Path::new("/repo"), "feat/x").unwrap_err();
            assert!(
                matches!(&err, PawError::BranchError(m)
                    if m.contains("feat/x") && m.contains("refusing to do that")),
                "delete must name the branch and carry stderr: {err:?}"
            );
        }

        #[test]
        fn a_spawn_failure_surfaces_as_a_paw_error() {
            let broken = unspawnable();

            assert!(
                matches!(
                    super::super::validate_repo_with(&broken, Path::new("/repo")),
                    Err(PawError::BranchError(ref m)) if m.contains("failed to run git")
                ),
                "an unspawnable git is an error, not 'not a repo'"
            );
            assert!(
                matches!(
                    super::super::uncommitted_files_with(&broken, Path::new("/wt")),
                    Err(PawError::WorktreeError(ref m)) if m.contains("failed to run git status")
                ),
                "unexpected result"
            );
        }

        #[test]
        fn the_update_index_helpers_stay_silent_when_git_fails() {
            // Belt-and-suspenders by design: `exclude_from_git` is the primary
            // protection, so a failing `update-index` must not surface.
            let fake = FakeCommandRunner::failing("fatal: Unable to mark file AGENTS.md");
            assert!(
                super::super::assume_unchanged_with(&fake, Path::new("/wt"), "AGENTS.md").is_ok()
            );
            assert!(
                super::super::no_assume_unchanged_with(&fake, Path::new("/wt"), "AGENTS.md")
                    .is_ok()
            );

            let broken = unspawnable();
            assert!(
                super::super::assume_unchanged_with(&broken, Path::new("/wt"), "AGENTS.md").is_ok(),
                "even an unspawnable git must stay silent"
            );
        }

        #[test]
        fn check_uncommitted_specs_probes_each_existing_spec_path() {
            let sandbox = TempDir::new().expect("tempdir");
            let repo = sandbox.path();
            std::fs::create_dir_all(repo.join("specs").join("alpha")).unwrap();
            std::fs::write(repo.join("specs").join("beta.md"), "# beta\n").unwrap();

            let specs = [
                spec_entry("alpha"),
                spec_entry("beta"),
                spec_entry("absent"),
            ];
            let fake = FakeCommandRunner::scripted(|_, args| {
                if args.last() == Some(&"specs/alpha") {
                    Ok(ok_with(" M specs/alpha/spec.md\n"))
                } else {
                    Ok(ok_with(""))
                }
            });

            assert_eq!(
                super::super::check_uncommitted_specs_with(&fake, repo, &specs).unwrap(),
                ["alpha"],
                "only the spec with porcelain output is reported dirty"
            );

            let cwd = repo.to_string_lossy().into_owned();
            let expected: Vec<Vec<&str>> = vec![
                vec!["-C", &cwd, "status", "--porcelain", "--", "specs/alpha"],
                vec!["-C", &cwd, "status", "--porcelain", "--", "specs/beta.md"],
            ];
            assert_eq!(
                argv_sequence(&fake),
                expected,
                "a directory spec probes the directory, a file spec the file, \
                 and a spec present in neither layout is skipped without a git call"
            );
        }

        #[test]
        fn create_worktree_adds_the_existing_branch_at_the_resolved_path() {
            let sandbox = TempDir::new().expect("tempdir");
            let repo = sandbox.path();

            let fake = FakeCommandRunner::succeeding("");
            let created = super::super::create_worktree_with(
                &fake,
                repo,
                "feat/x",
                false,
                WorktreePlacement::Child,
            )
            .expect("a succeeding worktree add");

            let expected_path = repo.join(".git-paw").join("worktrees").join("feat-x");
            assert_eq!(created.path, expected_path);
            assert!(
                !created.branch_created,
                "the plain `worktree add` path reuses an existing branch"
            );

            let cwd = repo.to_string_lossy().into_owned();
            let target = expected_path.to_string_lossy().into_owned();
            let expected: Vec<&str> = vec!["-C", &cwd, "worktree", "add", &target, "feat/x"];
            assert_eq!(only_call(&fake).1, expected);
        }

        #[test]
        fn create_worktree_retries_with_dash_b_on_an_invalid_reference() {
            let sandbox = TempDir::new().expect("tempdir");
            let repo = sandbox.path();

            let fake = FakeCommandRunner::scripted(|_, args| {
                if args.contains(&"-b") {
                    Ok(ok_with(""))
                } else {
                    Ok(fail_with(128, "fatal: invalid reference: feat/new"))
                }
            });
            let created = super::super::create_worktree_with(
                &fake,
                repo,
                "feat/new",
                false,
                WorktreePlacement::Child,
            )
            .expect("the -b retry succeeds");
            assert!(
                created.branch_created,
                "the -b path created the branch, so it must be reported"
            );

            let cwd = repo.to_string_lossy().into_owned();
            let target = repo
                .join(".git-paw")
                .join("worktrees")
                .join("feat-new")
                .to_string_lossy()
                .into_owned();
            let expected: Vec<Vec<&str>> = vec![
                vec!["-C", &cwd, "worktree", "add", &target, "feat/new"],
                vec!["-C", &cwd, "worktree", "add", "-b", "feat/new", &target],
            ];
            assert_eq!(argv_sequence(&fake), expected);
        }

        #[test]
        fn create_worktree_does_not_retry_a_failure_that_is_not_an_invalid_reference() {
            let sandbox = TempDir::new().expect("tempdir");
            let repo = sandbox.path();

            let fake = FakeCommandRunner::failing("fatal: '/wt/feat-x' already exists");
            let err = super::super::create_worktree_with(
                &fake,
                repo,
                "feat/x",
                false,
                WorktreePlacement::Child,
            )
            .unwrap_err();
            assert!(
                matches!(&err, PawError::WorktreeError(m)
                    if m.contains("git worktree add failed for branch 'feat/x'")
                        && m.contains("already exists")),
                "unexpected error: {err:?}"
            );
            assert_eq!(
                argv_sequence(&fake).len(),
                1,
                "only an 'invalid reference' failure justifies the -b retry"
            );
        }

        #[test]
        fn rebase_onto_default_switches_head_when_the_branch_is_not_checked_out() {
            let fake = FakeCommandRunner::scripted(|_, args| match args {
                [.., "symbolic-ref", "refs/remotes/origin/HEAD"] => {
                    Ok(ok_with("refs/remotes/origin/main\n"))
                }
                // The branch is checked out nowhere.
                [.., "worktree", "list", "--porcelain"] => {
                    Ok(ok_with("worktree /repo\nbranch refs/heads/main\n\n"))
                }
                [.., "symbolic-ref", "--short", "HEAD"] => Ok(ok_with("main\n")),
                _ => Ok(ok_with("")),
            });

            super::super::rebase_branch_onto_default_with(&fake, Path::new("/repo"), "feat/x")
                .expect("a succeeding rebase");

            let expected: Vec<Vec<&str>> = vec![
                vec!["-C", "/repo", "symbolic-ref", "refs/remotes/origin/HEAD"],
                vec!["-C", "/repo", "worktree", "list", "--porcelain"],
                vec!["-C", "/repo", "symbolic-ref", "--short", "HEAD"],
                // Branch operand present because HEAD was switched for us.
                vec!["-C", "/repo", "rebase", "main", "feat/x"],
                // The original HEAD is restored afterwards.
                vec!["-C", "/repo", "checkout", "main"],
            ];
            assert_eq!(argv_sequence(&fake), expected);
        }

        #[test]
        fn rebase_onto_default_runs_inside_the_worktree_holding_the_branch() {
            let fake = FakeCommandRunner::scripted(|_, args| match args {
                [.., "symbolic-ref", "refs/remotes/origin/HEAD"] => {
                    Ok(ok_with("refs/remotes/origin/main\n"))
                }
                [.., "worktree", "list", "--porcelain"] => Ok(ok_with(
                    "worktree /repo\nbranch refs/heads/main\n\nworktree /wt/feat-x\nbranch refs/heads/feat/x\n\n",
                )),
                _ => Ok(ok_with("")),
            });

            super::super::rebase_branch_onto_default_with(&fake, Path::new("/repo"), "feat/x")
                .expect("a succeeding rebase");

            let expected: Vec<Vec<&str>> = vec![
                vec!["-C", "/repo", "symbolic-ref", "refs/remotes/origin/HEAD"],
                vec!["-C", "/repo", "worktree", "list", "--porcelain"],
                // Runs in the occupied worktree, with no branch operand and no
                // HEAD probe or restore in the main repo.
                vec!["-C", "/wt/feat-x", "rebase", "main"],
            ];
            assert_eq!(argv_sequence(&fake), expected);
        }

        #[test]
        fn rebase_onto_default_aborts_and_restores_head_on_conflict() {
            let fake = FakeCommandRunner::scripted(|_, args| match args {
                [.., "symbolic-ref", "refs/remotes/origin/HEAD"] => {
                    Ok(ok_with("refs/remotes/origin/main\n"))
                }
                [.., "worktree", "list", "--porcelain"] => {
                    Ok(ok_with("worktree /repo\nbranch refs/heads/main\n\n"))
                }
                [.., "symbolic-ref", "--short", "HEAD"] => Ok(ok_with("main\n")),
                [.., "rebase", "main", "feat/x"] => {
                    Ok(fail_with(1, "CONFLICT (content): Merge conflict in a.txt"))
                }
                _ => Ok(ok_with("")),
            });

            let err =
                super::super::rebase_branch_onto_default_with(&fake, Path::new("/repo"), "feat/x")
                    .unwrap_err();
            assert!(
                matches!(&err, PawError::WorktreeError(m)
                    if m.contains("rebase onto main failed") && m.contains("CONFLICT")),
                "the error must carry git's stderr: {err:?}"
            );

            let expected: Vec<Vec<&str>> = vec![
                vec!["-C", "/repo", "symbolic-ref", "refs/remotes/origin/HEAD"],
                vec!["-C", "/repo", "worktree", "list", "--porcelain"],
                vec!["-C", "/repo", "symbolic-ref", "--short", "HEAD"],
                vec!["-C", "/repo", "rebase", "main", "feat/x"],
                // The abort and the HEAD restore both run after the failure.
                vec!["-C", "/repo", "rebase", "--abort"],
                vec!["-C", "/repo", "checkout", "main"],
            ];
            assert_eq!(argv_sequence(&fake), expected);
        }

        #[test]
        fn create_worktree_skips_the_rebase_when_the_branch_does_not_exist_locally() {
            let sandbox = TempDir::new().expect("tempdir");
            let repo = sandbox.path();

            let fake = FakeCommandRunner::scripted(|_, args| {
                if args.contains(&"--verify") {
                    Ok(fail_with(128, "fatal: Needed a single revision"))
                } else {
                    Ok(ok_with(""))
                }
            });
            super::super::create_worktree_with(
                &fake,
                repo,
                "feat/new",
                true,
                WorktreePlacement::Child,
            )
            .expect("worktree add succeeds");

            let cwd = repo.to_string_lossy().into_owned();
            let target = repo
                .join(".git-paw")
                .join("worktrees")
                .join("feat-new")
                .to_string_lossy()
                .into_owned();
            let expected: Vec<Vec<&str>> = vec![
                vec!["-C", &cwd, "rev-parse", "--verify", "refs/heads/feat/new"],
                // No rebase calls: the existence probe failed, so the whole
                // rebase step is skipped even with rebase_onto_main = true.
                vec!["-C", &cwd, "worktree", "add", &target, "feat/new"],
            ];
            assert_eq!(argv_sequence(&fake), expected);
        }
    }
}
