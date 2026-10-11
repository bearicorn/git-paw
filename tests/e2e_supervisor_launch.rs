//! E2E observable: `cmd_supervisor` launches the supervisor + dashboard +
//! coding agent panes.
//!
//! Maps to scenarios from supervisor-as-pane:
//!
//! - `Supervisor auto-start launches all panes including the supervisor pane`
//!   (task 12.6)
//! - `Supervisor registers itself on startup` (task 12.11, merged in)
//!
//! Plus, from boot-block:
//!
//! - `A launched supervisor session carries the governance section`
//! - `A launched supervisor session without governance paths is unchanged`
//!
//! Skips if tmux is unavailable.

use std::fs;
use std::process::Command as StdCommand;
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn tmux_available() -> bool {
    StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

#[test]
#[serial]
fn auto_start_launches_supervisor_and_agent_panes() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    // Broker disabled to keep the test fast. The pane count and indices
    // are independent of broker state — the supervisor layout always
    // emits one pane each for supervisor, dashboard, and N agents.
    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = r#"
default_cli = "echo"

[supervisor]
enabled = true
cli = "echo"
"#;
    fs::write(paw_dir.join("config.toml"), config).expect("write config");

    let home = TempDir::new().expect("home tempdir");
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--supervisor", "--branches", "a,b"])
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run start");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "supervisor start failed; stdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let session_name = stdout
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
        .expect("session name in stdout")
        .to_string();

    // List panes on the test-owned socket. The expected layout for 2
    // branches is 4 panes: 0 = supervisor, 1 = dashboard, 2 = agent a,
    // 3 = agent b.
    let list = StdCommand::new("tmux")
        .env("TMUX_TMPDIR", tmux_env.socket_dir())
        .args(["list-panes", "-t", &session_name, "-F", "#{pane_index}"])
        .output()
        .expect("tmux list-panes");
    let panes = String::from_utf8_lossy(&list.stdout).to_string();
    let indices: Vec<&str> = panes.lines().collect();

    // Clean up before asserting.
    let _ = StdCommand::new("tmux")
        .env("TMUX_TMPDIR", tmux_env.socket_dir())
        .args(["kill-session", "-t", &session_name])
        .status();

    assert!(
        list.status.success(),
        "tmux list-panes should succeed; panes:\n{panes}"
    );
    assert_eq!(
        indices.len(),
        4,
        "supervisor + dashboard + 2 agents = 4 panes; got: {indices:?}"
    );
    assert!(indices.contains(&"0"), "pane 0 (supervisor) missing");
    assert!(indices.contains(&"1"), "pane 1 (dashboard) missing");
    assert!(indices.contains(&"2"), "pane 2 (agent a) missing");
    assert!(indices.contains(&"3"), "pane 3 (agent b) missing");
}

/// Launches `git paw start --supervisor` with the given `.git-paw/config.toml`
/// body, kills the resulting tmux session, and returns the supervisor's boot
/// context as written at the repository root.
///
/// This is the observable the boot-block requirement names. The requirement
/// text calls it "the repository-root `AGENTS.md`"; git-paw writes that
/// combined view to the gitignored sidecar
/// [`git_paw::agents::SIDECAR_REL_PATH`] rather than the tracked file, so the
/// managed block never contaminates a commit. For the supervisor the worktree
/// root *is* the repository root, so the sidecar sits at
/// `<repo>/.git-paw/AGENTS.local.md` — that is the file pane 0's CLI reads.
fn supervisor_agents_md_after_launch(config: &str) -> String {
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(paw_dir.join("config.toml"), config).expect("write config");

    let home = TempDir::new().expect("home tempdir");
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--supervisor", "--branches", "a"])
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run start");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "supervisor start failed; stdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let agents_md = fs::read_to_string(tr.path().join(git_paw::agents::SIDECAR_REL_PATH))
        .expect("supervisor launch writes the repo-root AGENTS.md sidecar");

    if let Some(session_name) = stdout
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
    {
        let _ = StdCommand::new("tmux")
            .env("TMUX_TMPDIR", tmux_env.socket_dir())
            .args(["kill-session", "-t", session_name])
            .status();
    }

    agents_md
}

#[test]
#[serial]
fn launched_supervisor_boot_context_carries_the_governance_section() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let agents_md = supervisor_agents_md_after_launch(
        r#"
default_cli = "echo"

[supervisor]
enabled = true
cli = "echo"

[governance]
dod = "docs/dod.md"
"#,
    );

    // The supervisor skill mentions `## Governance documents` inline (inside
    // backticks) when describing what to look for; the injected section uses
    // the heading at start-of-line, so the `\n## ...\n` form distinguishes
    // "section actually inserted" from "prose names the heading".
    assert!(
        agents_md.contains("\n## Governance documents\n"),
        "launched supervisor AGENTS.md must carry the start-of-line governance heading, got:\n{agents_md}"
    );
    assert!(
        agents_md.contains("- dod: docs/dod.md"),
        "launched supervisor AGENTS.md must carry the configured dod bullet, got:\n{agents_md}"
    );

    let skill_pos = agents_md
        .find("\n## Supervisor Skills")
        .expect("Supervisor Skills heading must exist in the supervisor's boot context");
    let governance_pos = agents_md
        .find("\n## Governance documents\n")
        .expect("governance heading must exist when a path is configured");
    assert!(
        governance_pos > skill_pos,
        "governance section must follow the supervisor skill content, got skill@{skill_pos} governance@{governance_pos}"
    );

    // The requirement also positions the section before any per-agent task
    // content. The managed block renders `### Spec` / `### File Ownership`
    // ahead of the skill content, so the section can only satisfy that by the
    // supervisor carrying no task content at all — which is what makes the
    // supervisor's assignment different from a coding agent's. Pin it, so
    // giving the supervisor a spec later cannot silently invert the order.
    assert!(
        !agents_md.contains("\n### Spec\n") && !agents_md.contains("\n### File Ownership\n"),
        "the supervisor's boot context must carry no per-agent task content, or the \
         governance section would no longer precede it; got:\n{agents_md}"
    );
}

#[test]
#[serial]
fn launched_supervisor_without_governance_paths_has_no_governance_section() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let agents_md = supervisor_agents_md_after_launch(
        r#"
default_cli = "echo"

[supervisor]
enabled = true
cli = "echo"
"#,
    );

    assert!(
        !agents_md.contains("\n## Governance documents\n"),
        "an unconfigured [governance] table must leave the boot context without the section, got:\n{agents_md}"
    );
}
