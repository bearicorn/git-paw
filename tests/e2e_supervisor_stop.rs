//! E2E observable: `git paw stop` tears down tmux and the broker.
//!
//! Maps to scenarios from supervisor-as-pane:
//!
//! - `Stop kills tmux and broker shuts down` (task 12.4)
//! - `Stop in supervisor mode also terminates auto-approve` (task 12.5)
//!
//! These tests boot a supervisor session in detached mode, observe stop,
//! and assert the side effects. They skip if tmux is unavailable.
//!
//! The broker port is chosen with a process-id offset so concurrent test
//! workers do not collide.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command as StdCommand, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::Command;
use serial_test::serial;

mod helpers;
use helpers::*;

const BIN: &str = env!("CARGO_BIN_EXE_git-paw");

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
    // Bind an OS-assigned ephemeral port, read it back, then release it so the
    // broker can claim it. Replaces the former `24_000 + (pid % 200)` scheme,
    // which gave only 200 ports keyed on PID and collided across concurrent
    // test runs (F8 root cause). An OS-assigned port is collision-proof at any
    // concurrency. Each call yields a distinct free port.
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read local addr")
        .port()
}

fn write_supervisor_config(repo: &std::path::Path, port: u16, auto_approve: bool) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let auto = if auto_approve {
        "\n[supervisor.auto_approve]\nenabled = true\n"
    } else {
        ""
    };
    let config = format!(
        r#"
default_cli = "echo"

[broker]
enabled = true
port = {port}

[supervisor]
enabled = true
cli = "echo"
{auto}"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
}

fn kill_session(name: &str, tmpdir: &std::path::Path) {
    let _ = StdCommand::new("tmux")
        .env("TMUX_TMPDIR", tmpdir)
        .args(["kill-session", "-t", name])
        .status();
}

fn extract_session_name(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
        .map(str::to_string)
}

#[test]
#[serial]
fn stop_kills_tmux_and_shuts_down_broker() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();
    let port = pick_broker_port();
    write_supervisor_config(tr.path(), port, false);
    let home = tempfile::TempDir::new().expect("home tempdir");

    // Boot the session.
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--supervisor", "--branches", "a,b"])
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run start");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "start failed; stdout:\n{stdout}");
    let session_name = extract_session_name(&stdout).expect("session name in stdout");

    // Run `git paw stop`.
    let mut stop = cmd();
    tmux_env.apply_assert(&mut stop);
    let stop_out = stop
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["stop"])
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run stop");
    assert!(
        stop_out.status.success(),
        "stop failed; stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&stop_out.stdout),
        String::from_utf8_lossy(&stop_out.stderr)
    );

    // Within 5 seconds, the broker port must be freshly bindable AND tmux
    // session must be gone. The port check polls a *connect* (refused = no
    // listener), not a *bind*: a fresh bind can spuriously fail while the
    // just-closed broker's port lingers in TCP TIME_WAIT even though the broker
    // is gone, whereas connect is refused as soon as there is no listener — the
    // condition we actually care about, immune to TIME_WAIT (v0.10.0
    // test-isolation hardening). Window widened to absorb teardown under load.
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut port_free = false;
    let mut tmux_gone = false;
    while Instant::now() < deadline {
        if !port_free {
            port_free = std::net::TcpStream::connect_timeout(
                &format!("127.0.0.1:{port}").parse().unwrap(),
                Duration::from_millis(200),
            )
            .is_err();
        }
        if !tmux_gone {
            let has = StdCommand::new("tmux")
                .env("TMUX_TMPDIR", tmux_env.socket_dir())
                .args(["has-session", "-t", &session_name])
                .status()
                .is_ok_and(|s| s.success());
            tmux_gone = !has;
        }
        if port_free && tmux_gone {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    kill_session(&session_name, tmux_env.socket_dir());
    assert!(
        port_free,
        "broker port {port} should be released (no listener) after stop"
    );
    assert!(
        tmux_gone,
        "tmux session {session_name} should be gone after stop"
    );
}

/// Spec scenario "An attended session runs no auto-approver"
/// (`approval-auto`), stop half: a session that DECLARES the deprecated
/// `[supervisor.auto_approve] enabled = true` still tears down cleanly, and
/// no `auto_approved` status appears in the broker log at any point — the
/// table parses for compatibility and does nothing.
#[test]
#[serial]
fn stop_tears_down_cleanly_with_the_deprecated_auto_approve_table() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();
    let port = pick_broker_port();
    write_supervisor_config(tr.path(), port, true);
    let home = tempfile::TempDir::new().expect("home tempdir");

    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--supervisor", "--branches", "a,b"])
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run start");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "start failed; stdout:\n{stdout}");
    let session_name = extract_session_name(&stdout).expect("session name in stdout");

    // Nothing may ever publish `auto_approved`: no approver exists to do it.
    let log_before = read_broker_log(home.path());
    assert!(
        !log_before.contains("auto_approved"),
        "a declared (deprecated) auto_approve table must start no approver; broker log:\n{log_before}"
    );

    let mut stop = cmd();
    tmux_env.apply_assert(&mut stop);
    let stop_out = stop
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["stop"])
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run stop");
    assert!(stop_out.status.success(), "stop failed");

    // Give a moment for any in-flight auto_approve tick to land.
    std::thread::sleep(Duration::from_millis(500));

    let log_after = read_broker_log(home.path());
    let tmux_gone = StdCommand::new("tmux")
        .env("TMUX_TMPDIR", tmux_env.socket_dir())
        .args(["has-session", "-t", &session_name])
        .status()
        .map_or(true, |s| !s.success());
    kill_session(&session_name, tmux_env.socket_dir());
    assert!(
        !log_after.contains("auto_approved"),
        "no auto_approved status may appear at any point; broker log:\n{log_after}"
    );
    assert!(tmux_gone, "tmux session must be killed after stop");
}

