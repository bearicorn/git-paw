//! Integration tests for `git paw pause` and the restart-from-pause
//! flow. Each test wires together a temporary git repo, a manually
//! crafted session JSON (since `cmd_start` interactivity is hard to
//! drive headlessly), and a real tmux session that mimics the
//! dashboard + agent pane layout. The pause flow then runs against
//! that simulated session.

use std::fs;
use std::net::TcpStream;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_project_name(tag: &str) -> String {
    let pid = std::process::id();
    let n = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("pause-{tag}-{pid}-{n}")
}

fn skip_if_no_tmux() -> bool {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return true;
    }
    false
}

fn kill_tmux_session(name: &str) {
    let _ = StdCommand::new("tmux")
        .args(["kill-session", "-t", name])
        .status();
}

fn rename_repo_basename(tr: &TestRepo, new_basename: &str) -> std::path::PathBuf {
    let original = tr.path().to_path_buf();
    let parent = original.parent().expect("repo has parent").to_path_buf();
    let renamed = parent.join(new_basename);
    fs::rename(&original, &renamed).expect("rename repo dir");
    renamed
}

fn canonical_repo_root(repo: &std::path::Path) -> std::path::PathBuf {
    let out = StdCommand::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .expect("git rev-parse");
    assert!(out.status.success(), "git rev-parse must succeed");
    std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
}

fn sessions_dir_for(fake_home: &std::path::Path) -> std::path::PathBuf {
    if cfg!(target_os = "macos") {
        fake_home.join("Library/Application Support/git-paw/sessions")
    } else {
        fake_home.join(".local/share/git-paw/sessions")
    }
}

fn find_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral")
        .local_addr()
        .expect("local_addr")
        .port()
}

fn port_is_listening(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().unwrap(),
        Duration::from_millis(200),
    )
    .is_ok()
}

fn wait_until_port_free(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !port_is_listening(port) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    !port_is_listening(port)
}

/// Builds a tmux session that mirrors the bare-mode broker layout:
/// pane 0 = dashboard (running `git-paw __dashboard`), pane 1 = a
/// long-lived `sleep` agent stand-in.
fn build_simulated_session(
    session_name: &str,
    repo_path: &std::path::Path,
    broker_url: &str,
    sessions_dir: &std::path::Path,
    fake_home: &std::path::Path,
) {
    let bin = assert_cmd::cargo::cargo_bin("git-paw");
    let dashboard_cmd = format!(
        "GIT_PAW_BROKER_URL='{broker_url}' XDG_DATA_HOME='{xdg}' HOME='{home}' '{bin}' __dashboard",
        xdg = sessions_dir
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(sessions_dir)
            .display(),
        home = fake_home.display(),
        bin = bin.display(),
    );

    // Pane 0: dashboard (runs the broker subprocess).
    // -x/-y required when tmux runs without an attached client (CI).
    let st = StdCommand::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            session_name,
            "-x",
            "200",
            "-y",
            "50",
            "-c",
            repo_path.to_str().unwrap(),
        ])
        .status()
        .expect("tmux new-session");
    assert!(
        st.success(),
        "tmux new-session for {session_name} must succeed"
    );

    let pane0_target = format!("{session_name}:0.0");
    let st = StdCommand::new("tmux")
        .args(["send-keys", "-t", &pane0_target, &dashboard_cmd, "Enter"])
        .status()
        .expect("tmux send-keys pane 0");
    assert!(st.success());

    // Pane 1: long-lived agent stand-in.
    let st = StdCommand::new("tmux")
        .args(["split-window", "-t", session_name])
        .status()
        .expect("tmux split-window");
    assert!(st.success());

    let pane1_target = format!("{session_name}:0.1");
    let st = StdCommand::new("tmux")
        .args(["send-keys", "-t", &pane1_target, "sleep 3600", "Enter"])
        .status()
        .expect("tmux send-keys pane 1");
    assert!(st.success());
}

