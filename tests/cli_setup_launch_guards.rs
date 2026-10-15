//! Launch-path guards for `approval-command-safety`'s *No permission grants
//! are seeded into agent CLI settings files*.
//!
//! These drive the **real launch paths** — `git paw start`, `git paw add` and
//! recovery — against an isolated tmux socket and an isolated `HOME`, and
//! assert that none of them creates a vendor settings directory, writes a
//! settings file, or touches the setup record.
//!
//! They live apart from `cli_setup_e2e.rs` on purpose. That file needs no
//! tmux; these need a session, so they are `#[serial]` and skip when tmux is
//! unavailable. Keeping them separate stops the cheap, always-runnable
//! assertions from being dragged behind a tmux dependency.
//!
//! The point of each test is that it would **fail** if a launch path started
//! seeding permissions again — which is exactly what git-paw used to do, and
//! what this capability forbids.

use std::fs;
use std::path::{Path, PathBuf};
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

/// Writes a supervisor-mode config using `cat` as the stand-in CLI, so a
/// launch is fast and needs no real agent binary. `cat` stays alive and
/// consumes the injected boot block as stdin rather than executing it, which
/// keeps the worktree clean.
fn write_supervisor_config(repo: &Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"cat\"\n\n[supervisor]\nenabled = true\ncli = \"cat\"\n\n\
         [broker]\nenabled = false\n",
    )
    .expect("write config");
}

fn kill_session(session: &str, tmux_env: &TmuxTestEnv) {
    let mut c = StdCommand::new("tmux");
    tmux_env.apply(&mut c);
    let _ = c.args(["kill-session", "-t", session]).status();
}

/// Starts a supervisor session and returns its tmux session name.
fn start_session(repo: &Path, home: &Path, tmux_env: &TmuxTestEnv, branches: &str) -> String {
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(repo)
        .env("HOME", home)
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--supervisor", "--branches", branches])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "start failed; stdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
        .expect("session name in start stdout")
        .to_string()
}

/// Every `.claude` directory anywhere under `root`.
fn vendor_dirs(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !entry.metadata().is_ok_and(|m| m.is_dir()) {
                continue;
            }
            if path.file_name().is_some_and(|n| n == ".claude") {
                found.push(path.clone());
            }
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            stack.push(path);
        }
    }
    found
}

/// Every `.claude` directory anywhere in the sandbox — the repository **and**
/// its sibling worktrees — excluding the isolated `HOME`, which is not
/// git-paw's territory to assert about.
///
/// Scanning the sandbox rather than the repository matters: the default
/// worktree placement is a sibling of the repository, so a scan rooted at the
/// repository would miss exactly the directory these tests are about.
fn sandbox_vendor_dirs(repo: &Path) -> Vec<PathBuf> {
    let sandbox = repo.parent().expect("sandbox");
    vendor_dirs(sandbox)
        .into_iter()
        .filter(|p| !p.starts_with(sandbox.join("home")))
        .collect()
}

fn record_path(repo: &Path) -> PathBuf {
    repo.join(".git/git-paw/cli-setup.json")
}

