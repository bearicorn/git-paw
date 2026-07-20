//! Integration tests for the interactive `git paw init` prompts.
//!
//! `dialoguer`'s `Select`/`Confirm`/`Input` need a real TTY, so they cannot be
//! driven from an in-process unit test (`assert_cmd` gives the child pipes, not
//! a terminal). git-paw depends on tmux, so these tests run the real binary
//! inside a detached tmux pane — a genuine PTY — and drive it with `send-keys`,
//! then assert on the observable outcome: the sections written into
//! `.git-paw/config.toml`.
//!
//! Covered here:
//! - the `OpenSpec` scenario "interactive init records the chosen spec system"
//!   (project-initialization) — the spec-system `Select`;
//! - the supervisor `Confirm` + test-command `Input` path.
//!
//! The pure formatting each prompt feeds is unit-tested separately in
//! `src/init.rs` (`specs_section_for`, `supervisor_section`); these tests cover
//! only the keystroke -> written-config wiring the unit tests cannot reach.
//!
//! tmux socket isolation mirrors `auto_approve_integration.rs`: the test sets
//! `TMUX_TMPDIR` on the current process via
//! `helpers::tmux_test_env().apply_to_process()` so the `tmux` subprocesses it
//! spawns share a test-owned socket, and is `#[serial]` because it mutates the
//! process environment.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use serial_test::serial;

mod helpers;

fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn unique_session_name() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("paw-init-specs-{nanos}")
}

fn kill_session(name: &str) {
    let _ = Command::new("tmux")
        .args(["kill-session", "-t", name])
        .output();
}

/// Detached tmux session running a long-lived shell we can `send-keys` into.
fn create_detached_session(name: &str) {
    let status = Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            name,
            "-x",
            "200",
            "-y",
            "50",
            "sh",
        ])
        .status()
        .expect("tmux new-session");
    assert!(status.success(), "tmux new-session failed");
    std::thread::sleep(Duration::from_millis(150));
}

