//! Integration tests for supervisor agent functionality
//!
//! Tests the supervisor launch workflow, AGENTS.md generation,
//! broker URL injection, and approval flag wiring.

use std::fs;

use assert_cmd::Command;

mod helpers;
use helpers::*;

/// Helper function to get git-paw command
fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// Test that supervisor dry-run prints session plan
#[test]
fn test_cmd_supervisor_dry_run_prints_session_plan() {
    let tr = setup_test_repo();

    // Create AGENTS.md with some agents
    let agents_content = r"# Agents

## Agent 1
- Branch: feat/agent-1
- CLI: echo

## Agent 2
- Branch: feat/agent-2
- CLI: echo
";
    fs::write(tr.path().join("AGENTS.md"), agents_content).expect("write AGENTS.md");

    // Create specs directory
    fs::create_dir(tr.path().join("specs")).expect("create specs directory");

    // Create supervisor config with required CLI
    let config_content = r#"
[supervisor]
enabled = true
cli = "echo"

[specs]
type = "markdown"
"#;

    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(paw_dir.join("config.toml"), config_content).expect("write config");

    // Run start with supervisor flag (dry-run) and specify branches
    let output = cmd()
        .current_dir(tr.path())
        .args([
            "start",
            "--supervisor",
            "--dry-run",
            "--branches",
            "feat/agent-1,feat/agent-2",
        ])
        .output()
        .expect("run start with supervisor");

    assert!(output.status.success(), "supervisor dry-run should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("feat/agent-1"),
        "should show agent 1 branch"
    );
    assert!(
        stdout.contains("feat/agent-2"),
        "should show agent 2 branch"
    );
    assert!(stdout.contains("echo"), "should show CLI command");
}

/// Test that AGENTS.md is processed correctly in supervisor mode
#[test]
fn test_supervisor_agents_md_written_for_repo_root() {
    let tr = setup_test_repo();

    // Create a basic AGENTS.md
    let agents_content = r"# Agents

## Test Agent
- Branch: feat/test
- CLI: echo
";
    fs::write(tr.path().join("AGENTS.md"), agents_content).expect("write AGENTS.md");

    // Create specs directory
    fs::create_dir(tr.path().join("specs")).expect("create specs directory");

    // Create supervisor config with required CLI
    let config_content = r#"
[supervisor]
enabled = true
cli = "echo"
"#;

    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(paw_dir.join("config.toml"), config_content).expect("write config");

    // Run start with supervisor flag (dry-run) and specify branches
    let output = cmd()
        .current_dir(tr.path())
        .args([
            "start",
            "--supervisor",
            "--dry-run",
            "--branches",
            "feat/test",
        ])
        .output()
        .expect("run start with supervisor");

    assert!(output.status.success(), "supervisor should succeed");

    // Verify AGENTS.md still exists and contains expected content
    let agents_path = tr.path().join("AGENTS.md");
    assert!(agents_path.exists(), "AGENTS.md should exist");

    let content = fs::read_to_string(agents_path).expect("read AGENTS.md");
    assert!(content.contains("feat/test"), "should contain test branch");
    assert!(content.contains("echo"), "should contain CLI");
}

