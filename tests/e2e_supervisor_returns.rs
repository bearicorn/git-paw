//! E2E observable: `cmd_supervisor` returns immediately with an attach hint.
//!
//! Maps to scenario `cmd_supervisor returns immediately with attach hint`
//! from supervisor-as-pane. The supervisor launch must:
//!
//! - Exit with status 0 inside a 10-second window.
//! - Print `Supervisor session 'paw-...'` to stdout.
//! - Print the `tmux attach -t <session>` hint to stdout.
//!
//! Uses the test helper's tmux socket isolation so the live tmux session
//! the launch creates lives on a test-owned socket and does not pollute
//! the user's default tmux server.
//!
//! (test-coverage-v0-5-0 task 12.8)

use std::fs;
use std::process::Command as StdCommand;
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn tmux_available() -> bool {
    StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

#[test]
#[serial]
fn cmd_supervisor_returns_immediately_with_attach_hint() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    // Supervisor config with `echo` as the CLI so the spawned pane commands
    // do not require a real coding agent. Broker disabled to keep the
    // launch fast; the spec scenario is about *return-with-hint*, not about
    // broker wiring.
    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = r#"
default_cli = "echo"

[supervisor]
enabled = true
cli = "echo"
"#;
    fs::write(paw_dir.join("config.toml"), config).expect("write config");

    let mut command = cmd();
    tmux_env.apply_assert(&mut command);
    command
        .current_dir(tr.path())
        .args(["start", "--supervisor", "--branches", "a,b"])
        .timeout(Duration::from_secs(10));

    let output = command.output().expect("run start --supervisor");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert!(
        output.status.success(),
        "supervisor launch should exit 0; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("Supervisor session 'paw-"),
        "stdout should announce the supervisor session name; got:\n{stdout}"
    );
    assert!(
        stdout.contains("tmux attach -t"),
        "stdout should include the tmux attach hint; got:\n{stdout}"
    );

    // Clean up the launched tmux session inside the test-owned socket.
    let line = stdout
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .unwrap_or_default();
    if let Some(session) = line.split_whitespace().last() {
        let _ = StdCommand::new("tmux")
            .env("TMUX_TMPDIR", tmux_env.socket_dir())
            .args(["kill-session", "-t", session])
            .status();
    }
}
