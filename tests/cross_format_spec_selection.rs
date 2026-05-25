//! Integration tests for the `cross-format-spec-selection` change.
//!
//! Exercises the new `--from-all-specs` flag, the hidden `--from-specs`
//! alias, the `--specs NAME[,NAME...]` narrow form, and the bare `--specs`
//! picker via the production binary using `assert_cmd`. All tests use
//! `--dry-run` to avoid creating tmux sessions or worktrees.

use std::fs;

use assert_cmd::Command;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// Writes a `.git-paw/config.toml` with the `OpenSpec` backend and an `echo`
/// CLI so CLI detection succeeds in CI.
fn write_specs_config(repo: &std::path::Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = r#"
default_cli = "echo"

[specs]
type = "openspec"
dir = "specs"

[clis.echo]
command = "echo"
display_name = "Echo"
"#;
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
}

/// Writes and commits a pending `OpenSpec` change at `<repo>/specs/<id>/tasks.md`.
fn write_committed_spec(repo: &std::path::Path, id: &str, body: &str) {
    let change_dir = repo.join("specs").join(id);
    fs::create_dir_all(&change_dir).expect("create change dir");
    fs::write(change_dir.join("tasks.md"), body).expect("write tasks.md");

    std::process::Command::new("git")
        .current_dir(repo)
        .args(["add", "."])
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .current_dir(repo)
        .args(["commit", "-m", "add spec"])
        .output()
        .expect("git commit");
}

fn repo_with_three_specs() -> TestRepo {
    let tr = setup_test_repo();
    write_specs_config(tr.path());
    write_committed_spec(tr.path(), "add-auth", "Implement auth.");
    write_committed_spec(tr.path(), "fix-session", "Fix session bug.");
    write_committed_spec(tr.path(), "add-logging", "Add logging.");
    tr
}

// ---------------------------------------------------------------------------
// --from-all-specs (canonical)
// ---------------------------------------------------------------------------

