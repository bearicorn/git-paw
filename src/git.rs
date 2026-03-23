//! Git operations.
//!
//! Validates git repositories, lists branches, creates and removes worktrees,
//! and derives worktree directory names from project and branch names.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::PawError;

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
    Ok(parse_branch_output(&stdout))
}

/// Parses `git branch -a --format=%(refname:short)` output into a
/// deduplicated, sorted list of branch names with remote prefixes stripped.
fn parse_branch_output(output: &str) -> Vec<String> {
    let mut branches = BTreeSet::new();

    for line in output.lines() {
        let name = line.trim();
        if name.is_empty() {
            continue;
        }
        // Skip HEAD pointers like "origin/HEAD"
        if name.contains("HEAD") {
            continue;
        }
        // Strip remote prefix (e.g., "origin/feature/auth" → "feature/auth")
        let stripped = strip_remote_prefix(name);
        branches.insert(stripped.to_string());
    }

    branches.into_iter().collect()
}

/// Strips the remote prefix from a branch name.
///
/// `origin/feature/auth` becomes `feature/auth`.
/// `feature/auth` stays as `feature/auth`.
fn strip_remote_prefix(branch: &str) -> &str {
    // With --format=%(refname:short), remote branches appear as "origin/branch"
    // We need to strip the first component if it looks like a remote name
    if let Some(rest) = branch.strip_prefix("origin/") {
        rest
    } else {
        branch
    }
}

/// Derives the project name from the repository root path.
///
/// Uses the final component of the path (the directory name).
pub fn project_name(repo_root: &Path) -> String {
    repo_root.file_name().map_or_else(
        || "project".to_string(),
        |n| n.to_string_lossy().to_string(),
    )
}

/// Builds the worktree directory name from a project name and branch.
///
/// Replaces `/` with `-` and strips characters that are unsafe for directory
/// names.
///
/// # Examples
///
/// - `("git-paw", "feature/auth-flow")` → `"git-paw-feature-auth-flow"`
/// - `("git-paw", "fix/db")` → `"git-paw-fix-db"`
pub fn worktree_dir_name(project: &str, branch: &str) -> String {
    let sanitized: String = branch
        .chars()
        .map(|c| if c == '/' { '-' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect();

    format!("{project}-{sanitized}")
}

/// Creates a git worktree for the given branch.
///
/// The worktree is placed in the parent directory of `repo_root`, named using
/// [`worktree_dir_name`]. Returns the path to the created worktree.
pub fn create_worktree(repo_root: &Path, branch: &str) -> Result<PathBuf, PawError> {
    let project = project_name(repo_root);
    let dir_name = worktree_dir_name(&project, branch);

    let parent = repo_root.parent().ok_or_else(|| {
        PawError::WorktreeError("cannot determine parent directory of repo".to_string())
    })?;
    let worktree_path = parent.join(&dir_name);

    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["worktree", "add", &worktree_path.to_string_lossy(), branch])
        .output()
        .map_err(|e| PawError::WorktreeError(format!("failed to run git worktree add: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PawError::WorktreeError(format!(
            "git worktree add failed for branch '{branch}': {stderr}"
        )));
    }

    Ok(worktree_path)
}