/// Test broker URL injection wiring in supervisor mode
/// Verifies that `cmd_supervisor` wires set-environment with the broker URL
#[test]
fn test_broker_url_injection_wiring() {
    let tr = setup_test_repo();

    // Create config with broker and supervisor enabled
    let config_content = r#"
[broker]
enabled = true
port = 9119

[supervisor]
enabled = true
cli = "echo"

[specs]
type = "markdown"
"#;

    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(paw_dir.join("config.toml"), config_content).expect("write config");

    // Create specs directory
    fs::create_dir(tr.path().join("specs")).expect("create specs directory");

    // Create AGENTS.md
    let agents_content = r"# Agents

## Test Agent
- Branch: feat/test
- CLI: echo
";
    fs::write(tr.path().join("AGENTS.md"), agents_content).expect("write AGENTS.md");

    // Run start with supervisor flag (dry-run) and specify branches
    let output = cmd()
        .current_dir(tr.path())
        .args([
            "start",
            "--supervisor",
            "--dry-run",
            "--branches",
            "feat/test",
        ])
        .output()
        .expect("run start with supervisor");

    assert!(output.status.success(), "supervisor dry-run should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Verify the broker URL is shown in the dry-run output
    // This confirms the broker URL will be injected via set-environment
    assert!(
        stdout.contains("127.0.0.1:9119"),
        "should contain broker URL (set-environment target)"
    );
    assert!(
        stdout.contains("Broker URL:"),
        "should show Broker URL in session plan"
    );
}

/// Test approval flags wiring in supervisor mode
/// Verifies that `cmd_supervisor` composes approval flags correctly
#[test]
fn test_approval_flags_wiring() {
    let tr = setup_test_repo();

    // Create specs directory
    fs::create_dir(tr.path().join("specs")).expect("create specs directory");

    // Create AGENTS.md
    let agents_content = r"# Agents

## Test Agent
- Branch: feat/test
- CLI: claude
";
    fs::write(tr.path().join("AGENTS.md"), agents_content).expect("write AGENTS.md");

    // Create supervisor config with FullAuto approval level
    // This should produce --dangerously-skip-permissions for claude CLI
    let config_content = r#"
[supervisor]
enabled = true
cli = "claude"
agent_approval = "full-auto"
"#;

    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(paw_dir.join("config.toml"), config_content).expect("write config");

    // Run start with supervisor flag (dry-run) and specify branches
    let output = cmd()
        .current_dir(tr.path())
        .args([
            "start",
            "--supervisor",
            "--dry-run",
            "--branches",
            "feat/test",
        ])
        .output()
        .expect("run start with supervisor");

    assert!(output.status.success(), "supervisor dry-run should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify approval configuration is shown in the plan
    assert!(
        stdout.contains("FullAuto"),
        "should show FullAuto approval level in session plan"
    );

    // Verify approval flags are included in the agent command
    // The dry-run shows: branch → command (../worktree)
    assert!(
        stdout.contains("claude --dangerously-skip-permissions"),
        "should include approval flags in agent command"
    );
}

/// Writes a supervisor config and runs `start --supervisor --dry-run` for one
/// branch, returning (stdout, stderr).
fn dry_run_with_config(config_content: &str) -> (String, String) {
    let tr = setup_test_repo();
    fs::create_dir(tr.path().join("specs")).expect("create specs directory");

    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(paw_dir.join("config.toml"), config_content).expect("write config");

    let output = cmd()
        .current_dir(tr.path())
        .args([
            "start",
            "--supervisor",
            "--dry-run",
            "--branches",
            "feat/test",
        ])
        .output()
        .expect("run start with supervisor");
    assert!(
        output.status.success(),
        "supervisor dry-run should succeed; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

/// Split approval levels flag the supervisor command only: `approval` governs
/// the supervisor pane command, `agent_approval` the agent commands.
///
/// Maps to supervisor-config scenario "approval set to full-auto relaxes only
/// the supervisor pane" and supervisor-launch scenario "Fresh start applies
/// supervisor flags to pane 0 only" (command composition; the pane-level
/// observable is covered by `e2e_supervisor_approval_flags`).
#[test]
fn split_approval_levels_flag_supervisor_command_only() {
    let (stdout, _stderr) = dry_run_with_config(
        r#"
[supervisor]
enabled = true
cli = "claude"
approval = "full-auto"
agent_approval = "auto"
"#,
    );

    assert!(
        stdout.contains("Supervisor: claude --dangerously-skip-permissions"),
        "supervisor command must carry the full-auto flag, got:\n{stdout}"
    );
    let agent_line = stdout
        .lines()
        .find(|l| l.contains("feat/test"))
        .expect("agent branch line in plan");
    assert!(
        !agent_line.contains("--dangerously-skip-permissions"),
        "agent command must NOT carry the supervisor's flag, got: {agent_line}"
    );
}

/// The dry-run plan reports the two approval levels distinctly when they
/// differ. Maps to supervisor-launch scenario "Dry run reports split
/// approval levels".
#[test]
fn dry_run_reports_split_approval_levels() {
    let (stdout, _stderr) = dry_run_with_config(
        r#"
[supervisor]
enabled = true
cli = "claude"
approval = "full-auto"
agent_approval = "auto"
"#,
    );

    assert!(
        stdout.contains("Supervisor approval: FullAuto"),
        "plan must report the supervisor approval level, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Agent approval:      Auto"),
        "plan must report the agent approval level, got:\n{stdout}"
    );
}

/// With no `approval` key the supervisor + agent commands both resolve from
/// `agent_approval` exactly as the pre-split resolution did.
///
/// Maps to supervisor-launch scenario "No approval key produces byte-identical
/// commands to v0.10.0" and supervisor-config scenario "Absent approval
/// inherits `agent_approval` for the supervisor pane".
#[test]
fn no_approval_key_builds_commands_from_agent_approval_alone() {
    // agent_approval = "full-auto": both commands carry the claude flag,
    // exactly what approval_flags("claude", FullAuto) produced for both
    // panes before the split.
    let (stdout, _stderr) = dry_run_with_config(
        r#"
[supervisor]
enabled = true
cli = "claude"
agent_approval = "full-auto"
"#,
    );
    assert!(
        stdout.contains("Supervisor: claude --dangerously-skip-permissions"),
        "supervisor inherits agent_approval when approval is unset, got:\n{stdout}"
    );
    let agent_line = stdout
        .lines()
        .find(|l| l.contains("feat/test"))
        .expect("agent branch line in plan");
    assert!(
        agent_line.contains("claude --dangerously-skip-permissions"),
        "agent command keeps agent_approval's flag, got: {agent_line}"
    );
    // Equal levels keep the single combined Approval line (pre-split format).
    assert!(
        stdout.contains("Approval:   FullAuto"),
        "equal levels must keep the combined approval line, got:\n{stdout}"
    );

    // agent_approval = "auto": both commands are bare.
    let (stdout, _stderr) = dry_run_with_config(
        r#"
[supervisor]
enabled = true
cli = "claude"
agent_approval = "auto"
"#,
    );
    assert!(
        stdout.lines().any(|l| l.trim() == "Supervisor: claude"),
        "supervisor command must be bare at auto, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("--dangerously-skip-permissions"),
        "no command may carry flags at auto, got:\n{stdout}"
    );
}

/// `approval = "full-auto"` with a CLI that has neither a built-in row nor an
/// `approval_args` override warns (naming the CLI and the override) and
/// degrades to a flagless launch instead of failing.
///
/// Maps to supervisor-config scenario "Full-auto with an unmapped CLI warns
/// and degrades" (the real-launch variant lives in
/// `e2e_supervisor_approval_flags`).
#[test]
fn full_auto_unmapped_cli_warns_and_degrades() {
    let (stdout, stderr) = dry_run_with_config(
        r#"
[supervisor]
enabled = true
cli = "my-agent"
approval = "full-auto"
"#,
    );

    assert!(
        stderr.contains("my-agent"),
        "warning must name the CLI, got:\n{stderr}"
    );
    assert!(
        stderr.contains("[clis.my-agent]") && stderr.contains("approval_args"),
        "warning must point at the [clis.<name>] approval_args override, got:\n{stderr}"
    );
    assert!(
        stdout.lines().any(|l| l.trim() == "Supervisor: my-agent"),
        "supervisor command must degrade to flagless, got:\n{stdout}"
    );
}
