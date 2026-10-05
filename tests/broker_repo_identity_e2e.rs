//! End-to-end coverage for broker repository identity and the per-session
//! broker log, driven through the `git-paw` binary rather than through
//! re-implementations of the flows under test.
//!
//! Four behaviours live here:
//!
//! 1. A second repository launching against a port already held by another
//!    repository's broker is refused **before** any pane exists — the whole
//!    point of the pre-flight check, since every agent pane would otherwise
//!    be handed a `GIT_PAW_BROKER_URL` pointing into the other repository.
//! 2. A real `git paw start` records the per-session broker log name, so the
//!    launcher, the dashboard, and `purge` all name the same file.
//! 3. `git paw purge --force` deletes only the purged session's broker log —
//!    including when that session was saved by an older git-paw under the
//!    shared `broker.log` name.
//! 4. `git paw status` in the second repository reports the port as held by
//!    another repository rather than as its own broker running.
//!
//! NOTE: Run with `GIT_PAW_ALLOW_LIVE_SESSION=1` if there is a live `paw-*`
//! tmux session on the default socket (e.g. during dogfooding) —
//! `setup_test_repo` otherwise refuses to run.

use std::fs;
use std::time::Duration;

use assert_cmd::Command;
use git_paw::broker::{BrokerHandle, BrokerState, start_broker};
use git_paw::config::BrokerConfig;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn sessions_dir_for(home: &std::path::Path) -> std::path::PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-paw/sessions")
    } else {
        home.join(".local/share/git-paw/sessions")
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral")
        .local_addr()
        .expect("local_addr")
        .port()
}

fn port_is_listening(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().expect("parse addr"),
        Duration::from_millis(200),
    )
    .is_ok()
}

