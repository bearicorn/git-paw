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
    let home = tempfile::TempDir::new().expect("home tempdir");
    let output = cmd()
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
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
    let home = tempfile::TempDir::new().expect("home tempdir");
    let output = cmd()
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
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
    let home = tempfile::TempDir::new().expect("home tempdir");
    let output = cmd()
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
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
    let home = tempfile::TempDir::new().expect("home tempdir");
    let output = cmd()
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
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
    dry_run_with_config_and(config_content, &[])
}

/// [`dry_run_with_config`] with `extra` flags appended to the command line.
fn dry_run_with_config_and(config_content: &str, extra: &[&str]) -> (String, String) {
    let tr = setup_test_repo();
    fs::create_dir(tr.path().join("specs")).expect("create specs directory");

    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(paw_dir.join("config.toml"), config_content).expect("write config");

    let home = tempfile::TempDir::new().expect("home tempdir");
    let output = cmd()
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args([
            "start",
            "--supervisor",
            "--dry-run",
            "--branches",
            "feat/test",
        ])
        .args(extra)
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

/// The `--dry-run` plan reports the agent approval level only: no pane runs
/// an orchestrator, so `[supervisor].approval` has no command to shape and
/// the plan lists no supervisor command.
///
/// Maps to supervisor-launch's REMOVED *Supervisor pane approval flags*
/// migration ("`--dry-run` reports the agent approval level only") and
/// supervisor-config scenario "approval does not change any pane command"
/// (command composition; the pane-level observable is covered by
/// `e2e_supervisor_approval_flags`).
#[test]
fn dry_run_reports_the_agent_approval_level_only() {
    let (stdout, stderr) = dry_run_with_config(
        r#"
[supervisor]
enabled = true
cli = "claude"
approval = "full-auto"
agent_approval = "auto"
"#,
    );

    assert!(
        stdout.contains("Approval:   Auto"),
        "the plan reports agent_approval, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("Hand-off:"),
        "only an unattended plan reports the headless hand-off, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("Supervisor:") && !stdout.contains("Supervisor approval"),
        "the plan lists no orchestrator command or level, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("--dangerously-skip-permissions"),
        "no command carries approval's full-auto flag, got:\n{stdout}"
    );
    assert!(
        !stderr.contains("warning"),
        "setting approval produces no warning, got:\n{stderr}"
    );
}

/// The plan's agent commands carry `agent_approval`'s flags. Maps to
/// supervisor-config scenario "Coding agents resolve from `agent_approval`".
#[test]
fn dry_run_agent_commands_resolve_from_agent_approval() {
    let (stdout, _stderr) = dry_run_with_config(
        r#"
[supervisor]
enabled = true
cli = "claude"
approval = "manual"
agent_approval = "full-auto"
"#,
    );
    let agent_line = stdout
        .lines()
        .find(|l| l.contains("feat/test"))
        .expect("agent branch line in plan");
    assert!(
        agent_line.contains("claude --dangerously-skip-permissions"),
        "agent command carries agent_approval's flag, got: {agent_line}"
    );
}

/// Scenario "approval does not change any pane command": the planned pane
/// commands are byte-identical with and without `[supervisor].approval`.
#[test]
fn approval_key_leaves_every_planned_command_byte_identical() {
    // Compare the whole plan — every command and level it lists — except
    // what differs per temp repo: the session name and each
    // `<branch> → <command> (<worktree>)` line's worktree.
    let commands = |stdout: &str| -> Vec<String> {
        stdout
            .lines()
            .filter(|l| !l.trim_start().starts_with("Session:"))
            .map(|l| {
                if l.contains('\u{2192}') {
                    l.rsplit_once(" (").map_or(l, |(cmd, _)| cmd).to_string()
                } else {
                    l.to_string()
                }
            })
            .collect()
    };
    let (with_key, _) = dry_run_with_config(
        "[supervisor]\nenabled = true\ncli = \"claude\"\nagent_approval = \"auto\"\napproval = \"full-auto\"\n",
    );
    let (without_key, _) = dry_run_with_config(
        "[supervisor]\nenabled = true\ncli = \"claude\"\nagent_approval = \"auto\"\n",
    );
    assert!(
        !commands(&with_key).is_empty(),
        "plan lists commands:\n{with_key}"
    );
    assert_eq!(commands(&with_key), commands(&without_key));
}

/// `orchestrator-escalation-handoff` task 2.3: an unattended dry run reports
/// the resolved headless hand-off form, or why none resolved.
#[test]
fn unattended_dry_run_reports_the_headless_hand_off() {
    let (stdout, _) = dry_run_with_config_and(
        "[supervisor]\nenabled = true\ncli = \"claude\"\n",
        &["--unattended"],
    );
    assert!(
        stdout.contains("Hand-off:   headless hand-off: claude -p"),
        "got:\n{stdout}"
    );

    let (stdout, _) = dry_run_with_config_and(
        "[supervisor]\nenabled = true\ncli = \"my-agent\"\n\n[clis.my-agent]\ncommand = \"my-agent\"\n",
        &["--unattended"],
    );
    assert!(
        stdout.contains("`my-agent` has no headless form")
            && stdout.contains("[clis.my-agent].headless_args"),
        "got:\n{stdout}"
    );
}
