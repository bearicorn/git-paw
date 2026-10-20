//! Fixture tests for `broker-agent-helper`'s *sweep.sh events long-polls the
//! broker log* requirement.
//!
//! The bundled `sweep.sh` is driven as a real subprocess against a real
//! broker, so what is asserted is the JSON an orchestrator actually reads on
//! stdout — not a re-implementation of the verb.

use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;
use std::sync::Arc;
use std::time::Duration;

use git_paw::broker::delivery::publish_message;
use git_paw::broker::messages::{BrokerMessage, StatusPayload};
use git_paw::broker::{BrokerHandle, BrokerState, start_broker};
use git_paw::config::BrokerConfig;
use tempfile::TempDir;

fn init_git_repo(dir: &Path) {
    let run = |args: &[&str]| {
        StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git command");
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "t@e.st"]);
    run(&["config", "user.name", "Test"]);
    fs::write(dir.join("README.md"), "x").expect("readme");
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "init"]);
}

/// Copies the bundled `sweep.sh` and the `_paw_common.sh` preamble it sources
/// into `<repo>/.git-paw/scripts/`, mirroring `git paw init`'s co-deployment.
fn install_sweep(repo: &Path) -> std::path::PathBuf {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts");
    let dst_dir = repo.join(".git-paw/scripts");
    fs::create_dir_all(&dst_dir).expect("mk scripts dir");
    fs::copy(
        assets.join("_paw_common.sh"),
        dst_dir.join("_paw_common.sh"),
    )
    .expect("copy _paw_common.sh");
    let dst = dst_dir.join("sweep.sh");
    fs::copy(assets.join("sweep.sh"), &dst).expect("copy sweep.sh");
    dst
}

fn write_broker_config(repo: &Path, port: u16) {
    fs::create_dir_all(repo.join(".git-paw")).expect("mk .git-paw");
    fs::write(
        repo.join(".git-paw/config.toml"),
        format!("[broker]\nenabled = true\nport = {port}\n"),
    )
    .expect("write config.toml");
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral")
        .local_addr()
        .expect("local addr")
        .port()
}

/// Runs `sweep.sh events <args>` in `repo` and returns its stdout and exit
/// status.
fn run_events(repo: &Path, args: &[&str]) -> (String, bool) {
    let script = repo.join(".git-paw/scripts/sweep.sh");
    let output = StdCommand::new("bash")
        .arg(&script)
        .arg("events")
        .args(args)
        .current_dir(repo)
        .env_remove("TMUX")
        .output()
        .expect("run sweep.sh events");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        output.status.success(),
    )
}

fn status_msg(agent: &str) -> BrokerMessage {
    BrokerMessage::Status {
        agent_id: agent.to_string(),
        payload: StatusPayload {
            status: "working".to_string(),
            ..Default::default()
        },
    }
}

/// Starts a broker on a free port with a one-minute stall threshold, and
/// writes the repo config pointing at it. `None` when the port is busy.
fn broker_for(repo: &Path) -> Option<BrokerHandle> {
    let port = free_port();
    let config = BrokerConfig {
        enabled: true,
        port,
        bind: "127.0.0.1".to_string(),
        wave_stall_minutes: Some(1),
        ..Default::default()
    };
    let handle = start_broker(&config, BrokerState::new(None), Vec::new()).ok()?;
    write_broker_config(repo, port);
    Some(handle)
}

/// Spec scenario "events prints new entries and the cursor": a broker at
/// sequence 3, a publish during the wait, and stdout naming the new message
/// with the cursor advanced to 4.
#[test]
fn events_prints_new_entries_and_the_cursor() {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());
    install_sweep(tmp.path());
    let Some(handle) = broker_for(tmp.path()) else {
        return;
    };

    for n in 0..3 {
        publish_message(&handle.state, &status_msg(&format!("feat-seed{n}")));
    }

    let publisher = Arc::clone(&handle.state);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        publish_message(&publisher, &status_msg("feat-late"));
    });

    let (stdout, ok) = run_events(tmp.path(), &["3", "5"]);
    assert!(ok, "events must exit 0; got:\n{stdout}");

    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("{e}; stdout was:\n{stdout}"));
    let entries = parsed["entries"].as_array().expect("entries array");
    assert_eq!(entries.len(), 1, "only the new message; got:\n{stdout}");
    assert_eq!(entries[0]["message"]["agent_id"], "feat-late");
    assert_eq!(parsed["last_seq"], 4, "got:\n{stdout}");
}

