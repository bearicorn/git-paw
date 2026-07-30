//! Real-tmux e2e guard for session-logging CAPTURE on launch.
//!
//! The capture side of session logging — deriving the per-branch log path,
//! creating the session log directory, and attaching `tmux pipe-pane` — is
//! unit-tested in `src/logging.rs` (path shape, sanitization, dir creation)
//! and `src/tmux.rs` (the queued `pipe-pane` command shape). What was missing
//! is a launch-time behavioural guard that a logging-enabled `git paw start`
//! actually produces the per-branch log file on disk once a real tmux session
//! is running.
//!
//! Session-logging capture is wired only into the from-specs launch path
//! (`cmd_start_with_specs` → `launch_spec_session`; `src/main.rs`), so this
//! test drives `git paw start --from-specs` with `[logging] enabled = true`,
//! broker disabled (pane offset 0), and an `echo` fake CLI so panes boot
//! without an LLM.
//!
//! Isolation (see openspec/changes/test-tmux-isolation):
//!   * tmux runs on a test-owned socket via `helpers::TmuxTestEnv`, so the
//!     spawned server never touches the user's default socket / live
//!     supervisor session;
//!   * HOME/XDG are pointed at a tempdir so session-state writes don't
//!     pollute or collide with the real `~/.../git-paw/sessions/`.
//!
//! The assertion is a poll-until-exists on the real path
//! `git_paw::logging::log_file_path` derives — never a fixed sleep as the
//! gate.

use std::path::Path;
use std::time::{Duration, Instant};

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::{TmuxTestEnv, setup_test_repo, tmux_test_env};

/// Total budget for the launch-side log file to appear on disk. Generous —
/// the launch subprocess executes the `pipe-pane` command synchronously
/// before returning, so the file is expected almost immediately; the budget
/// only absorbs filesystem-flush jitter.
const LOG_APPEAR_TIMEOUT: Duration = Duration::from_secs(5);