/// Publishes one `agent.status` so the broker has something to flush to its
/// log. Written over a raw socket to keep the test free of an HTTP client.
fn publish_status(port: u16, agent_id: &str) {
    use std::io::Write;
    let body = format!(
        r#"{{"type":"agent.status","agent_id":"{agent_id}","payload":{{"status":"working"}}}}"#
    );
    let request = format!(
        "POST /publish HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    if let Ok(mut stream) = std::net::TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().expect("parse addr"),
        Duration::from_secs(2),
    ) {
        let _ = stream.write_all(request.as_bytes());
        let _ = stream.flush();
        // Let the broker finish handling it before the caller kills the pane.
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Starts a real broker serving `repo_root` on a free port, standing in for a
/// neighbouring repository's live session.
fn neighbour_broker(repo_root: &std::path::Path) -> (BrokerHandle, u16) {
    let port = free_port();
    let config = BrokerConfig {
        enabled: true,
        port,
        bind: "127.0.0.1".to_string(),
        ..Default::default()
    };
    let handle = start_broker(
        &config,
        BrokerState::new(None).with_repo_root(repo_root),
        Vec::new(),
    )
    .expect("start the neighbouring repository's broker");
    (handle, port)
}

/// Writes a `.git-paw/config.toml` enabling the broker on `port`.
fn write_broker_config(repo: &std::path::Path, port: u16) {
    let paw = repo.join(".git-paw");
    fs::create_dir_all(&paw).expect("create .git-paw");
    fs::write(
        paw.join("config.toml"),
        format!("[broker]\nenabled = true\nport = {port}\n"),
    )
    .expect("write config");
}

/// Puts an executable stub named `cli` on a fresh directory, returned with
/// the directory so detection finds a launchable CLI in the test environment.
fn stub_cli_dir(cli: &str) -> TempDir {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().expect("stub bin dir");
    let path = dir.path().join(cli);
    fs::write(&path, "#!/bin/sh\nsleep 30\n").expect("write stub cli");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod stub cli");
    dir
}

// ---------------------------------------------------------------------------
// A second repository is refused before any pane launches
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn start_in_a_second_repository_is_refused_before_any_pane_launches() {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return;
    }

    // Repository A holds the port with a live broker.
    let repo_a = setup_test_repo();
    let repo_a_root = repo_a
        .path()
        .canonicalize()
        .expect("canonicalize repository A");
    let (_held, port) = neighbour_broker(&repo_a_root);

    // Repository B is configured for the same port — the default-config
    // collision this change exists to catch.
    let repo_b = setup_test_repo();
    write_broker_config(repo_b.path(), port);

    let project = format!("brokeridentity-{}", std::process::id());
    let session_name = format!("paw-{project}");
    let stub = stub_cli_dir("claude");
    let inherited = std::env::var("PATH").unwrap_or_default();
    let fake_home = TempDir::new().expect("fake home");
    let tmux_env = tmux_test_env();

    let mut start = cmd();
    start
        .current_dir(repo_b.path())
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .env("PATH", format!("{}:{inherited}", stub.path().display()));
    tmux_env.apply_assert(&mut start);
    let output = start
        .args(["start", "--branches", "feat/alpha", "--cli", "claude"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run git paw start");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "the launch must fail; stdout:\n{}\nstderr:\n{stderr}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains(&port.to_string()),
        "the error must name the contested port; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("[broker] port"),
        "the error must name the config fix; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains(&repo_a_root.display().to_string()),
        "the error must name the holding repository; stderr:\n{stderr}"
    );

    // Nothing may have been created: no tmux session, so no agent pane was
    // ever handed the other repository's broker URL.
    let mut has_session = std::process::Command::new("tmux");
    tmux_env.apply(&mut has_session);
    let alive = has_session
        .args(["has-session", "-t", &session_name])
        .status()
        .expect("tmux has-session")
        .success();
    assert!(
        !alive,
        "the refusal must happen before the tmux session is created"
    );

    // And no session receipt was written.
    let sessions = sessions_dir_for(fake_home.path());
    assert!(
        !sessions.join(format!("{session_name}.json")).exists(),
        "a refused launch must not leave a session receipt behind"
    );
}

#[test]
#[serial]
fn a_dry_run_prints_its_plan_even_when_the_port_is_held() {
    // A dry run creates nothing, so there is no agent pane to misroute and
    // nothing for the clash to damage — it must still print its plan.
    let repo_a = setup_test_repo();
    let repo_a_root = repo_a
        .path()
        .canonicalize()
        .expect("canonicalize repository A");
    let (_held, port) = neighbour_broker(&repo_a_root);

    let repo_b = setup_test_repo();
    write_broker_config(repo_b.path(), port);

    let stub = stub_cli_dir("claude");
    let inherited = std::env::var("PATH").unwrap_or_default();
    let fake_home = TempDir::new().expect("fake home");

    let output = cmd()
        .current_dir(repo_b.path())
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .env("PATH", format!("{}:{inherited}", stub.path().display()))
        .args([
            "start",
            "--dry-run",
            "--branches",
            "feat/alpha",
            "--cli",
            "claude",
        ])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run git paw start --dry-run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "a dry run must not be refused; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("Dry run"),
        "the plan should still be printed; stdout:\n{stdout}"
    );
    assert!(
        !stderr.contains("[broker] port"),
        "a dry run must not raise the port-held error; stderr:\n{stderr}"
    );
}

#[test]
#[serial]
fn start_refuses_before_the_branch_and_cli_pickers() {
    // With neither --branches nor --cli, a launch that got as far as the
    // pickers would block on stdin. Refusing first means the command returns
    // the port-held error instead of asking the user to choose for a launch
    // that cannot proceed.
    let repo_a = setup_test_repo();
    let repo_a_root = repo_a
        .path()
        .canonicalize()
        .expect("canonicalize repository A");
    let (_held, port) = neighbour_broker(&repo_a_root);

    let repo_b = setup_test_repo();
    write_broker_config(repo_b.path(), port);

    let stub = stub_cli_dir("claude");
    let inherited = std::env::var("PATH").unwrap_or_default();
    let fake_home = TempDir::new().expect("fake home");
    let tmux_env = tmux_test_env();

    let mut start = cmd();
    start
        .current_dir(repo_b.path())
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .env("PATH", format!("{}:{inherited}", stub.path().display()));
    tmux_env.apply_assert(&mut start);
    let output = start
        .args(["start"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run git paw start");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "the launch must fail; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("[broker] port"),
        "the port-held error must be what fails it; stderr:\n{stderr}"
    );
    assert!(
        !stdout.contains("Select") && !stdout.contains("branches"),
        "no picker may have run before the refusal; stdout:\n{stdout}"
    );
}

#[test]
#[serial]
fn recovery_probes_the_port_the_agents_will_be_given() {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return;
    }

    // The saved session's port is what recovery hands the rebuilt agent panes
    // via GIT_PAW_BROKER_URL, and it can differ from the current config when
    // `[broker] port` changed since the session was saved. Probing the
    // configured port would miss the clash on the saved one.
    let repo_a = setup_test_repo();
    let repo_a_root = repo_a
        .path()
        .canonicalize()
        .expect("canonicalize repository A");
    let (_held, saved_port) = neighbour_broker(&repo_a_root);

    let repo_b = setup_test_repo();
    // Config points at a free port; the saved session points at the contested
    // one. Only a probe of the saved port can catch this.
    write_broker_config(repo_b.path(), free_port());

    let project = git_paw::git::project_name(repo_b.path());
    let session_name = format!("paw-{project}");
    let fake_home = TempDir::new().expect("fake home");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");
    seed_session_on_port(
        &sessions_dir,
        &session_name,
        repo_b.path(),
        &project,
        &sessions_dir.join(format!("{session_name}.broker.log")),
        saved_port,
    );

    let stub = stub_cli_dir("claude");
    let inherited = std::env::var("PATH").unwrap_or_default();
    let tmux_env = tmux_test_env();
    let mut start = cmd();
    start
        .current_dir(repo_b.path())
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .env("PATH", format!("{}:{inherited}", stub.path().display()));
    tmux_env.apply_assert(&mut start);
    let output = start
        .args(["start"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run git paw start");

    let mut kill = std::process::Command::new("tmux");
    tmux_env.apply(&mut kill);
    let _ = kill.args(["kill-session", "-t", &session_name]).status();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "recovery must be refused on the port the agents would actually use; \
         stdout:\n{}\nstderr:\n{stderr}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains(&saved_port.to_string()),
        "the error must name the SAVED port, not the configured one; stderr:\n{stderr}"
    );
}

#[test]
#[serial]
fn the_recovered_dashboard_writes_the_re_stamped_log_not_the_old_one() {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return;
    }

    // The end state the re-stamp exists for: the broker writes the
    // re-stamped per-session log — the one `purge` will delete — and never
    // the pre-re-stamp shared `broker.log`, which nothing would clean up.
    //
    // Note this does NOT discriminate the save-before-launch ordering in
    // `recover_session`: with the save moved back after `execute()` the test
    // still passes, because the dashboard subprocess takes far longer to
    // boot and read the session than the save takes to land. The ordering is
    // a correctness fix for a window too narrow to drive from a test.
    let repo = setup_test_repo();
    let port = free_port();
    write_broker_config(repo.path(), port);

    let project = git_paw::git::project_name(repo.path());
    let session_name = format!("paw-{project}");
    let fake_home = TempDir::new().expect("fake home");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    let legacy_log = sessions_dir.join("broker.log");
    let restamped_log = sessions_dir.join(format!("{session_name}.broker.log"));
    seed_session_on_port(
        &sessions_dir,
        &session_name,
        repo.path(),
        &project,
        &legacy_log,
        port,
    );

    let stub = stub_cli_dir("claude");
    let inherited = std::env::var("PATH").unwrap_or_default();
    let tmux_env = tmux_test_env();
    let mut start = cmd();
    start
        .current_dir(repo.path())
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .env("PATH", format!("{}:{inherited}", stub.path().display()));
    tmux_env.apply_assert(&mut start);
    let output = start
        .args(["start"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run git paw start");

    // Wait for the recovered dashboard pane to bind the broker port.
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while std::time::Instant::now() < deadline && !port_is_listening(port) {
        std::thread::sleep(Duration::from_millis(100));
    }
    let bound = port_is_listening(port);

    // Give the broker something to log, then kill the pane so the
    // BrokerHandle drop performs its final flush to disk.
    if bound {
        publish_status(port, "feat-alpha");
    }
    let mut kill = std::process::Command::new("tmux");
    tmux_env.apply(&mut kill);
    let _ = kill.args(["kill-session", "-t", &session_name]).status();
    let flushed = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < flushed && !restamped_log.exists() {
        std::thread::sleep(Duration::from_millis(100));
    }

    assert!(
        bound,
        "the recovered dashboard never bound port {port}, so this test proved nothing; \
         stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !legacy_log.exists(),
        "the dashboard must not write the pre-re-stamp shared broker.log — \
         that is the orphaned log purge would then miss"
    );
    assert!(
        restamped_log.exists(),
        "the dashboard must write the re-stamped per-session log the session \
         now records (and purge will delete)"
    );
}

#[test]
#[serial]
fn a_failed_resume_leaves_the_session_paused_with_the_log_path_re_stamped() {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return;
    }

    // Resume-from-pause publishes the re-stamped broker log path before the
    // dashboard pane exists (the dashboard reads its log path from the saved
    // session), but it must NOT publish `status = Active` that early: if the
    // pane recreation then fails, a session recorded active with tmux still
    // alive is one the next `git paw start` reattaches to instead of
    // restarting the broker.
    //
    // The failure is forced by pointing `dashboard_pane` at an index that
    // does not exist, so the `split-window -t <session>:0.<N>` below fails.
    let repo = setup_test_repo();
    let port = free_port();
    write_broker_config(repo.path(), port);

    let project = git_paw::git::project_name(repo.path());
    let session_name = format!("paw-{project}");
    let fake_home = TempDir::new().expect("fake home");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");
    let tmux_env = tmux_test_env();

    // A live tmux session, so the paused receipt resolves to Paused rather
    // than downgrading to Stopped (which would take the recovery path).
    let mut new_session = std::process::Command::new("tmux");
    tmux_env.apply(&mut new_session);
    let st = new_session
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
            repo.path().to_str().expect("repo path is utf-8"),
        ])
        .status()
        .expect("tmux new-session");
    assert!(st.success(), "tmux new-session must succeed");

    // A recent `created_at` keeps the session inside the agent-launch grace
    // period, so liveness is Alive without a real agent CLI in a pane.
    // `YYYY-MM-DDTHH:MM:SSZ` — the only shape the session parser accepts.
    let now_iso = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let legacy_log = sessions_dir.join("broker.log");
    let restamped_log = sessions_dir.join(format!("{session_name}.broker.log"));
    let canonical = repo.path().canonicalize().expect("canonicalize repo");
    let session_json = serde_json::json!({
        "session_name": session_name,
        "repo_path": canonical.to_string_lossy(),
        "project_name": project,
        "created_at": now_iso,
        "status": "paused",
        "worktrees": [],
        "broker_port": port,
        "broker_bind": "127.0.0.1",
        "broker_log_path": legacy_log.to_string_lossy(),
        // No such pane — the dashboard recreation below cannot succeed.
        "dashboard_pane": 99,
    });
    let receipt = sessions_dir.join(format!("{session_name}.json"));
    fs::write(
        &receipt,
        serde_json::to_string_pretty(&session_json).expect("serialize session"),
    )
    .expect("write session json");

    let stub = stub_cli_dir("claude");
    let inherited = std::env::var("PATH").unwrap_or_default();
    let mut start = cmd();
    start
        .current_dir(repo.path())
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .env("PATH", format!("{}:{inherited}", stub.path().display()));
    tmux_env.apply_assert(&mut start);
    let output = start
        .args(["start"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run git paw start");

    let mut kill = std::process::Command::new("tmux");
    tmux_env.apply(&mut kill);
    let _ = kill.args(["kill-session", "-t", &session_name]).status();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "the resume must fail when the dashboard pane cannot be recreated; \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&receipt).expect("read receipt"))
            .expect("parse receipt");
    assert_eq!(
        saved["status"], "paused",
        "a failed resume must leave the session paused, so the next \
         `git paw start` restarts the broker instead of reattaching to a \
         session it believes is already active; got: {saved}"
    );
    assert_eq!(
        saved["broker_log_path"],
        serde_json::json!(restamped_log.to_string_lossy()),
        "the log path must still have been re-stamped before the pane was \
         attempted — that write is what keeps the dashboard and purge in \
         agreement; got: {saved}"
    );
}

#[test]
#[serial]
fn recovering_a_session_without_a_broker_gets_no_log_path() {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return;
    }

    let repo = setup_test_repo();
    let project = git_paw::git::project_name(repo.path());
    let session_name = format!("paw-{project}");
    let fake_home = TempDir::new().expect("fake home");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    // A stopped session with no broker at all: no port, no bind, no log path.
    let canonical = repo.path().canonicalize().expect("canonicalize repo");
    let session_json = serde_json::json!({
        "session_name": session_name,
        "repo_path": canonical.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-05-01T00:00:00Z",
        "status": "stopped",
        "worktrees": [],
    });
    fs::write(
        sessions_dir.join(format!("{session_name}.json")),
        serde_json::to_string_pretty(&session_json).expect("serialize session"),
    )
    .expect("write session json");

    let stub = stub_cli_dir("claude");
    let inherited = std::env::var("PATH").unwrap_or_default();
    let tmux_env = tmux_test_env();
    let mut start = cmd();
    start
        .current_dir(repo.path())
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .env("PATH", format!("{}:{inherited}", stub.path().display()));
    tmux_env.apply_assert(&mut start);
    let output = start
        .args(["start"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run git paw start");

    let mut kill = std::process::Command::new("tmux");
    tmux_env.apply(&mut kill);
    let _ = kill.args(["kill-session", "-t", &session_name]).status();

    let saved: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(sessions_dir.join(format!("{session_name}.json")))
            .expect("read receipt"),
    )
    .expect("parse receipt");
    assert!(
        saved.get("broker_log_path").is_none(),
        "a session with no broker must not be handed a log path; got: {saved}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ---------------------------------------------------------------------------
// the launcher records the per-session broker log
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn start_records_a_per_session_broker_log_path() {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return;
    }

    let repo = setup_test_repo();
    let port = free_port();
    write_broker_config(repo.path(), port);

    let project = git_paw::git::project_name(repo.path());
    let session_name = format!("paw-{project}");
    let stub = stub_cli_dir("claude");
    let inherited = std::env::var("PATH").unwrap_or_default();
    let fake_home = TempDir::new().expect("fake home");
    let tmux_env = tmux_test_env();

    let mut start = cmd();
    start
        .current_dir(repo.path())
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .env("PATH", format!("{}:{inherited}", stub.path().display()));
    tmux_env.apply_assert(&mut start);
    let output = start
        .args(["start", "--branches", "feat/alpha", "--cli", "claude"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run git paw start");

    // Tear the session down before asserting, so a failure cannot strand it.
    let mut kill = std::process::Command::new("tmux");
    tmux_env.apply(&mut kill);
    let _ = kill.args(["kill-session", "-t", &session_name]).status();

    let receipt = sessions_dir_for(fake_home.path()).join(format!("{session_name}.json"));
    assert!(
        receipt.exists(),
        "start should have written a session receipt; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&receipt).expect("read receipt"))
            .expect("parse receipt");
    let log_path = saved["broker_log_path"]
        .as_str()
        .expect("a broker-enabled session records broker_log_path");
    assert!(
        log_path.ends_with(&format!("/{session_name}.broker.log")),
        "the launcher must record the per-session log name, so purge and the \
         dashboard agree and no neighbour's log is at risk; got: {log_path}"
    );
}

#[test]
#[serial]
fn recovery_re_stamps_a_shared_broker_log_path() {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return;
    }

    let repo = setup_test_repo();
    let port = free_port();
    write_broker_config(repo.path(), port);

    let project = git_paw::git::project_name(repo.path());
    let session_name = format!("paw-{project}");
    let fake_home = TempDir::new().expect("fake home");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    // A stopped session as an earlier git-paw would have saved it: the
    // single shared log name, with no session in it.
    let legacy_log = sessions_dir.join("broker.log");
    seed_session(
        &sessions_dir,
        &session_name,
        repo.path(),
        &project,
        &legacy_log,
    );

    let stub = stub_cli_dir("claude");
    let inherited = std::env::var("PATH").unwrap_or_default();
    let tmux_env = tmux_test_env();
    let mut start = cmd();
    start
        .current_dir(repo.path())
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .env("PATH", format!("{}:{inherited}", stub.path().display()));
    tmux_env.apply_assert(&mut start);
    let output = start
        .args(["start"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run git paw start");

    let mut kill = std::process::Command::new("tmux");
    tmux_env.apply(&mut kill);
    let _ = kill.args(["kill-session", "-t", &session_name]).status();

    let receipt = sessions_dir.join(format!("{session_name}.json"));
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&receipt).expect("read receipt"))
            .expect("parse receipt");
    let log_path = saved["broker_log_path"]
        .as_str()
        .expect("the recovered session still records a broker log path");
    assert!(
        log_path.ends_with(&format!("/{session_name}.broker.log")),
        "recovery must migrate the shared log name onto the per-session one; \
         got: {log_path}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ---------------------------------------------------------------------------
// purge deletes only its own session's broker log
// ---------------------------------------------------------------------------

/// Writes a stopped-session receipt for `session_name` with `broker_log_path`
/// set and the broker recorded on `port`.
fn seed_session_on_port(
    sessions_dir: &std::path::Path,
    session_name: &str,
    repo: &std::path::Path,
    project: &str,
    broker_log: &std::path::Path,
    port: u16,
) {
    let canonical = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let session_json = serde_json::json!({
        "session_name": session_name,
        "repo_path": canonical.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-05-01T00:00:00Z",
        "status": "stopped",
        "worktrees": [],
        "broker_port": port,
        "broker_bind": "127.0.0.1",
        "broker_log_path": broker_log.to_string_lossy(),
    });
    fs::write(
        sessions_dir.join(format!("{session_name}.json")),
        serde_json::to_string_pretty(&session_json).expect("serialize session"),
    )
    .expect("write session json");
}

/// [`seed_session_on_port`] on a port nothing is listening on, for tests that
/// do not care which port the session records.
fn seed_session(
    sessions_dir: &std::path::Path,
    session_name: &str,
    repo: &std::path::Path,
    project: &str,
    broker_log: &std::path::Path,
) {
    seed_session_on_port(
        sessions_dir,
        session_name,
        repo,
        project,
        broker_log,
        free_port(),
    );
}

#[test]
#[serial]
fn purge_force_deletes_only_its_own_sessions_broker_log() {
    let repo = setup_test_repo();
    let fake_home = TempDir::new().expect("fake home");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    let project = git_paw::git::project_name(repo.path());
    let session_name = format!("paw-{project}");

    // This session's log, named exactly as the launcher names it, plus a
    // neighbouring repository's log sitting in the same shared directory.
    let own_log = sessions_dir.join(format!("{session_name}.broker.log"));
    let neighbour_log = sessions_dir.join("paw-someone-else.broker.log");
    fs::write(&own_log, "our messages").expect("write own log");
    fs::write(&neighbour_log, "the neighbour's messages").expect("write neighbour log");

    seed_session(
        &sessions_dir,
        &session_name,
        repo.path(),
        &project,
        &own_log,
    );

    let output = cmd()
        .current_dir(repo.path())
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["purge", "--force"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run git paw purge --force");

    assert!(
        output.status.success(),
        "purge --force should succeed; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !own_log.exists(),
        "purge must delete the purged session's broker log"
    );
    assert!(
        neighbour_log.exists(),
        "purge must leave another session's broker log in place"
    );
}

/// A session receipt saved by a git-paw that wrote one shared `broker.log`.
/// Purge must still find and delete exactly the file that session recorded.
#[test]
#[serial]
fn purge_force_honours_an_old_format_shared_broker_log_path() {
    let repo = setup_test_repo();
    let fake_home = TempDir::new().expect("fake home");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    let project = git_paw::git::project_name(repo.path());
    let session_name = format!("paw-{project}");

    // The pre-change name: one shared file, with no session in it.
    let legacy_log = sessions_dir.join("broker.log");
    fs::write(&legacy_log, "messages from an older git-paw").expect("write legacy log");

    seed_session(
        &sessions_dir,
        &session_name,
        repo.path(),
        &project,
        &legacy_log,
    );

    let output = cmd()
        .current_dir(repo.path())
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["purge", "--force"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run git paw purge --force");

    assert!(
        output.status.success(),
        "purge --force should succeed; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !legacy_log.exists(),
        "purge must delete the path the session actually recorded, \
         whatever name an older git-paw gave it"
    );
}

// ---------------------------------------------------------------------------
// status does not report another repository's broker as ours
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn status_reports_a_port_held_by_another_repository() {
    let repo_a = setup_test_repo();
    let repo_a_root = repo_a
        .path()
        .canonicalize()
        .expect("canonicalize repository A");
    let (_held, port) = neighbour_broker(&repo_a_root);

    let repo_b = setup_test_repo();
    let fake_home = TempDir::new().expect("fake home");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    let project = git_paw::git::project_name(repo_b.path());
    let session_name = format!("paw-{project}");
    let canonical_b = repo_b.path().canonicalize().expect("canonicalize B");
    let session_json = serde_json::json!({
        "session_name": session_name,
        "repo_path": canonical_b.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-05-01T00:00:00Z",
        "status": "active",
        "worktrees": [],
        "broker_port": port,
        "broker_bind": "127.0.0.1",
    });
    fs::write(
        sessions_dir.join(format!("{session_name}.json")),
        serde_json::to_string_pretty(&session_json).expect("serialize session"),
    )
    .expect("write session json");

    let output = cmd()
        .current_dir(repo_b.path())
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["status"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run git paw status");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("another repository"),
        "status must not claim repository A's broker as ours; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("(running)"),
        "a broker serving a different repository is not this session's broker \
         running; got:\n{stdout}"
    );
}
