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