/// Returns `true` when tmux is available on PATH. tmux is a hard dependency,
/// but the skip guard keeps the suite green on a machine that lacks it.
fn tmux_available() -> bool {
    std::process::Command::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// `git-paw` binary builder with HOME/XDG and tmux socket isolation applied,
/// mirroring `tests/e2e_tests.rs::cmd_iso`.
fn cmd_iso(fake_home: &Path, tmux_env: &TmuxTestEnv) -> Command {
    let mut c = Command::cargo_bin("git-paw").expect("binary exists");
    c.env("HOME", fake_home).env_remove("XDG_DATA_HOME");
    tmux_env.apply_assert(&mut c);
    c
}

/// Writes a `.git-paw/config.toml` enabling session logging, the `OpenSpec`
/// backend, and an `echo` fake CLI. Broker + supervisor omitted so the launch
/// dispatches to the bare from-specs path (no dashboard pane, pane offset 0).
fn write_logging_specs_config(repo: &Path) {
    let paw_dir = repo.join(".git-paw");
    std::fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = r#"
[logging]
enabled = true

[specs]
type = "openspec"
dir = "openspec/changes"

[clis.echo]
command = "echo"
display_name = "Echo"
"#;
    std::fs::write(paw_dir.join("config.toml"), config).expect("write config");
}

/// Creates a pending `OpenSpec` change at `<repo>/openspec/changes/<id>/tasks.md`.
fn write_spec(repo: &Path, id: &str) {
    let change_dir = repo.join("openspec/changes").join(id);
    std::fs::create_dir_all(&change_dir).expect("create change dir");
    std::fs::write(change_dir.join("tasks.md"), format!("Implement {id}.\n"))
        .expect("write tasks.md");
}

/// Commits everything currently in the working tree.
fn commit_all(repo: &Path, message: &str) {
    for args in [&["add", "."][..], &["commit", "-m", message][..]] {
        let out = std::process::Command::new("git")
            .current_dir(repo)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// Extracts the session name from the detached-mode launch hint printed by
/// `attach_or_print_hint`: `Session '<name>' started in detached mode.`.
fn parse_session_name(stdout: &str) -> String {
    stdout
        .lines()
        .find_map(|line| {
            line.strip_prefix("Session '")
                .and_then(|rest| rest.split('\'').next())
        })
        .unwrap_or_else(|| panic!("could not parse session name from launch stdout:\n{stdout}"))
        .to_string()
}

/// Polls the filesystem until `path` exists or the timeout elapses. The gate
/// is the existence check with a bounded total budget — the sleep only paces
/// the poll loop, it is never the gate itself.
fn poll_until_exists(path: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if path.exists() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Queries a pane's `#{pane_pipe}` flag (1 when `pipe-pane` is attached),
/// polling until it reads `"1"` or the timeout elapses. Returns the last
/// observed value.
fn poll_pane_pipe(tmux_env: &TmuxTestEnv, target: &str, timeout: Duration) -> String {
    let deadline = Instant::now() + timeout;
    let mut last = String::new();
    loop {
        let mut cmd = std::process::Command::new("tmux");
        cmd.args(["display-message", "-p", "-t", target, "#{pane_pipe}"]);
        tmux_env.apply(&mut cmd);
        if let Ok(out) = cmd.output() {
            last = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if last == "1" {
                return last;
            }
        }
        if Instant::now() >= deadline {
            return last;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// A logging-enabled `git paw start --from-specs` SHALL create the session log
/// directory and a per-branch log file for every launched worktree, and attach
/// `tmux pipe-pane` to each pane.
///
/// Guards the session-logging capture contract end-to-end:
///   * "Create session log directory" — `.git-paw/logs/<session>/` exists;
///   * "Derive log file path per pane" — the per-branch file lands at the
///     exact path `logging::log_file_path` derives, with `/` sanitized to
///     `--` (`spec/add-auth` → `spec--add-auth.log`);
///   * "Attach pipe-pane to capture output" — asserted directly via the
///     pane's `#{pane_pipe}` flag, and consequentially by the log file (which
///     the piped `cat >> <log>` process creates).
#[test]
#[serial]
fn logging_enabled_start_creates_per_branch_logs() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let tr = setup_test_repo();
    let fake_home = TempDir::new().expect("home tempdir");
    let tmux_env = tmux_test_env();

    write_logging_specs_config(tr.path());
    write_spec(tr.path(), "add-auth");
    write_spec(tr.path(), "add-api");
    commit_all(tr.path(), "add specs and logging config");

    // Real (non-dry-run) launch. assert_cmd's `output()` runs the binary in a
    // non-TTY child, so `attach_or_print_hint` prints the detached-mode hint
    // and returns Ok instead of blocking on `tmux attach`.
    let output = cmd_iso(fake_home.path(), &tmux_env)
        .current_dir(tr.path())
        .args(["start", "--from-specs", "--cli", "echo"])
        .output()
        .expect("run start --from-specs");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "logging-enabled from-specs launch should succeed; stdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let session_name = parse_session_name(&stdout);

    // Ensure the live tmux session is torn down regardless of assertion
    // outcome. Dropping `tmux_env` also removes the socket dir (killing the
    // server), but an explicit kill keeps the test honest per the task.
    let kill = |tmux_env: &TmuxTestEnv, name: &str| {
        let mut cmd = std::process::Command::new("tmux");
        cmd.args(["kill-session", "-t", name]);
        tmux_env.apply(&mut cmd);
        let _ = cmd.output();
    };

    // Core guard: the per-branch log file appears at the exact path the
    // production derivation produces. `spec/<id>` sanitizes to `spec--<id>.log`.
    let branches = ["spec/add-auth", "spec/add-api"];
    for branch in branches {
        let log_path = git_paw::logging::log_file_path(tr.path(), &session_name, branch);

        // The path shape is load-bearing: session-scoped dir + `/`→`--` filename.
        let sanitized = branch.replace('/', "--");
        let expected_suffix = format!(".git-paw/logs/{session_name}/{sanitized}.log");
        assert!(
            log_path.ends_with(&expected_suffix)
                || log_path.to_string_lossy().ends_with(&expected_suffix),
            "derived log path {} should end with {expected_suffix}",
            log_path.display()
        );

        if !poll_until_exists(&log_path, LOG_APPEAR_TIMEOUT) {
            kill(&tmux_env, &session_name);
            panic!(
                "per-branch log file was not created within {:?} at {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
                LOG_APPEAR_TIMEOUT,
                log_path.display()
            );
        }
    }

    // The session log directory exists (shared parent of the per-branch files).
    let log_dir = tr.path().join(".git-paw/logs").join(&session_name);
    assert!(
        log_dir.is_dir(),
        "session log directory should exist at {}",
        log_dir.display()
    );

    // Direct pipe-pane-attach proof: pane 0.0 (first worktree, no broker
    // offset) reports `#{pane_pipe} == 1`. Panes stay alive as shells after
    // the fake `echo` CLI exits, so the flag is observable.
    let pane_pipe = poll_pane_pipe(
        &tmux_env,
        &format!("{session_name}:0.0"),
        Duration::from_secs(3),
    );

    // Tear down before the final assertion so a failing pipe check still
    // leaves no session behind.
    kill(&tmux_env, &session_name);

    assert_eq!(
        pane_pipe, "1",
        "pipe-pane should be attached to pane {session_name}:0.0 (#{{pane_pipe}} == 1), got {pane_pipe:?}"
    );
}
