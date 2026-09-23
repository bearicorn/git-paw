//! Integration tests for the `from-specs-launch-fixes` change.
//!
//! Covers three v0.4 bugfixes surfaced during dogfood:
//!
//! - **D5** — `--from-specs --supervisor` dispatches to `cmd_supervisor`,
//!   not `cmd_start_from_specs`. Verified via `--dry-run` output shape.
//! - **D4** — bare `cmd_start_from_specs` with broker enabled actually
//!   injects a boot block. See the D4 section below for how this is
//!   verified now that `start-spec-launch-parity` moved the spec-driven
//!   path off `build_boot_inject_args` onto `submit_prompt_to_pane`.
//! - **D2** — non-TTY launches exit cleanly with an attach hint instead
//!   of erroring. `assert_cmd::Command::output()` runs commands in a
//!   non-TTY child process by default, so `git paw start --dry-run`
//!   continues to work, and a real (non-`--dry-run`) launch from a
//!   non-TTY context exits with the hint text.
//!
//! Tests use `--dry-run` for the dispatch shape assertions (no tmux/CLI
//! side effects).

use std::fs;

use assert_cmd::Command;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// Writes a `.git-paw/config.toml` with the specs backend, broker enabled,
/// supervisor enabled, and an `echo` CLI so detection succeeds in CI.
fn write_supervisor_specs_config(repo: &std::path::Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = r#"
default_cli = "echo"

[specs]
type = "openspec"
dir = "specs"

[broker]
enabled = true
port = 19119

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

/// Writes a `.git-paw/config.toml` with broker enabled but NO `[supervisor]`
/// section — for testing bare `--from-specs` behaviour.
fn write_specs_config_no_supervisor(repo: &std::path::Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = r#"
default_cli = "echo"

[specs]
type = "openspec"
dir = "specs"

[broker]
enabled = true
port = 19120

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

/// Sets up a repo with `[supervisor] enabled = true`, broker enabled, and a
/// committed pending spec.
fn repo_with_supervisor_and_spec() -> (TestRepo, &'static str) {
    let tr = setup_test_repo();
    write_supervisor_specs_config(tr.path());
    let id = "feature-x";
    write_committed_spec(tr.path(), id, "Implement feature x.");
    (tr, id)
}

/// Sets up a repo with broker enabled, NO supervisor config, and a
/// committed pending spec.
fn repo_with_specs_no_supervisor() -> (TestRepo, &'static str) {
    let tr = setup_test_repo();
    write_specs_config_no_supervisor(tr.path());
    let id = "feature-y";
    write_committed_spec(tr.path(), id, "Implement feature y.");
    (tr, id)
}

// -----------------------------------------------------------------------
// D5 — Dispatcher routing tests (task 4.6)
// -----------------------------------------------------------------------

/// `start --from-specs --supervisor --dry-run` SHALL print the supervisor-mode
/// dry-run header (`Supervisor: ...`, `Agent CLI: ...`), NOT the from-specs
/// dry-run header (`Dry run — session plan (from specs):` with a plain branch
/// list).
///
/// This is the integration-level proof that the dispatcher fix routes
/// `--from-specs --supervisor` to `cmd_supervisor`. The pure unit tests for
/// `resolve_dispatch_target` cover the in-process logic; this test verifies
/// the actual binary picks the right path.
#[test]
fn from_specs_with_supervisor_routes_to_supervisor_dry_run() {
    let (tr, id) = repo_with_supervisor_and_spec();

    let output = cmd()
        .current_dir(tr.path())
        .args([
            "start",
            "--from-specs",
            "--supervisor",
            "--cli",
            "echo",
            "--dry-run",
        ])
        .output()
        .expect("run start --from-specs --supervisor --dry-run");

    assert!(
        output.status.success(),
        "dry-run should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Supervisor-mode dry-run header markers.
    assert!(
        stdout.contains("Supervisor:"),
        "supervisor-mode dry-run should print 'Supervisor:'; got: {stdout}"
    );
    assert!(
        stdout.contains("Agent CLI:"),
        "supervisor-mode dry-run should print 'Agent CLI:'; got: {stdout}"
    );
    assert!(
        stdout.contains("Approval:"),
        "supervisor-mode dry-run should print 'Approval:'; got: {stdout}"
    );

    // The spec id should still appear (cmd_supervisor's spec-scanning fallback
    // discovers it).
    assert!(
        stdout.contains(id),
        "dry-run should reference the spec branch '{id}'; got: {stdout}"
    );
}

/// `start --from-specs` (no `--supervisor`, no `[supervisor]` config) SHALL
/// route to `cmd_start_from_specs` and produce the from-specs dry-run header,
/// NOT the supervisor-mode header.
#[test]
fn from_specs_without_supervisor_routes_to_start_from_specs_dry_run() {
    let (tr, id) = repo_with_specs_no_supervisor();

    let output = cmd()
        .current_dir(tr.path())
        .args(["start", "--from-specs", "--cli", "echo", "--dry-run"])
        .output()
        .expect("run start --from-specs --dry-run");

    assert!(
        output.status.success(),
        "dry-run should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // From-specs dry-run header marker.
    assert!(
        stdout.contains("Dry run \u{2014} session plan (from specs):"),
        "from-specs dry-run should print its specific header; got: {stdout}"
    );

    // Supervisor-mode markers should NOT appear.
    assert!(
        !stdout.contains("Supervisor:"),
        "from-specs-only dry-run should NOT print 'Supervisor:'; got: {stdout}"
    );
    assert!(
        !stdout.contains("Approval:"),
        "from-specs-only dry-run should NOT print 'Approval:'; got: {stdout}"
    );

    // The spec id should still appear.
    assert!(
        stdout.contains(id),
        "from-specs dry-run should reference the spec branch '{id}'; got: {stdout}"
    );
}

/// `start --from-specs` with `[supervisor] enabled = true` in config SHALL
/// route to supervisor mode (the resolution chain picks up the config) —
/// supervisor flag NOT explicitly set on the CLI.
#[test]
fn from_specs_with_supervisor_config_routes_to_supervisor_dry_run() {
    let (tr, id) = repo_with_supervisor_and_spec();

    let output = cmd()
        .current_dir(tr.path())
        .args(["start", "--from-specs", "--cli", "echo", "--dry-run"])
        .output()
        .expect("run start --from-specs --dry-run");

    assert!(
        output.status.success(),
        "dry-run should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Supervisor:"),
        "config-driven supervisor should produce supervisor dry-run; got: {stdout}"
    );
    assert!(
        stdout.contains(id),
        "supervisor dry-run from spec config should reference spec '{id}'; got: {stdout}"
    );
}

// -----------------------------------------------------------------------
// D4 — Boot-block injection argv consistency (tasks 5.1-5.4)
// -----------------------------------------------------------------------
//
// `cmd_start_from_specs` (now `cmd_start_with_specs`) no longer calls
// `build_boot_inject_args` at all — `start-spec-launch-parity` replaced it
// with `submit_prompt_to_pane` (literal send, settle delay, separate Enter)
// so the spec-driven prompt is actually submitted rather than left
// pre-fill-only. The pure argv-shape tests that used to live here asserted
// on a helper this path no longer calls, which would pass regardless of
// whether the real launch path worked at all — a false sense of coverage.
//
// The injection contract for the spec-driven path (per-agent boot+task
// prompt, N > 1 panes with the dashboard offset, non-fatal `tmux` failure)
// is now verified against the REAL launch path (a live tmux pane, not a
// pure helper) in `tests/start_spec_launch_parity_integration.rs`:
// `spec_driven_launch_injects_per_pane_for_multiple_specs` and
// `spec_driven_launch_survives_a_send_keys_failure`.
//
// `build_boot_inject_args`'s own argv shape (still used by bare `cmd_start`'s
// pre-fill path) remains covered by `tests/boot_block_integration.rs`'s
// `boot_inject_args_use_literal_flag_with_no_enter`.

// -----------------------------------------------------------------------
// D2 — Non-TTY launch handling (tasks 6.1-6.4)
// -----------------------------------------------------------------------
//
// `assert_cmd::Command::output()` runs the binary in a non-TTY child
// process by default (stdin/stdout/stderr are pipes). Real interactive
// launches go through `tmux::attach` and `Command::new(supervisor_cli).status()`
// which require a TTY. The non-TTY handling must short-circuit those.

/// Non-TTY `start --from-specs --supervisor` (without `--dry-run`) attempts
/// the real launch path. Without the D2 fix, it errored with "failed to
/// attach". With the fix, it should exit cleanly with the attach-hint.
///
/// We verify the dry-run path here because a real launch would actually
/// create worktrees and a tmux session inside the test sandbox. The dry-run
/// is sufficient to verify the dispatch and arg parsing; the live-launch
/// non-TTY path is exercised manually per task 9.7.
#[test]
fn from_specs_supervisor_dry_run_succeeds_in_non_tty() {
    // assert_cmd's `output()` is non-TTY by default. The dispatch should
    // still complete cleanly because dry-run short-circuits before the
    // launch's TTY-sensitive steps.
    let (tr, _) = repo_with_supervisor_and_spec();

    let output = cmd()
        .current_dir(tr.path())
        .args([
            "start",
            "--from-specs",
            "--supervisor",
            "--cli",
            "echo",
            "--dry-run",
        ])
        .output()
        .expect("run dry-run in non-TTY context");

    assert!(
        output.status.success(),
        "non-TTY dry-run should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("failed to attach"),
        "non-TTY dry-run must NOT report attach failure; got: {stderr}"
    );
    assert!(
        !stderr.contains("open terminal failed"),
        "non-TTY dry-run must NOT report tmux open-terminal failure; got: {stderr}"
    );
}

/// Non-TTY bare `start --from-specs` (with broker enabled, no supervisor).
/// Same expectations as the supervisor-mode dry-run: clean exit, no
/// attach-failure noise.
#[test]
fn from_specs_dry_run_succeeds_in_non_tty() {
    let (tr, _) = repo_with_specs_no_supervisor();

    let output = cmd()
        .current_dir(tr.path())
        .args(["start", "--from-specs", "--cli", "echo", "--dry-run"])
        .output()
        .expect("run dry-run in non-TTY context");

    assert!(
        output.status.success(),
        "non-TTY from-specs dry-run should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("failed to attach"),
        "non-TTY dry-run must NOT report attach failure; got: {stderr}"
    );
}