// ---------------------------------------------------------------------------
// `stop_revokes_the_drive_loop_binding` support
//
// This scenario needs a drive loop that is still RUNNING while `stop` executes,
// which the synchronous `assert_cmd` driver above cannot express — hence the
// background-child spawn and the condition-based waits below.
// ---------------------------------------------------------------------------

/// Writes a config whose coding agents launch `agent_cli`.
///
/// [`write_supervisor_config`] pins `default_cli = "echo"`, which exits at once
/// and takes its pane with it; the binding scenario needs a pane that stays up
/// long enough to be driven, and whose stdin can be observed.
fn write_recorder_config(repo: &Path, port: u16, agent_cli: &Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let cli = agent_cli.display();
    let config = format!(
        "default_cli = \"{cli}\"\n\n\
         [broker]\n\
         enabled = true\n\
         port = {port}\n\n\
         [supervisor]\n\
         enabled = true\n\
         cli = \"echo\"\n"
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
}

/// Writes an executable dummy CLI that records every byte typed into its pane.
///
/// On each start it appends a line to `starts` and TRUNCATES `marker`, so
/// `marker` only ever holds what was typed since the most recent start. That is
/// what lets the test distinguish "the old loop typed into the *recovered*
/// session" from "something was typed into the original session before the
/// stop" — the recovery relaunches this script, which wipes the slate.
///
/// `exec cat` both holds the pane open and records, so a byte that arrived
/// cannot be lost to a reader that had already exited.
fn write_stdin_recorder_dummy(dir: &Path, name: &str, starts: &Path, marker: &Path) -> PathBuf {
    let path = dir.join(name);
    let starts = starts.display();
    let marker = marker.display();
    let script = format!(
        "#!/usr/bin/env bash\n\
         # Test dummy CLI: hold the pane and record everything typed into it.\n\
         echo start >> '{starts}'\n\
         : > '{marker}'\n\
         exec cat >> '{marker}'\n"
    );
    fs::write(&path, script).expect("write stdin-recorder dummy");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).expect("stat dummy").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("chmod dummy");
    }
    path
}

