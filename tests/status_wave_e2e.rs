//! End-to-end coverage for `git paw status` surfacing a stalled wave
//! (`broker-runtime`'s "Status surfaces a stalled wave"), driven through the
//! `git-paw` binary against a real in-process broker.
//!
//! Each test hosts a broker on a free port, seeds a session receipt pointing
//! at it, and ages the broker's last-progress stamp so the wave is stalled by
//! a known amount — no sleeping, and no dependence on how long the test took.
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

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after the epoch")
        .as_secs()
}

/// Starts a broker for `repo_root` whose wave last progressed `idle` seconds
/// ago, under a one-minute stall threshold. Returns the handle (keep it alive
/// for the test) and the port it serves.
fn broker_idle_for(repo_root: &std::path::Path, idle: u64) -> Option<(BrokerHandle, u16)> {
    let port = free_port();
    let config = BrokerConfig {
        enabled: true,
        port,
        bind: "127.0.0.1".to_string(),
        wave_stall_minutes: Some(1),
        ..Default::default()
    };
    let handle = start_broker(
        &config,
        BrokerState::new(None).with_repo_root(repo_root),
        Vec::new(),
    )
    .ok()?;
    handle.state.set_last_progress_unix_secs(now_unix() - idle);
    Some((handle, port))
}

/// Writes a running-session receipt naming the broker on `port`.
fn seed_session(
    sessions_dir: &std::path::Path,
    session_name: &str,
    repo: &std::path::Path,
    port: u16,
) {
    let canonical = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let session_json = serde_json::json!({
        "session_name": session_name,
        "repo_path": canonical.to_string_lossy(),
        "project_name": git_paw::git::project_name(repo),
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
}

/// Runs `git paw status` (optionally `--json`) in `repo` against `home`.
fn run_status(repo: &std::path::Path, home: &std::path::Path, json: bool) -> String {
    let mut command = cmd();
    command
        .current_dir(repo)
        .env("HOME", home)
        .env_remove("XDG_DATA_HOME")
        .arg("status");
    if json {
        command.arg("--json");
    }
    let output = command
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run git paw status");
    assert!(
        output.status.success(),
        "git paw status should succeed; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    // Guard against a vacuous pass: a seeded session that `status` fails to
    // find prints "No session for this repo." and would silently satisfy
    // every "must not contain" assertion below.
    assert!(
        !stdout.contains("No session for this repo"),
        "the seeded session must be found; got:\n{stdout}"
    );
    stdout
}

/// Sets up a repository, a fake home with a sessions directory, and a broker
/// idle for `idle` seconds. Returns `None` when the broker could not bind,
/// which makes the test inconclusive rather than failing.
fn session_with_idle_broker(idle: u64) -> Option<(TestRepo, TempDir, BrokerHandle)> {
    let repo = setup_test_repo();
    let fake_home = TempDir::new().expect("fake home");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    let (handle, port) = broker_idle_for(repo.path(), idle)?;
    let session_name = format!("paw-{}", git_paw::git::project_name(repo.path()));
    seed_session(&sessions_dir, &session_name, repo.path(), port);

    Some((repo, fake_home, handle))
}

/// Scenario "A stalled wave is visible in status": 11520 idle seconds reads
/// as 3h12m, in words an operator can act on.
#[test]
#[serial]
fn a_stalled_wave_is_visible_in_status() {
    let Some((repo, home, _broker)) = session_with_idle_broker(11_520) else {
        return;
    };

    let stdout = run_status(repo.path(), home.path(), false);

    assert!(
        stdout.contains("wave stalled") && stdout.contains("no progress for 3h12m"),
        "status must name the stall and how long it has lasted; got:\n{stdout}"
    );
}

/// Scenario "JSON status carries the wave object".
#[test]
#[serial]
fn json_status_carries_the_wave_object() {
    let Some((repo, home, _broker)) = session_with_idle_broker(11_520) else {
        return;
    };

    let stdout = run_status(repo.path(), home.path(), true);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).expect("status --json");

    let wave = &parsed["wave"];
    assert_eq!(wave["stalled"], true, "got:\n{stdout}");
    assert!(
        wave["idle_seconds"].as_u64().unwrap_or(0) >= 11_520,
        "got:\n{stdout}"
    );
}

/// A broker on the recorded port that serves a DIFFERENT repository must
/// not supply this session's wave.
///
/// The default port is shared across repositories, so a neighbour's live
/// broker routinely holds it. Attributing its wave to this session would
/// print a stalled-wave line — or hide one — on the strength of an
/// unrelated repository's agents, and `status` already refuses to call such
/// a broker "running" for exactly that reason.
#[test]
#[serial]
fn another_repositorys_broker_does_not_supply_this_sessions_wave() {
    let repo = setup_test_repo();
    let neighbour = TempDir::new().expect("neighbour repo");
    let fake_home = TempDir::new().expect("fake home");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    // The broker on the port belongs to the NEIGHBOUR, and its wave is
    // stalled. Our session records that same port.
    let port = free_port();
    let config = BrokerConfig {
        enabled: true,
        port,
        bind: "127.0.0.1".to_string(),
        wave_stall_minutes: Some(1),
        ..Default::default()
    };
    let Ok(handle) = start_broker(
        &config,
        BrokerState::new(None).with_repo_root(neighbour.path()),
        Vec::new(),
    ) else {
        return;
    };
    handle
        .state
        .set_last_progress_unix_secs(now_unix() - 11_520);

    let session_name = format!("paw-{}", git_paw::git::project_name(repo.path()));
    seed_session(&sessions_dir, &session_name, repo.path(), port);

    let stdout = run_status(repo.path(), fake_home.path(), false);
    assert!(
        stdout.contains("held by another repository"),
        "precondition: the port must be recognised as a neighbour's; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("wave stalled"),
        "a neighbour's wave must not be reported as this session's; got:\n{stdout}"
    );

    let json = run_status(repo.path(), fake_home.path(), true);
    let parsed: serde_json::Value = serde_json::from_str(json.trim()).expect("status --json");
    assert!(
        parsed.get("wave").is_none(),
        "--json must carry no wave from a foreign broker; got:\n{json}"
    );
}

/// Scenario "A progressing wave adds no line": a wave well inside its
/// threshold leaves the human output exactly as it was.
#[test]
#[serial]
fn a_progressing_wave_adds_no_line() {
    let Some((repo, home, _broker)) = session_with_idle_broker(0) else {
        return;
    };

    let stdout = run_status(repo.path(), home.path(), false);

    assert!(
        !stdout.contains("wave stalled"),
        "a progressing wave must add nothing; got:\n{stdout}"
    );
}
