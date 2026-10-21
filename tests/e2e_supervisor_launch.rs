//! E2E observable: `cmd_supervisor` launches the dashboard + coding agent
//! panes, and no orchestrator pane.
//!
//! Maps to scenarios from supervisor-launch and session-state (see each
//! test), plus, from boot-block:
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

/// What one supervised launch left behind, captured before its tmux session
/// is killed.
struct Launch {
    status_ok: bool,
    stdout: String,
    stderr: String,
    /// `pane_index|pane_current_path|@paw_role` per pane.
    panes: Vec<(String, String, String)>,
    /// The per-repo discovery file's contents.
    discovery: Option<serde_json::Value>,
    /// The saved session state's contents.
    session_state: Option<serde_json::Value>,
    repo_root: std::path::PathBuf,
}

/// Finds the first file named `name` anywhere under `dir`.
fn find_file(dir: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, name) {
                return Some(found);
            }
        } else if path.file_name().is_some_and(|n| n == name) {
            return Some(path);
        }
    }
    None
}

/// Launches `git paw start --supervisor <args>` with `config` as the repo's
/// `.git-paw/config.toml`, records the panes, discovery file and saved
/// session, then kills the session.
fn launch_supervised(config: &str, args: &[&str]) -> Launch {
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
        .args(["start", "--supervisor"])
        .args(args)
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run start");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    let mut panes = Vec::new();
    let mut discovery = None;
    let mut session_state = None;
    if let Some(session_name) = stdout
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
    {
        let list = StdCommand::new("tmux")
            .env("TMUX_TMPDIR", tmux_env.socket_dir())
            .args([
                "list-panes",
                "-t",
                session_name,
                "-F",
                "#{pane_index}|#{pane_current_path}|#{@paw_role}",
            ])
            .output()
            .expect("tmux list-panes");
        panes = String::from_utf8_lossy(&list.stdout)
            .lines()
            .filter_map(|l| {
                let mut parts = l.splitn(3, '|');
                let index = parts.next()?.to_string();
                // Canonicalised while the temp repo still exists.
                let path = std::path::Path::new(parts.next()?)
                    .canonicalize()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                Some((index, path, parts.next()?.to_string()))
            })
            .collect();
        let _ = StdCommand::new("tmux")
            .env("TMUX_TMPDIR", tmux_env.socket_dir())
            .args(["kill-session", "-t", session_name])
            .status();
        let file = format!("{session_name}.json");
        discovery = fs::read_to_string(paw_dir.join("sessions").join(&file))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());
        session_state = find_file(home.path(), &file)
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok());
    }

    Launch {
        status_ok: out.status.success(),
        stdout,
        stderr,
        panes,
        discovery,
        session_state,
        repo_root: tr.path().canonicalize().expect("canonical repo root"),
    }
}

/// Scenarios "Supervisor auto-start launches the dashboard and coding panes
/// only", "Supervised start records the dashboard at pane 0", "A
/// broker-less supervised start still records the dashboard", "Pane 0
/// title is dashboard" and "Launch output names the sidecar": a two-branch
/// launch builds exactly the dashboard at pane 0 (repo root) and the agents
/// at panes 1 and 2 (their own worktrees), records `dashboard_pane = 0`,
/// writes a roster from pane 1 with no orchestrator entry, and prints the
/// orchestrator-context path with the attach hint.
#[test]
#[serial]
fn auto_start_launches_the_dashboard_and_agent_panes_only() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    // Broker disabled: a broker-less supervised start still builds and
    // records the dashboard at pane 0.
    let launch = launch_supervised(
        r#"
default_cli = "echo"

[supervisor]
enabled = true
cli = "echo"
"#,
        &["--branches", "a,b"],
    );
    assert!(
        launch.status_ok,
        "supervisor start failed; stdout:\n{}\nstderr:\n{}",
        launch.stdout, launch.stderr
    );

    let indices: Vec<&str> = launch.panes.iter().map(|(i, _, _)| i.as_str()).collect();
    assert_eq!(indices, vec!["0", "1", "2"], "dashboard + 2 agents");
    let (_, dash_path, dash_role) = &launch.panes[0];
    assert_eq!(dash_role, "dashboard", "pane 0 is the dashboard");
    let root = launch.repo_root.display().to_string();
    assert_eq!(dash_path, &root, "the dashboard runs at the repo root");
    for (index, path, role) in &launch.panes[1..] {
        assert!(
            !path.is_empty() && path != &root,
            "agent pane {index} ({role}) must run in its worktree, not the repo root; got {path:?}"
        );
    }

    let discovery = launch.discovery.expect("discovery file written");
    let pane_indices: Vec<u64> = discovery["agents"]
        .as_array()
        .expect("agents roster")
        .iter()
        .map(|a| a["pane_index"].as_u64().expect("pane_index"))
        .collect();
    assert_eq!(pane_indices, vec![1, 2], "the roster starts at pane 1");
    assert!(
        discovery.get("orchestrator").is_none(),
        "no orchestrator entry is written: {discovery}"
    );

    let state = launch.session_state.expect("session state saved");
    assert_eq!(state["dashboard_pane"], 0, "dashboard recorded at pane 0");

    assert!(
        launch.stdout.contains(".git-paw/AGENTS.local.md"),
        "the launch names the orchestrator context; stdout:\n{}",
        launch.stdout
    );
    assert!(launch.stdout.contains("tmux attach -t paw-"));
}

/// Scenario "Launch does not require an orchestrator CLI": with no
/// `[supervisor].cli` and no `default_cli`, `--cli` alone launches the
/// session and every coding agent runs it.
#[test]
#[serial]
fn launch_does_not_require_an_orchestrator_cli() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let launch = launch_supervised(
        "[supervisor]\nenabled = true\n",
        &["--cli", "echo", "--branches", "a,b"],
    );
    assert!(
        launch.status_ok,
        "launch must not need an orchestrator CLI; stderr:\n{}",
        launch.stderr
    );
    let discovery = launch.discovery.expect("discovery file written");
    for agent in discovery["agents"].as_array().expect("agents roster") {
        assert_eq!(agent["cli"], "echo", "each agent runs --cli: {agent}");
    }
}

/// Scenario "A configured orchestrator CLI still serves as the agents' last
/// fallback": `[supervisor].cli` with no `default_cli` and no `--cli` is the
/// CLI every coding agent runs.
#[test]
#[serial]
fn supervisor_cli_is_the_agents_last_fallback() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let launch = launch_supervised(
        "[supervisor]\nenabled = true\ncli = \"echo\"\n",
        &["--branches", "a,b"],
    );
    assert!(launch.status_ok, "stderr:\n{}", launch.stderr);
    let discovery = launch.discovery.expect("discovery file written");
    for agent in discovery["agents"].as_array().expect("agents roster") {
        assert_eq!(
            agent["cli"], "echo",
            "each agent falls back to [supervisor].cli"
        );
    }
}

/// Launches `git paw start --supervisor` with the given `.git-paw/config.toml`
/// body, kills the resulting tmux session, and returns the orchestrator
/// context as written at the repository root.
///
/// This is the observable the boot-block requirement names: the gitignored
/// repo-root sidecar [`git_paw::agents::SIDECAR_REL_PATH`]
/// (`<repo>/.git-paw/AGENTS.local.md`), which the orchestrator — the
/// operator's own agent session outside tmux — reads. No pane reads it.
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