/// Spawns `git paw start --unattended` as a background child with the test's
/// isolation env applied and no controlling terminal on any stream, its
/// stdout/stderr redirected to files under `out_dir`.
///
/// Mirrors `unattended_drive_e2e.rs::spawn_unattended_detached`; the drive loop
/// runs in this child, so the test can act on the session while it is live.
fn spawn_unattended(
    repo: &Path,
    home: &Path,
    socket_dir: &Path,
    out_dir: &Path,
    branches: &str,
    poll_ms: &str,
    heartbeat_ms: &str,
) -> (Child, PathBuf) {
    let stdout_path = out_dir.join("stdout.log");
    let stdout = fs::File::create(&stdout_path).expect("create stdout log");
    let stderr = fs::File::create(out_dir.join("stderr.log")).expect("create stderr log");

    let child = StdCommand::new(BIN)
        .current_dir(repo)
        .args(["start", "--unattended", "--branches", branches])
        .env("TMUX_TMPDIR", socket_dir)
        .env("HOME", home)
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("GIT_PAW_READINESS_TIMEOUT_MS", "120")
        .env("GIT_PAW_DRIVE_POLL_MS", poll_ms)
        .env("GIT_PAW_DRIVE_HEARTBEAT_MS", heartbeat_ms)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .expect("spawn detached git paw start --unattended");
    (child, stdout_path)
}

/// Polls until `path` exists, or the budget elapses.
fn wait_for_path(path: &Path, budget: Duration) -> bool {
    wait_until(budget, || path.exists())
}

