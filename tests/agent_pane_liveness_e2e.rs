//! Behavioral coverage for GP-03c / DS-9: `git paw status` and
//! `git paw doctor` report a session whose agent panes have all reverted to
//! bare shells as stopped — not active, and not stale — even though the tmux
//! session object itself still exists (the full-auto boot-exit failure mode,
//! where every pane's CLI process exits within seconds of launch and the pane
//! reverts to its underlying interactive shell rather than closing).
//!
//! `🔴 stale` is reserved for an absent tmux session: here tmux IS running, so
//! status reports `Tmux: running` / `"tmux_running": true`, doctor's remedy is
//! `git paw start` rather than `purge --stale`, and `purge --stale` leaves the
//! receipt alone.
//!
//! Unlike `session_stale_hygiene.rs` (which uses session names guaranteed
//! absent from tmux), these tests create a REAL tmux session, so each one runs
//! on its own `TmuxTestEnv` socket rather than the user's default socket.

mod helpers;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::{Duration, SystemTime};

use assert_cmd::Command;
use git_paw::session::{Session, SessionMode, SessionStatus};
use tempfile::TempDir;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// The sessions directory the binary resolves for a given fake `HOME`,
/// mirroring `git_paw::dirs::data_dir()`.
fn sessions_dir_for_home(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-paw/sessions")
    } else {
        home.join(".local/share/git-paw/sessions")
    }
}

fn canon(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

fn init_git_repo(dir: &Path) {
    let run = |args: &[&str]| {
        StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git command");
    };
    run(&["init", "-q"]);
    run(&["config", "user.email", "t@e.st"]);
    run(&["config", "user.name", "Test"]);
    fs::write(dir.join("README.md"), "x").expect("write readme");
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "init"]);
}

/// Polls `condition` until it holds or `timeout` elapses. Returns whether it
/// held — the tests assert on that rather than on a fixed sleep.
fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    condition()
}

