//! Real-tmux smoke tests for the terminal port's service operations
//! (`code-architecture`, "Terminal backend hosts detached service surfaces").
//!
//! The argv the tmux adapter emits is pinned by golden fixtures in
//! `src/terminal/tmux_tests.rs`, and the fake's recording by its own unit
//! test; these tests show what only a live server can: the service is a
//! detached window outside window 0's pane grid, and stopping it hangs up its
//! process. Every test runs on a private tmux socket (`TmuxTestEnv`), never
//! the user's default server.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use git_paw::terminal::TerminalBackend;
use git_paw::terminal::tmux::TmuxBackend;
use serial_test::serial;

mod helpers;
use helpers::TmuxTestEnv;

const SESSION: &str = "paw-service-surface";

/// Runs `tmux args` against the test's private socket (the process env set by
/// `apply_to_process`), returning stdout.
fn tmux(args: &[&str]) -> String {
    let out = Command::new("tmux").args(args).output().expect("run tmux");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Starts a session with a two-pane window 0, ignoring the user's tmux config
/// so window numbering starts at 0 as git-paw's layout assumes.
fn start_session(dir: &Path) {
    let dir = dir.to_string_lossy();
    let status = Command::new("tmux")
        .args([
            "-f",
            "/dev/null",
            "new-session",
            "-d",
            "-s",
            SESSION,
            "-c",
            &dir,
        ])
        .args(["-x", "200", "-y", "50"])
        .status()
        .expect("start tmux");
    assert!(status.success(), "private tmux session must start");
    let status = Command::new("tmux")
        .args([
            "split-window",
            "-h",
            "-t",
            &format!("{SESSION}:0"),
            "-c",
            &dir,
        ])
        .status()
        .expect("split window 0");
    assert!(status.success());
}

/// Window 0's pane indices, in order.
fn window_zero_panes() -> String {
    tmux(&[
        "list-panes",
        "-t",
        &format!("{SESSION}:0"),
        "-F",
        "#{pane_index}",
    ])
}

fn window_names() -> Vec<String> {
    tmux(&["list-windows", "-t", SESSION, "-F", "#{window_name}"])
        .lines()
        .map(str::to_string)
        .collect()
}

fn wait_for(budget: Duration, mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    done()
}

/// Scenario "tmux service is a detached window".
#[test]
#[serial]
fn tmux_service_is_a_detached_window() {
    let env = TmuxTestEnv::new();
    let _guard = env.apply_to_process();
    let dir = tempfile::tempdir().unwrap();
    start_session(dir.path());
    let panes_before = window_zero_panes();
    assert_eq!(panes_before.lines().count(), 2);

    let backend = TmuxBackend::new();
    let handle = backend
        .spawn_service(SESSION, "broker", "sleep 300", dir.path())
        .expect("spawn service");

    assert!(
        window_names().iter().any(|n| n == "broker"),
        "a window named `broker` must exist; got {:?}",
        window_names()
    );
    let start_command = tmux(&[
        "list-panes",
        "-t",
        &format!("{SESSION}:broker"),
        "-F",
        "#{pane_start_command}",
    ]);
    assert!(
        start_command.contains("sleep 300"),
        "the broker window must run the service command; got {start_command:?}"
    );
    assert_eq!(
        window_zero_panes(),
        panes_before,
        "window 0's pane count and indices must be unchanged"
    );
    assert!(backend.service_alive(&handle).unwrap());
    assert!(
        backend
            .service_alive(&backend.service(SESSION, "broker"))
            .unwrap(),
        "a handle rebuilt by name must see the same service"
    );
}

/// Scenario "Stopping a service closes its window".
#[test]
#[serial]
fn stopping_a_service_closes_its_window() {
    let env = TmuxTestEnv::new();
    let _guard = env.apply_to_process();
    let dir = tempfile::tempdir().unwrap();
    start_session(dir.path());
    let marker = dir.path().join("hup-received");

    let command = format!(
        "sh -c 'trap \"echo hup > {}; exit 0\" HUP; while :; do sleep 0.1; done'",
        marker.display()
    );
    let backend = TmuxBackend::new();
    let handle = backend
        .spawn_service(SESSION, "broker", &command, dir.path())
        .expect("spawn service");
    assert!(backend.service_alive(&handle).unwrap());
    // Let the shell install its trap before the hang-up arrives.
    std::thread::sleep(Duration::from_millis(300));

    backend.stop_service(&handle).expect("stop service");

    assert!(
        !window_names().iter().any(|n| n == "broker"),
        "the `broker` window must be gone; got {:?}",
        window_names()
    );
    assert!(!backend.service_alive(&handle).unwrap());
    assert!(
        wait_for(Duration::from_secs(5), || marker.exists()),
        "the service process must have received SIGHUP"
    );
    // Stopping an already-stopped service is not an error.
    backend
        .stop_service(&handle)
        .expect("a service already gone stops cleanly");
}

/// A session that no longer exists has no running service, and stopping its
/// service is not an error.
#[test]
#[serial]
fn a_gone_session_has_no_running_service() {
    let env = TmuxTestEnv::new();
    let _guard = env.apply_to_process();
    let backend = TmuxBackend::new();
    let handle = backend.service("paw-no-such-session", "broker");
    assert!(!backend.service_alive(&handle).unwrap());
    backend
        .stop_service(&handle)
        .expect("stopping a service of a gone session is not an error");
}
