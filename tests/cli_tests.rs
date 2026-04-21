//! CLI binary tests.
//!
//! Tests the `git-paw` binary's argument parsing, help output, and version
//! output using `assert_cmd`.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

// ---------------------------------------------------------------------------
// Help output
// ---------------------------------------------------------------------------

#[test]
fn help_shows_all_subcommands() {
    cmd().arg("--help").assert().success().stdout(
        predicate::str::contains("start")
            .and(predicate::str::contains("stop"))
            .and(predicate::str::contains("purge"))
            .and(predicate::str::contains("status"))
            .and(predicate::str::contains("list-clis"))
            .and(predicate::str::contains("add-cli"))
            .and(predicate::str::contains("remove-cli"))
            .and(predicate::str::contains("init"))
            .and(predicate::str::contains("replay")),
    );
}

#[test]
fn help_contains_quick_start() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Quick Start"));
}

#[test]
fn start_help_shows_flags() {
    cmd().args(["start", "--help"]).assert().success().stdout(
        predicate::str::contains("--cli")
            .and(predicate::str::contains("--branches"))
            .and(predicate::str::contains("--dry-run"))
            .and(predicate::str::contains("--preset")),
    );
}

#[test]
fn purge_help_shows_force_flag() {
    cmd()
        .args(["purge", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--force"));
}

#[test]
fn add_cli_help_shows_arguments() {
    cmd().args(["add-cli", "--help"]).assert().success().stdout(
        predicate::str::contains("--display-name")
            .and(predicate::str::contains("<NAME>").or(predicate::str::contains("<name>")))
            .and(predicate::str::contains("<COMMAND>").or(predicate::str::contains("<command>"))),
    );
}

// ---------------------------------------------------------------------------
// Version
// ---------------------------------------------------------------------------

#[test]
fn version_output() {
    cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("git-paw"));
}

// ---------------------------------------------------------------------------
// Default subcommand
// ---------------------------------------------------------------------------

#[test]
fn no_args_behaves_like_start() {
    let tmp = TempDir::new().expect("create temp dir");

    // Both `git-paw` and `git-paw start` should produce the same error
    // when run outside a git repo.
    let no_args = cmd().current_dir(tmp.path()).assert().failure();
    let start = cmd()
        .current_dir(tmp.path())
        .arg("start")
        .assert()
        .failure();

    let no_args_stderr = String::from_utf8_lossy(&no_args.get_output().stderr);
    let start_stderr = String::from_utf8_lossy(&start.get_output().stderr);

    assert_eq!(no_args_stderr, start_stderr);
}

// ---------------------------------------------------------------------------
// Subcommand stubs respond without error
// ---------------------------------------------------------------------------

#[test]
fn stop_runs_without_error() {
    cmd().arg("stop").assert().success();
}

#[test]
fn status_runs_without_error() {
    cmd().arg("status").assert().success();
}

#[test]
fn list_clis_runs_without_error() {
    cmd().arg("list-clis").assert().success();
}

// ---------------------------------------------------------------------------
// Not-a-repo error
// ---------------------------------------------------------------------------

#[test]
fn start_from_non_git_dir() {
    let tmp = TempDir::new().expect("create temp dir");

    cmd()
        .current_dir(tmp.path())
        .arg("start")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

// ---------------------------------------------------------------------------
// Invalid usage
// ---------------------------------------------------------------------------

#[test]
fn unknown_subcommand_fails() {
    cmd()
        .arg("nonexistent")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

#[test]
fn add_cli_requires_arguments() {
    cmd()
        .arg("add-cli")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn remove_cli_requires_argument() {
    cmd()
        .arg("remove-cli")
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

// Supervisor CLI tests
//
// These tests exercise the real purge code path. Production reads the session
// file from the XDG data dir (`~/.local/share/git-paw/sessions/<name>.json` on
// Linux, `~/Library/Application Support/git-paw/sessions/<name>.json` on
// macOS) — *not* from `<repo>/.git-paw/`. The earlier versions of these tests
// wrote `<repo>/.git-paw/session.json`, which was never read by `cmd_purge`,
// so they only exercised the missing-session "nothing to do" branch and would
// pass even if the entire purge implementation were deleted.
//
// The fixed tests:
//   * point `HOME` at a temp dir so the binary's XDG data dir lands inside it,
//   * write a real session file with the production filename
//     (`<session_name>.json`) and the production schema (matching
//     `git_paw::session::Session`'s serde representation), and
//   * assert observable side effects (session file removed, `Purged`
//     reported, force flag bypasses prompts).

use std::path::Path;
use std::time::SystemTime;

use git_paw::session::{Session, SessionStatus, WorktreeEntry, save_session_in};

/// XDG data-dir layout used by the binary, mirroring `dirs::data_dir()`.
fn sessions_dir_under_home(home: &Path) -> std::path::PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-paw/sessions")
    } else {
        home.join(".local/share/git-paw/sessions")
    }
}

/// Writes a real session JSON via the production serializer to the XDG-style
/// sessions dir under `home`. Returns the resolved sessions dir.
fn write_real_session_for(home: &Path, session: &Session) -> std::path::PathBuf {
    let dir = sessions_dir_under_home(home);
    fs::create_dir_all(&dir).expect("create sessions dir");
    save_session_in(session, &dir).expect("save session");
    dir
}

/// Builds a Session pointing at `repo` with one worktree for `branch`. The
/// worktree is created on disk via real git so production purge can remove
/// it; the worktree path is returned alongside the session.
///
/// The session's `repo_path` is set to the canonical path returned by
/// `git rev-parse --show-toplevel` to match what production
/// `find_session_for_repo` compares against.
fn make_session_with_worktree(
    repo: &Path,
    branch: &str,
    extra_commit: bool,
) -> (Session, std::path::PathBuf) {
    let canonical_repo = git_paw::git::validate_repo(repo).expect("validate repo");
    let wt = git_paw::git::create_worktree(&canonical_repo, branch).expect("create worktree");
    if extra_commit {
        std::fs::write(wt.path.join("change.txt"), "x").unwrap();
        std::process::Command::new("git")
            .current_dir(&wt.path)
            .args(["add", "."])
            .output()
            .expect("git add");
        std::process::Command::new("git")
            .current_dir(&wt.path)
            .args(["commit", "-q", "-m", "feature work"])
            .output()
            .expect("git commit");
    }
    let session = Session {
        session_name: "paw-cli-purge".to_string(),
        repo_path: canonical_repo,
        project_name: "cli-purge".to_string(),
        created_at: SystemTime::UNIX_EPOCH,
        status: SessionStatus::Active,
        worktrees: vec![WorktreeEntry {
            branch: branch.to_string(),
            worktree_path: wt.path.clone(),
            cli: "echo".to_string(),
            branch_created: wt.branch_created,
        }],
        broker_port: None,
        broker_bind: None,
        broker_log_path: None,
    };
    (session, wt.path)
}

/// `purge --force` with a clean (no unmerged commits) session must succeed,
/// remove the session file, remove the worktree, and not warn about
/// unmerged work.
#[test]
fn test_purge_no_unmerged_runs_without_warning() {
    let tr = setup_test_repo();
    let home = TempDir::new().expect("home tempdir");
    let (session, worktree_path) =
        make_session_with_worktree(tr.path(), "feature/clean-purge", false);
    let sessions_dir = write_real_session_for(home.path(), &session);
    let session_file = sessions_dir.join(format!("{}.json", session.session_name));
    assert!(session_file.exists(), "precondition: session file written");

    let output = cmd()
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["purge", "--force"])
        .output()
        .expect("run purge");

    assert!(
        output.status.success(),
        "purge --force should succeed (stderr: {})",
        String::from_utf8_lossy(&output.stderr)
    );

    // Production purge consumed the session file.
    assert!(
        !session_file.exists(),
        "session file should be removed after purge"
    );
    // Stdout reports the actual purge.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("Purged session '{}'", session.session_name)),
        "stdout should report the purge: {stdout}"
    );
    // No unmerged warning when the worktree has no extra commits.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unmerged"),
        "should not warn about unmerged work; stderr: {stderr}"
    );
    // The worktree is gone too.
    assert!(
        !worktree_path.exists(),
        "worktree dir should be removed by purge"
    );
}

