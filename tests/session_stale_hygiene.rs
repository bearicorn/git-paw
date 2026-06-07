//! End-to-end integration tests for `session-receipt-hygiene` (Bug 2).
//!
//! Exercises the production binary against a controlled receipt directory
//! (via a `HOME` override) so the cross-module flow — receipt on disk →
//! `tmux has-session` liveness probe → `cmd_status` / `cmd_purge --stale`
//! display & purge decision — is covered behaviourally, not just at the unit
//! level (`DisplayStatus::from_receipt`).
//!
//! These tests deliberately do NOT call `setup_test_repo()` (the live-session
//! guard helper): every session name used here is guaranteed-absent from tmux,
//! so the probe returns non-zero without ever creating or touching a real tmux
//! session — the run is socket-isolated.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::SystemTime;

use assert_cmd::Command;
use git_paw::session::{Session, SessionMode, SessionStatus};
use tempfile::TempDir;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// The sessions directory the binary resolves for a given fake `HOME`,
/// mirroring `git_paw::dirs::data_dir()`.
fn sessions_dir_for_home(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-paw/sessions")
    } else {
        home.join(".local/share/git-paw/sessions")
    }
}

/// Canonicalises a path so it matches what `git rev-parse --show-toplevel`
/// (via `git::validate_repo`) returns — on macOS `/var/...` resolves to
/// `/private/var/...`, and a receipt keyed on the un-canonicalised path would
/// never match the running `status` invocation's repo root.
fn canon(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

/// Initialises a minimal committed git repo (used as the receipt's
/// `repo_path`, so purge's git probes operate against a real repository).
fn init_git_repo(dir: &Path) {
    let run = |args: &[&str]| {
        StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git command");
    };
    run(&["init", "-q"]);
    run(&["config", "user.email", "t@e.st"]);
    run(&["config", "user.name", "Test"]);
    fs::write(dir.join("README.md"), "x").expect("write readme");
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "init"]);
}

fn receipt(session_name: &str, repo_path: &Path, status: SessionStatus) -> Session {
    Session {
        session_name: session_name.to_string(),
        repo_path: repo_path.to_path_buf(),
        project_name: "stale-hygiene".to_string(),
        created_at: SystemTime::now(),
        status,
        worktrees: vec![],
        broker_port: None,
        broker_bind: None,
        broker_log_path: None,
        mode: SessionMode::Bare,
        dashboard_pane: None,
    }
}

/// A unique-ish session name that is guaranteed absent from tmux. Avoids
/// `Math.random`-style flakiness by deriving from the test's repo path.
fn dead_name(tag: &str) -> String {
    format!("paw-stale-hygiene-{tag}-doesnotexist")
}

// ---------------------------------------------------------------------------
// status — stale display
// ---------------------------------------------------------------------------

#[test]
fn status_reports_stale_for_active_receipt_with_dead_tmux() {
    let home = TempDir::new().expect("home");
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());

    let sdir = sessions_dir_for_home(home.path());
    fs::create_dir_all(&sdir).expect("create sessions dir");
    let name = dead_name("status");
    git_paw::session::save_session_in(
        &receipt(&name, &canon(repo.path()), SessionStatus::Active),
        &sdir,
    )
    .expect("save receipt");

    // JSON output carries the additive "stale" status value.
    let json = cmd()
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["status", "--json"])
        .output()
        .expect("status --json");
    assert!(json.status.success(), "status --json should succeed");
    let stdout = String::from_utf8_lossy(&json.stdout);
    assert!(
        stdout.contains("\"status\":\"stale\"") || stdout.contains("\"status\": \"stale\""),
        "JSON status should be stale; got:\n{stdout}"
    );

    // Human output shows the red icon + the self-heal hint.
    let human = cmd()
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .arg("status")
        .output()
        .expect("status");
    let h = String::from_utf8_lossy(&human.stdout);
    assert!(
        h.contains("stale"),
        "human status should mention stale;\n{h}"
    );
    assert!(
        h.contains('\u{1f534}'),
        "human status should show the red stale icon;\n{h}"
    );
}

// ---------------------------------------------------------------------------
// start — auto-invalidation of a stale receipt (design D5)
// ---------------------------------------------------------------------------