/// Polls until `cond` holds, or the budget elapses.
fn wait_until(budget: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + budget;
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Waits up to `budget` for `child` to exit, returning its status (or `None` on
/// timeout, after killing and reaping it).
fn wait_child(child: &mut Child, budget: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + budget;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {}
            Err(_) => return None,
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Tolerant file read — a log that does not exist yet reads as empty.
fn read_log(path: &Path) -> String {
    let mut s = String::new();
    if let Ok(mut f) = fs::File::open(path) {
        let _ = f.read_to_string(&mut s);
    }
    s
}

/// The session name from a launch's "Supervisor session 'paw-…' launched"
/// stdout line (the unattended path never prints the `tmux attach -t` hint
/// [`extract_session_name`] parses — it runs the loop instead).
fn launched_session_name(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .find(|l| l.contains("Supervisor session 'paw-"))
        .and_then(|l| l.split('\'').nth(1))
        .map(ToString::to_string)
}

/// Spec scenario "Stop revokes the binding before a quick recovery"
/// (`supervisor-unattended-operation`): a running drive loop whose binding
/// `git paw stop` revoked exits `session-torn-down` on its next tick and sends
/// no keystroke to the session recovered in the meantime.
///
/// The poll interval is deliberately long and the stop-then-recover deliberately
/// quick, because that is the only arrangement under which the binding is doing
/// any work. Recovery clones the receipt, `created_at` and all, and flips the
/// status back to `Active`, so a loop that ticks *after* the recovery sees a
/// session record identical to the one it bound to — the GP-06 record check
/// passes, and only the revoked binding can end the loop. A short poll interval
/// would let the loop observe the intermediate `Stopped` and exit for the old
/// reason, and the test would pass whether or not `stop` revoked anything.
#[test]
#[serial]
fn stop_revokes_the_drive_loop_binding() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();
    let home = tempfile::TempDir::new().expect("home tempdir");
    let out = tempfile::TempDir::new().expect("out tempdir");
    let port = pick_broker_port();

    let starts = out.path().join("starts.log");
    let typed = out.path().join("typed.log");
    let dummy = write_stdin_recorder_dummy(out.path(), "recorder.sh", &starts, &typed);
    write_recorder_config(tr.path(), port, &dummy);

    // A long poll interval (the loop must not tick between the stop and the
    // recovery) and a heartbeat far beyond the test, so the only exit on offer
    // is the torn-down one.
    let (mut child, stdout_path) = spawn_unattended(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        out.path(),
        "feat/alpha",
        "15000",
        "600000",
    );

    // The binding file appearing is the proof the loop is bound and running —
    // it is taken after the session is saved and before the loop starts.
    let binding = git_paw::supervisor::loop_binding::binding_path(tr.path());
    let bound = wait_for_path(&binding, Duration::from_secs(60));
    if !bound {
        let _ = child.kill();
        let _ = child.wait();
    }
    assert!(
        bound,
        "the drive loop must take its binding at {}; stdout:\n{}",
        binding.display(),
        read_log(&stdout_path)
    );

    // Stop, then recover, both well inside one poll interval.
    let mut stop = cmd();
    tmux_env.apply_assert(&mut stop);
    let stop_out = stop
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["stop"])
        .timeout(Duration::from_secs(20))
        .output()
        .expect("run stop");
    assert!(
        stop_out.status.success(),
        "stop failed; stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&stop_out.stdout),
        String::from_utf8_lossy(&stop_out.stderr)
    );
    assert!(
        !binding.exists(),
        "stop must revoke the drive-loop binding at {}",
        binding.display()
    );

    // `git paw start` on a Stopped session recovers it. No loop runs here, so
    // the binding stays revoked — which is the state the old loop will read.
    let mut recover = cmd();
    tmux_env.apply_assert(&mut recover);
    let recover_out = recover
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start"])
        .timeout(Duration::from_secs(60))
        .output()
        .expect("run start (recover)");
    let recover_stdout = String::from_utf8_lossy(&recover_out.stdout).to_string();
    assert!(
        recover_out.status.success(),
        "recovery failed; stdout:\n{recover_stdout}\nstderr: {}",
        String::from_utf8_lossy(&recover_out.stderr)
    );

    // The premise of the scenario: the recovery landed BEFORE the loop's next
    // tick. If this ever fails the machine was slow enough to let a tick
    // through, and the run below proves less than it claims.
    let still_running = matches!(child.try_wait(), Ok(None));

    // The recovered pane relaunches the recorder, which wipes `typed`; from
    // here on, anything in it was typed into the RECOVERED session.
    let relaunched = wait_until(Duration::from_secs(30), || {
        read_log(&starts).lines().count() >= 2
    });

    let status = wait_child(&mut child, Duration::from_secs(60));
    let stdout = read_log(&stdout_path);

    let session_name = launched_session_name(&stdout)
        .or_else(|| extract_session_name(&recover_stdout))
        .unwrap_or_default();
    if !session_name.is_empty() {
        kill_session(&session_name, tmux_env.socket_dir());
    }

    assert!(
        still_running,
        "the loop ticked before the recovery, so this run cannot show the \
         binding (not the receipt) ending it; stdout:\n{stdout}"
    );
    let status = status.unwrap_or_else(|| panic!("drive loop did not exit; stdout:\n{stdout}"));
    assert!(
        status.success(),
        "the loop should exit cleanly; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Unattended drive loop exited: session-torn-down"),
        "a revoked binding must end the loop as session-torn-down; stdout:\n{stdout}"
    );
    assert!(
        relaunched,
        "the recovered session should relaunch the coding agent; starts:\n{}",
        read_log(&starts)
    );
    assert!(
        read_log(&typed).is_empty(),
        "the loop must send no keystroke to the recovered session; typed:\n{:?}",
        read_log(&typed)
    );
}

/// Reads every `*.broker.log` under the test's isolated `HOME` (where
/// `session_state_dir()` resolves to), concatenated.
fn read_broker_log(home: &std::path::Path) -> String {
    fn walk(dir: &std::path::Path, out: &mut String) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".broker.log"))
                && let Ok(text) = fs::read_to_string(&path)
            {
                out.push_str(&text);
            }
        }
    }
    let mut out = String::new();
    walk(home, &mut out);
    out
}
