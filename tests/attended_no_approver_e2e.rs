//! E2E observable: an ATTENDED session runs no auto-approver.
//!
//! Spec scenario "An attended session runs no auto-approver" (`approval-auto`,
//! requirement *Approval keystrokes are sent only by an explicit approve
//! action*).
//!
//! Before this change, a broker-enabled supervisor session spawned a poll
//! thread inside the `__dashboard` subprocess that swept stalled panes on the
//! `[supervisor.auto_approve] stall_threshold_seconds` cadence and typed
//! approvals into them. That thread is deleted. This test proves it by the
//! only honest means available for a deleted background worker: declare the
//! configuration that used to switch it on, leave a pane sitting on a live
//! permission prompt for several of its former sweep periods, and show that
//! nothing was ever typed into that pane.
//!
//! The agent dummy appends every byte reaching its stdin to a marker file, so
//! "no keystroke" is observed directly rather than inferred. A dummy that
//! merely ignored stdin could not tell "nothing was sent" from "sent and
//! discarded".
//!
//! The session is started WITHOUT `--unattended`, so no drive loop runs
//! either — this is the attended path specifically.

use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

/// The former `stall_threshold_seconds`, i.e. how often the deleted thread
/// would have swept.
const STALL_SECONDS: u64 = 5;

/// How many of those former sweep periods to wait through. More than three,
/// per the task, so a thread that fired even once would be caught.
const PERIODS_OBSERVED: u64 = 4;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn tmux_available() -> bool {
    StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn pick_broker_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read local addr")
        .port()
}

/// Writes a supervisor config that DECLARES the deprecated auto-approval
/// table with the settings that used to start the dashboard thread.
fn write_config(repo: &Path, port: u16, agent_cli: &str) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = format!(
        r#"
default_cli = "{agent_cli}"

[broker]
enabled = true
port = {port}

[supervisor]
enabled = true
cli = "cat"

[supervisor.auto_approve]
enabled = true
stall_threshold_seconds = {STALL_SECONDS}
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
}

/// Writes a dummy CLI that reprints a live permission prompt forever AND
/// records every byte arriving on its stdin to `stdin_marker`.
fn write_prompt_recorder(dir: &Path, name: &str, stdin_marker: &Path) -> std::path::PathBuf {
    let path = dir.join(name);
    let marker = stdin_marker.display();
    let script = format!(
        "#!/usr/bin/env bash\n\
         : > '{marker}'\n\
         ( while IFS= read -r -n1 ch; do printf '%s' \"$ch\" >> '{marker}'; done ) &\n\
         while true; do\n\
         \x20 printf 'Bash command\\n  ls -la\\nDo you want to proceed?\\n> 1. Yes\\n  2. No\\n(esc to cancel)\\n'\n\
         \x20 sleep 0.4\n\
         done\n"
    );
    fs::write(&path, script).expect("write recorder dummy");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).expect("stat dummy").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("chmod dummy");
    }
    path
}

/// Reads the session name out of `start`'s "tmux attach -t <name>" hint.
fn extract_session_name(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
        .map(str::to_string)
}

fn kill_session(name: &str, tmpdir: &Path) {
    let _ = StdCommand::new("tmux")
        .env("TMUX_TMPDIR", tmpdir)
        .args(["kill-session", "-t", name])
        .status();
}

#[test]
#[serial]
fn attended_session_sends_no_approval_keystroke() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();
    let home = TempDir::new().expect("home tempdir");
    let out_dir = TempDir::new().expect("out dir");
    let port = pick_broker_port();

    let stdin_marker = out_dir.path().join("agent-stdin.bin");
    let dummy = write_prompt_recorder(tr.path(), "dummy_recorder.sh", &stdin_marker);
    write_config(tr.path(), port, &dummy.to_string_lossy());

    // Attended: NO --unattended, so no drive loop either. The only thing that
    // could ever type here is the deleted dashboard thread.
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--supervisor", "--branches", "a"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run start");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "start failed; stdout:\n{stdout}");
    let session_name = extract_session_name(&stdout).expect("session name in stdout");

    // Sit through several of the former sweep periods with a live prompt up.
    std::thread::sleep(Duration::from_secs(STALL_SECONDS * PERIODS_OBSERVED));

    let marker_exists = stdin_marker.exists();
    let recorded = fs::read(&stdin_marker).unwrap_or_default();
    kill_session(&session_name, tmux_env.socket_dir());

    // Prove the probe ran before trusting its silence. The dummy truncates
    // the marker at startup, so the file existing is what distinguishes
    // "nothing was sent" from "the recorder never started" — without this the
    // test passes vacuously whenever the agent pane fails to launch.
    assert!(
        marker_exists,
        "the recording dummy must have started; {} was never created, so this \
         test would prove nothing",
        stdin_marker.display()
    );
    assert!(
        recorded.is_empty(),
        "an attended session must run no auto-approver: after {}s on a live \
         prompt the agent pane's stdin recorded {:?}",
        STALL_SECONDS * PERIODS_OBSERVED,
        String::from_utf8_lossy(&recorded)
    );
}
