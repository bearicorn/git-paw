//! Integration tests for the `git paw start --from-specs --force` flag.
//!
//! Verifies that the `--force` flag bypasses the uncommitted-spec warning
//! emitted by `cmd_start_from_specs` when the configured specs directory
//! has uncommitted spec changes.

use std::fs;

use assert_cmd::Command;

mod helpers;
use helpers::*;

/// Returns a fresh `git-paw` `Command`.
fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// Writes a `.git-paw/config.toml` with openspec specs at `specs/` and an
/// `echo` custom CLI so detection succeeds in test environments.
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

/// Writes a pending `OpenSpec` change at `<repo>/specs/<id>/tasks.md`. The id is
/// also the directory name, which is what `check_uncommitted_specs` consults.
fn write_pending_openspec(repo: &std::path::Path, id: &str, body: &str) {
    let change_dir = repo.join("specs").join(id);
    fs::create_dir_all(&change_dir).expect("create change dir");
    fs::write(change_dir.join("tasks.md"), body).expect("write tasks.md");
}

/// Sets up a repo with an *uncommitted* pending spec under `specs/<id>/`.
fn repo_with_uncommitted_spec() -> (TestRepo, &'static str) {
    let tr = setup_test_repo();
    write_specs_config(tr.path());
    let id = "uncommitted-spec";
    write_pending_openspec(tr.path(), id, "Implement the feature.");
    (tr, id)
}

// ---------------------------------------------------------------------------
// C12: --force bypasses the uncommitted-spec warning
// ---------------------------------------------------------------------------

#[test]
fn start_from_specs_without_force_emits_uncommitted_warning() {
    let (tr, id) = repo_with_uncommitted_spec();

    let output = cmd()
        .current_dir(tr.path())
        .args(["start", "--from-specs", "--cli", "echo", "--dry-run"])
        .output()
        .expect("run start --from-specs");

    assert!(
        output.status.success(),
        "dry run should succeed even without --force; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Uncommitted spec changes detected"),
        "stderr should contain the uncommitted-spec warning, got: {stderr}"
    );
    // Should NOT contain the --force acknowledgement when --force is absent.
    assert!(
        !stderr.contains("Proceeding with --force"),
        "stderr should not contain the --force ack, got: {stderr}"
    );

    // The dry-run plan must still be printed when only the warning fires.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Dry run \u{2014} session plan"),
        "stdout should print the dry-run plan, got: {stdout}"
    );
    assert!(
        stdout.contains(id),
        "stdout should reference the spec branch, got: {stdout}"
    );
}

#[test]
fn start_from_specs_with_force_bypasses_warning() {
    let (tr, id) = repo_with_uncommitted_spec();

    let output = cmd()
        .current_dir(tr.path())
        .args([
            "start",
            "--from-specs",
            "--cli",
            "echo",
            "--dry-run",
            "--force",
        ])
        .output()
        .expect("run start --from-specs --force");

    assert!(
        output.status.success(),
        "dry run with --force should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);

    // The "uncommitted spec changes detected" *warning* must not be shown
    // because --force suppresses it.
    assert!(
        !stderr.contains("Uncommitted spec changes detected"),
        "stderr should NOT contain the uncommitted warning when --force is set, got: {stderr}"
    );
    // Instead, the --force acknowledgement is emitted.
    assert!(
        stderr.contains("Proceeding with --force"),
        "stderr should acknowledge --force, got: {stderr}"
    );

    // Dry-run plan is printed and the spec branch shows up as expected.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Dry run \u{2014} session plan"),
        "stdout should print the dry-run plan, got: {stdout}"
    );
    assert!(
        stdout.contains(id),
        "stdout should reference the spec branch, got: {stdout}"
    );
}