fn tmux_available() -> bool {
    StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// A receipt for a `Bare`-mode, broker-disabled session — `agent_pane_offset`
/// resolves to `0`, so every pane in the window (including pane 0, tmux's
/// default interactive shell) is a coding-agent pane for the liveness probe.
/// `created_at` is set well past the launch grace period so the probe's
/// agent-pane check is actually exercised rather than the launch-window
/// tolerance.
fn dead_panes_receipt(session_name: &str, repo_path: &Path) -> Session {
    Session {
        session_name: session_name.to_string(),
        repo_path: repo_path.to_path_buf(),
        project_name: "gp03c".to_string(),
        created_at: SystemTime::now() - Duration::from_secs(120),
        status: SessionStatus::Active,
        worktrees: vec![],
        broker_port: None,
        broker_bind: None,
        broker_log_path: None,
        mode: SessionMode::Bare,
        dashboard_pane: None,
        layout: None,
        broker_authority: None,
    }
}

/// Everything a dead-panes scenario needs: a live tmux session whose only pane
/// runs the default interactive shell (never a coding-agent CLI), a git repo,
/// and a fake `HOME` holding an `active` receipt pointing at that session.
///
/// The tmux session lives on a per-test socket ([`helpers::TmuxTestEnv`]), so
/// the fixture never touches the user's default socket and the tests can run
/// in parallel.
struct DeadPanes {
    tmux_env: helpers::TmuxTestEnv,
    home: TempDir,
    _repo: TempDir,
    repo_path: PathBuf,
    sessions_dir: PathBuf,
    session_name: String,
}

impl DeadPanes {
    fn set_up(session_name: &str) -> Self {
        let tmux_env = helpers::TmuxTestEnv::new();
        let mut new_session = StdCommand::new("tmux");
        tmux_env.apply(&mut new_session);
        let created = new_session
            .args(["new-session", "-d", "-s", session_name])
            .status()
            .expect("tmux new-session");
        assert!(created.success(), "failed to create the test tmux session");

        let home = TempDir::new().expect("home");
        let repo = TempDir::new().expect("repo");
        init_git_repo(repo.path());
        let repo_path = canon(repo.path());

        let sessions_dir = sessions_dir_for_home(home.path());
        fs::create_dir_all(&sessions_dir).expect("create sessions dir");
        git_paw::session::save_session_in(
            &dead_panes_receipt(session_name, &repo_path),
            &sessions_dir,
        )
        .expect("save receipt");

        Self {
            tmux_env,
            home,
            _repo: repo,
            repo_path,
            sessions_dir,
            session_name: session_name.to_string(),
        }
    }

    /// Runs `git paw <args>` in the repo against the fake `HOME` and this
    /// fixture's tmux socket.
    fn run(&self, args: &[&str]) -> std::process::Output {
        let mut command = cmd();
        self.tmux_env.apply_assert(&mut command);
        command
            .current_dir(&self.repo_path)
            .env("HOME", self.home.path())
            .env_remove("XDG_DATA_HOME")
            .args(args)
            .output()
            .expect("git-paw command")
    }
}

/// Spec scenarios "A session whose agent panes all exited reports Stopped",
/// "Status shows exited agent panes as stopped with tmux running" and "JSON
/// status for exited agent panes reports stopped with tmux running".
#[test]
fn status_reports_a_session_whose_panes_reverted_to_shells_as_stopped_with_tmux_running() {
    helpers::guard_against_live_session();
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let fixture = DeadPanes::set_up("paw-ds9-status-e2e-test");

    let json_out = fixture.run(&["status", "--json"]);
    let stdout = String::from_utf8_lossy(&json_out.stdout);
    assert!(
        stdout.contains("\"status\":\"stopped\"") || stdout.contains("\"status\": \"stopped\""),
        "a session with no live agent-pane CLI must report stopped; got:\n{stdout}"
    );
    assert!(
        stdout.contains("\"tmux_running\":true") || stdout.contains("\"tmux_running\": true"),
        "its tmux session is still running, so tmux_running must be true; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("stale"),
        "stale is reserved for an absent tmux session; got:\n{stdout}"
    );

    let human_out = fixture.run(&["status"]);
    let human = String::from_utf8_lossy(&human_out.stdout);
    assert!(
        human.contains("\u{1f7e1} stopped"),
        "human status must show the stopped icon; got:\n{human}"
    );
    assert!(
        human.contains("agent panes exited"),
        "human status must hint that the agent panes exited; got:\n{human}"
    );
    assert!(
        human.contains("git paw start"),
        "the hint must name 'git paw start' as the remedy; got:\n{human}"
    );
    assert!(
        human.contains("Tmux:    running"),
        "the tmux session is still running; got:\n{human}"
    );
    assert!(
        !human.contains("no longer exists"),
        "status must not claim the tmux session is gone; got:\n{human}"
    );
}

/// Spec scenario "purge --stale leaves a session whose agent panes exited".
#[test]
fn purge_stale_leaves_a_session_whose_agent_panes_exited() {
    helpers::guard_against_live_session();
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let fixture = DeadPanes::set_up("paw-ds9-purge-e2e-test");
    let receipts_before: Vec<_> = fs::read_dir(&fixture.sessions_dir)
        .expect("read sessions dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    assert!(
        !receipts_before.is_empty(),
        "the fixture must have written a receipt"
    );

    let out = fixture.run(&["purge", "--stale"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "purge --stale must exit 0; got:\n{stdout}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("No stale sessions"),
        "purge --stale must report nothing to purge; got:\n{stdout}"
    );
    for receipt in receipts_before {
        assert!(
            receipt.exists(),
            "purge --stale must leave {} in place",
            receipt.display()
        );
    }
}

/// Builds the pane layout a session has after `git paw pause`: a dashboard
/// pane and an agent pane running a live (non-shell) process, then the
/// dashboard killed — which is all pause does to the panes. tmux renumbers the
/// survivors, so the agent ends up at the dashboard's old index.
fn set_up_paused_layout(tmux_env: &helpers::TmuxTestEnv, session_name: &str) {
    let tmux = |args: &[&str]| {
        let mut command = StdCommand::new("tmux");
        tmux_env.apply(&mut command);
        command.args(args).output().expect("tmux command")
    };

    assert!(
        tmux(&["new-session", "-d", "-s", session_name])
            .status
            .success(),
        "failed to create the test tmux session"
    );
    tmux(&["split-window", "-t", session_name]);
    for (pane, role) in [("0", "dashboard"), ("1", "feat/auth")] {
        tmux(&[
            "set-option",
            "-p",
            "-t",
            &format!("{session_name}:0.{pane}"),
            "@paw_role",
            role,
        ]);
    }
    // A long-running non-shell foreground process stands in for the agent CLI.
    tmux(&[
        "send-keys",
        "-t",
        &format!("{session_name}:0.1"),
        "sleep 600",
        "Enter",
    ]);
    let agent_started = wait_until(Duration::from_secs(10), || {
        let out = tmux(&[
            "list-panes",
            "-t",
            session_name,
            "-F",
            "#{pane_current_command}",
        ]);
        String::from_utf8_lossy(&out.stdout).contains("sleep")
    });
    assert!(agent_started, "the stand-in agent process never started");

    tmux(&["kill-pane", "-t", &format!("{session_name}:0.0")]);
}

/// A paused session keeps its agent panes: `git paw pause` kills only the
/// dashboard pane, and tmux renumbers the survivors, so the live agent lands
/// at the dashboard's old index. Neither status nor doctor may report that
/// its agent panes exited — `git paw start` would then recover (tear down and
/// relaunch) a session whose agent is still running.
#[test]
fn a_paused_session_with_a_live_agent_is_not_reported_as_exited() {
    helpers::guard_against_live_session();
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let tmux_env = helpers::TmuxTestEnv::new();
    let session_name = "paw-ds9-paused-e2e-test";
    set_up_paused_layout(&tmux_env, session_name);

    let home = TempDir::new().expect("home");
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let repo_path = canon(repo.path());
    let sessions_dir = sessions_dir_for_home(home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");
    let mut receipt = dead_panes_receipt(session_name, &repo_path);
    receipt.status = SessionStatus::Paused;
    // A broker-enabled bare session: `agent_pane_offset` is 1, the index the
    // dashboard occupied before pause killed it.
    receipt.broker_port = Some(9_317);
    receipt.broker_bind = Some("127.0.0.1".to_string());
    receipt.dashboard_pane = Some(0);
    git_paw::session::save_session_in(&receipt, &sessions_dir).expect("save receipt");

    let run = |args: &[&str]| {
        let mut command = cmd();
        tmux_env.apply_assert(&mut command);
        command
            .current_dir(&repo_path)
            .env("HOME", home.path())
            .env_remove("XDG_DATA_HOME")
            .args(args)
            .output()
            .expect("git-paw command")
    };

    let human = String::from_utf8_lossy(&run(&["status"]).stdout).to_string();
    assert!(
        !human.contains("agent panes exited"),
        "a paused session whose agent is still running must not report exited panes; got:\n{human}"
    );
    assert!(
        human.contains("\u{1f535} paused"),
        "it is still paused, with its agent alive; got:\n{human}"
    );

    let doctor_stdout = String::from_utf8_lossy(&run(&["doctor", "--json"]).stdout).to_string();
    let document: serde_json::Value =
        serde_json::from_str(&doctor_stdout).expect("doctor --json parses");
    let session_state = document["checks"]
        .as_array()
        .expect("checks array present")
        .iter()
        .find(|c| c["name"] == "session state")
        .expect("'session state' check present")
        .clone();
    assert!(
        !session_state["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("agent panes exited"),
        "doctor must not flag a paused session's live agent as exited; got:\n{session_state}"
    );

    let mut teardown = StdCommand::new("tmux");
    tmux_env.apply(&mut teardown);
    let _ = teardown.args(["kill-session", "-t", session_name]).output();
}

/// Spec scenario "Session whose agent panes exited" (preflight-diagnostics).
#[test]
fn doctor_warns_that_the_agent_panes_exited_with_the_start_remedy() {
    helpers::guard_against_live_session();
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let fixture = DeadPanes::set_up("paw-ds9-doctor-e2e-test");

    let doctor_out = fixture.run(&["doctor", "--json"]);
    let doctor_stdout = String::from_utf8_lossy(&doctor_out.stdout);
    let document: serde_json::Value =
        serde_json::from_str(&doctor_stdout).expect("doctor --json parses");
    let checks = document["checks"].as_array().expect("checks array present");
    let session_state = checks
        .iter()
        .find(|c| c["name"] == "session state")
        .expect("'session state' check present");
    assert_eq!(
        session_state["status"], "warn",
        "doctor must flag the dead-paned session, got:\n{doctor_stdout}"
    );
    let detail = session_state["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains(&fixture.session_name),
        "the detail must name the dead-paned session, got:\n{doctor_stdout}"
    );
    assert!(
        detail.contains("agent panes exited"),
        "the detail must say the agent panes exited, got:\n{doctor_stdout}"
    );
    assert!(
        !detail.contains("gone"),
        "the finding must not describe the tmux session as gone, got:\n{doctor_stdout}"
    );
    let remedy = session_state["remedy"].as_str().unwrap_or_default();
    assert!(
        remedy.contains("git paw start"),
        "the remedy must be to run git paw start, got:\n{doctor_stdout}"
    );
    assert!(
        !remedy.contains("purge --stale"),
        "purge --stale does not purge this session, got:\n{doctor_stdout}"
    );
}
