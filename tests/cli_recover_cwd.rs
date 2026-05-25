//! Integration test for Bug B (drift `supervisor-bugfixes-v0-5-x` §2):
//! after `git paw stop` + `git paw start`, each resumed coding-agent pane's
//! `pane_current_path` SHALL equal its `worktree_path` from the session JSON.
//!
//! Drives recovery via the `git paw start` binary with a pre-seeded session
//! JSON (so the test does not need to actually `stop` a live session). The
//! recovered tmux session is inspected via `tmux display-message -p
//! "#{pane_current_path}"`.
//!
//! NOTE: Run with `GIT_PAW_ALLOW_LIVE_SESSION=1` if there is a live
//! `paw-*` tmux session on the default socket (e.g. during dogfooding).
//! The test uses `setup_test_repo`, which otherwise refuses to run.

use std::fs;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn skip_if_no_tmux() -> bool {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return true;
    }
    false
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_project(tag: &str) -> String {
    let pid = std::process::id();
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("recovercwd-{tag}-{pid}-{n}")
}

fn rename_repo_basename(tr: &TestRepo, new_basename: &str) -> std::path::PathBuf {
    let original = tr.path().to_path_buf();
    let parent = original.parent().expect("repo has parent").to_path_buf();
    let renamed = parent.join(new_basename);
    fs::rename(&original, &renamed).expect("rename repo dir");
    renamed
}

fn canonical_path(p: &std::path::Path) -> std::path::PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

fn git_toplevel(repo: &std::path::Path) -> std::path::PathBuf {
    let out = StdCommand::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .expect("git rev-parse");
    assert!(out.status.success(), "git rev-parse must succeed");
    std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
}

fn create_worktree(repo: &std::path::Path, project: &str, branch: &str) -> std::path::PathBuf {
    let parent = repo.parent().expect("repo has parent").to_path_buf();
    let slug = branch.replace('/', "-");
    let wt_path = parent.join(format!("{project}-{slug}"));

    let st = StdCommand::new("git")
        .current_dir(repo)
        .args(["branch", branch])
        .status()
        .expect("git branch");
    assert!(st.success(), "git branch {branch} must succeed");

    let st = StdCommand::new("git")
        .current_dir(repo)
        .args(["worktree", "add", wt_path.to_str().unwrap(), branch])
        .status()
        .expect("git worktree add");
    assert!(st.success(), "git worktree add must succeed");

    wt_path
}

fn sessions_dir_for(home: &std::path::Path) -> std::path::PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-paw/sessions")
    } else {
        home.join(".local/share/git-paw/sessions")
    }
}

fn find_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral")
        .local_addr()
        .expect("local_addr")
        .port()
}