/// Spec scenario "events reports a stalled wave on timeout": nothing new, so
/// the caller's value is the wave — it still learns the session is stuck.
#[test]
fn events_reports_a_stalled_wave_on_timeout() {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());
    install_sweep(tmp.path());
    let Some(handle) = broker_for(tmp.path()) else {
        return;
    };

    for n in 0..3 {
        publish_message(&handle.state, &status_msg(&format!("feat-seed{n}")));
    }
    let aged = handle.state.wave_status().last_progress_unix_secs - 600;
    handle.state.set_last_progress_unix_secs(aged);

    let (stdout, ok) = run_events(tmp.path(), &["3", "1"]);
    assert!(ok, "events must exit 0; got:\n{stdout}");

    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("{e}; stdout was:\n{stdout}"));
    assert!(
        parsed["entries"].as_array().expect("entries").is_empty(),
        "got:\n{stdout}"
    );
    assert_eq!(
        parsed["last_seq"], 3,
        "the cursor must hold; got:\n{stdout}"
    );
    assert_eq!(parsed["wave"]["stalled"], true, "got:\n{stdout}");
}

/// Spec scenario "events without a session degrades": an unreachable broker
/// yields a successful, flagged, empty answer rather than an error — an
/// orchestrator polling a repository with no live session keeps working.
#[test]
fn events_without_a_session_degrades() {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());
    install_sweep(tmp.path());
    // A port deliberately left with nothing on it, rather than the default,
    // so the test can never reach a real broker on the developer's machine.
    write_broker_config(tmp.path(), free_port());

    let (stdout, ok) = run_events(tmp.path(), &["0"]);
    assert!(ok, "a missing broker must still exit 0; got:\n{stdout}");

    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("{e}; stdout was:\n{stdout}"));
    assert_eq!(parsed["no_session"], true, "got:\n{stdout}");
    assert!(
        parsed["entries"].as_array().expect("entries").is_empty(),
        "got:\n{stdout}"
    );
}

/// Spec scenario "events reports the broker epoch": the caller needs it to
/// notice a restarted broker, whose sequence numbers begin again at zero and
/// against which its cursor would otherwise skip everything.
#[test]
fn events_reports_the_broker_epoch() {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());
    install_sweep(tmp.path());
    let Some(handle) = broker_for(tmp.path()) else {
        return;
    };
    publish_message(&handle.state, &status_msg("feat-a"));

    let (stdout, ok) = run_events(tmp.path(), &["0", "1"]);
    assert!(ok, "events must exit 0; got:\n{stdout}");

    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("{e}; stdout was:\n{stdout}"));
    assert_eq!(
        parsed["epoch"].as_u64(),
        Some(handle.state.epoch()),
        "the helper must pass the serving broker's epoch through; got:\n{stdout}"
    );
}

/// The verb is read-only: its implementation sends no keystroke and makes no
/// publish. Asserted against the bundled source, so a later edit that adds
/// either has to face this test.
#[test]
fn the_events_verb_sends_no_keystroke_and_publishes_nothing() {
    let source =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts/sweep.sh"))
            .expect("read bundled sweep.sh");

    let body = source
        .split_once("cmd_events() {")
        .expect("cmd_events must exist")
        .1
        .split_once("\ncmd_")
        .expect("cmd_events must be followed by another subcommand")
        .0;

    for forbidden in ["send-keys", "publish", "POST", "paw_publish"] {
        assert!(
            !body.contains(forbidden),
            "cmd_events must not reference `{forbidden}` — it is a read-only verb"
        );
    }
}
