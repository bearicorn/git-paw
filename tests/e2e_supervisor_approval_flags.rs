//! E2E observable: the supervisor pane's launch command carries the flags
//! resolved from the SUPERVISOR-specific approval level, while coding-agent
//! panes keep resolving from `agent_approval`.
//!
//! Maps to supervisor-launch scenarios:
//!
//! - `Fresh start applies supervisor flags to pane 0 only`
//! - `Recovery rebuilds the supervisor pane with the same flags`
//!
//! and the supervisor-config scenario `Full-auto with an unmapped CLI warns
//! and degrades` (real-launch variant; the command-composition variants live
//! in `supervisor_integration.rs`).
//!
//! The tests use `echo` as the CLI with a sentinel flag supplied through the
//! `[clis.echo] approval_args` override, so the pane's scrollback shows the
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
/// enough that it can only appear in a pane via the supervisor command.
const SENTINEL_FLAG: &str = "--paw-supervisor-full-auto-sentinel";

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

/// Captures `pane` of `session` once.
fn capture_pane(env: &TmuxTestEnv, session: &str, pane: usize) -> String {
    let target = format!("{session}:0.{pane}");
    let mut capture_cmd = StdCommand::new("tmux");
    env.apply(&mut capture_cmd);
    let out = capture_cmd
        .args(["capture-pane", "-t", &target, "-p", "-S", "-2000"])
        .output()
        .expect("tmux capture-pane");
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn kill_session(env: &TmuxTestEnv, name: &str) {
    let mut kill_cmd = StdCommand::new("tmux");
    env.apply(&mut kill_cmd);
    let _ = kill_cmd.args(["kill-session", "-t", name]).status();
}

/// Fresh start with split levels: pane 0's command carries the supervisor
/// flag, the agent panes' commands do not.
#[test]
#[serial]
fn fresh_start_applies_supervisor_flags_to_pane_zero_only() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    // Broker disabled to keep the test fast; the pane commands are
    // independent of broker state. The [clis.echo] override supplies the
    // sentinel full-auto flag (echo has no built-in table row).
    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = format!(
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
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");

    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(tr.path())
        .args(["start", "--supervisor", "--branches", "a,b"])
        .timeout(Duration::from_secs(15))
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

    // Pane 0 = supervisor, 1 = dashboard, 2/3 = agents a/b.
    let pane0 = capture_pane_until(&tmux_env, &session_name, 0, SENTINEL_FLAG);
    let pane2 = capture_pane(&tmux_env, &session_name, 2);
    let pane3 = capture_pane(&tmux_env, &session_name, 3);

    kill_session(&tmux_env, &session_name);

    assert!(
        pane0.contains(SENTINEL_FLAG),
        "supervisor pane 0 must be launched with the full-auto flag; capture:\n{pane0}"
    );
    assert!(
        !pane2.contains(SENTINEL_FLAG) && !pane3.contains(SENTINEL_FLAG),
        "agent panes must NOT carry the supervisor's flag; pane2:\n{pane2}\npane3:\n{pane3}"
    );
}

/// Recovery rebuilds the supervisor pane with the flags resolved from the
/// supervisor-specific level.
#[test]
#[serial]
fn recovery_rebuilds_supervisor_pane_with_the_same_flags() {
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

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = format!(
        r#"
[supervisor]
enabled = true
cli = "echo"
approval = "full-auto"

[clis.echo]
command = "echo"
approval_args = {{ "full-auto" = "{SENTINEL_FLAG}" }}
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");

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
    let session_json = serde_json::json!({
        "session_name": session_name,
        "repo_path": canonical_repo.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-01-01T00:00:00Z",
        "status": "stopped",
        "mode": "supervisor",
        "worktrees": [{
            "branch": branch,
            "worktree_path": wt_path.to_string_lossy(),
            "cli": "echo",
            "branch_created": true,
        }],
    });
    fs::write(
        sessions_dir.join(format!("{session_name}.json")),
        serde_json::to_string_pretty(&session_json).expect("serialize session"),
    )
    .expect("write session json");

    // Drive recovery via `git paw start --no-supervisor`: the recovery check
    // lives in `cmd_start` (bare `start` with [supervisor] enabled dispatches
    // to the fresh-launch supervisor flow, which has no recovery check).
    // `--no-supervisor` only affects fresh-launch dispatch — the receipt's
    // `mode: supervisor` still wins in `recover_session`, so the rebuilt
    // session uses the supervisor layout and flag resolution. Attach fails
    // without a TTY, but the session has been rebuilt by then.
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

    let pane0 = capture_pane_until(&tmux_env, &session_name, 0, SENTINEL_FLAG);
    kill_session(&tmux_env, &session_name);

    assert!(
        pane0.contains(SENTINEL_FLAG),
        "recovered supervisor pane must carry the full-auto flag; capture:\n{pane0}"
    );
}

/// `full-auto` with a CLI that has no flag mapping warns on stderr and still
/// launches the session (flagless supervisor pane) — the launch MUST NOT fail.
#[test]
#[serial]
fn full_auto_unmapped_cli_warns_and_launch_does_not_fail() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    // `echo` has no built-in table row and no [clis.echo] override here, so
    // full-auto resolves to "" and must warn-and-degrade.
    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = r#"
[supervisor]
enabled = true
cli = "echo"
approval = "full-auto"
"#;
    fs::write(paw_dir.join("config.toml"), config).expect("write config");

    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(tr.path())
        .args(["start", "--supervisor", "--branches", "a"])
        .timeout(Duration::from_secs(15))
        .output()
        .expect("run start");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    let session_name = stdout
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
        .map(ToString::to_string);
    if let Some(name) = &session_name {
        kill_session(&tmux_env, name);
    }

    assert!(
        out.status.success(),
        "full-auto with an unmapped CLI must not fail the launch; stderr:\n{stderr}"
    );
    assert!(
        session_name.is_some(),
        "launch must proceed to a session; stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("echo")
            && stderr.contains("[clis.echo]")
            && stderr.contains("approval_args"),
        "warning must name the CLI and the [clis.<name>] approval_args override; stderr:\n{stderr}"
    );
}