/// `purge --force` succeeds and removes the session file; this exercises the
/// non-interactive code path of `cmd_purge` end-to-end.
#[test]
fn test_purge_succeeds_with_force_flag() {
    let tr = setup_test_repo();
    let home = TempDir::new().expect("home tempdir");
    let (session, _wt) = make_session_with_worktree(tr.path(), "feature/force-flag", false);
    let sessions_dir = write_real_session_for(home.path(), &session);
    let session_file = sessions_dir.join(format!("{}.json", session.session_name));
    assert!(session_file.exists(), "precondition: session file written");

    let output = cmd()
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["purge", "--force"])
        .output()
        .expect("run purge");

    assert!(
        output.status.success(),
        "purge --force should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !session_file.exists(),
        "session file should be removed by purge --force"
    );
}

/// With `--force` and unmerged commits, purge still proceeds (no prompt) but
/// must emit the `Warning: ... unmerged commits` notice on stderr.
#[test]
fn test_purge_force_flag_works() {
    let tr = setup_test_repo();
    let home = TempDir::new().expect("home tempdir");
    let (session, _wt) = make_session_with_worktree(tr.path(), "feature/with-unmerged", true);
    let sessions_dir = write_real_session_for(home.path(), &session);
    let session_file = sessions_dir.join(format!("{}.json", session.session_name));

    let output = cmd()
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["purge", "--force"])
        .output()
        .expect("run purge");

    assert!(
        output.status.success(),
        "purge --force should succeed even with unmerged work; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !session_file.exists(),
        "session file should be removed by purge --force"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Warning:") && stderr.contains("unmerged"),
        "force mode must still emit unmerged warning; stderr: {stderr}"
    );
}

/// Without `--force`, purge prompts on the controlling terminal; the binary
/// is invoked with no tty here, so the dialoguer prompt errors out and the
/// session file is left in place. This verifies that purge does NOT delete
/// state when the user is not given a chance to confirm.
#[test]
fn test_purge_cancel_preserves_worktrees() {
    let tr = setup_test_repo();
    let home = TempDir::new().expect("home tempdir");
    let (session, worktree_path) = make_session_with_worktree(tr.path(), "feature/no-force", false);
    let sessions_dir = write_real_session_for(home.path(), &session);
    let session_file = sessions_dir.join(format!("{}.json", session.session_name));

    let output = cmd()
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .arg("purge") // no --force
        .output()
        .expect("run purge");

    // Without a tty and without --force, the prompt fails, so the binary
    // exits with an error and must NOT touch the session or worktree.
    assert!(
        !output.status.success(),
        "purge without --force in a non-interactive context should not succeed"
    );
    assert!(
        session_file.exists(),
        "session file must survive a cancelled purge"
    );
    assert!(
        worktree_path.exists(),
        "worktree must survive a cancelled purge"
    );
}
