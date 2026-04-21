//! Integration tests for `git_paw::merge_loop::run_merge_loop`.
//!
//! Validates the merge ordering, test-command execution, and per-branch
//! reporting that the supervisor relies on at session-end.
//!
//! Note: the production `run_merge_loop` does not currently publish
//! `merged` / `merge_failed` broker messages — those variants do not yet
//! exist in `BrokerMessage` (see `openspec/changes/fix-supervisor-merge-loop`).
//! When that lands, an additional assertion on broker-state messages
//! belongs here.

use std::path::Path;
use std::process::Command as StdCommand;
use std::time::SystemTime;

use git_paw::config::BrokerConfig;
use git_paw::merge_loop::{MergeResults, run_merge_loop};
use git_paw::session::{Session, SessionStatus, WorktreeEntry};

mod helpers;
use helpers::*;

/// Runs `git -C <repo> <args>...` and asserts success.
fn git(repo: &Path, args: &[&str]) {
    let st = StdCommand::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        st.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&st.stderr)
    );
}

/// Sets up a repo with branches `a`, `b`, `c`, each adding a distinct file
/// from `main`. The branches do not depend on each other (and have no
/// conflicting changes), so any topological order is a valid merge order.
/// We keep the repo's default branch as `main`.
fn setup_three_branch_repo() -> TestRepo {
    let tr = setup_test_repo();
    let repo = tr.path();

    // Rename the default branch to `main` so `run_merge_loop`'s
    // `git checkout main` works regardless of git's default.
    git(repo, &["branch", "-M", "main"]);

    // Branch a: add file a.txt
    git(repo, &["checkout", "-b", "a"]);
    std::fs::write(repo.join("a.txt"), "a\n").unwrap();
    git(repo, &["add", "a.txt"]);
    git(repo, &["commit", "-m", "feat a"]);
    git(repo, &["checkout", "main"]);

    // Branch b: add file b.txt
    git(repo, &["checkout", "-b", "b"]);
    std::fs::write(repo.join("b.txt"), "b\n").unwrap();
    git(repo, &["add", "b.txt"]);
    git(repo, &["commit", "-m", "feat b"]);
    git(repo, &["checkout", "main"]);

    // Branch c: add file c.txt
    git(repo, &["checkout", "-b", "c"]);
    std::fs::write(repo.join("c.txt"), "c\n").unwrap();
    git(repo, &["add", "c.txt"]);
    git(repo, &["commit", "-m", "feat c"]);
    git(repo, &["checkout", "main"]);

    tr
}

fn make_session(repo_path: &Path, branches: &[&str]) -> Session {
    Session {
        session_name: "paw-merge-loop-test".to_string(),
        repo_path: repo_path.to_path_buf(),
        project_name: "merge-loop-test".to_string(),
        created_at: SystemTime::now(),
        status: SessionStatus::Active,
        worktrees: branches
            .iter()
            .map(|b| WorktreeEntry {
                branch: (*b).to_string(),
                worktree_path: repo_path.join(format!("wt-{b}")),
                cli: "echo".to_string(),
                branch_created: false,
            })
            .collect(),
        broker_port: None,
        broker_bind: None,
        broker_log_path: None,
    }
}

fn default_broker_config() -> BrokerConfig {
    BrokerConfig {
        enabled: false,
        port: 0,
        bind: "127.0.0.1".to_string(),
    }
}

// ---------------------------------------------------------------------------
// C13.1: orders, runs the test command, and reports success per branch
// ---------------------------------------------------------------------------

#[test]
fn merge_loop_merges_all_branches_and_records_test_results() {
    let tr = setup_three_branch_repo();
    let repo = tr.path();
    let session = make_session(repo, &["a", "b", "c"]);
    let test_command = "true".to_string();
    let broker_config = default_broker_config();

    let MergeResults {
        merge_order,
        test_results,
    } = run_merge_loop(repo, &session, Some(&test_command), &broker_config)
        .expect("merge loop runs");

    // All three branches were attempted in some topological order.
    assert_eq!(
        merge_order.len(),
        3,
        "merge order should include all branches, got: {merge_order:?}"
    );
    for b in ["a", "b", "c"] {
        assert!(
            merge_order.contains(&b.to_string()),
            "merge order missing branch {b}: {merge_order:?}"
        );
    }

    // Every branch has a test result, all marked successful (since the
    // test command was `true` and there were no merge conflicts).
    assert_eq!(test_results.len(), 3);
    for b in ["a", "b", "c"] {
        let tr = test_results
            .get(b)
            .unwrap_or_else(|| panic!("missing test result for {b}"));
        assert!(
            tr.success,
            "branch {b} test_result.success should be true; output: {}",
            tr.output
        );
    }

    // The branches were actually merged into main (the working copies of
    // a.txt, b.txt, c.txt should all be present on main now).
    git(repo, &["checkout", "main"]);
    for f in ["a.txt", "b.txt", "c.txt"] {
        assert!(
            repo.join(f).exists(),
            "main should contain {f} after the merge loop"
        );
    }
}

// ---------------------------------------------------------------------------
// C13.2: a failing test command marks the failed branch but keeps going
// ---------------------------------------------------------------------------

#[test]
fn merge_loop_records_failure_when_test_command_fails_for_one_branch() {
    let tr = setup_three_branch_repo();
    let repo = tr.path();
    let session = make_session(repo, &["a", "b", "c"]);
    // The shell command fails only when b.txt is on main. The first merge
    // in the loop's order does not yet have b.txt; once b is merged, all
    // subsequent merges see b.txt and the test command fails for them.
    //
    // The exact order is deterministic but depends on `topological_merge_order`
    // — we don't hard-code the order, instead we infer expected per-branch
    // outcomes from the index of `b` in `merge_order`.
    let test_command = "if [ -f b.txt ]; then exit 1; else exit 0; fi".to_string();
    let broker_config = default_broker_config();

    let MergeResults {
        merge_order,
        test_results,
    } = run_merge_loop(repo, &session, Some(&test_command), &broker_config)
        .expect("merge loop runs");

    // c must still have been attempted — the loop continues past failures.
    assert!(
        merge_order.contains(&"c".to_string()),
        "branch c should still be processed even if b fails; got merge_order: {merge_order:?}"
    );

    // b's outcome must be a recorded failure (b's own merge introduces b.txt
    // and the test command sees it).
    let b_result = test_results.get("b").expect("b should have a test result");
    assert!(
        !b_result.success,
        "b's test_command failed; test_result.success must be false. output:\n{}",
        b_result.output
    );

    // For every branch *after* b in the merge order, the test command also
    // fails (b.txt is still on main from b's merge); for every branch
    // *before* b, it succeeds.
    let b_idx = merge_order
        .iter()
        .position(|s| s == "b")
        .expect("b is in merge order");
    for (i, branch) in merge_order.iter().enumerate() {
        if branch == "b" {
            continue;
        }
        let r = test_results
            .get(branch)
            .unwrap_or_else(|| panic!("missing test result for {branch}"));
        if i < b_idx {
            assert!(
                r.success,
                "branch {branch} runs before b in the merge order — \
                 b.txt is not yet on main, test_command should succeed.\nOutput:\n{}",
                r.output
            );
        } else {
            assert!(
                !r.success,
                "branch {branch} runs after b in the merge order — \
                 b.txt is on main, test_command should fail.\nOutput:\n{}",
                r.output
            );
        }
    }
}
