//! E2E observable: `[supervisor].approval` shapes no pane command — no pane
//! runs an orchestrator — while coding-agent panes keep resolving their flags
//! from `agent_approval`; and supervised recovery rebuilds the dashboard plus
//! the agents, whatever layout the stopped session was saved with.
//!
//! Maps to supervisor-config scenarios `approval does not change any pane
//! command`, `Coding agents resolve from agent_approval` and `Pre-relocation
//! config with approval still loads`, and to session-state scenarios
//! `Recovery of a pre-relocation session rebuilds the new layout` and
//! supervisor-launch `Supervised recovery applies the same fallback`.
//!
//! The tests use `echo` as the CLI with a sentinel flag supplied through the
//! `[clis.echo] approval_args` override, so a pane's scrollback shows the
//! exact command the pane was launched with.
//!
//! Skips if tmux is unavailable.

use std::fs;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

/// Sentinel flag mapped to `full-auto` via the `[clis.echo]` override; unique
/// enough that it can only appear in a pane through a full-auto command.
const SENTINEL_FLAG: &str = "--paw-full-auto-sentinel";

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// Atomic counter so each test gets a unique tmux project name.
static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Returns a project name unique to this run.
fn unique_project_name(tag: &str) -> String {
    let pid = std::process::id();
    let n = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("approval-{tag}-{pid}-{n}")
}

fn skip_if_no_tmux() -> bool {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return true;
    }
    false
}

/// Renames the test repo's basename so `tmux::resolve_session_name` produces
/// a deterministic, unique session name we can target.
fn rename_repo_basename(tr: &TestRepo, new_basename: &str) -> std::path::PathBuf {
    let original = tr.path().to_path_buf();
    let parent = original.parent().expect("repo has parent").to_path_buf();
    let renamed = parent.join(new_basename);
    fs::rename(&original, &renamed).expect("rename repo dir");
    renamed
}

