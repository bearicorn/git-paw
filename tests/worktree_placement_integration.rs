//! Worktree-placement round-trip integration tests.
//!
//! Exercises the `worktree-embedded-placement` capability end-to-end: a
//! worktree created under either placement records its concrete path in the
//! session JSON, survives a save/reload, and is torn down at that recorded
//! path regardless of the configured placement (so a config flip never
//! orphans an existing session's worktree). All isolated via `tempfile`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use tempfile::TempDir;

use git_paw::config::WorktreePlacement;
use git_paw::git;
use git_paw::session::{
    Session, SessionMode, SessionStatus, WorktreeEntry, load_session_from, save_session_in,
};

struct TestRepo {
    _sandbox: TempDir,
    repo: PathBuf,
}

impl TestRepo {
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

fn setup_test_repo() -> TestRepo {
    let sandbox = TempDir::new().expect("create temp dir");
    let repo = sandbox.path().join("test-repo");
    std::fs::create_dir_all(&repo).expect("create repo dir");

    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.email", "test@test.com"]);
    run_git(&repo, &["config", "user.name", "Test"]);
    std::fs::write(repo.join("README.md"), "# Test repo").expect("write README");
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "initial commit"]);

    TestRepo {
        _sandbox: sandbox,
        repo,
    }
}

/// Builds a one-worktree session pointing at `wt_path` on branch `branch`.
fn session_with_worktree(repo: &Path, branch: &str, wt_path: &Path) -> Session {
    Session {
        session_name: "paw-placement-test".to_string(),
        repo_path: repo.to_path_buf(),
        project_name: "test-repo".to_string(),
        created_at: SystemTime::now(),
        status: SessionStatus::Active,
        worktrees: vec![WorktreeEntry {
            branch: branch.to_string(),
            worktree_path: wt_path.to_path_buf(),
            cli: "claude".to_string(),
            branch_created: true,
            pending_boot_prompt: None,
        }],
        broker_port: None,
        broker_bind: None,
        broker_log_path: None,
        mode: SessionMode::Bare,
        dashboard_pane: None,
    }
}

#[test]
fn child_layout_session_round_trips_through_save_reload_purge() {
    let tr = setup_test_repo();
    let sessions = TempDir::new().expect("sessions dir");

    let wt = git::create_worktree(tr.path(), "feat/x", false, WorktreePlacement::Child)
        .expect("create child worktree");
    let expected = tr.path().join(".git-paw").join("worktrees").join("feat-x");
    assert_eq!(wt.path, expected, "child worktree path mismatch");

    let session = session_with_worktree(tr.path(), "feat/x", &wt.path);
    save_session_in(&session, sessions.path()).expect("save session");

    let loaded = load_session_from(&session.session_name, sessions.path())
        .expect("load session")
        .expect("session exists");
    assert_eq!(
        loaded.worktrees[0].worktree_path, expected,
        "reloaded session must report the recorded child path"
    );

    git::remove_worktree(tr.path(), &loaded.worktrees[0].worktree_path)
        .expect("purge removes the worktree at the recorded child path");
    assert!(
        !expected.exists(),
        "child worktree must be gone after purge at the recorded path"
    );
}

#[test]
fn sibling_layout_session_round_trips_through_save_reload_purge() {
    let tr = setup_test_repo();
    let sessions = TempDir::new().expect("sessions dir");

    let wt = git::create_worktree(tr.path(), "feat/y", false, WorktreePlacement::Sibling)
        .expect("create sibling worktree");
    let expected = tr.path().parent().unwrap().join("test-repo-feat-y");
    assert_eq!(wt.path, expected, "sibling worktree path mismatch");

    let session = session_with_worktree(tr.path(), "feat/y", &wt.path);
    save_session_in(&session, sessions.path()).expect("save session");

    let loaded = load_session_from(&session.session_name, sessions.path())
        .expect("load session")
        .expect("session exists");
    assert_eq!(
        loaded.worktrees[0].worktree_path, expected,
        "reloaded session must report the recorded sibling path"
    );

    git::remove_worktree(tr.path(), &loaded.worktrees[0].worktree_path)
        .expect("purge removes the worktree at the recorded sibling path");
    assert!(
        !expected.exists(),
        "sibling worktree must be gone after purge at the recorded path"
    );
}

#[test]
fn config_flip_purges_at_recorded_path_not_rederived() {
    // A session created under sibling placement, then the config flips to
    // child. Purge must operate on the recorded sibling path, NOT a path
    // re-derived from the (now child) placement.
    let tr = setup_test_repo();
    let sessions = TempDir::new().expect("sessions dir");

    let wt = git::create_worktree(tr.path(), "feat/z", false, WorktreePlacement::Sibling)
        .expect("create sibling worktree");
    let sibling_path = tr.path().parent().unwrap().join("test-repo-feat-z");
    assert_eq!(wt.path, sibling_path);

    let session = session_with_worktree(tr.path(), "feat/z", &wt.path);
    save_session_in(&session, sessions.path()).expect("save session");

    let loaded = load_session_from(&session.session_name, sessions.path())
        .expect("load session")
        .expect("session exists");

    // The (hypothetical) re-derived child path must NOT be what purge targets.
    let rederived_child = tr.path().join(".git-paw").join("worktrees").join("feat-z");
    assert_ne!(
        loaded.worktrees[0].worktree_path, rederived_child,
        "recorded path must remain the sibling path after a config flip"
    );
    assert_eq!(loaded.worktrees[0].worktree_path, sibling_path);

    git::remove_worktree(tr.path(), &loaded.worktrees[0].worktree_path)
        .expect("purge removes at the recorded sibling path");
    assert!(!sibling_path.exists(), "sibling worktree must be removed");
    assert!(
        !rederived_child.exists(),
        "no child worktree should ever have been created"
    );
}
