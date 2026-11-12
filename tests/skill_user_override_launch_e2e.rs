//! `skill-agent` / *Production callers resolve skills through the real search
//! roots*: a user skill override under the platform config directory of the
//! launching process's `HOME` reaches the managed context of an agent
//! launched by `git paw start` and of one added by `git paw add`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Output};
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

const MARKER: &str = "USER-OVERRIDE-MARKER";

/// An OS-assigned free port for the session's broker.
fn pick_broker_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read local addr")
        .port()
}

/// Supervisor-mode config with the broker enabled (the coordination skill is
/// injected only then) and `cat` as the stand-in CLI.
fn write_supervisor_config(repo: &Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        format!(
            "default_cli = \"cat\"\n\n[clis.cat]\ncommand = \"cat\"\n\n\
             [broker]\nenabled = true\nport = {}\n\n\
             [supervisor]\nenabled = true\ncli = \"cat\"\n",
            pick_broker_port()
        ),
    )
    .expect("write config");
}

/// Writes the user override for `coordination` under `home`'s platform
/// config directory (`XDG_CONFIG_HOME` is removed for every run).
fn write_user_override(home: &Path) {
    let config = if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support")
    } else {
        home.join(".config")
    };
    let dir = config
        .join("git-paw")
        .join("agent-skills")
        .join("coordination");
    fs::create_dir_all(&dir).expect("create override dir");
    fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: coordination\ndescription: user override\n---\n\n{MARKER}\n"),
    )
    .expect("write override");
}

fn paw(repo: &Path, home: &Path, tmux_env: &TmuxTestEnv, args: &[&str]) -> Output {
    let mut c = Command::cargo_bin("git-paw").expect("binary exists");
    tmux_env.apply_assert(&mut c);
    c.current_dir(repo)
        .env("HOME", home)
        .env_remove("XDG_DATA_HOME")
        .env_remove("XDG_CONFIG_HOME")
        .args(args)
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run git-paw")
}

fn text(out: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// The managed context of the agent on `branch` (worktree beside the repo).
fn agent_context(repo: &Path, branch: &str) -> String {
    let path: PathBuf = repo
        .parent()
        .expect("repo parent")
        .join(format!("repo-{branch}/.git-paw/AGENTS.local.md"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn session_name(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
        .expect("session name in start stdout")
        .to_string()
}

fn kill(tmux_env: &TmuxTestEnv, session: &str) {
    let mut c = StdCommand::new("tmux");
    tmux_env.apply(&mut c);
    let _ = c.args(["kill-session", "-t", session]).status();
}

/// Scenarios "A user override reaches the agent launched by start" and "A
/// user override reaches the agent added by add".
#[test]
#[serial]
fn a_user_override_reaches_agents_launched_by_start_and_added_by_add() {
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().expect("home");
    write_user_override(home.path());
    let tmux_env = tmux_test_env();

    let start = paw(
        tr.path(),
        home.path(),
        &tmux_env,
        &["start", "--supervisor", "--branches", "a"],
    );
    assert!(start.status.success(), "start failed; {}", text(&start));
    let session = session_name(&start);
    let add = paw(tr.path(), home.path(), &tmux_env, &["add", "b"]);
    kill(&tmux_env, &session);
    assert!(add.status.success(), "add failed; {}", text(&add));

    let started = agent_context(tr.path(), "a");
    assert!(started.contains(MARKER), "start: {started}");
    let added = agent_context(tr.path(), "b");
    assert!(added.contains(MARKER), "add: {added}");
}
