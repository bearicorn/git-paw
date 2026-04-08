//! CLI binary tests.
//!
//! Tests the `git-paw` binary's argument parsing, help output, and version
//! output using `assert_cmd`.

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

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
