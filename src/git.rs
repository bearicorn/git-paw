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

/// Creates a git worktree for `branch`.
///
/// If the branch already exists, checks it out in a new worktree. If the
/// branch does not exist, creates it from HEAD with `git worktree add -b`.
/// Returns both the worktree path and whether the branch was newly created,
/// so the session can track which branches to delete on purge.
pub fn create_worktree(repo_root: &Path, branch: &str) -> Result<WorktreeCreation, PawError> {
    let project = project_name(repo_root);
    let dir_name = worktree_dir_name(&project, branch);

    let parent = repo_root.parent().ok_or_else(|| {
        PawError::WorktreeError("cannot determine parent directory of repo".to_string())
    })?;
    let worktree_path = parent.join(&dir_name);

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
///
/// # Panics
///
/// Panics if the worktree path contains non-Unicode characters.
pub fn remove_worktree(repo_root: &Path, worktree_path: &Path) -> Result<(), PawError> {
    // Always pass --force per `git-operations/spec.md`'s "SHALL force-remove a
    // worktree" requirement. `remove_worktree` is only called from purge,
    // which is destructive by nature: an agent that produced uncommitted or
    // untracked files in its worktree would otherwise trip "contains modified
    // or untracked files, use --force to delete it" and leak the worktree on
    // disk even though the user already typed `--force` at the CLI.
    let output = Command::new("git")
        .current_dir(repo_root)
        .args([
            "worktree",
            "remove",
            "--force",
            worktree_path.to_str().unwrap(),
        ])
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
    let _ = std::process::Command::new("git")
        .current_dir(worktree_root)
        .args(["update-index", "--assume-unchanged", filename])
        .status();
    Ok(())
}
