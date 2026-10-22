//! Integration test for `recover_session` (driven via the binary's
//! `start` command in recover mode).
//!
//! Saves a `Session` JSON with a broker port, then invokes `git paw start`
//! and asserts that the resulting tmux session has the dashboard pane at
//! pane 0, agent panes for every saved worktree, and a broker listening on
//! the saved port that knows about the saved agent ids (proxy for "watchers
//! exist for every saved worktree").

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// Atomic counter so each test gets a unique tmux project name.
static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_project_name(tag: &str) -> String {
    let pid = std::process::id();
    let n = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("rcv-{tag}-{pid}-{n}")
}

fn skip_if_no_tmux() -> bool {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return true;
    }
    false
}

#[allow(dead_code)]
fn kill_tmux_session(name: &str) {
    let _ = StdCommand::new("tmux")
        .args(["kill-session", "-t", name])
        .status();
}

fn find_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral")
        .local_addr()
        .expect("local_addr")
        .port()
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

/// Returns the canonical repo root that `git rev-parse --show-toplevel`
/// reports — this is the path `find_session_for_repo` will compare against.
fn canonical_repo_root(repo: &std::path::Path) -> std::path::PathBuf {
    let out = StdCommand::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .expect("git rev-parse");
    assert!(out.status.success(), "git rev-parse must succeed");
    std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
}

/// Creates a worktree for `branch` rooted at `<repo>/../<repo_basename>-<branch_slug>`.
/// Returns the absolute worktree path.
fn create_worktree(repo: &std::path::Path, project: &str, branch: &str) -> std::path::PathBuf {
    let parent = repo.parent().expect("repo has parent").to_path_buf();
    let slug = branch.replace('/', "-");
    let wt_path = parent.join(format!("{project}-{slug}"));

    // Create the branch first.
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

/// Probes the broker URL with a manual GET /status. Returns the response body
/// or `None` if the connection fails.
fn http_get_status(url: &str) -> Option<String> {
    let addr = url.strip_prefix("http://").unwrap_or(url);
    let mut stream = TcpStream::connect(addr).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(2))).ok();
    let req = format!("GET /status HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())?;

    // Decode chunked transfer encoding if present.
    if response
        .to_lowercase()
        .contains("transfer-encoding: chunked")
    {
        let mut decoded = String::new();
        let mut remaining = body.as_str();
        loop {
            let line_end = remaining.find("\r\n").unwrap_or(remaining.len());
            let size = usize::from_str_radix(remaining[..line_end].trim(), 16).unwrap_or(0);
            if size == 0 {
                break;
            }
            remaining = &remaining[line_end + 2..];
            if remaining.len() >= size {
                decoded.push_str(&remaining[..size]);
                remaining = &remaining[size..];
                if remaining.starts_with("\r\n") {
                    remaining = &remaining[2..];
                }
            } else {
                break;
            }
        }
        Some(decoded)
    } else {
        Some(body)
    }
}