/// Captures the visible buffer of pane 0 of `session`.
fn capture(session: &str) -> String {
    let out = Command::new("tmux")
        .args(["capture-pane", "-t", &format!("{session}:0.0"), "-p"])
        .output()
        .expect("tmux capture-pane");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Sends `keys` (literal text or named keys like `Down`/`Enter`) to pane 0.
fn send_keys(session: &str, keys: &[&str]) {
    let target = format!("{session}:0.0");
    let mut args = vec!["send-keys", "-t", &target];
    args.extend_from_slice(keys);
    let status = Command::new("tmux")
        .args(&args)
        .status()
        .expect("tmux send-keys");
    assert!(status.success(), "tmux send-keys failed");
}

/// Polls the pane until `needle` appears, or panics after `timeout`.
fn wait_for_pane(session: &str, needle: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        let buf = capture(session);
        if buf.contains(needle) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {needle:?} in pane; last capture:\n{buf}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Polls until `path` exists, or panics after `timeout`.
fn wait_for_file(path: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {} to be written",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
#[serial]
fn interactive_init_records_chosen_spec_system_in_config() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tmux_env = helpers::TmuxTestEnv::new();
    let _proc_env = tmux_env.apply_to_process();

    // A fresh git repo for `git paw init` to operate on. Created here (not in
    // the pane) so we can read the written config back from the test process.
    let repo = tempfile::TempDir::new().expect("tempdir");
    let status = Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo.path())
        .status()
        .expect("git init");
    assert!(status.success(), "git init failed");

    let session = unique_session_name();
    create_detached_session(&session);

    // Launch the real binary in the pane. Paths from TempDir/Cargo never
    // contain single quotes, so single-quote wrapping is sufficient.
    let bin = env!("CARGO_BIN_EXE_git-paw");
    let cmd = format!("cd '{}' && '{bin}' init", repo.path().display());
    send_keys(&session, &[&cmd, "Enter"]);

    // Prompt 1: supervisor Confirm (default No). Accept the default with Enter.
    wait_for_pane(&session, "Enable supervisor", Duration::from_secs(10));
    send_keys(&session, &["Enter"]);

    // Prompt 2: spec-system Select (default index 0 = openspec). Move down
    // twice to index 2 (speckit) and confirm.
    wait_for_pane(&session, "Which spec system", Duration::from_secs(10));
    send_keys(&session, &["Down", "Down", "Enter"]);

    // Assert on the outcome: the config records the chosen system, uncommented.
    let config_path = repo.path().join(".git-paw").join("config.toml");
    wait_for_file(&config_path, Duration::from_secs(10));
    // Give init a beat to finish writing the full file after creation.
    std::thread::sleep(Duration::from_millis(200));
    let content = std::fs::read_to_string(&config_path).expect("read config");
    kill_session(&session);

    // Parse the config the way git-paw itself does: commented documentation
    // blocks (the base template ships a commented `# [specs]` example) are
    // ignored, so only the active section the interactive choice appended is
    // observed.
    let cfg: git_paw::config::PawConfig =
        toml::from_str(&content).unwrap_or_else(|e| panic!("parse config: {e}\n{content}"));
    let specs = cfg.specs.unwrap_or_else(|| {
        panic!("interactive init must record an active [specs] section:\n{content}")
    });
    assert_eq!(
        specs.spec_type.as_deref(),
        Some("speckit"),
        "chosen spec system (index 2 = speckit) must be recorded"
    );
    assert_eq!(
        specs.dir.as_deref(),
        Some(".specify/specs"),
        "speckit's conventional dir must be recorded"
    );
}

#[test]
#[serial]
fn interactive_init_records_supervisor_choice_in_config() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tmux_env = helpers::TmuxTestEnv::new();
    let _proc_env = tmux_env.apply_to_process();

    let repo = tempfile::TempDir::new().expect("tempdir");
    let status = Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo.path())
        .status()
        .expect("git init");
    assert!(status.success(), "git init failed");

    let session = unique_session_name();
    create_detached_session(&session);

    let bin = env!("CARGO_BIN_EXE_git-paw");
    let cmd = format!("cd '{}' && '{bin}' init", repo.path().display());
    send_keys(&session, &[&cmd, "Enter"]);

    // Prompt 1: supervisor Confirm. dialoguer's Confirm resolves on the `y`
    // key alone (no Enter), so sending Enter here would leak into the Input.
    wait_for_pane(&session, "Enable supervisor", Duration::from_secs(10));
    send_keys(&session, &["y"]);

    // Prompt 2: the test-command Input (only shown when supervisor is on).
    // Type a command and submit with Enter.
    wait_for_pane(&session, "Test command", Duration::from_secs(10));
    send_keys(&session, &["just check", "Enter"]);

    // Prompt 3: spec-system Select — accept the default (index 0 = openspec).
    wait_for_pane(&session, "Which spec system", Duration::from_secs(10));
    send_keys(&session, &["Enter"]);

    let config_path = repo.path().join(".git-paw").join("config.toml");
    wait_for_file(&config_path, Duration::from_secs(10));
    std::thread::sleep(Duration::from_millis(200));
    let content = std::fs::read_to_string(&config_path).expect("read config");
    kill_session(&session);

    let cfg: git_paw::config::PawConfig =
        toml::from_str(&content).unwrap_or_else(|e| panic!("parse config: {e}\n{content}"));
    let supervisor = cfg.supervisor.unwrap_or_else(|| {
        panic!("interactive init must record a [supervisor] section:\n{content}")
    });
    assert!(
        supervisor.enabled,
        "answering 'y' must enable supervisor; got:\n{content}"
    );
    assert_eq!(
        supervisor.test_command.as_deref(),
        Some("just check"),
        "the typed test command must be recorded"
    );
}
