//! Integration tests for the `no-supervisor-flag` change.
//!
//! Covers the spec scenario in `supervisor-cli`:
//!
//! > GIVEN a config with `[supervisor] enabled = true`
//! > WHEN `git paw start --no-supervisor --dry-run` is run
//! > THEN supervisor mode SHALL NOT be entered
//! > AND the dry-run plan SHALL reflect supervisor-disabled state

use std::fs;

use assert_cmd::Command;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// Writes a `.git-paw/config.toml` with `[supervisor] enabled = true` and a
/// custom `echo` CLI so detection succeeds in CI.
fn write_supervisor_enabled_config(repo: &std::path::Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = r#"
default_cli = "echo"

[supervisor]
enabled = true
cli = "echo"
test_command = "true"
agent_approval = "manual"

[clis.echo]
command = "echo"
display_name = "Echo"
"#;
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
}

/// `git paw start --no-supervisor --dry-run` SHALL produce the bare-start
/// dry-run plan (no `Supervisor:` / `Agent CLI:` headers) even when the
/// repo's `[supervisor] enabled = true` config would normally route to
/// supervisor mode.
#[test]
fn no_supervisor_overrides_config_enabled_in_dry_run() {
    let tr = setup_test_repo();
    write_supervisor_enabled_config(tr.path());

    let output = cmd()
        .current_dir(tr.path())
        .args([
            "start",
            "--no-supervisor",
            "--cli",
            "echo",
            "--branches",
            "feat/a",
            "--dry-run",
        ])
        .output()
        .expect("run start --no-supervisor --dry-run");

    assert!(
        output.status.success(),
        "dry-run should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Bare-start dry-run header is present.
    assert!(
        stdout.contains("Dry run \u{2014} session plan:"),
        "--no-supervisor should produce the bare-start dry-run header; got: {stdout}"
    );

    // Supervisor-mode dry-run markers MUST NOT appear.
    assert!(
        !stdout.contains("Supervisor:"),
        "--no-supervisor must skip supervisor mode; got: {stdout}"
    );
    assert!(
        !stdout.contains("Agent CLI:"),
        "--no-supervisor must skip supervisor mode; got: {stdout}"
    );
    assert!(
        !stdout.contains("Approval:"),
        "--no-supervisor must skip supervisor mode; got: {stdout}"
    );

    // The requested branch should still appear in the plan.
    assert!(
        stdout.contains("feat/a"),
        "dry-run should reference branch 'feat/a'; got: {stdout}"
    );
}

/// Sanity check the precondition: without `--no-supervisor`, the same repo
/// routes to supervisor mode (config-driven). This guards against the
/// negative test above silently passing because supervisor mode itself
/// stopped firing.
#[test]
fn supervisor_config_enabled_routes_to_supervisor_dry_run() {
    let tr = setup_test_repo();
    write_supervisor_enabled_config(tr.path());

    let output = cmd()
        .current_dir(tr.path())
        .args([
            "start",
            "--cli",
            "echo",
            "--branches",
            "feat/a",
            "--dry-run",
        ])
        .output()
        .expect("run start --dry-run");

    assert!(
        output.status.success(),
        "dry-run should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Supervisor:"),
        "config enabled should route to supervisor mode; got: {stdout}"
    );
}
