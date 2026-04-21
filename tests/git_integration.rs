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
    let wt = git::create_worktree(tr.path(), "feature/test-wt").expect("create worktree");
    assert!(wt.path.exists(), "worktree directory should exist");
    assert!(
        wt.path.join("README.md").exists(),
        "worktree should contain repo files"
    );
    assert!(
        !wt.branch_created,
        "branch already existed, should not be marked as created"
    );

    // Remove worktree
    git::remove_worktree(tr.path(), &wt.path).expect("remove worktree");
    assert!(!wt.path.exists(), "worktree directory should be removed");
}

#[test]
fn remove_worktree_force_removes_dirty_worktree() {
    // Regression: prior to passing --force, a worktree with uncommitted or
    // untracked content tripped "contains modified or untracked files, use
    // --force to delete it" and leaked the directory on disk even when the
    // user invoked `git paw purge --force`. The spec at
    // openspec/specs/git-operations/spec.md:113 mandates that
    // `remove_worktree` SHALL force-remove a worktree.
    let tr = setup_test_repo();
    create_branch(tr.path(), "feature/dirty");

    let wt = git::create_worktree(tr.path(), "feature/dirty").expect("create worktree");

    // Make the worktree dirty in two ways: modify a tracked file AND add a
    // brand-new untracked file. Both individually trip non-force removal.
    std::fs::write(wt.path.join("README.md"), "modified by agent\n")
        .expect("modify tracked file in worktree");
    std::fs::write(wt.path.join("scratch.txt"), "untracked agent output\n")
        .expect("write untracked file in worktree");

    git::remove_worktree(tr.path(), &wt.path)
        .expect("remove worktree must succeed even when dirty");
    assert!(
        !wt.path.exists(),
        "dirty worktree directory must be removed when --force is passed"
    );
}

#[test]
fn worktree_placed_as_sibling_of_repo() {
    let tr = setup_test_repo();
    create_branch(tr.path(), "feature/sibling");

    let wt = git::create_worktree(tr.path(), "feature/sibling").expect("create worktree");

    // Worktree should be in the same parent directory as the repo
    assert_eq!(
        wt.path.parent().unwrap(),
        tr.path().parent().unwrap(),
        "worktree should be a sibling of the repo"
    );

    // Clean up
    git::remove_worktree(tr.path(), &wt.path).expect("remove worktree");
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

// ---------------------------------------------------------------------------
// Remote branch deduplication and prefix stripping
// ---------------------------------------------------------------------------

#[test]
fn list_branches_strips_remote_prefix_and_deduplicates() {
    // Create a bare repo to act as the remote
    let sandbox = TempDir::new().expect("create sandbox");
    let bare_path = sandbox.path().join("bare.git");
    std::fs::create_dir_all(&bare_path).expect("create bare dir");

    Command::new("git")
        .current_dir(&bare_path)
        .args(["init", "--bare"])
        .output()
        .expect("init bare repo");

    // Clone the bare repo to get a local with remote tracking
    let clone_path = sandbox.path().join("clone");
    Command::new("git")
        .args([
            "clone",
            bare_path.to_string_lossy().as_ref(),
            clone_path.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("clone repo");

    run_git(&clone_path, &["config", "user.email", "test@test.com"]);
    run_git(&clone_path, &["config", "user.name", "Test"]);

    // Initial commit + push
    std::fs::write(clone_path.join("README.md"), "# test").expect("write file");
    run_git(&clone_path, &["add", "."]);
    run_git(&clone_path, &["commit", "-m", "initial"]);

    // Detect the default branch name (main or master depending on git version)
    let output = Command::new("git")
        .current_dir(&clone_path)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .expect("get default branch");
    let default_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    run_git(&clone_path, &["push", "-u", "origin", &default_branch]);

    // Create a feature branch locally and push so it exists as both
    // local and remote-tracking (origin/feature/auth)
    run_git(&clone_path, &["checkout", "-b", "feature/auth"]);
    run_git(&clone_path, &["push", "-u", "origin", "feature/auth"]);
    run_git(&clone_path, &["checkout", &default_branch]);

    let branches = git::list_branches(&clone_path).expect("list branches");

    // Each branch should appear exactly once (deduplicated across local + remote)
    let auth_count = branches.iter().filter(|b| *b == "feature/auth").count();
    assert_eq!(
        auth_count, 1,
        "feature/auth should appear once (deduplicated), got: {branches:?}"
    );

    let default_count = branches.iter().filter(|b| *b == &default_branch).count();
    assert_eq!(
        default_count, 1,
        "{default_branch} should appear once (deduplicated), got: {branches:?}"
    );

    // No branch should retain the origin/ prefix
    assert!(
        branches.iter().all(|b| !b.starts_with("origin/")),
        "no branch should have origin/ prefix, got: {branches:?}"
    );
}

// ---------------------------------------------------------------------------
// AGENTS.md injection protection in real worktrees
// ---------------------------------------------------------------------------

#[test]
fn agents_md_injection_not_staged_by_git_add_in_worktree() {
    // Setup: create a repo with a tracked AGENTS.md (like any project using
    // the Linux Foundation AGENTS.md standard).
    let tr = setup_test_repo();
    std::fs::write(tr.path().join("AGENTS.md"), "# Project Rules\n").unwrap();
    run_git(tr.path(), &["add", "AGENTS.md"]);
    run_git(tr.path(), &["commit", "-m", "add agents"]);

    // Create a real worktree (this is what git paw start does)
    create_branch(tr.path(), "feat/test-injection");
    let wt = git::create_worktree(tr.path(), "feat/test-injection").expect("create worktree");

    // Inject session content (this is what setup_worktree_agents_md does)
    let assignment = git_paw::agents::WorktreeAssignment {
        branch: "feat/test-injection".to_string(),
        cli: "claude".to_string(),
        spec_content: Some("Implement the widget.\n".to_string()),
        owned_files: Some(vec!["src/widget.rs".to_string()]),
        skill_content: None,
        inter_agent_rules: None,
    };
    git_paw::agents::setup_worktree_agents_md(tr.path(), &wt.path, &assignment)
        .expect("inject agents md");

    // Verify AGENTS.md was modified (session content injected)
    let content = std::fs::read_to_string(wt.path.join("AGENTS.md")).unwrap();
    assert!(
        content.contains("feat/test-injection"),
        "AGENTS.md should contain injected session content"
    );

    // THE CRITICAL ASSERTION: git add -A should NOT stage AGENTS.md
    run_git(&wt.path, &["add", "-A"]);
    let output = Command::new("git")
        .current_dir(&wt.path)
        .args(["diff", "--cached", "--name-only"])
        .output()
        .expect("git diff --cached");
    let staged = String::from_utf8_lossy(&output.stdout);
    assert!(
        !staged.contains("AGENTS.md"),
        "AGENTS.md should NOT be staged after git add -A, but got: {staged}"
    );

    // Also verify git status doesn't show it
    let output = Command::new("git")
        .current_dir(&wt.path)
        .args(["status", "--porcelain"])
        .output()
        .expect("git status");
    let status = String::from_utf8_lossy(&output.stdout);
    assert!(
        !status.contains("AGENTS.md"),
        "AGENTS.md should not appear in git status, got: {status}"
    );

    // Cleanup
    git::remove_worktree(tr.path(), &wt.path).expect("remove worktree");
}