fn pane_current_path(
    tmux_env: &TmuxTestEnv,
    session_name: &str,
    pane_index: usize,
) -> Option<std::path::PathBuf> {
    // Wait up to ~5s for the pane to actually exist.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        let mut display = StdCommand::new("tmux");
        tmux_env.apply(&mut display);
        let out = display
            .args([
                "display-message",
                "-t",
                &format!("{session_name}:0.{pane_index}"),
                "-p",
                "#{pane_current_path}",
            ])
            .output()
            .ok()?;
        if out.status.success() {
            let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !raw.is_empty() {
                return Some(std::path::PathBuf::from(raw));
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    None
}

#[test]
#[serial]
fn recovered_agent_panes_use_each_agents_worktree_cwd() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project("sup");
    let repo = rename_repo_basename(&tr, &project);

    // Two coding-agent worktrees.
    let branch_a = "feat/alpha";
    let branch_b = "feat/beta";
    let wt_a = create_worktree(&repo, &project, branch_a);
    let wt_b = create_worktree(&repo, &project, branch_b);

    let broker_port = find_free_port();
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    // Bare-mode recovery: lighter setup than supervisor-mode (no
    // supervisor pane, no dashboard, no broker auto-start) so the
    // resumed agent panes' cwd is the only thing under test. The fix in
    // `TmuxSessionBuilder::build` applies to BOTH bare and supervisor
    // modes (see `src/tmux.rs::tests` for the supervisor command-list
    // unit-test coverage).
    let config_content = format!(
        "default_cli = \"sh\"\n\n[broker]\nenabled = true\nport = {broker_port}\n\n[clis.sh]\ncommand = \"sh\"\n"
    );
    fs::write(paw_dir.join("config.toml"), config_content).expect("write config");

    // Seed the session JSON for recovery.
    let fake_home = TempDir::new().expect("create temp HOME");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    let session_name = format!("paw-{project}");
    let broker_log = sessions_dir.join("broker.log");

    let canonical_repo = git_toplevel(&repo);
    let canonical_wt_a = canonical_path(&wt_a);
    let canonical_wt_b = canonical_path(&wt_b);

    let session_json = serde_json::json!({
        "session_name": session_name,
        "repo_path": canonical_repo.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-01-01T00:00:00Z",
        "status": "stopped",
        "mode": "bare",
        "worktrees": [
            {
                "branch": branch_a,
                "worktree_path": canonical_wt_a.to_string_lossy(),
                "cli": "sh",
                "branch_created": true,
            },
            {
                "branch": branch_b,
                "worktree_path": canonical_wt_b.to_string_lossy(),
                "cli": "sh",
                "branch_created": true,
            },
        ],
        "broker_port": broker_port,
        "broker_bind": "127.0.0.1",
        "broker_log_path": broker_log.to_string_lossy(),
    });
    fs::write(
        sessions_dir.join(format!("{session_name}.json")),
        serde_json::to_string_pretty(&session_json).expect("serialize session"),
    )
    .expect("write session json");

    // Drive recovery via `git paw start`.
    let mut start_cmd = cmd();
    start_cmd
        .current_dir(&repo)
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME");
    tmux_env.apply_assert(&mut start_cmd);
    let start_output = start_cmd
        .args(["start"])
        .timeout(Duration::from_secs(15))
        .output()
        .expect("run git paw start");

    // Confirm the session is live before inspecting panes.
    let mut has_session = StdCommand::new("tmux");
    tmux_env.apply(&mut has_session);
    let alive = has_session
        .args(["has-session", "-t", &session_name])
        .status()
        .expect("tmux has-session")
        .success();
    if !alive {
        let stdout = String::from_utf8_lossy(&start_output.stdout);
        let stderr = String::from_utf8_lossy(&start_output.stderr);
        panic!(
            "recover_session did not create tmux session '{session_name}'\n\
             start stdout:\n{stdout}\nstart stderr:\n{stderr}"
        );
    }

    // Bare-mode layout: pane :0.0 is the dashboard, pane :0.1 is the first
    // agent (wt_a) and pane :0.2 is the second agent (wt_b).
    let cwd_a = pane_current_path(&tmux_env, &session_name, 1)
        .expect("pane :0.1 pane_current_path readable");
    let cwd_b = pane_current_path(&tmux_env, &session_name, 2)
        .expect("pane :0.2 pane_current_path readable");

    // Tear down before assertion noise.
    let mut kill = StdCommand::new("tmux");
    tmux_env.apply(&mut kill);
    let _ = kill.args(["kill-session", "-t", &session_name]).status();

    let canonical_cwd_a = canonical_path(&cwd_a);
    let canonical_cwd_b = canonical_path(&cwd_b);

    assert_eq!(
        canonical_cwd_a, canonical_wt_a,
        "agent pane :0.1 should be cwd'd into its worktree {canonical_wt_a:?}; got {cwd_a:?}"
    );
    assert_eq!(
        canonical_cwd_b, canonical_wt_b,
        "agent pane :0.2 should be cwd'd into its worktree {canonical_wt_b:?}; got {cwd_b:?}"
    );
    assert_ne!(
        canonical_cwd_a, canonical_repo,
        "agent pane :0.1 must not be anchored in the repo root"
    );
}
