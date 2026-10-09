//! Safety boundary: a MUTATING `git paw` command run from inside an agent
//! worktree must not act on the parent repository's session.
//!
//! Every mutating command resolves its repository with `git::validate_repo`,
//! which inside a linked worktree returns that WORKTREE's root, then looks the
//! session up by that root. The lookup is therefore the boundary: if it
//! resolved a worktree to its owning repository, `git paw stop` run by an
//! agent in its own worktree would kill the whole session (and strip the
//! session boot block from the repository's `AGENTS.md`), and
//! `git paw purge --force` would delete its SIBLINGS' worktrees and branches.
//!
//! `git paw mcp`'s read-only session view deliberately DOES resolve the owner
//! (`session::find_session_for_repo_or_owner`) — covered by
//! `tests/mcp_worktree_session_lookup.rs`; these tests pin that the widening
//! stopped there.
//!
//! Skips when tmux is unavailable.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn skip_if_no_tmux() -> bool {
    let available = StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success());
    if !available {
        eprintln!("skipping: tmux not available");
    }
    !available
}

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_project_name(tag: &str) -> String {
    let pid = std::process::id();
    let n = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("wtiso-{tag}-{pid}-{n}")
}

fn find_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

fn rename_repo_basename(tr: &TestRepo, new_basename: &str) -> PathBuf {
    let old = tr.path().to_path_buf();
    let renamed = old
        .parent()
        .expect("repo has a parent sandbox dir")
        .join(new_basename);
    fs::rename(&old, &renamed).expect("rename repo dir");
    renamed
}

fn write_config(repo: &Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let port = find_free_port();
    fs::write(
        paw_dir.join("config.toml"),
        format!(
            "default_cli = \"cat\"\n\n[broker]\nenabled = true\nport = {port}\n\n\
             [clis.cat]\ncommand = \"cat\"\n"
        ),
    )
    .expect("write config");
}

fn kill_session(env: &TmuxTestEnv, name: &str) {
    let mut kill_cmd = StdCommand::new("tmux");
    env.apply(&mut kill_cmd);
    let _ = kill_cmd.args(["kill-session", "-t", name]).status();
}

fn session_alive(env: &TmuxTestEnv, name: &str) -> bool {
    let mut c = StdCommand::new("tmux");
    env.apply(&mut c);
    c.args(["has-session", "-t", name])
        .output()
        .is_ok_and(|o| o.status.success())
}

fn agent_worktrees(repo: &Path, session: &str) -> Vec<PathBuf> {
    let path = repo
        .join(".git-paw")
        .join("sessions")
        .join(format!("{session}.json"));
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read discovery file {}: {e}", path.display()));
    let file: git_paw::session::RepoSessionFile =
        serde_json::from_str(&raw).expect("parse discovery file");
    file.agents.into_iter().map(|a| a.worktree_path).collect()
}

fn branch_exists(repo: &Path, branch: &str) -> bool {
    StdCommand::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--verify", &format!("refs/heads/{branch}")])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Launches a bare two-agent session and returns `(repo, session_name,
/// worktrees)`.
fn launch(
    tmux_env: &TmuxTestEnv,
    repo: &Path,
    home: &Path,
    project: &str,
) -> (String, Vec<PathBuf>) {
    let session_name = format!("paw-{project}");
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(repo)
        .env("HOME", home)
        .env_remove("XDG_DATA_HOME")
        .args([
            "start",
            "--branches",
            "feat/alpha,feat/beta",
            "--cli",
            "cat",
        ])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start --branches");
    assert!(
        out.status.success(),
        "launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let worktrees = agent_worktrees(repo, &session_name);
    assert_eq!(worktrees.len(), 2, "expected two agent worktrees");
    (session_name, worktrees)
}

/// `git paw stop` from inside an agent worktree must NOT kill the session.
#[test]
#[serial]
fn stop_inside_an_agent_worktree_does_not_stop_the_parent_session() {
    if skip_if_no_tmux() {
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("stop");
    let repo = rename_repo_basename(&tr, &project);
    write_config(&repo);
    let home = TempDir::new().expect("home tempdir");

    let (session_name, worktrees) = launch(&tmux_env, &repo, home.path(), &project);

    let mut stop = cmd();
    tmux_env.apply_assert(&mut stop);
    let out = stop
        .current_dir(&worktrees[0])
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["stop"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run stop from the worktree");
    let stdout = String::from_utf8_lossy(&out.stdout);

    let alive = session_alive(&tmux_env, &session_name);
    kill_session(&tmux_env, &session_name);

    assert!(
        alive,
        "stop run inside an agent worktree must not kill the parent session; \
         stdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("No active session"),
        "stop should report that this worktree has no session of its own; \
         stdout:\n{stdout}"
    );
}

/// `git paw purge --force` from inside an agent worktree must NOT delete the
/// session's sibling worktrees or branches.
#[test]
#[serial]
fn purge_force_inside_an_agent_worktree_does_not_purge_the_parent_session() {
    if skip_if_no_tmux() {
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("purge");
    let repo = rename_repo_basename(&tr, &project);
    write_config(&repo);
    let home = TempDir::new().expect("home tempdir");

    let (session_name, worktrees) = launch(&tmux_env, &repo, home.path(), &project);
    let sibling = worktrees[1].clone();

    let mut purge = cmd();
    tmux_env.apply_assert(&mut purge);
    let out = purge
        .current_dir(&worktrees[0])
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["purge", "--force"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run purge --force from the worktree");
    let stdout = String::from_utf8_lossy(&out.stdout);

    let sibling_exists = sibling.exists();
    let alpha = branch_exists(&repo, "feat/alpha");
    let beta = branch_exists(&repo, "feat/beta");
    let alive = session_alive(&tmux_env, &session_name);
    kill_session(&tmux_env, &session_name);

    assert!(
        sibling_exists,
        "purge --force inside an agent worktree must not delete a SIBLING \
         agent's worktree ({}); stdout:\n{stdout}",
        sibling.display()
    );
    assert!(
        alpha && beta,
        "purge --force inside an agent worktree must not delete the session's \
         branches (feat/alpha present: {alpha}, feat/beta present: {beta}); \
         stdout:\n{stdout}"
    );
    assert!(
        alive,
        "purge --force inside an agent worktree must not kill the parent \
         session; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("No session to purge"),
        "purge should report that this worktree has no session of its own; \
         stdout:\n{stdout}"
    );
}

/// `git paw status` is the mutation-free sibling of the two commands above and
/// uses the same EXACT lookup, so it must not surface the parent session from
/// inside an agent worktree either. (The complementary half — that
/// `git paw mcp`'s read-only view DOES resolve the owner, so an agent's MCP
/// server still finds the broker — is covered by
/// `tests/mcp_worktree_session_lookup.rs`.)
#[test]
#[serial]
fn status_inside_an_agent_worktree_does_not_show_the_parent_session() {
    if skip_if_no_tmux() {
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("mcp");
    let repo = rename_repo_basename(&tr, &project);
    write_config(&repo);
    let home = TempDir::new().expect("home tempdir");

    let (session_name, worktrees) = launch(&tmux_env, &repo, home.path(), &project);

    let mut status = cmd();
    tmux_env.apply_assert(&mut status);
    let out = status
        .current_dir(&worktrees[0])
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["status"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run status from the worktree");
    let stdout = String::from_utf8_lossy(&out.stdout);
    kill_session(&tmux_env, &session_name);

    assert!(
        !stdout.contains(&session_name),
        "the exact lookup must not surface the parent session from inside an \
         agent worktree; stdout:\n{stdout}"
    );
}
