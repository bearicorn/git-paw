//! E2E observable: `cmd_supervisor` launches the supervisor + dashboard +
//! coding agent panes.
//!
//! Maps to scenarios from supervisor-as-pane:
//!
//! - `Supervisor auto-start launches all panes including the supervisor pane`
//!   (task 12.6)
//! - `Supervisor registers itself on startup` (task 12.11, merged in)
//!
//! Skips if tmux is unavailable.

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
fn auto_start_launches_supervisor_and_agent_panes() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    // Broker disabled to keep the test fast. The pane count and indices
    // are independent of broker state — the supervisor layout always
    // emits one pane each for supervisor, dashboard, and N agents.
    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = r#"
default_cli = "echo"

[supervisor]
enabled = true
cli = "echo"
"#;
    fs::write(paw_dir.join("config.toml"), config).expect("write config");

    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(tr.path())
        .args(["start", "--supervisor", "--branches", "a,b"])
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run start");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "supervisor start failed; stdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let session_name = stdout
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
        .expect("session name in stdout")
        .to_string();

    // List panes on the test-owned socket. The expected layout for 2
    // branches is 4 panes: 0 = supervisor, 1 = dashboard, 2 = agent a,
    // 3 = agent b.
    let list = StdCommand::new("tmux")
        .env("TMUX_TMPDIR", tmux_env.socket_dir())
        .args(["list-panes", "-t", &session_name, "-F", "#{pane_index}"])
        .output()
        .expect("tmux list-panes");
    let panes = String::from_utf8_lossy(&list.stdout).to_string();
    let indices: Vec<&str> = panes.lines().collect();

    // Clean up before asserting.
    let _ = StdCommand::new("tmux")
        .env("TMUX_TMPDIR", tmux_env.socket_dir())
        .args(["kill-session", "-t", &session_name])
        .status();

    assert!(
        list.status.success(),
        "tmux list-panes should succeed; panes:\n{panes}"
    );
    assert_eq!(
        indices.len(),
        4,
        "supervisor + dashboard + 2 agents = 4 panes; got: {indices:?}"
    );
    assert!(indices.contains(&"0"), "pane 0 (supervisor) missing");
    assert!(indices.contains(&"1"), "pane 1 (dashboard) missing");
    assert!(indices.contains(&"2"), "pane 2 (agent a) missing");
    assert!(indices.contains(&"3"), "pane 3 (agent b) missing");
}
