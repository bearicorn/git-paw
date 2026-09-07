//! End-to-end regression tests for GP-13: `git paw start` must reattach to
//! (or, non-interactively, refuse in favour of) an existing live session for
//! the current repository instead of forking a `-N`-suffixed parallel one.
//!
//! The bug lived specifically in `cmd_supervisor`: `resolve_dispatch_target`
//! routes a supervisor-enabled repo's bare `git paw start` straight to it,
//! bypassing `cmd_start`'s own existing-session pre-check entirely, so
//! `cmd_supervisor` forked `paw-<project>-2` on every re-invocation instead of
//! reattaching. Every test here therefore exercises `--branches` (no
//! `--supervisor` flag needed — `[supervisor] enabled = true` is enough) so
//! the *default*, most common launch path is what gets proven.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

fn skip_if_no_tmux() -> bool {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return true;
    }
    false
}

/// A `git-paw` command with an isolated `HOME` (own session-state directory)
/// and an isolated tmux socket, so this test's sessions can never collide
/// with — or be mistaken for — the real user's.
fn cmd_iso(fake_home: &Path, tmux_env: &TmuxTestEnv) -> Command {
    let mut c = Command::cargo_bin("git-paw").expect("binary exists");
    c.env("HOME", fake_home).env_remove("XDG_DATA_HOME");
    tmux_env.apply_assert(&mut c);
    c
}

fn run_git(dir: &Path, args: &[&str]) {
    let status = StdCommand::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

fn write_supervisor_config(repo: &Path) {
    let paw = repo.join(".git-paw");
    fs::create_dir_all(&paw).expect("create .git-paw");
    fs::write(
        paw.join("config.toml"),
        "default_cli = \"echo\"\n\n\
         [supervisor]\nenabled = true\ncli = \"echo\"\n\n\
         [clis.echo]\ncommand = \"echo\"\n",
    )
    .expect("write config");
}

/// Builds a git repo at `<parent>/<name>` with an initial commit, so
/// `git::project_name` (the directory basename) resolves to `name`
/// regardless of `parent` — used to construct two genuinely distinct
/// repositories that happen to share a project name.
fn init_repo_named(parent: &Path, name: &str) -> PathBuf {
    let repo = parent.join(name);
    fs::create_dir_all(&repo).expect("create repo dir");
    run_git(&repo, &["init", "-b", "main"]);
    run_git(&repo, &["config", "user.email", "test@test.com"]);
    run_git(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("a.txt"), "one\n").unwrap();
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "init"]);
    write_supervisor_config(&repo);
    repo
}

fn kill(tmux_env: &TmuxTestEnv, name: &str) {
    let _ = StdCommand::new("tmux")
        .env("TMUX_TMPDIR", tmux_env.socket_dir())
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .args(["kill-session", "-t", name])
        .status();
}

fn session_alive(tmux_env: &TmuxTestEnv, name: &str) -> bool {
    StdCommand::new("tmux")
        .env("TMUX_TMPDIR", tmux_env.socket_dir())
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .args(["has-session", "-t", name])
        .status()
        .is_ok_and(|s| s.success())
}

#[test]
#[serial]
fn supervisor_restart_refuses_rather_than_forking_a_parallel_session() {
    if skip_if_no_tmux() {
        return;
    }

    let fake_home = TempDir::new().expect("home tempdir");
    let tmux_env = tmux_test_env();
    let sandbox = TempDir::new().expect("sandbox");
    let repo = init_repo_named(sandbox.path(), "gp13-restart");

    // Bare `start` in a supervisor-enabled repo dispatches straight to
    // `cmd_supervisor` — no `--supervisor` flag required.
    let first = cmd_iso(fake_home.path(), &tmux_env)
        .current_dir(&repo)
        .args(["start", "--branches", "feat/x"])
        .output()
        .expect("run first git paw start");
    assert!(
        first.status.success(),
        "first launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&first.stderr)
    );

    let session_name = "paw-gp13-restart";
    assert!(
        session_alive(&tmux_env, session_name),
        "first launch should create '{session_name}'"
    );

    // Re-run non-interactively against the SAME repository. Before the fix
    // this forked `paw-gp13-restart-2`.
    let second = cmd_iso(fake_home.path(), &tmux_env)
        .current_dir(&repo)
        .args(["start", "--branches", "feat/x"])
        .output()
        .expect("run second git paw start");

    assert!(
        !second.status.success(),
        "a non-interactive re-start against a live session must refuse, not silently succeed"
    );
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(
        stderr.contains("already running for this repository"),
        "the refusal must be actionable; got:\n{stderr}"
    );
    assert!(
        !session_alive(&tmux_env, &format!("{session_name}-2")),
        "must not fork a '{session_name}-2' parallel session"
    );
    assert!(
        session_alive(&tmux_env, session_name),
        "the original session must be left untouched"
    );

    kill(&tmux_env, session_name);
}

#[test]
#[serial]
fn a_distinct_repo_sharing_a_project_name_still_gets_its_own_suffixed_session() {
    if skip_if_no_tmux() {
        return;
    }

    let fake_home = TempDir::new().expect("home tempdir");
    let tmux_env = tmux_test_env();
    let sandbox_a = TempDir::new().expect("sandbox a");
    let sandbox_b = TempDir::new().expect("sandbox b");
    let repo_a = init_repo_named(sandbox_a.path(), "gp13-shared-name");
    let repo_b = init_repo_named(sandbox_b.path(), "gp13-shared-name");

    let first = cmd_iso(fake_home.path(), &tmux_env)
        .current_dir(&repo_a)
        .args(["start", "--branches", "feat/x"])
        .output()
        .expect("run start in repo A");
    assert!(
        first.status.success(),
        "repo A launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&first.stderr)
    );

    // A SECOND, genuinely distinct repository (different path) that happens
    // to share a project name must NOT be mistaken for repo A's session —
    // it gets its own, `-2`-suffixed session.
    let second = cmd_iso(fake_home.path(), &tmux_env)
        .current_dir(&repo_b)
        .args(["start", "--branches", "feat/x"])
        .output()
        .expect("run start in repo B");
    assert!(
        second.status.success(),
        "repo B (a distinct repository) should launch fresh, not refuse; stderr:\n{}",
        String::from_utf8_lossy(&second.stderr)
    );

    assert!(
        session_alive(&tmux_env, "paw-gp13-shared-name"),
        "repo A's session must exist"
    );
    assert!(
        session_alive(&tmux_env, "paw-gp13-shared-name-2"),
        "repo B (distinct repo, same project name) must get its own -2 session"
    );

    kill(&tmux_env, "paw-gp13-shared-name");
    kill(&tmux_env, "paw-gp13-shared-name-2");
}