fn exclude_text(repo: &Path) -> String {
    fs::read_to_string(repo.join(".git/info/exclude")).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Scenario: No settings file is written at session start
// Scenario: No vendor settings directory is created in a worktree
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn starting_a_session_writes_no_settings_file_and_no_record() {
    if !tmux_available() {
        eprintln!("skipping: tmux unavailable");
        return;
    }
    let tmux_env = tmux_test_env();
    let test_repo = setup_test_repo();
    let repo = test_repo.path().to_path_buf();
    let home = repo.parent().expect("sandbox").join("home");
    fs::create_dir_all(&home).expect("create home");
    write_supervisor_config(&repo);

    let session = start_session(&repo, &home, &tmux_env, "feat/alpha");

    let dirs = sandbox_vendor_dirs(&repo);
    assert!(
        dirs.is_empty(),
        "a session start must create no vendor settings directory, found: {dirs:?}"
    );
    assert!(
        !record_path(&repo).exists(),
        "a session start must write no setup record"
    );
    assert!(
        !exclude_text(&repo).contains(".claude"),
        "a session start must add no exclusion entry for a settings file"
    );
    kill_session(&session, &tmux_env);
}

// ---------------------------------------------------------------------------
// Scenario: Pre-existing settings files are left untouched
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn a_settings_file_a_previous_version_wrote_is_left_byte_identical() {
    if !tmux_available() {
        eprintln!("skipping: tmux unavailable");
        return;
    }
    let tmux_env = tmux_test_env();
    let test_repo = setup_test_repo();
    let repo = test_repo.path().to_path_buf();
    let home = repo.parent().expect("sandbox").join("home");
    fs::create_dir_all(&home).expect("create home");
    write_supervisor_config(&repo);

    // Stand in for a file an older git-paw seeded: git-paw must neither
    // rewrite nor delete it.
    let settings = repo.join(".claude/settings.json");
    fs::create_dir_all(settings.parent().expect("parent")).expect("mkdir");
    let original = b"{\n  \"allowedBashPrefixes\": [\"git status\"]\n}\n";
    fs::write(&settings, original).expect("write settings");

    let session = start_session(&repo, &home, &tmux_env, "feat/alpha");

    assert!(settings.is_file(), "the file must not be deleted");
    assert_eq!(
        fs::read(&settings).expect("read settings"),
        original,
        "the file must be left byte-identical"
    );
    assert!(!record_path(&repo).exists(), "and no record is written");

    kill_session(&session, &tmux_env);
}

// ---------------------------------------------------------------------------
// Scenario: Earlier setup is not extended by a later launch
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn adding_a_worktree_after_setup_extends_nothing() {
    if !tmux_available() {
        eprintln!("skipping: tmux unavailable");
        return;
    }
    let tmux_env = tmux_test_env();
    let test_repo = setup_test_repo();
    let repo = test_repo.path().to_path_buf();
    let home = repo.parent().expect("sandbox").join("home");
    fs::create_dir_all(&home).expect("create home");
    write_supervisor_config(&repo);

    // Apply setup for real first, so there is something a launch could extend.
    cmd()
        .current_dir(&repo)
        .env("HOME", &home)
        .args(["setup-cli", "claude", "--allowlist", "--yes"])
        .write_stdin("")
        .assert()
        .success();

    let settings = repo.join(".claude/settings.local.json");
    let settings_before = fs::read(&settings).expect("settings");
    let record_before = fs::read(record_path(&repo)).expect("record");
    // setup-cli legitimately created the main checkout's directory; the
    // question is whether a launch adds any OTHER one.
    let vendor_before = sandbox_vendor_dirs(&repo);

    let session = start_session(&repo, &home, &tmux_env, "feat/alpha");

    // `add` creates a second worktree in the live session — the real path the
    // scenario names.
    let mut add = cmd();
    tmux_env.apply_assert(&mut add);
    let out = add
        .current_dir(&repo)
        .env("HOME", &home)
        .env_remove("XDG_DATA_HOME")
        .args(["add", "feat/beta"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run add");
    assert!(
        out.status.success(),
        "add failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Prove `add` really produced a worktree, without guessing its directory
    // name or its placement (sibling by default, internal when configured).
    let listed = StdCommand::new("git")
        .current_dir(&repo)
        .args(["worktree", "list"])
        .output()
        .expect("git worktree list");
    let worktrees = String::from_utf8_lossy(&listed.stdout);
    assert!(
        worktrees.lines().count() >= 3,
        "start plus add should leave two linked worktrees:\n{worktrees}"
    );
    assert!(
        worktrees.contains("beta"),
        "the added branch has a worktree:\n{worktrees}"
    );
    assert_eq!(
        sandbox_vendor_dirs(&repo),
        vendor_before,
        "a launch must create no settings directory beyond the one setup-cli \
         wrote in the main checkout"
    );
    assert_eq!(
        fs::read(&settings).expect("settings"),
        settings_before,
        "the main checkout's settings file must be byte-identical"
    );
    assert_eq!(
        fs::read(record_path(&repo)).expect("record"),
        record_before,
        "the setup record must be byte-identical"
    );

    kill_session(&session, &tmux_env);
}

// ---------------------------------------------------------------------------
// Scenario: Recovery and the drive loop write no setup entries
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn recovering_a_stopped_session_writes_no_setup_entries() {
    if !tmux_available() {
        eprintln!("skipping: tmux unavailable");
        return;
    }
    let tmux_env = tmux_test_env();
    let test_repo = setup_test_repo();
    let repo = test_repo.path().to_path_buf();
    let home = repo.parent().expect("sandbox").join("home");
    fs::create_dir_all(&home).expect("create home");
    write_supervisor_config(&repo);

    cmd()
        .current_dir(&repo)
        .env("HOME", &home)
        .args(["setup-cli", "claude", "--allowlist", "--yes"])
        .write_stdin("")
        .assert()
        .success();

    let settings = repo.join(".claude/settings.local.json");
    let settings_before = fs::read(&settings).expect("settings");
    let record_before = fs::read(record_path(&repo)).expect("record");
    let vendor_before = sandbox_vendor_dirs(&repo);

    let session = start_session(&repo, &home, &tmux_env, "feat/alpha");

    // Stop the session, then start again — the recovery path, which relaunches
    // every pane from the recorded roster.
    let mut stop = cmd();
    tmux_env.apply_assert(&mut stop);
    let _ = stop
        .current_dir(&repo)
        .env("HOME", &home)
        .env_remove("XDG_DATA_HOME")
        .args(["stop"])
        .timeout(Duration::from_secs(40))
        .output();

    let recovered = start_session(&repo, &home, &tmux_env, "feat/alpha");

    assert_eq!(
        fs::read(&settings).expect("settings"),
        settings_before,
        "recovery must leave the settings file byte-identical"
    );
    assert_eq!(
        fs::read(record_path(&repo)).expect("record"),
        record_before,
        "recovery must leave the setup record byte-identical"
    );
    assert_eq!(
        sandbox_vendor_dirs(&repo),
        vendor_before,
        "recovery must create no settings directory beyond the one setup-cli \
         wrote in the main checkout"
    );

    kill_session(&recovered, &tmux_env);
    kill_session(&session, &tmux_env);
}
