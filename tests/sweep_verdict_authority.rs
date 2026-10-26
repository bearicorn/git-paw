//! `broker-agent-helper` "Root-side helper verbs carry the authority
//! credential", against a live broker: `sweep.sh verified`, `conflict-ack`
//! and `feedback-gate` fetch the credential through `git paw __authority`
//! and send it as `X-Paw-Authority`.
//!
//! The broker is started holding a known credential; the session file under a
//! temp `HOME` carries the credential `git paw __authority` will print, and the
//! built `git-paw` binary is put first on `PATH` so `git paw` resolves to it.
//! A request is accepted (202) only when its header equals the broker's
//! credential, so an accepted publish proves the header carried it.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::SystemTime;

use git_paw::broker::{BrokerHandle, BrokerState, start_broker};
use git_paw::config::BrokerConfig;
use git_paw::session::{Session, SessionMode, SessionStatus, save_session_in};
use tempfile::TempDir;

const CREDENTIAL: &str = "5ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn shell_tools_available() -> bool {
    ["bash", "curl", "git", "python3"]
        .iter()
        .all(|tool| which::which(tool).is_ok())
}

/// Starts a loopback broker holding [`CREDENTIAL`] on a free port.
fn spawn_broker() -> (BrokerHandle, String, u16) {
    let mut attempts = 0;
    loop {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("bind ephemeral port")
            .local_addr()
            .expect("local addr")
            .port();
        let config = BrokerConfig {
            enabled: true,
            port,
            bind: "127.0.0.1".to_string(),
            ..Default::default()
        };
        let state = BrokerState::new(None).with_authority(Some(CREDENTIAL.to_string()));
        match start_broker(&config, state, Vec::new()) {
            Ok(handle) => return (handle, format!("http://127.0.0.1:{port}"), port),
            Err(_) if attempts < 10 => attempts += 1,
            Err(e) => panic!("failed to start test broker: {e}"),
        }
    }
}

/// A throwaway repository wired to the broker on `port`, with the bundled
/// helpers under `.git-paw/scripts/`. Returns its canonical root.
fn fixture_repo(dir: &Path, port: u16) -> PathBuf {
    let run = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?} failed");
    };
    run(&["init", "-q", "--initial-branch=main"]);
    run(&["config", "user.email", "fixture@example.invalid"]);
    run(&["config", "user.name", "fixture"]);

    let paw = dir.join(".git-paw");
    fs::create_dir_all(paw.join("scripts")).expect("create .git-paw/scripts");
    fs::write(
        paw.join("config.toml"),
        format!("[broker]\nenabled = true\nport = {port}\nbind = \"127.0.0.1\"\n"),
    )
    .expect("write config.toml");
    for script in ["_paw_common.sh", "broker.sh", "sweep.sh"] {
        let dst = paw.join("scripts").join(script);
        fs::copy(repo_root().join("assets/scripts").join(script), &dst)
            .unwrap_or_else(|e| panic!("copy {script}: {e}"));
        fs::set_permissions(&dst, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    fs::write(dir.join("seed.txt"), "seed\n").expect("write seed");
    run(&["add", "seed.txt"]);
    run(&["commit", "-q", "-m", "seed"]);
    git_paw::git::validate_repo(dir).expect("validate repo")
}

/// Saves the repository's session, carrying `credential`, under `home`.
fn save_session(home: &Path, repo: &Path, port: u16, credential: Option<&str>) {
    let dir = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-paw/sessions")
    } else {
        home.join(".local/share/git-paw/sessions")
    };
    let session = Session {
        session_name: "paw-sweep-authority".to_string(),
        repo_path: repo.to_path_buf(),
        project_name: "sweep-authority".to_string(),
        created_at: SystemTime::UNIX_EPOCH,
        status: SessionStatus::Active,
        worktrees: Vec::new(),
        broker_port: Some(port),
        broker_bind: Some("127.0.0.1".to_string()),
        broker_log_path: None,
        mode: SessionMode::Bare,
        dashboard_pane: None,
        layout: None,
        broker_authority: credential.map(str::to_string),
    };
    save_session_in(&session, &dir).expect("save session");
}

/// Runs `sweep.sh args` at `repo` with `HOME` at `home` and the built binary
/// first on `PATH`.
fn sweep(repo: &Path, home: &Path, args: &[&str]) -> Output {
    let bin_dir = Path::new(env!("CARGO_BIN_EXE_git-paw"))
        .parent()
        .expect("binary dir")
        .to_path_buf();
    let path = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    Command::new("bash")
        .arg(".git-paw/scripts/sweep.sh")
        .args(args)
        .current_dir(repo)
        .env("HOME", home)
        .env_remove("XDG_DATA_HOME")
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .env("PATH", path)
        .output()
        .expect("run sweep.sh")
}

/// POSTs an `agent.status` for `agent` so it has an inbox.
fn register(url: &str, agent: &str) {
    let body = format!(
        r#"{{"type":"agent.status","agent_id":"{agent}","payload":{{"status":"working","modified_files":[]}}}}"#
    );
    let out = Command::new("curl")
        .args(["-sS", "-o", "/dev/null", "-w", "%{http_code}", "-X", "POST"])
        .arg(format!("{url}/publish"))
        .args(["-H", "Content-Type: application/json", "-d", &body])
        .output()
        .expect("curl");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "202");
}

