//! Shared test helpers for integration tests.
//!
//! Provides utilities used across multiple integration test files, such as
//! temporary git repository creation and PATH manipulation helpers.

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// A test sandbox containing a git repository.
///
/// The repo lives at `<sandbox>/repo/` so that worktrees created at
/// `../<project>-<branch>/` land inside `<sandbox>/` and are automatically
/// cleaned up when dropped.
pub struct TestRepo {
    _sandbox: TempDir,
    repo: PathBuf,
}

impl TestRepo {
    /// Returns the path to the git repository root.
    pub fn path(&self) -> &Path {
        &self.repo
    }

    /// Creates a branch in the test repository.
    pub fn create_branch(&self, name: &str) {
        Command::new("git")
            .current_dir(&self.repo)
            .args(["branch", name])
            .output()
            .unwrap_or_else(|_| panic!("failed to create branch '{name}'"));
    }
}

/// Creates a temporary git repository with an initial commit.
///
/// The repo is nested inside a sandbox directory so worktrees land as siblings
/// and are cleaned up automatically.
pub fn setup_test_repo() -> TestRepo {
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

/// Creates a temporary directory with fake executable binaries.
///
/// Returns the `TempDir` (must be held alive) and its path.
pub fn fake_path_with_binaries(names: &[&str]) -> (TempDir, PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().expect("create temp dir");
    for name in names {
        let bin_path = dir.path().join(name);
        std::fs::write(&bin_path, "#!/bin/sh\n").expect("write fake binary");
        std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755))
            .expect("set permissions");
    }
    let path = dir.path().to_path_buf();
    (dir, path)
}
