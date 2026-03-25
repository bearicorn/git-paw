//! Git worktree lifecycle integration tests.
//!
//! Tests git operations against real temporary git repositories using `tempfile`.

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use git_paw::git;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A test sandbox that owns an outer temp directory containing the git repo.
/// Worktrees created via `create_worktree` land as siblings of the repo inside
/// this outer dir, so everything is cleaned up when the sandbox is dropped.
struct TestRepo {
    _sandbox: TempDir,
    repo: PathBuf,
}

impl TestRepo {
    fn path(&self) -> &Path {
        &self.repo
    }
}

/// Creates a temporary git repository with an initial commit.
fn setup_test_repo() -> TestRepo {
    let sandbox = TempDir::new().expect("create temp dir");
    let repo = sandbox.path().join("test-repo");
    std::fs::create_dir_all(&repo).expect("create repo dir");

    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.email", "test@test.com"]);
    run_git(&repo, &["config", "user.name", "Test"]);

    // Create an initial commit so HEAD exists
    let readme = repo.join("README.md");
    std::fs::write(&readme, "# Test repo").expect("write README");
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "initial commit"]);

    TestRepo {
        _sandbox: sandbox,
        repo,
    }
}

/// Creates a branch in the test repo without switching to it.
fn create_branch(repo: &Path, name: &str) {
    run_git(repo, &["branch", name]);
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

// ---------------------------------------------------------------------------
// Repository validation
// ---------------------------------------------------------------------------

#[test]
fn validate_repo_succeeds_inside_git_repo() {
    let tr = setup_test_repo();
    let root = git::validate_repo(tr.path()).expect("should succeed");
    assert_eq!(root, tr.path().canonicalize().unwrap());
}

#[test]
fn validate_repo_fails_outside_git_repo() {
    let tmp = TempDir::new().expect("create temp dir");
    let result = git::validate_repo(tmp.path());
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Branch listing
// ---------------------------------------------------------------------------

#[test]
fn list_branches_includes_created_branches() {
    let tr = setup_test_repo();
    create_branch(tr.path(), "feature/auth");
    create_branch(tr.path(), "fix/db");

    let branches = git::list_branches(tr.path()).expect("list branches");

    assert!(branches.contains(&"feature/auth".to_string()));
    assert!(branches.contains(&"fix/db".to_string()));
}

#[test]
fn list_branches_returns_sorted() {
    let tr = setup_test_repo();
    create_branch(tr.path(), "z-last");
    create_branch(tr.path(), "a-first");

    let branches = git::list_branches(tr.path()).expect("list branches");

    let a_pos = branches.iter().position(|b| b == "a-first").unwrap();
    let z_pos = branches.iter().position(|b| b == "z-last").unwrap();
    assert!(a_pos < z_pos, "branches should be sorted alphabetically");
}

#[test]
fn list_branches_deduplicates_local_and_remote() {
    let tr = setup_test_repo();

    // Default branch (main/master) should appear only once
    let branches = git::list_branches(tr.path()).expect("list branches");
    let default_branch = &branches[0]; // whatever the default is
    let count = branches.iter().filter(|b| *b == default_branch).count();
    assert_eq!(count, 1, "default branch should appear exactly once");
}

// ---------------------------------------------------------------------------
// Worktree lifecycle
// ---------------------------------------------------------------------------

#[test]
fn create_and_remove_worktree() {
    let tr = setup_test_repo();
    create_branch(tr.path(), "feature/test-wt");

    // Create worktree
    let wt_path = git::create_worktree(tr.path(), "feature/test-wt").expect("create worktree");
    assert!(wt_path.exists(), "worktree directory should exist");
    assert!(
        wt_path.join("README.md").exists(),
        "worktree should contain repo files"
    );

    // Remove worktree
    git::remove_worktree(tr.path(), &wt_path).expect("remove worktree");
    assert!(!wt_path.exists(), "worktree directory should be removed");
}

#[test]
fn worktree_placed_as_sibling_of_repo() {
    let tr = setup_test_repo();
    create_branch(tr.path(), "feature/sibling");

    let wt_path = git::create_worktree(tr.path(), "feature/sibling").expect("create worktree");

    // Worktree should be in the same parent directory as the repo
    assert_eq!(
        wt_path.parent().unwrap(),
        tr.path().parent().unwrap(),
        "worktree should be a sibling of the repo"
    );

    // Clean up
    git::remove_worktree(tr.path(), &wt_path).expect("remove worktree");
}

#[test]
fn create_worktree_fails_for_checked_out_branch() {
    let tr = setup_test_repo();

    // Try to create a worktree for the currently checked out branch
    let output = Command::new("git")
        .current_dir(tr.path())
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .expect("get current branch");
    let current = String::from_utf8_lossy(&output.stdout).trim().to_string();

    let result = git::create_worktree(tr.path(), &current);
    assert!(result.is_err(), "should fail for checked-out branch");
}

// ---------------------------------------------------------------------------
// Directory naming
// ---------------------------------------------------------------------------

#[test]
fn project_name_from_repo_path() {
    let tr = setup_test_repo();
    let name = git::project_name(tr.path());
    assert_eq!(name, "test-repo");
}

#[test]
fn worktree_dir_name_replaces_slashes() {
    let name = git::worktree_dir_name("my-project", "feature/auth-flow");
    assert_eq!(name, "my-project-feature-auth-flow");
}

#[test]
fn worktree_dir_name_strips_unsafe_chars() {
    let name = git::worktree_dir_name("proj", "feat/special@chars!");
    // Only alphanumeric, -, _, . should survive
    assert!(!name.contains('@'));
    assert!(!name.contains('!'));
}

#[test]
fn worktree_dir_name_handles_nested_slashes() {
    let name = git::worktree_dir_name("proj", "feature/deep/nested/branch");
    assert_eq!(name, "proj-feature-deep-nested-branch");
}
