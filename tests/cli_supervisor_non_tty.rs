//! Asserts that `git paw start --supervisor --from-specs` with a non-TTY
//! stdin exits cleanly and prints the attach hint.
//!
//! Maps to scenario `Non-TTY --supervisor skips supervisor CLI launch` from
//! from-specs-launch-fixes. (test-coverage-v0-5-0 task 1.2)
//!
//! Behavioural shape: with `Stdio::null()` for stdin (the default for
//! `assert_cmd::Command::output()`), `cmd_supervisor` launches the tmux
//! session detached and returns immediately with the
//! `Supervisor session 'paw-...' launched` + `Attach with: tmux attach -t`
//! hint pair. No interactive supervisor-CLI attach is attempted.

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

fn write_supervisor_specs_config(repo: &std::path::Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = r#"
default_cli = "echo"

[specs]
type = "openspec"
dir = "specs"

[supervisor]
enabled = true
cli = "echo"
"#;
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
}

fn write_committed_spec(repo: &std::path::Path, id: &str, body: &str) {
    let change_dir = repo.join("specs").join(id);
    fs::create_dir_all(&change_dir).expect("create change dir");
    fs::write(change_dir.join("tasks.md"), body).expect("write tasks.md");

    let _ = StdCommand::new("git")
        .current_dir(repo)
        .args(["add", "."])
        .output();
    let _ = StdCommand::new("git")
        .current_dir(repo)
        .args(["commit", "-m", "add spec"])
        .output();
}

#[test]
#[serial]
fn non_tty_supervisor_skips_cli_launch() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    write_supervisor_specs_config(tr.path());
    write_committed_spec(tr.path(), "feature-x", "Implement feature x.");

    let mut command = cmd();
    tmux_env.apply_assert(&mut command);
    // assert_cmd's default `output()` already provides a non-TTY stdin
    // (the parent test process inherits a non-interactive stdin). No
    // explicit `Stdio::null()` configuration is needed.
    let output = command
        .current_dir(tr.path())
        .args(["start", "--supervisor", "--from-specs"])
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run start --supervisor --from-specs");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert!(
        output.status.success(),
        "non-TTY supervisor launch should exit 0; stdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // The non-TTY attach hint pair. `cmd_supervisor` prints these two
    // lines and returns Ok(()) instead of attempting an interactive
    // attach. Together they constitute the "needs interactive terminal"
    // signal the user sees.
    assert!(
        stdout.contains("Supervisor session 'paw-"),
        "stdout should announce the supervisor session name; got:\n{stdout}"
    );
    assert!(
        stdout.contains("tmux attach -t"),
        "stdout should include the tmux attach hint; got:\n{stdout}"
    );
    // No tmux-attach failure noise — the supervisor CLI launch path was
    // skipped, not attempted.
    assert!(
        !stderr.contains("failed to attach"),
        "non-TTY supervisor launch must NOT report attach failure; got:\n{stderr}"
    );
    assert!(
        !stderr.contains("open terminal failed"),
        "non-TTY supervisor launch must NOT report tmux open-terminal failure; got:\n{stderr}"
    );

    // Clean up the launched session.
    if let Some(line) = stdout.lines().find(|l| l.contains("tmux attach -t"))
        && let Some(session) = line.split_whitespace().last()
    {
        let _ = StdCommand::new("tmux")
            .env("TMUX_TMPDIR", tmux_env.socket_dir())
            .args(["kill-session", "-t", session])
            .status();
    }
}