/// Writes a Session JSON to the simulated sessions dir.
#[allow(clippy::too_many_arguments)]
fn write_session_json(
    sessions_dir: &std::path::Path,
    session_name: &str,
    repo_canonical: &std::path::Path,
    project: &str,
    branch: &str,
    wt_path: &std::path::Path,
    broker_port: u16,
    broker_log: &std::path::Path,
) {
    let session_json = serde_json::json!({
        "session_name": session_name,
        "repo_path": repo_canonical.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-05-01T00:00:00Z",
        "status": "active",
        "worktrees": [{
            "branch": branch,
            "worktree_path": wt_path.to_string_lossy(),
            "cli": "sleep 3600",
            "branch_created": true,
        }],
        "broker_port": broker_port,
        "broker_bind": "127.0.0.1",
        "broker_log_path": broker_log.to_string_lossy(),
        "dashboard_pane": 0,
    });
    fs::write(
        sessions_dir.join(format!("{session_name}.json")),
        serde_json::to_string_pretty(&session_json).expect("serialize session"),
    )
    .expect("write session json");
}

fn create_branch_worktree(
    repo: &std::path::Path,
    project: &str,
    branch: &str,
) -> std::path::PathBuf {
    let parent = repo.parent().expect("repo has parent").to_path_buf();
    let slug = branch.replace('/', "-");
    let wt_path = parent.join(format!("{project}-{slug}"));

    let st = StdCommand::new("git")
        .current_dir(repo)
        .args(["branch", branch])
        .status()
        .expect("git branch");
    assert!(st.success());

    let st = StdCommand::new("git")
        .current_dir(repo)
        .args(["worktree", "add", wt_path.to_str().unwrap(), branch])
        .status()
        .expect("git worktree add");
    assert!(st.success());

    wt_path
}

// ---------------------------------------------------------------------------
// 10.1 pause_detaches_and_stops_broker
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn pause_detaches_and_stops_broker() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let project = unique_project_name("detach");
    let repo = rename_repo_basename(&tr, &project);
    let canonical = canonical_repo_root(&repo);

    let branch = "feat/alpha";
    let wt_path = create_branch_worktree(&repo, &project, branch);

    let broker_port = find_free_port();
    let fake_home = TempDir::new().expect("create temp HOME");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    let session_name = format!("paw-{project}");
    let broker_log = sessions_dir.join("broker.log");
    let broker_url = format!("http://127.0.0.1:{broker_port}");

    // Write a minimal config so __dashboard can find broker settings.
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        format!("[broker]\nenabled = true\nport = {broker_port}\n"),
    )
    .expect("write config");

    build_simulated_session(
        &session_name,
        &repo,
        &broker_url,
        &sessions_dir,
        fake_home.path(),
    );
    write_session_json(
        &sessions_dir,
        &session_name,
        &canonical,
        &project,
        branch,
        &wt_path,
        broker_port,
        &broker_log,
    );

    // Give the dashboard pane a moment to bind the broker port.
    let bind_deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < bind_deadline && !port_is_listening(broker_port) {
        std::thread::sleep(Duration::from_millis(100));
    }

    // Drive pause.
    let out = cmd()
        .current_dir(&repo)
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["pause"])
        .output()
        .expect("run pause");

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();

    // Assertion 1: pause exited 0.
    assert!(
        out.status.success(),
        "pause should exit 0, got status {:?}\nstdout: {stdout}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    // Assertion 2: stdout includes session name + resume hint.
    assert!(
        stdout.contains(&session_name),
        "stdout should mention session name. got: {stdout}"
    );
    assert!(
        stdout.contains("Run 'git paw start' to resume")
            || stdout.contains("Run `git paw start` to resume"),
        "stdout should include resume hint. got: {stdout}"
    );

    // Assertion 3: tmux session is still alive.
    let alive = StdCommand::new("tmux")
        .args(["has-session", "-t", &session_name])
        .status()
        .expect("tmux has-session")
        .success();

    // Always tear down the tmux session before asserting.
    //
    // De-flake (selftest-harness §3.4): the broker shuts down when `pause`
    // kills the dashboard pane, but under full-suite load (especially under
    // `cargo llvm-cov` + concurrent shards) the OS can take longer than the
    // former fixed 5s to actually release the listener socket. This is a
    // port-RELEASE timing race, not a selection collision — the port was
    // already OS-ephemeral. `wait_until_port_free` polls in a bounded loop and
    // returns the instant the port frees, so a wider 30s bound costs nothing
    // in the common case and absorbs the slow-release tail.
    let port_free = wait_until_port_free(broker_port, Duration::from_secs(30));
    kill_tmux_session(&session_name);

    assert!(alive, "tmux session should still be alive after pause");

    // Assertion 4: broker port released within the bounded retry window.
    assert!(
        port_free,
        "broker port {broker_port} should be free after pause (waited up to 30s for release)"
    );

    // Assertion 5: session JSON status flipped to paused.
    let json = fs::read_to_string(sessions_dir.join(format!("{session_name}.json")))
        .expect("read session json");
    assert!(
        json.contains("\"status\": \"paused\""),
        "session.status should be paused after pause. JSON: {json}"
    );
}