#[test]
fn from_all_specs_dry_run_lists_every_discovered_spec() {
    let tr = repo_with_three_specs();
    let output = cmd()
        .current_dir(tr.path())
        .args(["start", "--from-all-specs", "--cli", "echo", "--dry-run"])
        .output()
        .expect("run start --from-all-specs --dry-run");

    assert!(
        output.status.success(),
        "dry-run should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("add-auth"), "got: {stdout}");
    assert!(stdout.contains("fix-session"), "got: {stdout}");
    assert!(stdout.contains("add-logging"), "got: {stdout}");
}

// ---------------------------------------------------------------------------
// --from-specs (hidden alias)
// ---------------------------------------------------------------------------

#[test]
fn from_specs_alias_produces_identical_dry_run_plan_to_canonical() {
    let tr_canon = repo_with_three_specs();
    let canonical = cmd()
        .current_dir(tr_canon.path())
        .args(["start", "--from-all-specs", "--cli", "echo", "--dry-run"])
        .output()
        .expect("canonical dry-run");

    let tr_alias = repo_with_three_specs();
    let alias = cmd()
        .current_dir(tr_alias.path())
        .args(["start", "--from-specs", "--cli", "echo", "--dry-run"])
        .output()
        .expect("alias dry-run");

    assert!(canonical.status.success(), "canonical should succeed");
    assert!(alias.status.success(), "alias should succeed");

    let canon_stdout = String::from_utf8_lossy(&canonical.stdout);
    let alias_stdout = String::from_utf8_lossy(&alias.stdout);
    for id in ["add-auth", "fix-session", "add-logging"] {
        assert!(canon_stdout.contains(id), "canonical missing {id}");
        assert!(alias_stdout.contains(id), "alias missing {id}");
    }
    assert!(canon_stdout.contains("Dry run"));
    assert!(alias_stdout.contains("Dry run"));
}

/// Verifies the hidden alias emits no deprecation warning on stderr — the
/// migration nudge is via release notes and absent help text only. Mirrors
/// the spec scenario "--from-specs emits no stderr warning".
#[test]
fn from_specs_alias_emits_no_deprecation_warning_on_stderr() {
    let tr = repo_with_three_specs();
    let output = cmd()
        .current_dir(tr.path())
        .args(["start", "--from-specs", "--cli", "echo", "--dry-run"])
        .output()
        .expect("alias dry-run");

    assert!(output.status.success(), "alias should succeed");
    let stderr = String::from_utf8_lossy(&output.stderr);
    // The alias is silent: no "deprecated", "renamed", or "removed" text.
    let lower = stderr.to_lowercase();
    assert!(
        !lower.contains("deprecat"),
        "stderr should not warn about deprecation; got: {stderr}"
    );
    assert!(
        !lower.contains("renamed to"),
        "stderr should not warn about rename; got: {stderr}"
    );
    assert!(
        !lower.contains("from-specs"),
        "stderr should not echo the alias name as a warning; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// --specs NAME[,NAME...] (narrow)
// ---------------------------------------------------------------------------

#[test]
fn specs_single_name_narrows_dry_run_to_one_spec() {
    let tr = repo_with_three_specs();
    let output = cmd()
        .current_dir(tr.path())
        .args(["start", "--specs", "add-auth", "--cli", "echo", "--dry-run"])
        .output()
        .expect("run start --specs add-auth --dry-run");

    assert!(
        output.status.success(),
        "dry-run should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("add-auth"), "got: {stdout}");
    assert!(
        !stdout.contains("fix-session"),
        "narrow should exclude fix-session; got: {stdout}"
    );
    assert!(
        !stdout.contains("add-logging"),
        "narrow should exclude add-logging; got: {stdout}"
    );
}

#[test]
fn specs_comma_separated_narrows_dry_run_to_listed_specs() {
    let tr = repo_with_three_specs();
    let output = cmd()
        .current_dir(tr.path())
        .args([
            "start",
            "--specs",
            "add-auth,fix-session",
            "--cli",
            "echo",
            "--dry-run",
        ])
        .output()
        .expect("run start --specs add-auth,fix-session --dry-run");

    assert!(
        output.status.success(),
        "dry-run should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("add-auth"), "got: {stdout}");
    assert!(stdout.contains("fix-session"), "got: {stdout}");
    assert!(
        !stdout.contains("add-logging"),
        "narrow should exclude add-logging; got: {stdout}"
    );
}

#[test]
fn specs_unknown_name_errors_with_candidates_listed() {
    let tr = repo_with_three_specs();
    let output = cmd()
        .current_dir(tr.path())
        .args([
            "start",
            "--specs",
            "no-such-spec",
            "--cli",
            "echo",
            "--dry-run",
        ])
        .output()
        .expect("run start --specs no-such-spec --dry-run");

    assert!(
        !output.status.success(),
        "unknown spec should fail; stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no-such-spec"),
        "stderr should name unresolved spec; got: {stderr}"
    );
    assert!(
        stderr.contains("add-auth"),
        "stderr should list candidates; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Bare --specs (picker — non-TTY guard)
// ---------------------------------------------------------------------------

#[test]
fn bare_specs_in_non_tty_environment_exits_with_actionable_error() {
    let tr = repo_with_three_specs();
    let output = cmd()
        .current_dir(tr.path())
        .args(["start", "--specs", "--cli", "echo", "--dry-run"])
        .output()
        .expect("run bare --specs in non-TTY env");

    assert!(
        !output.status.success(),
        "bare --specs without TTY should fail; stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--specs NAME"),
        "stderr should mention --specs NAME form; got: {stderr}"
    );
    assert!(
        stderr.contains("--from-all-specs"),
        "stderr should mention --from-all-specs; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Mutual exclusion (clap-enforced)
// ---------------------------------------------------------------------------

#[test]
fn from_all_specs_and_specs_together_are_rejected_at_parse_time() {
    let tr = setup_test_repo();
    let output = cmd()
        .current_dir(tr.path())
        .args([
            "start",
            "--from-all-specs",
            "--specs",
            "add-auth",
            "--cli",
            "echo",
            "--dry-run",
        ])
        .output()
        .expect("run mutex combination");

    assert!(
        !output.status.success(),
        "should reject both flags together"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--from-all-specs"),
        "stderr should mention --from-all-specs; got: {stderr}"
    );
    assert!(
        stderr.contains("--specs"),
        "stderr should mention --specs; got: {stderr}"
    );
}
