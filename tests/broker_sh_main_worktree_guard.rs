//! The bundled `broker.sh` derives no agent id in the main working tree.
//!
//! `agent-id-slug-safety` (`broker-protocol`, "No publishing agent id is
//! derived in the main working tree"): with `slugify_branch` prefixing a
//! single word, a publish from the main checkout on `main` without `--agent`
//! would land as a phantom `agent-main` — previously it was an HTTP 400.
//! Agents publish from linked worktrees, so the helper refuses to derive there
//! instead, while an explicit `--agent` is unaffected and a linked worktree
//! derives exactly as before.
//!
//! Driven against a REAL broker, so "published nothing" is asserted against
//! what the broker actually holds rather than a stub's idea of it.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Output};
use std::time::Duration;

use serial_test::serial;
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) -> Output {
    StdCommand::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git command")
}

fn init_git_repo(dir: &Path) {
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@e.st"]);
    git(dir, &["config", "user.name", "Test"]);
    fs::write(dir.join("README.md"), "x").expect("readme");
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "init"]);
}

/// Copies the bundled helpers into `<root>/.git-paw/scripts/`, mirroring
/// `git paw init`'s co-deployment.
fn install_helpers(root: &Path) -> PathBuf {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts");
    let dst = root.join(".git-paw/scripts");
    fs::create_dir_all(&dst).expect("mk scripts dir");
    for name in ["_paw_common.sh", "broker.sh"] {
        fs::copy(assets.join(name), dst.join(name)).unwrap_or_else(|e| panic!("copy {name}: {e}"));
    }
    dst.join("broker.sh")
}

fn write_broker_config(root: &Path, port: u16) {
    fs::create_dir_all(root.join(".git-paw")).expect("mk .git-paw");
    fs::write(
        root.join(".git-paw/config.toml"),
        format!("[broker]\nenabled = true\nport = {port}\n"),
    )
    .expect("write config.toml");
}

fn pick_broker_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read local addr")
        .port()
}

fn spawn_broker() -> (git_paw::broker::BrokerHandle, u16) {
    let mut attempts = 0;
    loop {
        let port = pick_broker_port();
        let config = git_paw::config::BrokerConfig {
            enabled: true,
            port,
            bind: "127.0.0.1".to_string(),
            ..Default::default()
        };
        let state = git_paw::broker::BrokerState::new(None);
        match git_paw::broker::start_broker(&config, state, Vec::new()) {
            Ok(handle) => return (handle, port),
            Err(_) if attempts < 10 => attempts += 1,
            Err(e) => panic!("failed to start test broker after retries: {e}"),
        }
    }
}

/// Runs the helper from `cwd` (which decides whether it sees the main working
/// tree or a linked worktree).
fn run(script: &Path, cwd: &Path, args: &[&str]) -> Output {
    StdCommand::new("bash")
        .arg(script)
        .args(args)
        .current_dir(cwd)
        .env_remove("TMUX")
        .output()
        .expect("run helper")
}

fn get(port: u16, path: &str) -> String {
    let addr = format!("127.0.0.1:{port}");
    let mut stream = TcpStream::connect(&addr).expect("connect to broker");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).expect("write request");
    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("read response");
    raw.split_once("\r\n\r\n")
        .map_or_else(String::new, |(_, body)| body.to_string())
}

#[test]
#[serial]
fn main_working_tree_without_agent_refuses_and_publishes_nothing() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let broker_sh = install_helpers(repo.path());
    let (handle, port) = spawn_broker();
    write_broker_config(repo.path(), port);

    let out = run(&broker_sh, repo.path(), &["status", "booting"]);

    assert!(
        !out.status.success(),
        "helper must refuse in the main working tree; stdout:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--agent"),
        "the diagnostic must name the --agent remedy; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("main working tree"),
        "the diagnostic must name the main working tree; stderr:\n{stderr}"
    );

    // Exit 2 is the helper's usage-refusal code. Asserting it specifically —
    // not merely "non-zero" — is what pins that the guard ABORTED rather than
    // printing and falling through to a publish that then failed for some
    // other reason. `resolve_agent` runs in a command substitution, where an
    // `exit` would otherwise leave only the subshell and the script would
    // carry on with an empty agent id and POST anyway.
    assert_eq!(
        out.status.code(),
        Some(2),
        "the guard must abort with the usage code, not fall through; stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("publish to"),
        "the helper must not attempt a publish after refusing; stderr:\n{stderr}"
    );

    let status = get(port, "/status");
    assert!(
        !status.contains("agent-main"),
        "a refused derivation must publish nothing; /status:\n{status}"
    );

    drop(handle);
}

#[test]
#[serial]
fn explicit_agent_still_publishes_from_the_main_working_tree() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let broker_sh = install_helpers(repo.path());
    let (handle, port) = spawn_broker();
    write_broker_config(repo.path(), port);

    let out = run(
        &broker_sh,
        repo.path(),
        &["--agent", "supervisor", "question", "which order?"],
    );

    assert!(
        out.status.success(),
        "an explicit --agent must publish as before; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let log = get(port, "/log?since=0");
    assert!(
        log.contains("agent.question") && log.contains("supervisor"),
        "the broker should hold the supervisor's question; /log:\n{log}"
    );

    drop(handle);
}

#[test]
#[serial]
fn linked_worktree_derives_its_id_as_before() {
    let sandbox = TempDir::new().expect("sandbox");
    let repo = sandbox.path().join("repo");
    fs::create_dir(&repo).expect("mkdir repo");
    init_git_repo(&repo);

    // The helper resolves `.git-paw/` from the worktree root it runs in, so
    // install the scripts and config where the agent will actually be.
    let worktree = sandbox.path().join("wt-feat-x");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feat/x",
            worktree.to_str().expect("utf-8 path"),
        ],
    );

    let broker_sh = install_helpers(&worktree);
    let (handle, port) = spawn_broker();
    write_broker_config(&worktree, port);

    let out = run(&broker_sh, &worktree, &["status", "booting"]);

    assert!(
        out.status.success(),
        "a linked worktree must still derive its id; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let log = get(port, "/log?since=0");
    assert!(
        log.contains("feat-x"),
        "the published id should be feat-x; /log:\n{log}"
    );

    drop(handle);
}