// ---------------------------------------------------------------------------
// 10.3 pause_idempotent_on_already_paused
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn pause_idempotent_on_already_paused() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let project = unique_project_name("idem");
    let repo = rename_repo_basename(&tr, &project);
    let canonical = canonical_repo_root(&repo);

    let branch = "feat/alpha";
    let wt_path = create_branch_worktree(&repo, &project, branch);

    let fake_home = TempDir::new().expect("create temp HOME");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    let session_name = format!("paw-{project}");

    // Write a pre-paused session JSON. We don't need a real broker or
    // tmux pane 0 — just a live tmux session so effective_status returns
    // Paused (rather than degrading to Stopped).
    let st = StdCommand::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            &session_name,
            "-x",
            "200",
            "-y",
            "50",
            "-c",
            repo.to_str().unwrap(),
        ])
        .status()
        .expect("tmux new-session");
    assert!(st.success());

    let json = serde_json::json!({
        "session_name": session_name,
        "repo_path": canonical.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-05-01T00:00:00Z",
        "status": "paused",
        "worktrees": [{
            "branch": branch,
            "worktree_path": wt_path.to_string_lossy(),
            "cli": "sleep 3600",
            "branch_created": true,
        }],
    });
    fs::write(
        sessions_dir.join(format!("{session_name}.json")),
        serde_json::to_string_pretty(&json).expect("serialize"),
    )
    .expect("write session json");

    let out = cmd()
        .current_dir(&repo)
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["pause"])
        .output()
        .expect("run pause");

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    kill_tmux_session(&session_name);

    assert!(
        out.status.success(),
        "pause should exit 0 on already-paused"
    );
    assert!(
        stdout.to_lowercase().contains("already paused"),
        "stdout should mention 'already paused', got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// 10.4 stop_after_pause_kills_remaining_panes
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn stop_after_pause_kills_remaining_panes() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let project = unique_project_name("stop-after-pause");
    let repo = rename_repo_basename(&tr, &project);
    let canonical = canonical_repo_root(&repo);

    let branch = "feat/alpha";
    let wt_path = create_branch_worktree(&repo, &project, branch);

    let fake_home = TempDir::new().expect("create temp HOME");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    let session_name = format!("paw-{project}");

    // Spawn a live tmux session and write a paused JSON.
    let st = StdCommand::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            &session_name,
            "-x",
            "200",
            "-y",
            "50",
            "-c",
            repo.to_str().unwrap(),
        ])
        .status()
        .expect("tmux new-session");
    assert!(st.success());

    let json = serde_json::json!({
        "session_name": session_name,
        "repo_path": canonical.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-05-01T00:00:00Z",
        "status": "paused",
        "worktrees": [{
            "branch": branch,
            "worktree_path": wt_path.to_string_lossy(),
            "cli": "sleep 3600",
            "branch_created": true,
        }],
    });
    fs::write(
        sessions_dir.join(format!("{session_name}.json")),
        serde_json::to_string_pretty(&json).expect("serialize"),
    )
    .expect("write session json");

    // Stop --force: skips the prompt, kills the tmux session.
    let out = cmd()
        .current_dir(&repo)
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["stop", "--force"])
        .output()
        .expect("run stop --force");

    let alive_after = StdCommand::new("tmux")
        .args(["has-session", "-t", &session_name])
        .status()
        .is_ok_and(|s| s.success());
    kill_tmux_session(&session_name); // belt-and-braces cleanup

    assert!(
        out.status.success(),
        "stop --force should exit 0. stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !alive_after,
        "tmux session should be gone after stop --force"
    );

    let final_json = fs::read_to_string(sessions_dir.join(format!("{session_name}.json")))
        .expect("read final session json");
    assert!(
        final_json.contains("\"status\": \"stopped\""),
        "session.status should be stopped. JSON: {final_json}"
    );
}
