//! Asserts that `git paw start --supervisor` falls back to a synthesized
//! `SupervisorConfig::default()` when the repo's `.git-paw/config.toml` has
//! no `[supervisor]` section. Maps to scenarios under
//! `supervisor-bugfixes-v0-5-x` / `supervisor-launch` / "Interactive prompt
//! yes accepts default supervisor config" and the no-config + no-default-cli
//! error scenario (tasks 1.5 and 1.6).

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

fn write_config(repo: &std::path::Path, contents: &str) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(paw_dir.join("config.toml"), contents).expect("write config");
}

#[test]
#[serial]
fn supervisor_without_section_uses_default_when_default_cli_present() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    // No `[supervisor]` block on purpose — only top-level `default_cli`.
    write_config(tr.path(), "default_cli = \"echo\"\n");

    let mut command = cmd();
    tmux_env.apply_assert(&mut command);
    let output = command
        .current_dir(tr.path())
        .args(["start", "--supervisor", "--branches", "a,b"])
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run start --supervisor --branches a,b");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert!(
        output.status.success(),
        "supervisor launch without [supervisor] should succeed using default_cli; \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("Supervisor session 'paw-"),
        "stdout should announce the supervisor session name; got:\n{stdout}"
    );
    assert!(
        !stderr.contains("supervisor mode enabled but [supervisor] config missing"),
        "stderr must NOT mention the legacy hard-error string; got:\n{stderr}"
    );

    // Clean up the launched session if we can identify it.
    if let Some(line) = stdout.lines().find(|l| l.contains("tmux attach -t"))
        && let Some(session) = line.split_whitespace().last()
    {
        let _ = StdCommand::new("tmux")
            .env("TMUX_TMPDIR", tmux_env.socket_dir())
            .args(["kill-session", "-t", session])
            .status();
    }
}

#[test]
#[serial]
fn supervisor_without_section_or_default_cli_still_errors() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    // Neither `default_cli` nor `[supervisor]` — the existing CLI-resolution
    // error path SHALL still fire.
    write_config(tr.path(), "# no default_cli, no supervisor section\n");

    let mut command = cmd();
    tmux_env.apply_assert(&mut command);
    let output = command
        .current_dir(tr.path())
        .args(["start", "--supervisor", "--branches", "a,b"])
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run start --supervisor --branches a,b");

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert!(
        !output.status.success(),
        "supervisor launch without CLI resolution should fail"
    );
    assert!(
        stderr.contains("requires either [supervisor].cli or default_cli"),
        "stderr should explain the missing-CLI condition; got:\n{stderr}"
    );
}
