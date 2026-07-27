//! Interactive `git paw init` prompt tests — the TTY-*shown* rows of the
//! `cli-interaction-e2e` matrix.
//!
//! `dialoguer`'s `Select`/`Confirm`/`Input` need a real TTY, so these drive the
//! real binary inside a detached tmux pane via the shared PTY harness
//! (`helpers::pty`) and assert on the written `.git-paw/config.toml`. Socket
//! isolation via `helpers::TmuxTestEnv`; `#[serial]` + tmux-availability-gated.
//!
//! The non-TTY *bypass* rows (the "bypassed when" half of the matrix) live in
//! `cli_prompt_matrix.rs`; the pure formatting each prompt feeds is unit-tested
//! in `src/init.rs`.

use std::process::Command;
use std::time::Duration;

use serial_test::serial;

mod helpers;
use helpers::pty::{
    create_detached_session, kill_session, send_keys, tmux_available, unique_session_name,
    wait_for_file, wait_for_pane,
};

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

    let session = unique_session_name("paw-init-specs");
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

    let session = unique_session_name("paw-init-sup");
    create_detached_session(&session);

    let bin = env!("CARGO_BIN_EXE_git-paw");
    let cmd = format!("cd '{}' && '{bin}' init", repo.path().display());
    send_keys(&session, &[&cmd, "Enter"]);

    // Prompt 1: supervisor Confirm. dialoguer's Confirm resolves on the `y`
    // key alone (no Enter), so sending Enter here would leak into the Input.
    wait_for_pane(&session, "Enable supervisor", Duration::from_secs(10));
    send_keys(&session, &["y"]);

    // Prompt 2: the test-command Input (only shown when supervisor is on).
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
