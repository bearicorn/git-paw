//! Git operations.
//!
//! Validates git repositories, lists branches, creates and removes worktrees,
//! and derives worktree directory names from project and branch names.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::PawError;
use crate::specs::SpecEntry;

/// Validates that the given path is inside a git repository.
///
/// Returns the absolute path to the repository root.
pub fn validate_repo(path: &Path) -> Result<PathBuf, PawError> {
    let output = Command::new("git")
        .current_dir(path)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| PawError::BranchError(format!("failed to run git: {e}")))?;

    if !output.status.success() {
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
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["branch", "-a", "--format=%(refname:short)"])
        .output()
        .map_err(|e| PawError::BranchError(format!("failed to run git branch: {e}")))?;

    if !output.status.success() {
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

/// Returns the name of the default branch (usually "main" or "master").
pub fn default_branch(repo_root: &Path) -> Result<String, PawError> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
        .output()
        .map_err(|e| PawError::BranchError(format!("failed to run git symbolic-ref: {e}")))?;

    if !output.status.success() {
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
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["branch", "--show-current"])
        .output()
        .map_err(|e| PawError::BranchError(format!("failed to run git branch: {e}")))?;

    if !output.status.success() {
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
fn find_worktree_for_branch(repo_root: &Path, branch: &str) -> Result<Option<PathBuf>, PawError> {
    let list = Command::new("git")
        .current_dir(repo_root)
        .args(["worktree", "list", "--porcelain"])
        .output()
        .map_err(|e| PawError::WorktreeError(format!("failed to run git worktree list: {e}")))?;
    if !list.status.success() {
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
fn rebase_branch_onto_default(repo_root: &Path, branch: &str) -> Result<(), PawError> {
    let default = default_branch(repo_root)?;

    let occupied_at = find_worktree_for_branch(repo_root, branch)?;
    let (workdir, original_head): (PathBuf, Option<String>) = if let Some(wt) = occupied_at {
        (wt, None)
    } else {
        let original = Command::new("git")
            .current_dir(repo_root)
            .args(["symbolic-ref", "--short", "HEAD"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        (repo_root.to_path_buf(), original)
    };

    let mut invocation = Command::new("git");
    invocation.current_dir(&workdir);
    if original_head.is_some() {
        invocation.args(["rebase", &default, branch]);
    } else {
        invocation.args(["rebase", &default]);
    }
    let output = invocation
        .output()
        .map_err(|e| PawError::WorktreeError(format!("failed to run git rebase: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let _ = Command::new("git")
            .current_dir(&workdir)
            .args(["rebase", "--abort"])
            .output();
        if let Some(orig) = &original_head
            && orig != branch
        {
            let _ = Command::new("git")
                .current_dir(repo_root)
                .args(["checkout", orig])
                .output();
        }
        return Err(PawError::WorktreeError(format!(
            "rebase onto main failed: {stderr}"
        )));
    }

    if let Some(orig) = original_head
        && orig != branch
    {
        let _ = Command::new("git")
            .current_dir(repo_root)
            .args(["checkout", &orig])
            .output();
    }

    Ok(())
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
pub fn create_worktree(
    repo_root: &Path,
    branch: &str,
    rebase_onto_main: bool,
) -> Result<WorktreeCreation, PawError> {
    let project = project_name(repo_root);
    let dir_name = worktree_dir_name(&project, branch);

    let parent = repo_root.parent().ok_or_else(|| {
        PawError::WorktreeError("cannot determine parent directory of repo".to_string())
    })?;
    let worktree_path = parent.join(&dir_name);

    // Rebase agent branch onto the repo's default branch BEFORE the
    // idempotency check. Resolves MILESTONE.md drift item 48: the supervisor
    // advances main while agents are running, so on resume (or fresh launch
    // of an existing branch) the agent's worktree would otherwise be N
    // commits behind main and every subsequent commit chains from a stale
    // baseline. Order matters: rebasing before the idempotency check means a
    // surviving worktree's branch ref is updated transparently on resume.
    if rebase_onto_main {
        let branch_exists = Command::new("git")
            .current_dir(repo_root)
            .args(["rev-parse", "--verify", &format!("refs/heads/{branch}")])
            .output()
            .is_ok_and(|o| o.status.success());
        if branch_exists {
            rebase_branch_onto_default(repo_root, branch)?;
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
        let list = Command::new("git")
            .current_dir(repo_root)
            .args(["worktree", "list", "--porcelain"])
            .output()
            .map_err(|e| {
                PawError::WorktreeError(format!("failed to run git worktree list: {e}"))
            })?;
        if list.status.success() {
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
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["worktree", "add", &worktree_path.to_string_lossy(), branch])
        .output()
        .map_err(|e| PawError::WorktreeError(format!("failed to run git worktree add: {e}")))?;

    if output.status.success() {
        return Ok(WorktreeCreation {
            path: worktree_path,
            branch_created: false,
        });
    }

    let stderr = String::from_utf8_lossy(&output.stderr);

    // If the branch doesn't exist, create it with -b.
    if stderr.contains("invalid reference") {
        let output = Command::new("git")
            .current_dir(repo_root)
            .args([
                "worktree",
                "add",
                "-b",
                branch,
                &worktree_path.to_string_lossy(),
            ])
            .output()
            .map_err(|e| {
                PawError::WorktreeError(format!("failed to run git worktree add -b: {e}"))
            })?;

        if output.status.success() {
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
    // Always pass --force per `git-operations/spec.md`'s "SHALL force-remove a
    // worktree" requirement. `remove_worktree` is only called from purge,
    // which is destructive by nature: an agent that produced uncommitted or
    // untracked files in its worktree would otherwise trip "contains modified
    // or untracked files, use --force to delete it" and leak the worktree on
    // disk even though the user already typed `--force` at the CLI.
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["worktree", "remove", "--force"])
        .arg(worktree_path.as_os_str())
        .output()
        .map_err(|e| {
            PawError::WorktreeError(format!(
                "failed to remove worktree at {}: {e}",
                worktree_path.display()
            ))
        })?;

    if !output.status.success() {
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
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["worktree", "prune"])
        .output()
        .map_err(|e| PawError::WorktreeError(format!("failed to prune worktrees: {e}")))?;

    if !output.status.success() {
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
    let mut uncommitted_specs = Vec::new();

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

        let output = Command::new("git")
            .current_dir(repo_root)
            .args(["status", "--porcelain", "--", &porcelain_target])
            .output()
            .map_err(|e| {
                PawError::BranchError(format!(
                    "failed to run git status for spec {}: {e}",
                    spec.id
                ))
            })?;

        if !output.status.success() {
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
/// Runs `git status --porcelain` inside the worktree and parses each line's
/// path (the portion after the 3-character XY-status prefix). Untracked,
/// modified, staged, and renamed entries are all included. An empty vec means
/// the worktree is clean. Used by `git paw remove`'s uncommitted-work safety
/// check (design D7) to tell the user exactly what would be lost.
pub fn uncommitted_files(worktree_root: &Path) -> Result<Vec<String>, PawError> {
    let output = Command::new("git")
        .current_dir(worktree_root)
        .args(["status", "--porcelain"])
        .output()
        .map_err(|e| {
            PawError::WorktreeError(format!(
                "failed to run git status in {}: {e}",
                worktree_root.display()
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PawError::WorktreeError(format!(
            "git status failed in {}: {stderr}",
            worktree_root.display()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut files = Vec::new();
    for line in stdout.lines() {
        if line.len() <= 3 {
            continue;
        }
        // Porcelain v1 format: `XY <path>` (or `XY <old> -> <new>` for
        // renames). The path starts at byte 3. For renames, report the new
        // path (the portion after `-> `).
        let path = &line[3..];
        let reported = path.rsplit(" -> ").next().unwrap_or(path);
        files.push(reported.trim().to_string());
    }
    Ok(files)
}

/// Merges the specified branch into the current branch.
///
/// Returns `true` if the merge was successful, `false` if there were conflicts.
pub fn merge_branch(repo_root: &Path, branch: &str) -> Result<bool, PawError> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["merge", "--no-ff", "--no-commit", branch])
        .output()
        .map_err(|e| {
            PawError::WorktreeError(format!("failed to run git merge for branch {branch}: {e}"))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Check if this is a conflict (exit code 1) vs other error
        if output.status.code() == Some(1) {
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
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["branch", "-D", branch])
        .output()
        .map_err(|e| PawError::BranchError(format!("failed to delete branch {branch}: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PawError::BranchError(format!(
            "git branch -D failed for branch {branch}: {stderr}"
        )));
    }

    Ok(())
}

/// Excludes a file from git tracking by adding it to `.git/info/exclude`.
///
/// This prevents the file from being tracked by git without modifying the
/// repository's `.gitignore` file, which is useful for worktree-specific
/// files that should not be committed.
pub fn exclude_from_git(worktree_root: &Path, filename: &str) -> Result<(), PawError> {
    let exclude_file = worktree_root.join(".git/info/exclude");

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

        // Create .git/info directory if it doesn't exist
        if let Some(parent) = exclude_file.parent() {
            // Check if .git (the grandparent) is a file (worktree case)
            if let Some(git_dir) = parent.parent()
                && git_dir.is_file()
            {
                // This is a worktree - .git is a file pointing to main repo
                // The actual git directory is inside the main repo
                let main_git_dir = std::fs::read_to_string(git_dir)
                    .ok()
                    .and_then(|s| s.strip_prefix("gitdir: ").map(|s| s.trim().to_owned()))
                    .unwrap_or_default();
                let main_git_info = PathBuf::from(main_git_dir).join("info");
                if !main_git_info.try_exists().unwrap_or(false) {
                    std::fs::create_dir_all(&main_git_info).map_err(|e| {
                        PawError::SessionError(format!("failed to create main .git/info: {e}"))
                    })?;
                }
                let main_exclude = main_git_info.join("exclude");
                std::fs::write(&main_exclude, updated).map_err(|e| {
                    PawError::SessionError(format!(
                        "failed to write to main .git/info/exclude: {e}"
                    ))
                })?;
                return Ok(());
            }
            if parent.exists() && parent.is_file() {
                std::fs::remove_file(parent).map_err(|e| {
                    PawError::SessionError(format!("failed to remove .git/info file: {e}"))
                })?;
            }
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
    // `.output()` rather than `.status()` so git's "fatal: Unable to mark
    // file" stderr (emitted when the file isn't tracked) doesn't bleed
    // through to the parent process. This is belt-and-suspenders — failure
    // is silent by design because `exclude_from_git` is the primary
    // protection for untracked AGENTS.md.
    let _ = std::process::Command::new("git")
        .current_dir(worktree_root)
        .args(["update-index", "--assume-unchanged", filename])
        .output();
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use tempfile::TempDir;

    use crate::error::PawError;
    use crate::git::{WorktreeCreation, create_worktree};

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

        let result = create_worktree(r.path(), "feat/example", true).expect("rebase succeeds");
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
        let _result =
            create_worktree(r.path(), "feat/example", true).expect("noop rebase succeeds");
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
        let result = create_worktree(r.path(), "feat/example", true);
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
        let result =
            create_worktree(r.path(), "feat/example", false).expect("no-rebase path succeeds");
        let after = head_sha(r.path(), "feat/example");
        assert_eq!(before, after, "rebase_onto_main=false must not change HEAD");
        assert!(result.path.exists(), "worktree directory must be created");
    }

    #[test]
    fn create_worktree_new_branch_skips_rebase_regardless_of_flag() {
        let r = setup_rebase_repo();
        // feat/new does NOT exist locally.
        let result =
            create_worktree(r.path(), "feat/new", true).expect("new-branch creation succeeds");
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
        // non-UTF-8 bytes (legal on Unix) must flow through `Command::arg(...)`
        // via `as_os_str()` without ever unwrapping to `&str`. The `git`
        // invocation is expected to fail (the path does not exist); the test
        // asserts only that we reach the failure path without panicking.
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
}