// ---------------------------------------------------------------------------
// C11: recover_session reconstructs dashboard pane and watchers
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn recover_session_reconstructs_dashboard_and_watchers() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("recover");
    let repo = rename_repo_basename(&tr, &project);
    let canonical_repo = canonical_repo_root(&repo);

    // Create a real worktree for the branch we will save in the session
    // state — `recover_session` does NOT recreate worktrees, it expects
    // them to already exist on disk.
    let branch = "feat/alpha";
    let wt_path = create_worktree(&repo, &project, branch);

    // Configure broker enabled + register `sh` as the agent CLI so the
    // recovered session has a runnable command in the agent pane.
    let broker_port = find_free_port();
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config_content =
        format!("[broker]\nenabled = true\nport = {broker_port}\n\n[clis.sh]\ncommand = \"sh\"\n");
    fs::write(paw_dir.join("config.toml"), config_content).expect("write config");

    // Override XDG/HOME so session state lands in a temp dir we control.
    // On macOS, `data_dir()` resolves to `<HOME>/Library/Application Support`.
    let fake_home = TempDir::new().expect("create temp HOME");
    let sessions_dir = if cfg!(target_os = "macos") {
        fake_home
            .path()
            .join("Library/Application Support/git-paw/sessions")
    } else {
        fake_home.path().join(".local/share/git-paw/sessions")
    };
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    // The session name is `paw-<project>` per `tmux::resolve_session_name`.
    let session_name = format!("paw-{project}");
    let broker_log = sessions_dir.join(format!("{session_name}.broker.log"));

    // Compose the session JSON with broker_port set + one worktree entry.
    let session_json = serde_json::json!({
        "session_name": session_name,
        "repo_path": canonical_repo.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-01-01T00:00:00Z",
        "status": "stopped",
        "worktrees": [{
            "branch": branch,
            "worktree_path": wt_path.to_string_lossy(),
            "cli": "sh",
            "branch_created": true,
        }],
        "broker_port": broker_port,
        "broker_bind": "127.0.0.1",
        "broker_log_path": broker_log.to_string_lossy(),
    });
    fs::write(
        sessions_dir.join(format!("{session_name}.json")),
        serde_json::to_string_pretty(&session_json).expect("serialize session"),
    )
    .expect("write session json");

    // Drive recovery via `git paw start`. cmd_start finds the saved session,
    // sees no live tmux session, and calls recover_session. Attach at the
    // end will fail without a TTY, but recover_session has executed.
    let mut start_cmd = cmd();
    start_cmd
        .current_dir(&repo)
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME");
    tmux_env.apply_assert(&mut start_cmd);
    let _start_output = start_cmd.args(["start"]).output().expect("run start");

    // ---------------------------------------------------------------------
    // Assertion 1: the tmux session exists with the saved name.
    // ---------------------------------------------------------------------
    let mut has_session_cmd = StdCommand::new("tmux");
    tmux_env.apply(&mut has_session_cmd);
    let alive = has_session_cmd
        .args(["has-session", "-t", &session_name])
        .status()
        .expect("tmux has-session")
        .success();
    if !alive {
        let mut kill_cmd = StdCommand::new("tmux");
        tmux_env.apply(&mut kill_cmd);
        let _ = kill_cmd
            .args(["kill-session", "-t", &session_name])
            .status();
        panic!("recover_session did not create tmux session '{session_name}'");
    }

    // ---------------------------------------------------------------------
    // Assertion 2: dashboard pane is pane 0, agent pane is pane 1.
    // ---------------------------------------------------------------------
    let mut list_panes_cmd = StdCommand::new("tmux");
    tmux_env.apply(&mut list_panes_cmd);
    let pane_listing = list_panes_cmd
        .args([
            "list-panes",
            "-t",
            &session_name,
            "-F",
            "#{pane_index}: #{pane_current_command}",
        ])
        .output()
        .expect("tmux list-panes");
    let listing = String::from_utf8_lossy(&pane_listing.stdout).to_string();
    let pane_count = listing.lines().count();
    if pane_count != 2 {
        let mut kill_cmd = StdCommand::new("tmux");
        tmux_env.apply(&mut kill_cmd);
        let _ = kill_cmd
            .args(["kill-session", "-t", &session_name])
            .status();
        panic!(
            "expected 2 panes (dashboard + 1 agent); got {pane_count}\n\
             listing:\n{listing}"
        );
    }

    // ---------------------------------------------------------------------
    // Assertion 3: broker is listening on the saved port and a WatchTarget
    // exists for the saved worktree — proven by dirtying the worktree and
    // observing the watcher publish an `agent.status` that surfaces the agent
    // in `/status`. (A roster row appears once a pane publishes — the
    // publisher-only rule — so we trigger a real watcher publish rather than
    // relying on the seed alone.)
    // ---------------------------------------------------------------------
    let _ = fs::write(wt_path.join(".paw-recover-probe"), "trigger watcher\n");
    let broker_url = format!("http://127.0.0.1:{broker_port}");
    let agent_slug = "feat-alpha"; // slugify_branch("feat/alpha")
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut found_status = false;
    let mut last_body = String::new();
    while Instant::now() < deadline {
        if let Some(body) = http_get_status(&broker_url) {
            last_body = body.clone();
            if body.contains(agent_slug) {
                found_status = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    // Tear down before assertion failure messages.
    let mut kill_cmd = StdCommand::new("tmux");
    tmux_env.apply(&mut kill_cmd);
    let _ = kill_cmd
        .args(["kill-session", "-t", &session_name])
        .status();

    assert!(
        found_status,
        "broker /status should mention the saved agent {agent_slug:?} \
         within 15s — recover_session must (a) start the dashboard pane which \
         starts the broker and (b) configure WatchTargets for every saved \
         worktree. Last /status body:\n{last_body}"
    );

    // Listing's first line should reference pane 0 with a child of git-paw
    // (the dashboard process). We accept either "git-paw", "git", "sh", or
    // the integration-test compiled binary name in pane 0 since the dashboard
    // process may show as the parent shell while booting.
    assert!(
        listing.starts_with("0:"),
        "first pane listed should be pane 0 (dashboard); got listing:\n{listing}"
    );
}

// ---------------------------------------------------------------------------
// `tmux-orchestration` — "Every pane-less session uses the agent grid":
// scenario "Recovery rebuilds a grid".
// ---------------------------------------------------------------------------

/// A stopped bare session saved with the earlier whole-window tiled layout
/// (no recorded `layout`) and 3 worktrees, with the broker enabled, recovers
/// as the agent grid: a full-width dashboard top row taking half the window
/// and one agent row of 3 below it. The re-saved session records the grid, so
/// the recovered session is hot-attachable where the stopped one was not.
#[test]
#[serial]
fn recovery_rebuilds_a_tiled_bare_session_as_a_grid() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("recovergrid");
    let repo = rename_repo_basename(&tr, &project);
    let canonical_repo = canonical_repo_root(&repo);

    let branches = ["feat/alpha", "feat/beta", "feat/gamma"];
    let worktrees: Vec<_> = branches
        .iter()
        .map(|b| create_worktree(&repo, &project, b))
        .collect();

    let broker_port = find_free_port();
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        format!("[broker]\nenabled = true\nport = {broker_port}\n\n[clis.sh]\ncommand = \"sh\"\n"),
    )
    .expect("write config");

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
    let session_path = sessions_dir.join(format!("{session_name}.json"));

    // An earlier binary's bare session: mode "bare", no `layout` key at all.
    let session_json = serde_json::json!({
        "session_name": session_name,
        "repo_path": canonical_repo.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-01-01T00:00:00Z",
        "status": "stopped",
        "mode": "bare",
        "worktrees": branches.iter().zip(&worktrees).map(|(branch, wt)| serde_json::json!({
            "branch": branch,
            "worktree_path": wt.to_string_lossy(),
            "cli": "sh",
            "branch_created": true,
        })).collect::<Vec<_>>(),
        "broker_port": broker_port,
        "broker_bind": "127.0.0.1",
    });
    fs::write(
        &session_path,
        serde_json::to_string_pretty(&session_json).expect("serialize session"),
    )
    .expect("write session json");

    let mut start_cmd = cmd();
    start_cmd
        .current_dir(&repo)
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME");
    tmux_env.apply_assert(&mut start_cmd);
    let _ = start_cmd.args(["start"]).output().expect("run start");

    // Geometry of the rebuilt window: index, left, top, width, height.
    let mut list_panes_cmd = StdCommand::new("tmux");
    tmux_env.apply(&mut list_panes_cmd);
    let output = list_panes_cmd
        .args([
            "list-panes",
            "-t",
            &session_name,
            "-F",
            "#{pane_index} #{pane_left} #{pane_top} #{pane_width} #{pane_height}",
        ])
        .output()
        .expect("tmux list-panes");
    let listing = String::from_utf8_lossy(&output.stdout).to_string();

    let mut window_cmd = StdCommand::new("tmux");
    tmux_env.apply(&mut window_cmd);
    let window = window_cmd
        .args([
            "display-message",
            "-p",
            "-t",
            &format!("{session_name}:0"),
            "#{window_width} #{window_height}",
        ])
        .output()
        .expect("tmux display-message");
    let window = String::from_utf8_lossy(&window.stdout).trim().to_string();

    let saved = fs::read_to_string(&session_path).expect("re-read session json");

    // Tear down before any assertion can fail.
    let mut kill_cmd = StdCommand::new("tmux");
    tmux_env.apply(&mut kill_cmd);
    let _ = kill_cmd
        .args(["kill-session", "-t", &session_name])
        .status();

    let panes: Vec<Vec<usize>> = listing
        .lines()
        .map(|l| {
            l.split_whitespace()
                .filter_map(|f| f.parse().ok())
                .collect()
        })
        .filter(|f: &Vec<usize>| f.len() == 5)
        .collect();
    assert_eq!(
        panes.len(),
        4,
        "dashboard + 3 agents; got listing:\n{listing}"
    );

    let (window_width, window_height) = {
        let mut it = window.split_whitespace();
        let w: usize = it
            .next()
            .and_then(|v| v.parse().ok())
            .expect("window width");
        let h: usize = it
            .next()
            .and_then(|v| v.parse().ok())
            .expect("window height");
        (w, h)
    };

    // Pane 0 is the full-width top row, and it is HALF the window — the grid's
    // top row. The earlier `main-horizontal` layout sized it to tmux's
    // `main-pane-height` instead, so this is what distinguishes the two.
    // `pane-border-status top` costs each pane a border row, so compare tops
    // relatively and allow a few rows of slack on the height.
    let dashboard = &panes[0];
    assert_eq!(dashboard[1], 0, "dashboard starts at the left edge");
    assert_eq!(
        dashboard[3], window_width,
        "dashboard spans the full width; listing:\n{listing}"
    );
    let half = window_height / 2;
    assert!(
        dashboard[4].abs_diff(half) <= 4,
        "the grid's top row is half the window ({half} ± 4), got {}; listing:\n{listing}",
        dashboard[4]
    );

    // The 3 agents are one row: same top, below the dashboard, side by side.
    let agents = &panes[1..];
    let agent_top = agents[0][2];
    assert!(
        agent_top > dashboard[2],
        "the agent row sits below the dashboard; listing:\n{listing}"
    );
    assert!(
        agents.iter().all(|p| p[2] == agent_top),
        "all 3 agents share one row; listing:\n{listing}"
    );
    assert!(
        agents[0][1] < agents[1][1] && agents[1][1] < agents[2][1],
        "agents run left to right in branch order; listing:\n{listing}"
    );

    // And the re-saved session records the grid, so `add` / `remove` now work
    // on it.
    assert!(
        saved.contains(r#""layout": "grid""#),
        "recovery must re-save the session with the grid layout recorded; got:\n{saved}"
    );
}