/// Removes a git worktree at the given path.
///
/// Runs `git worktree remove --force` and then prunes stale worktree entries.
pub fn remove_worktree(repo_root: &Path, worktree_path: &Path) -> Result<(), PawError> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args([
            "worktree",
            "remove",
            "--force",
            &worktree_path.to_string_lossy(),
        ])
        .output()
        .map_err(|e| PawError::WorktreeError(format!("failed to run git worktree remove: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PawError::WorktreeError(format!(
            "git worktree remove failed: {stderr}"
        )));
    }

    // Prune stale worktree entries
    let _ = Command::new("git")
        .current_dir(repo_root)
        .args(["worktree", "prune"])
        .output();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::process::Command;
    use tempfile::TempDir;

    /// A test sandbox that owns an outer temp directory containing the git
    /// repo. Worktrees created via `create_worktree` land as siblings of the
    /// repo inside this outer dir, so everything is cleaned up when the
    /// sandbox is dropped — even if a test panics.
    struct TestRepo {
        _sandbox: TempDir,
        repo: PathBuf,
    }

    impl TestRepo {
        fn path(&self) -> &Path {
            &self.repo
        }
    }

    /// Creates a temporary git repository inside a sandbox directory.
    ///
    /// The repo lives at `<sandbox>/repo/` so that worktrees created at
    /// `../<project>-<branch>/` land inside `<sandbox>/` and are automatically
    /// cleaned up when the returned `TestRepo` is dropped.
    fn setup_test_repo() -> TestRepo {
        let sandbox = TempDir::new().expect("create sandbox dir");
        let repo = sandbox.path().join("repo");
        std::fs::create_dir(&repo).expect("create repo dir");

        Command::new("git")
            .current_dir(&repo)
            .args(["init"])
            .output()
            .expect("git init");

        Command::new("git")
            .current_dir(&repo)
            .args(["config", "user.email", "test@test.com"])
            .output()
            .expect("git config email");

        Command::new("git")
            .current_dir(&repo)
            .args(["config", "user.name", "Test"])
            .output()
            .expect("git config name");

        // Create initial commit so branches work
        std::fs::write(repo.join("README.md"), "# test").expect("write file");
        Command::new("git")
            .current_dir(&repo)
            .args(["add", "."])
            .output()
            .expect("git add");
        Command::new("git")
            .current_dir(&repo)
            .args(["commit", "-m", "initial"])
            .output()
            .expect("git commit");

        TestRepo {
            _sandbox: sandbox,
            repo,
        }
    }

    // --- validate_repo ---

    #[test]
    #[serial]
    fn validate_repo_returns_root_inside_repo() {
        let repo = setup_test_repo();
        let result = validate_repo(repo.path());
        assert!(result.is_ok());
        let root = result.unwrap();
        // The returned root should match the repo dir (canonicalize for symlinks)
        assert_eq!(
            root.canonicalize().unwrap(),
            repo.path().canonicalize().unwrap()
        );
    }

    #[test]
    fn validate_repo_returns_not_a_git_repo_outside() {
        let dir = TempDir::new().expect("create temp dir");
        let result = validate_repo(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, PawError::NotAGitRepo),
            "expected NotAGitRepo, got: {err}"
        );
    }

    // --- list_branches ---

    #[test]
    #[serial]
    fn list_branches_returns_sorted_branches() {
        let repo = setup_test_repo();

        // Create branches in non-alphabetical order
        for branch in ["zebra", "alpha", "feature/auth"] {
            Command::new("git")
                .current_dir(repo.path())
                .args(["branch", branch])
                .output()
                .expect("create branch");
        }

        let branches = list_branches(repo.path()).expect("list branches");

        assert_eq!(
            branches,
            vec!["alpha", "feature/auth", "main", "zebra"],
            "branches should be sorted alphabetically"
        );
    }

    #[test]
    fn parse_branch_output_deduplicates_local_and_remote() {
        // Simulate git branch -a output where main exists both locally and as origin/main
        let output = "main\nfeature/auth\norigin/main\norigin/feature/auth\norigin/HEAD\n";
        let branches = parse_branch_output(output);
        assert_eq!(
            branches,
            vec!["feature/auth", "main"],
            "should deduplicate and exclude HEAD"
        );
    }

    #[test]
    fn parse_branch_output_strips_remote_prefix() {
        let output = "origin/feature/deep/nested\n";
        let branches = parse_branch_output(output);
        assert_eq!(branches, vec!["feature/deep/nested"]);
    }

    #[test]
    fn parse_branch_output_empty_input() {
        let branches = parse_branch_output("");
        assert!(branches.is_empty());
    }

    // --- project_name ---

    #[test]
    fn project_name_from_path() {
        assert_eq!(
            project_name(Path::new("/Users/jie/code/git-paw")),
            "git-paw"
        );
    }

    #[test]
    fn project_name_fallback_for_root() {
        assert_eq!(project_name(Path::new("/")), "project");
    }

    // --- worktree_dir_name ---

    #[test]
    fn worktree_dir_name_replaces_slash_with_dash() {
        assert_eq!(
            worktree_dir_name("git-paw", "feature/auth-flow"),
            "git-paw-feature-auth-flow"
        );
    }

    #[test]
    fn worktree_dir_name_handles_multiple_slashes() {
        assert_eq!(
            worktree_dir_name("git-paw", "feat/auth/v2"),
            "git-paw-feat-auth-v2"
        );
    }

    #[test]
    fn worktree_dir_name_strips_special_chars() {
        assert_eq!(
            worktree_dir_name("my-proj", "fix/issue#42"),
            "my-proj-fix-issue42"
        );
    }

    #[test]
    fn worktree_dir_name_simple_branch() {
        assert_eq!(worktree_dir_name("git-paw", "main"), "git-paw-main");
    }

    // --- strip_remote_prefix ---

    #[test]
    fn strip_remote_prefix_removes_origin() {
        assert_eq!(strip_remote_prefix("origin/feature/auth"), "feature/auth");
    }

    #[test]
    fn strip_remote_prefix_preserves_local() {
        assert_eq!(strip_remote_prefix("feature/auth"), "feature/auth");
    }

    #[test]
    fn strip_remote_prefix_origin_simple() {
        assert_eq!(strip_remote_prefix("origin/main"), "main");
    }

    // --- create_worktree ---

    #[test]
    #[serial]
    fn create_worktree_at_correct_path() {
        let test_repo = setup_test_repo();
        let repo_root = test_repo.path();

        Command::new("git")
            .current_dir(repo_root)
            .args(["branch", "feature/test"])
            .output()
            .expect("create branch");

        let worktree_path = create_worktree(repo_root, "feature/test").expect("create worktree");

        // Verify path follows ../<project>-<sanitized-branch> convention
        let expected_dir_name = worktree_dir_name(&project_name(repo_root), "feature/test");
        assert_eq!(
            worktree_path.file_name().unwrap().to_string_lossy(),
            expected_dir_name,
            "worktree should be at ../<project>-feature-test"
        );
        assert_eq!(
            worktree_path.parent().unwrap().canonicalize().unwrap(),
            repo_root.parent().unwrap().canonicalize().unwrap(),
            "worktree should be in the parent of repo root"
        );

        // Verify files exist
        assert!(worktree_path.exists());
        assert!(worktree_path.join("README.md").exists());

        // Cleanup
        remove_worktree(repo_root, &worktree_path).expect("remove worktree");
    }

    #[test]
    #[serial]
    fn create_worktree_errors_on_checked_out_branch() {
        let test_repo = setup_test_repo();
        let repo_root = test_repo.path();

        let output = Command::new("git")
            .current_dir(repo_root)
            .args(["branch", "--show-current"])
            .output()
            .expect("get branch");
        let current = String::from_utf8_lossy(&output.stdout).trim().to_string();

        let result = create_worktree(repo_root, &current);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, PawError::WorktreeError(_)),
            "expected WorktreeError, got: {err}"
        );
    }

    // --- remove_worktree ---

    #[test]
    #[serial]
    fn remove_worktree_cleans_up_fully() {
        let test_repo = setup_test_repo();
        let repo_root = test_repo.path();

        Command::new("git")
            .current_dir(repo_root)
            .args(["branch", "feature/cleanup"])
            .output()
            .expect("create branch");

        let worktree_path = create_worktree(repo_root, "feature/cleanup").expect("create worktree");
        assert!(worktree_path.exists());

        remove_worktree(repo_root, &worktree_path).expect("remove worktree");

        assert!(
            !worktree_path.exists(),
            "worktree directory should be removed"
        );

        // Verify git no longer tracks this worktree
        let output = Command::new("git")
            .current_dir(repo_root)
            .args(["worktree", "list", "--porcelain"])
            .output()
            .expect("list worktrees");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.contains("feature/cleanup"),
            "worktree should not appear in git worktree list"
        );
    }
}
