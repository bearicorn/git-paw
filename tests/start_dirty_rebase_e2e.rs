//! `git paw start` never rebases over uncommitted work.
//!
//! Maps to the `git-operations` scenarios of `start-skips-rebase-on-dirty-worktree`:
//! a fresh start (no saved session) over a surviving dirty worktree, over a
//! dirty main checkout for a branch with no worktree, the supervised launch,
//! `--no-rebase`, and the stderr/stdout split of the skip notice.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

fn tmux_available() -> bool {
    StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Run `git <args>` in `dir`, asserting success; returns trimmed stdout.
fn git_ok(dir: &Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn commit_file(dir: &Path, file: &str, body: &str, message: &str) {
    fs::write(dir.join(file), body).expect("write file");
    git_ok(dir, &["add", file]);
    git_ok(dir, &["commit", "-m", message]);
}

/// A bare config with the broker off and `cat` as the stand-in CLI.
fn write_bare_config(repo: &Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"cat\"\n\n[clis.cat]\ncommand = \"cat\"\n\n[broker]\nenabled = false\n",
    )
    .expect("write config");
}

/// A supervisor config (broker off) with `cat` as the stand-in CLI.
fn write_supervisor_config(repo: &Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"cat\"\n\n[supervisor]\nenabled = true\ncli = \"cat\"\n\n\
         [broker]\nenabled = false\n",
    )
    .expect("write config");
}

/// A worktree for `feat/x` left on disk with no saved session: the branch
/// carries a commit of its own and `main` has moved on since. Returns the
/// worktree path.
fn surviving_worktree_behind_main(repo: &Path) -> PathBuf {
    let wt = repo.parent().unwrap().join("repo-feat-x");
    git_ok(
        repo,
        &["worktree", "add", "-b", "feat/x", wt.to_str().unwrap()],
    );
    commit_file(&wt, "work.txt", "agent work\n", "agent work");
    commit_file(repo, "main2.txt", "main\n", "main moves on");
    wt
}

/// Run `git paw <args>` in `repo` against the test socket and `HOME`.
fn paw(repo: &Path, home: &Path, tmux_env: &TmuxTestEnv, args: &[&str]) -> std::process::Output {
    let mut c = Command::cargo_bin("git-paw").expect("binary exists");
    tmux_env.apply_assert(&mut c);
    c.current_dir(repo)
        .env("HOME", home)
        .env_remove("XDG_DATA_HOME")
        .env_remove("XDG_CONFIG_HOME")
        .args(args)
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run git paw")
}

fn session_name(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
        .map(str::to_string)
}

fn kill(tmux_env: &TmuxTestEnv, stdout: &str) {
    if let Some(session) = session_name(stdout) {
        let mut c = StdCommand::new("tmux");
        tmux_env.apply(&mut c);
        let _ = c.args(["kill-session", "-t", &session]).status();
    }
}

fn skip_notices(stderr: &str) -> usize {
    skip_notices_for(stderr, "feat/x")
}

fn skip_notices_for(stderr: &str, branch: &str) -> usize {
    stderr
        .matches(&format!("rebase skipped for '{branch}'"))
        .count()
}

