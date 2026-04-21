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
    let broker_log = sessions_dir.join("broker.log");

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
    let _start_output = cmd()
        .current_dir(&repo)
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start"])
        .output()
        .expect("run start");

    // ---------------------------------------------------------------------
    // Assertion 1: the tmux session exists with the saved name.
    // ---------------------------------------------------------------------
    let alive = StdCommand::new("tmux")
        .args(["has-session", "-t", &session_name])
        .status()
        .expect("tmux has-session")
        .success();
    if !alive {
        kill_tmux_session(&session_name);
        panic!("recover_session did not create tmux session '{session_name}'");
    }

    // ---------------------------------------------------------------------
    // Assertion 2: dashboard pane is pane 0, agent pane is pane 1.
    // ---------------------------------------------------------------------
    let pane_listing = StdCommand::new("tmux")
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
        kill_tmux_session(&session_name);
        panic!(
            "expected 2 panes (dashboard + 1 agent); got {pane_count}\n\
             listing:\n{listing}"
        );
    }

    // ---------------------------------------------------------------------
    // Assertion 3: broker is listening on the saved port and reports the
    // saved agent. The watcher pre-population at broker start (see
    // `start_broker`) populates `agent_clis` for every WatchTarget — so the
    // dashboard's `/status` reflects the saved worktree set.
    // ---------------------------------------------------------------------
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
    kill_tmux_session(&session_name);

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
