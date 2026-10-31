//! `cli-provider` scenarios observed through `git paw start --dry-run` and
//! `git paw doctor`: the plan shows the real pane command, and an ambiguous
//! PATH shim is refused by the dry run and flagged by doctor. No tmux needed.

use std::fs;
use std::path::Path;

use assert_cmd::Command;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn write_config(repo: &Path, body: &str) {
    let paw = repo.join(".git-paw");
    fs::create_dir_all(&paw).expect("create .git-paw");
    fs::write(paw.join("config.toml"), body).expect("write config");
}

fn dry_run(repo: &Path, cli: &str) -> std::process::Output {
    let home = tempfile::TempDir::new().expect("home tempdir");
    cmd()
        .current_dir(repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--cli", cli, "--branches", "feat-a", "--dry-run"])
        .output()
        .expect("run start --dry-run")
}

/// Scenario "A dry run shows the real launch command".
#[test]
fn a_dry_run_shows_the_command_and_args_the_pane_would_run() {
    let tr = setup_test_repo();
    write_config(
        tr.path(),
        "[clis.sonnet]\ncommand = \"echo\"\nfamily = \"claude\"\nargs = \"--model sonnet\"\n",
    );

    let out = dry_run(tr.path(), "sonnet");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("runs: echo --model sonnet"),
        "plan should show the pane command; got:\n{stdout}"
    );
}

/// Scenarios "An ambiguous shim is refused" (dry run) and "Doctor flags an
/// ambiguous shim". `git` is always on PATH and is not a known agent CLI, so
/// a `[clis.git]` entry launching something else is ambiguous.
#[test]
fn an_ambiguous_shim_fails_the_dry_run_and_doctor() {
    let tr = setup_test_repo();
    write_config(tr.path(), "[clis.git]\ncommand = \"echo\"\n");

    let out = dry_run(tr.path(), "git");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "the dry run must refuse");
    assert!(stderr.contains("[clis.git] is ambiguous"), "{stderr}");

    let home = tempfile::TempDir::new().expect("home tempdir");
    let doctor = cmd()
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["doctor", "--json"])
        .output()
        .expect("run doctor");
    let report = String::from_utf8_lossy(&doctor.stdout);
    let json: serde_json::Value = serde_json::from_str(&report).expect("doctor json");
    let row = json["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|c| c["name"] == "[clis.git]")
        .unwrap_or_else(|| panic!("no [clis.git] row in:\n{report}"));
    assert_eq!(row["status"], "fail", "{row}");
    assert!(
        row["remedy"]
            .as_str()
            .unwrap_or_default()
            .contains("rename the [clis.git] key"),
        "{row}"
    );
}
