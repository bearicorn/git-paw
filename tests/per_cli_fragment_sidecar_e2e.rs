//! `skill-agent` / *Bundled skill text carries the agent's own CLI fragment*,
//! scenario "Agents on different CLIs in one session get different
//! fragments": in one session, the agent on a `claude`-family CLI and the
//! agent on an unrecognised CLI each get their own family's skill fragment in
//! the managed block of their sidecar.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Output};
use std::time::Duration;

use assert_cmd::Command;
use git_paw::agent_cli::CliFamily;
use git_paw::skills::{FragmentSlot, bundled_package, select_fragment};
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

/// An OS-assigned free port for the session's broker.
fn pick_broker_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read local addr")
        .port()
}

/// A stand-in agent CLI that ignores its arguments and drains stdin, so any
/// approval flags a family appends cannot make it exit.
fn write_stub(bin_dir: &Path) -> PathBuf {
    let path = bin_dir.join("paw-fragment-stub");
    fs::write(&path, "#!/bin/sh\nexec cat >/dev/null\n").expect("write stub");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod stub");
    path
}

/// Broker enabled (the coordination skill is injected only then), one
/// `claude`-family custom CLI and one with no declared family.
fn write_config(repo: &Path, stub: &Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        format!(
            "[broker]\nenabled = true\nport = {port}\n\n\
             [clis.claude-stub]\ncommand = \"{stub}\"\nfamily = \"claude\"\n\n\
             [clis.plain-stub]\ncommand = \"{stub}\"\n",
            port = pick_broker_port(),
            stub = stub.display()
        ),
    )
    .expect("write config");
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

fn skill_fragment(family: CliFamily) -> &'static str {
    let coordination = bundled_package("coordination").expect("registered");
    select_fragment(family, FragmentSlot::Skill, coordination).trim_end()
}

#[test]
#[serial]
fn agents_on_different_clis_in_one_session_get_different_fragments() {
    let tr = setup_test_repo();
    let bin = TempDir::new().expect("stub dir");
    let stub = write_stub(bin.path());
    write_config(tr.path(), &stub);
    let home = TempDir::new().expect("home");
    let tmux_env = tmux_test_env();

    let start = paw(
        tr.path(),
        home.path(),
        &tmux_env,
        &[
            "start",
            "--no-supervisor",
            "--cli",
            "claude-stub",
            "--branches",
            "a",
        ],
    );
    assert!(start.status.success(), "start failed; {}", text(&start));
    let session = session_name(&start);
    let add = paw(
        tr.path(),
        home.path(),
        &tmux_env,
        &["add", "b", "--cli", "plain-stub"],
    );
    kill(&tmux_env, &session);
    assert!(add.status.success(), "add failed; {}", text(&add));

    let claude = skill_fragment(CliFamily::Claude);
    let generic = skill_fragment(CliFamily::Generic);

    let claude_agent = agent_context(tr.path(), "a");
    assert!(
        claude_agent.contains(claude),
        "claude-family agent: {claude_agent}"
    );
    assert!(
        !claude_agent.contains(generic),
        "claude-family agent: {claude_agent}"
    );

    let generic_agent = agent_context(tr.path(), "b");
    assert!(
        generic_agent.contains(generic),
        "unrecognised-CLI agent: {generic_agent}"
    );
    assert!(
        !generic_agent.contains(claude),
        "unrecognised-CLI agent: {generic_agent}"
    );
}