/// The JSON messages in `agent`'s inbox.
fn inbox(url: &str, agent: &str) -> Vec<serde_json::Value> {
    let out = Command::new("curl")
        .args(["-sS", &format!("{url}/messages/{agent}?since=0")])
        .output()
        .expect("curl");
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("messages response is JSON");
    json["messages"].as_array().cloned().unwrap_or_default()
}

fn combined(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Scenario: sweep.sh verified sends the header — accepted, delivered, and
/// the credential is never printed.
#[test]
fn sweep_verified_sends_the_authority_header() {
    if !shell_tools_available() {
        eprintln!("skipping: bash/curl/git/python3 not all on PATH");
        return;
    }
    let (_handle, url, port) = spawn_broker();
    let tmp = TempDir::new().expect("repo");
    let home = TempDir::new().expect("home");
    let repo = fixture_repo(tmp.path(), port);
    save_session(home.path(), &repo, port, Some(CREDENTIAL));
    register(&url, "feat-xx");

    let out = sweep(
        &repo,
        home.path(),
        &["verified", "feat-xx", "all gates green"],
    );
    assert!(out.status.success(), "{}", combined(&out));
    assert!(!combined(&out).contains(CREDENTIAL), "credential leaked");
    assert!(
        inbox(&url, "feat-xx")
            .iter()
            .any(|m| m["type"] == "agent.verified"),
        "the verified message must be delivered"
    );
}

/// Scenario: sweep.sh conflict-ack sends the header — both acknowledgements
/// accepted and delivered.
#[test]
fn sweep_conflict_ack_sends_the_authority_header() {
    if !shell_tools_available() {
        eprintln!("skipping: bash/curl/git/python3 not all on PATH");
        return;
    }
    let (_handle, url, port) = spawn_broker();
    let tmp = TempDir::new().expect("repo");
    let home = TempDir::new().expect("home");
    let repo = fixture_repo(tmp.path(), port);
    save_session(home.path(), &repo, port, Some(CREDENTIAL));
    register(&url, "feat-xx");
    register(&url, "feat-yy");

    let out = sweep(
        &repo,
        home.path(),
        &["conflict-ack", "feat-xx", "feat-yy", "src/lib.rs"],
    );
    assert!(out.status.success(), "{}", combined(&out));
    for agent in ["feat-xx", "feat-yy"] {
        assert!(
            inbox(&url, agent)
                .iter()
                .any(|m| m["type"] == "agent.feedback"
                    && m["payload"]["errors"][0]
                        .as_str()
                        .is_some_and(|e| e.starts_with("[conflict-ack]"))),
            "{agent} must receive the acknowledgement"
        );
    }
}

/// `sweep.sh feedback-gate` also carries the header.
#[test]
fn sweep_feedback_gate_sends_the_authority_header() {
    if !shell_tools_available() {
        eprintln!("skipping: bash/curl/git/python3 not all on PATH");
        return;
    }
    let (_handle, url, port) = spawn_broker();
    let tmp = TempDir::new().expect("repo");
    let home = TempDir::new().expect("home");
    let repo = fixture_repo(tmp.path(), port);
    save_session(home.path(), &repo, port, Some(CREDENTIAL));
    register(&url, "feat-xx");

    let out = sweep(
        &repo,
        home.path(),
        &["feedback-gate", "feat-xx", "testing", "two failures"],
    );
    assert!(out.status.success(), "{}", combined(&out));
    assert!(
        inbox(&url, "feat-xx")
            .iter()
            .any(|m| m["type"] == "agent.feedback")
    );
}

/// A session credential that is not the broker's is refused: the verb exits
/// non-zero with the broker's error and nothing is delivered.
#[test]
fn sweep_verified_with_a_wrong_credential_is_refused() {
    if !shell_tools_available() {
        eprintln!("skipping: bash/curl/git/python3 not all on PATH");
        return;
    }
    let (_handle, url, port) = spawn_broker();
    let tmp = TempDir::new().expect("repo");
    let home = TempDir::new().expect("home");
    let repo = fixture_repo(tmp.path(), port);
    save_session(home.path(), &repo, port, Some("not-the-credential"));
    register(&url, "feat-xx");

    let out = sweep(&repo, home.path(), &["verified", "feat-xx", "nope"]);
    assert_eq!(out.status.code(), Some(1), "{}", combined(&out));
    assert!(combined(&out).contains("authority required"));
    assert!(
        !inbox(&url, "feat-xx")
            .iter()
            .any(|m| m["type"] == "agent.verified")
    );
}

/// Without a session credential `git paw __authority` fails, and the verb
/// exits non-zero without publishing.
#[test]
fn sweep_verified_without_a_credential_publishes_nothing() {
    if !shell_tools_available() {
        eprintln!("skipping: bash/curl/git/python3 not all on PATH");
        return;
    }
    let (_handle, url, port) = spawn_broker();
    let tmp = TempDir::new().expect("repo");
    let home = TempDir::new().expect("home");
    let repo = fixture_repo(tmp.path(), port);
    register(&url, "feat-xx");

    let out = sweep(&repo, home.path(), &["verified", "feat-xx", "nope"]);
    assert_eq!(out.status.code(), Some(1), "{}", combined(&out));
    assert!(combined(&out).contains("git paw start"));
    assert!(
        !inbox(&url, "feat-xx")
            .iter()
            .any(|m| m["type"] == "agent.verified")
    );
}