/// Captures `pane` of `session` repeatedly until `needle` appears or the
/// timeout elapses. Returns the final capture (which may lack the needle —
/// callers assert).
fn capture_pane_until(env: &TmuxTestEnv, session: &str, pane: usize, needle: &str) -> String {
    let target = format!("{session}:0.{pane}");
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut buffer = String::new();
    while Instant::now() < deadline {
        let mut capture_cmd = StdCommand::new("tmux");
        env.apply(&mut capture_cmd);
        if let Ok(out) = capture_cmd
            .args(["capture-pane", "-t", &target, "-p", "-S", "-2000"])
            .output()
            && out.status.success()
        {
            buffer = String::from_utf8_lossy(&out.stdout).to_string();
            if buffer.contains(needle) {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    buffer
}

fn kill_session(env: &TmuxTestEnv, name: &str) {
    let mut kill_cmd = StdCommand::new("tmux");
    env.apply(&mut kill_cmd);
    let _ = kill_cmd.args(["kill-session", "-t", name]).status();
}

/// `pane_index|@paw_role` for every pane of `session`.
fn pane_roles(env: &TmuxTestEnv, session: &str) -> Vec<String> {
    let mut list = StdCommand::new("tmux");
    env.apply(&mut list);
    let out = list
        .args([
            "list-panes",
            "-t",
            session,
            "-F",
            "#{pane_index}|#{@paw_role}",
        ])
        .output()
        .expect("tmux list-panes");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(ToString::to_string)
        .collect()
}

/// Launches `git paw start --supervisor --branches a,b` with `config` and
/// returns `(session name, stdout, stderr)`; the caller kills the session.
fn launch(tr: &TestRepo, env: &TmuxTestEnv, config: &str) -> (Option<String>, String, String) {
    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(paw_dir.join("config.toml"), config).expect("write config");

    let home = TempDir::new().expect("home tempdir");
    let mut start = cmd();
    env.apply_assert(&mut start);
    let out = start
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--supervisor", "--branches", "a,b"])
        .timeout(Duration::from_secs(15))
        .output()
        .expect("run start");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        out.status.success(),
        "supervisor start failed; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let session = stdout
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
        .map(ToString::to_string);
    (session, stdout, stderr)
}

/// Scenario "approval does not change any pane command": `approval =
/// "full-auto"` with `agent_approval = "auto"` puts the full-auto flag on no
/// pane — the dashboard and both agents launch without it.
#[test]
#[serial]
fn approval_changes_no_pane_command() {
    if skip_if_no_tmux() {
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    let (session, _stdout, _stderr) = launch(
        &tr,
        &tmux_env,
        &format!(
            r#"
[supervisor]
enabled = true
cli = "echo"
approval = "full-auto"
agent_approval = "auto"

[clis.echo]
command = "echo"
approval_args = {{ "full-auto" = "{SENTINEL_FLAG}" }}
"#
        ),
    );
    let session = session.expect("session name in stdout");

    // Wait for both agents' commands to render, then inspect every pane.
    let captures: Vec<String> = (0..=2)
        .map(|pane| capture_pane_until(&tmux_env, &session, pane, "echo"))
        .collect();
    kill_session(&tmux_env, &session);

    for (pane, capture) in captures.iter().enumerate() {
        assert!(
            !capture.contains(SENTINEL_FLAG),
            "pane {pane} must not carry the orchestrator's full-auto flag; capture:\n{capture}"
        );
    }
}

/// Scenario "Coding agents resolve from `agent_approval`": `agent_approval =
/// "full-auto"` reaches both agent panes even with `approval = "manual"`.
#[test]
#[serial]
fn coding_agents_resolve_flags_from_agent_approval() {
    if skip_if_no_tmux() {
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    let (session, _stdout, _stderr) = launch(
        &tr,
        &tmux_env,
        &format!(
            r#"
[supervisor]
enabled = true
cli = "echo"
approval = "manual"
agent_approval = "full-auto"

[clis.echo]
command = "echo"
approval_args = {{ "full-auto" = "{SENTINEL_FLAG}" }}
"#
        ),
    );
    let session = session.expect("session name in stdout");
    let pane1 = capture_pane_until(&tmux_env, &session, 1, SENTINEL_FLAG);
    let pane2 = capture_pane_until(&tmux_env, &session, 2, SENTINEL_FLAG);
    kill_session(&tmux_env, &session);

    assert!(
        pane1.contains(SENTINEL_FLAG) && pane2.contains(SENTINEL_FLAG),
        "both agent panes carry agent_approval's flag; pane1:\n{pane1}\npane2:\n{pane2}"
    );
}

/// Scenario "Pre-relocation config with approval still loads": a config that
/// sets `[supervisor] approval` loads and launches with no warning about it.
#[test]
#[serial]
fn a_config_with_approval_launches_without_a_warning() {
    if skip_if_no_tmux() {
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    let (session, _stdout, stderr) = launch(
        &tr,
        &tmux_env,
        r#"
[supervisor]
enabled = true
cli = "echo"
approval = "full-auto"
"#,
    );
    if let Some(name) = &session {
        kill_session(&tmux_env, name);
    }
    assert!(session.is_some(), "the launch proceeds to a session");
    assert!(
        !stderr.contains("approval"),
        "setting approval must produce no warning; stderr:\n{stderr}"
    );
}

/// Scenarios "Recovery of a pre-relocation session rebuilds the new layout"
/// (`session-state`) and "Supervised recovery applies the same fallback"
/// (`supervisor-launch`): a stopped supervised session saved by an earlier
/// binary (`dashboard_pane = 1`) is rebuilt as the dashboard at pane 0 plus
/// its one agent — no orchestrator pane — from a config with no
/// `[supervisor]` section at all, and is re-saved with `dashboard_pane = 0`.
#[test]
#[serial]
fn recovery_of_a_pre_relocation_session_rebuilds_the_new_layout() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();
    let project = unique_project_name("recover");
    let repo = rename_repo_basename(&tr, &project);
    // `find_session_for_repo` compares repo_path against what
    // `git rev-parse --show-toplevel` reports (NOT fs::canonicalize, which
    // resolves /var -> /private/var on macOS and would never match).
    let canonical_repo = {
        let out = StdCommand::new("git")
            .current_dir(&repo)
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .expect("git rev-parse");
        assert!(out.status.success(), "git rev-parse must succeed");
        std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
    };

    // `recover_session` expects the saved worktree to already exist on disk.
    let branch = "feat/alpha";
    let wt_path = {
        let parent = repo.parent().expect("repo has parent");
        let wt = parent.join(format!("{project}-feat-alpha"));
        let st = StdCommand::new("git")
            .current_dir(&repo)
            .args(["branch", branch])
            .status()
            .expect("git branch");
        assert!(st.success(), "git branch must succeed");
        let st = StdCommand::new("git")
            .current_dir(&repo)
            .args(["worktree", "add", wt.to_str().unwrap(), branch])
            .status()
            .expect("git worktree add");
        assert!(st.success(), "git worktree add must succeed");
        wt
    };

    // No `[supervisor]` section and no CLI keys: recovery needs neither.
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(paw_dir.join("config.toml"), "# no [supervisor] section\n").expect("write config");

    // Override HOME so session state lands in a temp dir we control. On
    // macOS `data_dir()` resolves to `<HOME>/Library/Application Support`.
    let fake_home = TempDir::new().expect("create temp HOME");
    let sessions_dir = if cfg!(target_os = "macos") {
        fake_home
            .path()
            .join("Library/Application Support/git-paw/sessions")
    } else {
        fake_home.path().join(".local/share/git-paw/sessions")
    };
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    let session_name = format!("paw-{project}");
    let session_file = sessions_dir.join(format!("{session_name}.json"));
    let session_json = serde_json::json!({
        "session_name": session_name,
        "repo_path": canonical_repo.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-01-01T00:00:00Z",
        "status": "stopped",
        "mode": "supervisor",
        "dashboard_pane": 1,
        "worktrees": [{
            "branch": branch,
            "worktree_path": wt_path.to_string_lossy(),
            "cli": "echo",
            "branch_created": true,
        }],
    });
    fs::write(
        &session_file,
        serde_json::to_string_pretty(&session_json).expect("serialize session"),
    )
    .expect("write session json");

    // Drive recovery via `git paw start --no-supervisor`: the recovery check
    // lives in `cmd_start`. `--no-supervisor` only affects fresh-launch
    // dispatch — the receipt's `mode: supervisor` still wins in
    // `recover_session`. Attach fails without a TTY, but the session has
    // been rebuilt by then.
    let mut start_cmd = cmd();
    start_cmd
        .current_dir(&repo)
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME");
    tmux_env.apply_assert(&mut start_cmd);
    let start_out = start_cmd
        .args(["start", "--no-supervisor"])
        .timeout(Duration::from_secs(15))
        .output()
        .expect("run start");
    let start_stdout = String::from_utf8_lossy(&start_out.stdout).to_string();
    assert!(
        start_stdout.contains("Recovering session"),
        "start must take the recovery path; stdout:\n{start_stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&start_out.stderr)
    );

    let roles = pane_roles(&tmux_env, &session_name);
    kill_session(&tmux_env, &session_name);

    assert_eq!(
        roles,
        vec!["0|dashboard".to_string(), "1|feat/alpha".to_string()],
        "dashboard + one agent, no orchestrator pane"
    );
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&session_file).expect("re-saved session"))
            .expect("session json parses");
    assert_eq!(
        saved["dashboard_pane"], 0,
        "re-saved with the dashboard at 0"
    );
}