/// "A surviving dirty worktree is reused without a rebase" and "start prints
/// the skip notice on stderr".
#[test]
#[serial]
fn start_reuses_a_dirty_surviving_worktree_without_a_rebase() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_bare_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();
    let wt = surviving_worktree_behind_main(tr.path());

    fs::write(wt.join("README.md"), "# edited, not committed\n").unwrap();
    fs::write(wt.join("scratch.txt"), "untracked work\n").unwrap();
    let head_before = git_ok(tr.path(), &["rev-parse", "feat/x"]);
    let stash_before = git_ok(tr.path(), &["stash", "list"]);

    let out = paw(
        tr.path(),
        home.path(),
        &tmux_env,
        &["start", "--cli", "cat", "--branches", "feat/x"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let result = std::panic::catch_unwind(|| {
        assert!(
            out.status.success(),
            "start must succeed; stderr:\n{stderr}"
        );
        assert!(
            session_name(&stdout).is_some(),
            "the agent is launched:\n{stdout}"
        );
        assert_eq!(git_ok(tr.path(), &["rev-parse", "feat/x"]), head_before);
        assert_eq!(
            fs::read_to_string(wt.join("README.md")).unwrap(),
            "# edited, not committed\n"
        );
        assert_eq!(
            fs::read_to_string(wt.join("scratch.txt")).unwrap(),
            "untracked work\n"
        );
        assert_eq!(git_ok(tr.path(), &["stash", "list"]), stash_before);
        assert_eq!(skip_notices(&stderr), 1, "one notice on stderr:\n{stderr}");
        assert!(
            stderr.contains("--no-rebase"),
            "notice names --no-rebase:\n{stderr}"
        );
        assert!(
            !stdout.contains("rebase skipped") && !stdout.contains("--no-rebase"),
            "the skip notice stays off stdout:\n{stdout}"
        );
    });
    kill(&tmux_env, &stdout);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

/// "A branch without a worktree is rebased in its own worktree, never in the
/// main checkout".
#[test]
#[serial]
fn start_rebases_a_branch_without_a_worktree_and_leaves_a_dirty_main_checkout_alone() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_bare_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    git_ok(tr.path(), &["branch", "feat/y"]);
    commit_file(tr.path(), "main2.txt", "main\n", "main moves on");
    fs::write(tr.path().join("README.md"), "# main checkout edit\n").unwrap();
    fs::write(tr.path().join("scratch.txt"), "untracked\n").unwrap();
    let main_head_before = git_ok(tr.path(), &["rev-parse", "HEAD"]);

    let out = paw(
        tr.path(),
        home.path(),
        &tmux_env,
        &["start", "--cli", "cat", "--branches", "feat/y"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let result = std::panic::catch_unwind(|| {
        assert!(
            out.status.success(),
            "start must succeed; stderr:\n{stderr}"
        );
        let wt = tr.path().parent().unwrap().join("repo-feat-y");
        assert!(wt.exists(), "the worktree for feat/y is created");
        assert_eq!(
            git_ok(tr.path(), &["rev-list", "--count", "feat/y..main"]),
            "0",
            "feat/y is rebased onto main"
        );
        assert_eq!(
            git_ok(&wt, &["rev-parse", "HEAD"]),
            git_ok(tr.path(), &["rev-parse", "feat/y"]),
            "its worktree is on the rebased tip"
        );
        assert_eq!(git_ok(tr.path(), &["branch", "--show-current"]), "main");
        assert_eq!(git_ok(tr.path(), &["rev-parse", "HEAD"]), main_head_before);
        assert_eq!(
            fs::read_to_string(tr.path().join("README.md")).unwrap(),
            "# main checkout edit\n"
        );
        assert_eq!(
            fs::read_to_string(tr.path().join("scratch.txt")).unwrap(),
            "untracked\n"
        );
        assert_eq!(
            skip_notices_for(&stderr, "feat/y"),
            0,
            "no skip notice:\n{stderr}"
        );
    });
    kill(&tmux_env, &stdout);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

/// "A clean worktree on a moved default branch is still rebased" and
/// "git-paw-managed files alone do not block a start rebase".
#[test]
#[serial]
fn start_rebases_a_surviving_worktree_whose_only_changes_are_managed() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_bare_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();
    let wt = surviving_worktree_behind_main(tr.path());
    fs::create_dir_all(wt.join(".git-paw")).unwrap();
    fs::write(wt.join(".git-paw/AGENTS.local.md"), "sidecar\n").unwrap();

    let out = paw(
        tr.path(),
        home.path(),
        &tmux_env,
        &["start", "--cli", "cat", "--branches", "feat/x"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let result = std::panic::catch_unwind(|| {
        assert!(
            out.status.success(),
            "start must succeed; stderr:\n{stderr}"
        );
        assert_eq!(
            git_ok(tr.path(), &["rev-list", "--count", "feat/x..main"]),
            "0",
            "feat/x is rebased onto main"
        );
        assert_eq!(skip_notices(&stderr), 0, "no skip notice:\n{stderr}");
    });
    kill(&tmux_env, &stdout);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

/// "A supervised launch completes over a dirty worktree".
#[test]
#[serial]
fn supervised_start_completes_over_a_dirty_worktree() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();
    let wt = surviving_worktree_behind_main(tr.path());
    fs::write(wt.join("README.md"), "# edited, not committed\n").unwrap();
    let head_before = git_ok(tr.path(), &["rev-parse", "feat/x"]);

    let out = paw(
        tr.path(),
        home.path(),
        &tmux_env,
        &["start", "--supervisor", "--branches", "feat/x"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let result = std::panic::catch_unwind(|| {
        assert!(
            out.status.success(),
            "supervised start must succeed; stderr:\n{stderr}"
        );
        assert!(
            session_name(&stdout).is_some(),
            "the session launches:\n{stdout}"
        );
        assert_eq!(git_ok(tr.path(), &["rev-parse", "feat/x"]), head_before);
        assert_eq!(skip_notices(&stderr), 1, "one notice on stderr:\n{stderr}");
        assert!(
            stderr.contains("--no-rebase"),
            "notice names --no-rebase:\n{stderr}"
        );
    });
    kill(&tmux_env, &stdout);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

/// `--help` for `start` and `add` documents the dirty-worktree skip and that
/// the rebase never runs in the main checkout.
#[test]
fn help_documents_the_dirty_tree_rebase_skip() {
    for sub in ["start", "add"] {
        let out = Command::cargo_bin("git-paw")
            .expect("binary exists")
            .args([sub, "--help"])
            .output()
            .expect("run --help");
        let help = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "{sub} --help failed");
        assert!(
            help.contains("uncommitted changes") && help.contains("never in the main checkout"),
            "{sub} --help must document the skip and the worktree-only rebase:\n{help}"
        );
    }
}

/// "--no-rebase on a dirty worktree prints no notice".
#[test]
#[serial]
fn start_no_rebase_on_a_dirty_worktree_prints_no_notice() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_bare_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();
    let wt = surviving_worktree_behind_main(tr.path());
    fs::write(wt.join("README.md"), "# edited, not committed\n").unwrap();
    let head_before = git_ok(tr.path(), &["rev-parse", "feat/x"]);

    let out = paw(
        tr.path(),
        home.path(),
        &tmux_env,
        &[
            "start",
            "--no-rebase",
            "--cli",
            "cat",
            "--branches",
            "feat/x",
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let result = std::panic::catch_unwind(|| {
        assert!(
            out.status.success(),
            "start must succeed; stderr:\n{stderr}"
        );
        assert_eq!(git_ok(tr.path(), &["rev-parse", "feat/x"]), head_before);
        assert_eq!(
            fs::read_to_string(wt.join("README.md")).unwrap(),
            "# edited, not committed\n"
        );
        assert!(!stderr.contains("rebase skipped"), "no notice:\n{stderr}");
    });
    kill(&tmux_env, &stdout);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