#[test]
fn start_with_stale_receipt_emits_notice_and_purges() {
    let home = TempDir::new().expect("home");
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());

    // A config whose only CLI is not on PATH, so the fresh-launch path errors
    // with NoCLIsFound *after* the stale-receipt invalidation has already run
    // — the launch never reaches tmux-session creation, keeping the test
    // socket-isolated while still exercising the real `cmd_start` entry.
    let paw = repo.path().join(".git-paw");
    fs::create_dir_all(&paw).expect("mk .git-paw");
    fs::write(
        paw.join("config.toml"),
        "default_cli = \"ghost-cli-xyz\"\n\n[clis.ghost-cli-xyz]\ncommand = \"ghost-cli-xyz-not-on-path\"\n",
    )
    .expect("write config");

    let sdir = sessions_dir_for_home(home.path());
    fs::create_dir_all(&sdir).expect("create sessions dir");
    let name = dead_name("start");
    git_paw::session::save_session_in(
        &receipt(&name, &canon(repo.path()), SessionStatus::Active),
        &sdir,
    )
    .expect("save receipt");

    let out = cmd()
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--no-supervisor"])
        .output()
        .expect("start");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("removed stale session receipt"),
        "start should emit the stale-invalidation notice;\nstderr:\n{stderr}"
    );
    assert!(
        !sdir.join(format!("{name}.json")).exists(),
        "stale receipt should have been purged before the fresh launch"
    );
}

// ---------------------------------------------------------------------------
// purge --stale
// ---------------------------------------------------------------------------

#[test]
fn purge_stale_removes_stale_receipt_only() {
    let home = TempDir::new().expect("home");
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());

    let sdir = sessions_dir_for_home(home.path());
    fs::create_dir_all(&sdir).expect("create sessions dir");

    // One stale receipt (active + dead tmux) and one stopped receipt (never
    // stale — must survive). Different repo_path per receipt so neither is
    // skipped as a duplicate.
    let stale_name = dead_name("purge-stale");
    let stopped_name = dead_name("purge-stopped");
    git_paw::session::save_session_in(
        &receipt(&stale_name, repo.path(), SessionStatus::Active),
        &sdir,
    )
    .expect("save stale");
    git_paw::session::save_session_in(
        &receipt(&stopped_name, repo.path(), SessionStatus::Stopped),
        &sdir,
    )
    .expect("save stopped");

    let out = cmd()
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["purge", "--stale"])
        .output()
        .expect("purge --stale");
    assert!(out.status.success(), "purge --stale should exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Purged stale session"),
        "should announce the purged stale session;\n{stdout}"
    );

    // The stale receipt file is gone; the stopped one remains untouched.
    assert!(
        !sdir.join(format!("{stale_name}.json")).exists(),
        "stale receipt should be purged"
    );
    assert!(
        sdir.join(format!("{stopped_name}.json")).exists(),
        "stopped (non-stale) receipt must be left intact"
    );
}

#[test]
fn purge_stale_with_nothing_stale_exits_zero() {
    let home = TempDir::new().expect("home");
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());

    let sdir = sessions_dir_for_home(home.path());
    fs::create_dir_all(&sdir).expect("create sessions dir");
    // Only a stopped receipt — nothing stale to purge.
    let stopped_name = dead_name("nothing");
    git_paw::session::save_session_in(
        &receipt(&stopped_name, repo.path(), SessionStatus::Stopped),
        &sdir,
    )
    .expect("save stopped");

    let out = cmd()
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["purge", "--stale"])
        .output()
        .expect("purge --stale");
    assert!(
        out.status.success(),
        "purge --stale should exit 0 with nothing stale"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("No stale sessions to purge"),
        "should report nothing to purge;\n{stdout}"
    );
    assert!(
        sdir.join(format!("{stopped_name}.json")).exists(),
        "stopped receipt must survive a --stale purge"
    );
}

#[test]
fn purge_stale_with_force_is_well_defined() {
    // `--stale --force` behaves identically to `--stale` alone: only the stale
    // receipt is purged, the stopped (non-stale) one survives, exit 0.
    let home = TempDir::new().expect("home");
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());

    let sdir = sessions_dir_for_home(home.path());
    fs::create_dir_all(&sdir).expect("create sessions dir");
    let stale_name = dead_name("sf-stale");
    let stopped_name = dead_name("sf-stopped");
    git_paw::session::save_session_in(
        &receipt(&stale_name, repo.path(), SessionStatus::Active),
        &sdir,
    )
    .expect("save stale");
    git_paw::session::save_session_in(
        &receipt(&stopped_name, repo.path(), SessionStatus::Stopped),
        &sdir,
    )
    .expect("save stopped");

    let out = cmd()
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["purge", "--stale", "--force"])
        .output()
        .expect("purge --stale --force");
    assert!(out.status.success(), "purge --stale --force should exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Purged stale session"),
        "should purge the stale entry (force is a no-op here);\n{stdout}"
    );
    assert!(
        !sdir.join(format!("{stale_name}.json")).exists(),
        "stale receipt should be purged under --stale --force"
    );
    assert!(
        sdir.join(format!("{stopped_name}.json")).exists(),
        "--force must NOT widen --stale to purge the non-stale (stopped) receipt"
    );
}
