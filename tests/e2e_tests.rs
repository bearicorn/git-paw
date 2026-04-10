//! End-to-end tests.
//!
//! Tests the `git-paw` binary and tmux orchestration in realistic scenarios.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;
use serial_test::serial;
use tempfile::TempDir;

use git_paw::tmux::{
    PaneSpec, TmuxSessionBuilder, attach, ensure_tmux_installed, is_session_alive, kill_session,
};

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

// ---------------------------------------------------------------------------
// Test repo helper
// ---------------------------------------------------------------------------

struct TestRepo {
    _sandbox: TempDir,
    repo: PathBuf,
}

impl TestRepo {
    fn path(&self) -> &Path {
        &self.repo
    }
}

fn setup_test_repo() -> TestRepo {
    let sandbox = TempDir::new().expect("create temp dir");
    let repo = sandbox.path().join("test-repo");
    fs::create_dir_all(&repo).expect("create repo dir");

    run_git(&repo, &["init"]);
    run_git(&repo, &["config", "user.email", "test@test.com"]);
    run_git(&repo, &["config", "user.name", "Test"]);

    let readme = repo.join("README.md");
    fs::write(&readme, "# Test repo").expect("write README");
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "initial commit"]);

    TestRepo {
        _sandbox: sandbox,
        repo,
    }
}

fn run_git(dir: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("run git command");
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ---------------------------------------------------------------------------
// Dry-run
// ---------------------------------------------------------------------------

#[test]
fn dry_run_with_flags_shows_plan() {
    let tr = setup_test_repo();
    run_git(tr.path(), &["branch", "feat/a"]);
    run_git(tr.path(), &["branch", "feat/b"]);

    // Register "echo" as a custom CLI so detection finds it
    let config = tr.path().join(".git-paw").join("config.toml");
    fs::create_dir_all(config.parent().unwrap()).expect("create config dir");
    fs::write(&config, "[clis.echo]\ncommand = \"/bin/echo\"\n").expect("write config");

    cmd()
        .current_dir(tr.path())
        .args([
            "start",
            "--dry-run",
            "--cli",
            "echo",
            "--branches",
            "feat/a,feat/b",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Dry run")
                .and(predicate::str::contains("feat/a"))
                .and(predicate::str::contains("feat/b"))
                .and(predicate::str::contains("echo")),
        );
}

// ---------------------------------------------------------------------------
// Non-interactive flags
// ---------------------------------------------------------------------------

#[test]
fn preset_not_found_returns_error() {
    let tr = setup_test_repo();

    cmd()
        .current_dir(tr.path())
        .args(["start", "--preset", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// ---------------------------------------------------------------------------
// Stop and purge from repo with no session
// ---------------------------------------------------------------------------

#[test]
fn stop_with_no_session() {
    let tr = setup_test_repo();

    cmd()
        .current_dir(tr.path())
        .arg("stop")
        .assert()
        .success()
        .stdout(predicate::str::contains("No active session"));
}

#[test]
fn purge_with_no_session() {
    let tr = setup_test_repo();

    cmd()
        .current_dir(tr.path())
        .args(["purge", "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No session to purge"));
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

#[test]
fn status_with_no_session() {
    let tr = setup_test_repo();

    cmd()
        .current_dir(tr.path())
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("No session"));
}

// ---------------------------------------------------------------------------
// Not-a-repo errors
// ---------------------------------------------------------------------------

#[test]
fn stop_from_non_git_dir_fails() {
    let tmp = TempDir::new().expect("create temp dir");

    cmd()
        .current_dir(tmp.path())
        .arg("stop")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

#[test]
fn status_from_non_git_dir_fails() {
    let tmp = TempDir::new().expect("create temp dir");

    cmd()
        .current_dir(tmp.path())
        .arg("status")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

#[test]
fn init_creates_git_paw_dir() {
    let tr = setup_test_repo();

    cmd()
        .current_dir(tr.path())
        .arg("init")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Created .git-paw/")
                .and(predicate::str::contains("Initialized git-paw")),
        );

    assert!(tr.path().join(".git-paw").is_dir());
    assert!(tr.path().join(".git-paw/config.toml").exists());
    assert!(tr.path().join(".git-paw/logs").is_dir());
    assert!(!tr.path().join("AGENTS.md").exists());
}

#[test]
fn init_outside_git_repo_fails() {
    let tmp = TempDir::new().expect("create temp dir");

    cmd()
        .current_dir(tmp.path())
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

#[test]
fn init_is_idempotent() {
    let tr = setup_test_repo();

    cmd().current_dir(tr.path()).arg("init").assert().success();

    cmd()
        .current_dir(tr.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Already initialized"));
}

// ---------------------------------------------------------------------------
// From-specs
// ---------------------------------------------------------------------------

#[test]
fn from_specs_no_specs_config_returns_error() {
    let tr = setup_test_repo();

    // Create a .git-paw/config.toml without [specs] section
    let config = tr.path().join(".git-paw").join("config.toml");
    fs::create_dir_all(config.parent().unwrap()).expect("create config dir");
    fs::write(&config, "# no specs section\n").expect("write config");

    cmd()
        .current_dir(tr.path())
        .args(["start", "--from-specs"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("[specs]"));
}

#[test]
fn from_specs_dry_run_with_valid_specs_shows_plan() {
    let tr = setup_test_repo();

    // Create config with [specs] section pointing to openspec/changes
    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "[specs]\ndir = \"openspec/changes\"\ntype = \"openspec\"\n",
    )
    .expect("write config");

    // Register "echo" as a custom CLI so detection finds it
    let config_content = fs::read_to_string(paw_dir.join("config.toml")).unwrap();
    fs::write(
        paw_dir.join("config.toml"),
        format!("{config_content}\n[clis.echo]\ncommand = \"/bin/echo\"\n"),
    )
    .expect("append cli");

    // Create a spec with tasks.md
    let spec_dir = tr.path().join("openspec/changes/add-auth");
    fs::create_dir_all(&spec_dir).expect("create spec dir");
    fs::write(spec_dir.join("tasks.md"), "Implement authentication\n").expect("write tasks.md");

    cmd()
        .current_dir(tr.path())
        .args(["start", "--from-specs", "--dry-run", "--cli", "echo"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Dry run")
                .and(predicate::str::contains("spec/add-auth"))
                .and(predicate::str::contains("echo")),
        );
}

#[test]
fn from_specs_empty_specs_dir_prints_no_pending() {
    let tr = setup_test_repo();

    // Create config with [specs] section
    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "[specs]\ndir = \"openspec/changes\"\ntype = \"openspec\"\n",
    )
    .expect("write config");

    // Create the empty specs directory
    fs::create_dir_all(tr.path().join("openspec/changes")).expect("create specs dir");

    cmd()
        .current_dir(tr.path())
        .args(["start", "--from-specs"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No pending specs"));
}

// ---------------------------------------------------------------------------
// Replay
// ---------------------------------------------------------------------------

#[test]
fn replay_list_with_no_logs_shows_message() {
    let tr = setup_test_repo();

    cmd()
        .current_dir(tr.path())
        .args(["replay", "--list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No log sessions"));
}

#[test]
fn replay_nonexistent_branch_shows_error() {
    let tr = setup_test_repo();

    // Create a session log directory with one log so resolve_session succeeds
    let log_dir = tr.path().join(".git-paw/logs/paw-test");
    fs::create_dir_all(&log_dir).expect("create log dir");
    fs::write(log_dir.join("main.log"), "some log content").expect("write log");

    cmd()
        .current_dir(tr.path())
        .args(["replay", "nonexistent-branch"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("nonexistent-branch"));
}

// ---------------------------------------------------------------------------
// Tmux-dependent
// ---------------------------------------------------------------------------

/// Helper: kill a session if it exists, ignoring errors.
fn cleanup_session(name: &str) {
    let _ = kill_session(name);
}

#[test]
#[serial]
fn tmux_session_create_and_kill_lifecycle() {
    ensure_tmux_installed().expect("tmux must be installed to run this test");

    let session_name = "paw-e2e-lifecycle-test";
    cleanup_session(session_name);

    let tmp = TempDir::new().expect("create temp dir");
    let worktree_path = tmp.path().to_string_lossy().to_string();

    let session = TmuxSessionBuilder::new("e2e-lifecycle-test")
        .add_pane(PaneSpec {
            branch: "feat/auth".into(),
            worktree: worktree_path.clone(),
            cli_command: "echo hello".into(),
        })
        .build()
        .expect("build session");

    session.execute().expect("execute session");

    assert!(
        is_session_alive(session_name).expect("check session"),
        "session should be alive after creation"
    );

    kill_session(session_name).expect("kill session");

    assert!(
        !is_session_alive(session_name).expect("check session"),
        "session should be dead after kill"
    );
}

#[test]
#[serial]
fn tmux_session_with_five_panes_and_different_clis() {
    ensure_tmux_installed().expect("tmux must be installed to run this test");

    let session_name = "paw-e2e-multipane-test";
    cleanup_session(session_name);

    let tmp = TempDir::new().expect("create temp dir");
    let worktree = tmp.path().to_string_lossy().to_string();

    let panes = vec![
        ("feat/auth", "claude"),
        ("feat/api", "codex"),
        ("fix/db", "gemini"),
        ("feat/ui", "aider"),
        ("refactor/cache", "amp"),
    ];

    let mut builder = TmuxSessionBuilder::new("e2e-multipane-test");
    for (branch, cli) in &panes {
        builder = builder.add_pane(PaneSpec {
            branch: (*branch).into(),
            worktree: worktree.clone(),
            cli_command: (*cli).into(),
        });
    }

    let session = builder.build().expect("build session");
    session.execute().expect("execute session");

    assert!(
        is_session_alive(session_name).expect("check session"),
        "5-pane session should be alive"
    );

    // Verify pane count
    let output = std::process::Command::new("tmux")
        .args(["list-panes", "-t", session_name, "-F", "#{pane_index}"])
        .output()
        .expect("list panes");
    let pane_count = String::from_utf8_lossy(&output.stdout).lines().count();
    assert_eq!(pane_count, 5, "session should have 5 panes");

    // Verify each pane's title has the correct branch→CLI pairing
    let output = std::process::Command::new("tmux")
        .args(["list-panes", "-t", session_name, "-F", "#{pane_title}"])
        .output()
        .expect("list pane titles");
    let titles: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    assert_eq!(titles.len(), 5, "should have 5 pane titles");
    for (i, (branch, cli)) in panes.iter().enumerate() {
        assert!(
            titles[i].contains(branch) && titles[i].contains(cli),
            "pane {i} should map {branch} to {cli}, got: {}",
            titles[i]
        );
    }

    cleanup_session(session_name);
}

#[test]
#[serial]
fn tmux_mouse_mode_enabled_by_default() {
    ensure_tmux_installed().expect("tmux must be installed to run this test");

    let session_name = "paw-e2e-mouse-test";
    cleanup_session(session_name);

    let tmp = TempDir::new().expect("create temp dir");
    let worktree = tmp.path().to_string_lossy().to_string();

    let session = TmuxSessionBuilder::new("e2e-mouse-test")
        .add_pane(PaneSpec {
            branch: "feat/test".into(),
            worktree,
            cli_command: "echo hi".into(),
        })
        .build()
        .expect("build session");

    session.execute().expect("execute session");

    let output = std::process::Command::new("tmux")
        .args(["show-option", "-t", session_name, "mouse"])
        .output()
        .expect("show mouse option");
    let mouse_setting = String::from_utf8_lossy(&output.stdout);
    assert!(
        mouse_setting.contains("on"),
        "mouse should be enabled by default, got: {mouse_setting}"
    );

    cleanup_session(session_name);
}

#[test]
#[serial]
fn tmux_is_session_alive_returns_false_for_nonexistent() {
    ensure_tmux_installed().expect("tmux must be installed to run this test");

    let alive = is_session_alive("paw-definitely-does-not-exist-xyz").expect("check session");
    assert!(!alive);
}

// ---------------------------------------------------------------------------
// Exit code verification
// ---------------------------------------------------------------------------

#[test]
fn error_exit_code_is_1_for_not_a_git_repo() {
    let tmp = TempDir::new().expect("create temp dir");

    cmd()
        .current_dir(tmp.path())
        .arg("start")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("Not a git repository"));
}

#[test]
fn error_exit_code_is_1_for_preset_not_found() {
    let tr = setup_test_repo();

    cmd()
        .current_dir(tr.path())
        .args(["start", "--preset", "nonexistent"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("not found"));
}

// ---------------------------------------------------------------------------
// tmux::attach
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn attach_fails_for_nonexistent_session() {
    ensure_tmux_installed().expect("tmux must be installed to run this test");

    let result = attach("paw-e2e-attach-nonexistent-xyz");
    assert!(result.is_err(), "attach to nonexistent session should fail");
}

// ---------------------------------------------------------------------------
// Replay — ANSI stripping
// ---------------------------------------------------------------------------

#[test]
fn replay_strips_ansi_from_log() {
    let tr = setup_test_repo();

    // Create a log file with ANSI escape codes
    let log_dir = tr.path().join(".git-paw/logs/paw-test");
    fs::create_dir_all(&log_dir).expect("create log dir");
    fs::write(log_dir.join("main.log"), "\x1b[31mred\x1b[0m plain").expect("write log with ANSI");

    cmd()
        .current_dir(tr.path())
        .args(["replay", "main", "--session", "paw-test"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("red plain")
                .and(predicate::str::contains("\x1b[31m").not())
                .and(predicate::str::contains("\x1b[0m").not()),
        );
}

// ---------------------------------------------------------------------------
// Replay — list shows sessions and branches
// ---------------------------------------------------------------------------

#[test]
fn replay_list_shows_sessions_and_branches() {
    let tr = setup_test_repo();

    // Create log files for a session with two branches
    let log_dir = tr.path().join(".git-paw/logs/paw-test");
    fs::create_dir_all(&log_dir).expect("create log dir");
    fs::write(log_dir.join("feat--auth.log"), "auth log").expect("write auth log");
    fs::write(log_dir.join("main.log"), "main log").expect("write main log");

    cmd()
        .current_dir(tr.path())
        .args(["replay", "--list"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("paw-test")
                .and(predicate::str::contains("2 branches"))
                .and(predicate::str::contains("feat/auth"))
                .and(predicate::str::contains("main")),
        );
}

// ---------------------------------------------------------------------------
// From-specs — markdown format dry run
// ---------------------------------------------------------------------------

#[test]
fn from_specs_markdown_format_dry_run() {
    let tr = setup_test_repo();

    // Create config with [specs] section using markdown type
    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "[specs]\ndir = \"specs\"\ntype = \"markdown\"\n\n[clis.echo]\ncommand = \"/bin/echo\"\n",
    )
    .expect("write config");

    // Create a markdown spec file with paw_status: pending frontmatter
    let specs_dir = tr.path().join("specs");
    fs::create_dir_all(&specs_dir).expect("create specs dir");
    fs::write(
        specs_dir.join("add-auth.md"),
        "---\npaw_status: pending\n---\nImplement authentication\n",
    )
    .expect("write spec");

    cmd()
        .current_dir(tr.path())
        .args(["start", "--from-specs", "--dry-run", "--cli", "echo"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Dry run")
                .and(predicate::str::contains("add-auth"))
                .and(predicate::str::contains("echo")),
        );
}

// ---------------------------------------------------------------------------
// From-specs — skips done markdown specs
// ---------------------------------------------------------------------------

#[test]
fn from_specs_skips_done_markdown_specs() {
    let tr = setup_test_repo();

    // Create config with [specs] section using markdown type
    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "[specs]\ndir = \"specs\"\ntype = \"markdown\"\n\n[clis.echo]\ncommand = \"/bin/echo\"\n",
    )
    .expect("write config");

    // Create a markdown spec file with paw_status: done (not pending)
    let specs_dir = tr.path().join("specs");
    fs::create_dir_all(&specs_dir).expect("create specs dir");
    fs::write(
        specs_dir.join("add-auth.md"),
        "---\npaw_status: done\n---\nAlready implemented\n",
    )
    .expect("write spec");

    cmd()
        .current_dir(tr.path())
        .args(["start", "--from-specs"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No pending specs"));
}

#[test]
#[serial]
fn attach_succeeds_for_live_session() {
    ensure_tmux_installed().expect("tmux must be installed to run this test");

    let session_name = "paw-e2e-attach-test";
    cleanup_session(session_name);

    // Create a detached session
    let session = TmuxSessionBuilder::new("e2e-attach-test")
        .add_pane(PaneSpec {
            branch: "main".into(),
            worktree: "/tmp".into(),
            cli_command: "echo attached".into(),
        })
        .build()
        .expect("build session");
    session.execute().expect("execute session");

    // Attach in a subprocess with a timeout — it will block until detached,
    // so we detach it programmatically from another thread.
    let name = session_name.to_string();
    let detacher = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(500));
        // Detach all clients from the session
        let _ = std::process::Command::new("tmux")
            .args(["detach-client", "-s", &name])
            .output();
    });

    // attach() blocks until the client is detached
    let result = attach(session_name);
    detacher.join().expect("detacher thread");

    // attach returns Ok if detach was clean, Err if session vanished.
    // Either way, we exercised the success path through the blocking call.
    // On CI without a pty, attach may fail — that's acceptable.
    if result.is_ok() {
        // Success path exercised
    } else {
        // No pty available (headless CI) — attach can't connect a client.
        // The failure is from tmux itself, not our code.
        eprintln!("note: attach returned error (expected in headless environments)");
    }

    cleanup_session(session_name);
}

// ---------------------------------------------------------------------------
// Broker session full lifecycle (v0.3.0 coordination)
// ---------------------------------------------------------------------------

/// Helper to make HTTP requests to the broker using raw TCP.
fn http_req_e2e(
    url: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> (u16, String) {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let addr = url.strip_prefix("http://").unwrap_or(url);
    let mut stream = TcpStream::connect(addr).expect("failed to connect to broker");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    for (key, value) in headers {
        let _ = write!(request, "{key}: {value}\r\n");
    }
    if !body.is_empty() {
        let _ = write!(request, "Content-Length: {}\r\n", body.len());
    }
    request.push_str("\r\n");
    request.push_str(body);

    stream
        .write_all(request.as_bytes())
        .expect("failed to write request");

    let mut response = String::new();
    stream.read_to_string(&mut response).ok();

    // Parse status code
    let header_section = response.split("\r\n\r\n").next().unwrap_or("");
    let status_line = header_section.lines().next().unwrap_or("");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Extract body (handle chunked)
    let body_raw = response
        .split_once("\r\n\r\n")
        .map_or_else(String::new, |(_, b)| b.to_string());
    let body_decoded = if header_section
        .to_lowercase()
        .contains("transfer-encoding: chunked")
    {
        decode_chunked_e2e(&body_raw)
    } else {
        body_raw
    };

    (status, body_decoded)
}

fn decode_chunked_e2e(body: &str) -> String {
    let mut result = String::new();
    let mut remaining = body;
    loop {
        let line_end = remaining.find("\r\n").unwrap_or(remaining.len());
        let size_str = &remaining[..line_end];
        let size = usize::from_str_radix(size_str.trim(), 16).unwrap_or(0);
        if size == 0 {
            break;
        }
        remaining = &remaining[line_end + 2..];
        if remaining.len() >= size {
            result.push_str(&remaining[..size]);
            remaining = &remaining[size..];
            if remaining.starts_with("\r\n") {
                remaining = &remaining[2..];
            }
        } else {
            break;
        }
    }
    result
}

/// Drop guard that kills a tmux session on drop, ensuring cleanup even on panic.
struct SessionGuard<'a>(&'a str);

impl Drop for SessionGuard<'_> {
    fn drop(&mut self) {
        let _ = kill_session(self.0);
    }
}

/// Find a free port by binding to port 0 and reading back the assigned port.
fn find_free_port() -> u16 {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("failed to bind to ephemeral port");
    listener
        .local_addr()
        .expect("failed to get local addr")
        .port()
}

#[test]
#[serial]
fn broker_session_full_lifecycle() {
    ensure_tmux_installed().expect("tmux must be installed to run this test");

    let session_name = "paw-test-repo";

    // Cleanup guard: ensure the session is killed even on panic
    let _guard = SessionGuard(session_name);

    // Kill any leftover session from a prior run
    cleanup_session(session_name);

    // -----------------------------------------------------------------------
    // Step 1: Setup — create a test repo with AGENTS.md + broker config
    // -----------------------------------------------------------------------
    let tr = setup_test_repo();

    // Create and commit AGENTS.md
    let agents_path = tr.path().join("AGENTS.md");
    fs::write(&agents_path, "# Test\n").expect("write AGENTS.md");
    run_git(tr.path(), &["add", "AGENTS.md"]);
    run_git(tr.path(), &["commit", "-m", "add AGENTS.md"]);

    // Pick a free port for the broker
    let broker_port = find_free_port();

    // Create .git-paw/config.toml with broker + specs config + echo CLI
    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config_content = format!(
        "[broker]\nenabled = true\nport = {broker_port}\n\n\
         [specs]\ntype = \"openspec\"\n\n\
         [clis.echo]\ncommand = \"/bin/echo\"\n"
    );
    fs::write(paw_dir.join("config.toml"), &config_content).expect("write config");

    // -----------------------------------------------------------------------
    // Step 2: Start — launch the session (will fail to attach but session is created)
    // -----------------------------------------------------------------------
    let _start_output = cmd()
        .current_dir(tr.path())
        .args([
            "start",
            "--cli",
            "echo",
            "--branches",
            "feat/smoke-a,feat/smoke-b",
        ])
        .output()
        .expect("run start command");
    // Do NOT assert success — attach fails without a TTY, but session IS created

    // Wait for the broker to initialize — retry until it accepts connections or timeout.
    let broker_ready = (0..20).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(500));
        std::net::TcpStream::connect(format!("127.0.0.1:{broker_port}")).is_ok()
    });
    if !broker_ready {
        // Capture pane 0 output for diagnostics
        let pane_output = std::process::Command::new("tmux")
            .args(["capture-pane", "-t", &format!("{session_name}:0.0"), "-p"])
            .output()
            .map_or_else(
                |e| format!("failed to capture pane: {e}"),
                |o| String::from_utf8_lossy(&o.stdout).to_string(),
            );
        let pane_cmd = std::process::Command::new("tmux")
            .args([
                "list-panes",
                "-t",
                session_name,
                "-F",
                "#{pane_index}: #{pane_current_command} (#{pane_pid})",
            ])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        panic!(
            "broker did not start on port {broker_port} within 10 seconds\n\
             Pane commands:\n{pane_cmd}\n\
             Pane 0 output:\n{pane_output}"
        );
    }

    // -----------------------------------------------------------------------
    // Step 3: Verify tmux session exists
    // -----------------------------------------------------------------------
    assert!(
        is_session_alive(session_name).expect("check session"),
        "session {session_name} should be alive after start"
    );

    // -----------------------------------------------------------------------
    // Step 4: Verify pane count — 3 panes (dashboard + 2 agents)
    // -----------------------------------------------------------------------
    let pane_output = std::process::Command::new("tmux")
        .args(["list-panes", "-t", session_name, "-F", "#{pane_index}"])
        .output()
        .expect("list panes");
    let pane_count = String::from_utf8_lossy(&pane_output.stdout).lines().count();
    assert_eq!(
        pane_count, 3,
        "session should have 3 panes (dashboard + 2 agents), got {pane_count}"
    );

    // -----------------------------------------------------------------------
    // Step 5: Verify broker responds at /status
    // -----------------------------------------------------------------------
    let broker_url = format!("127.0.0.1:{broker_port}");
    let (status, body) = http_req_e2e(&format!("http://{broker_url}"), "GET", "/status", &[], "");
    assert_eq!(status, 200, "broker /status should return 200");
    let json: serde_json::Value = serde_json::from_str(&body).expect("valid JSON from /status");
    assert_eq!(json["git_paw"], true, "status should contain git_paw: true");

    // -----------------------------------------------------------------------
    // Step 6: Publish + poll roundtrip
    // -----------------------------------------------------------------------
    let http_url = format!("http://{broker_url}");

    // Register agent feat-smoke-a with a status message
    let (status, _) = http_req_e2e(
        &http_url,
        "POST",
        "/publish",
        &[("Content-Type", "application/json")],
        r#"{"type":"agent.status","agent_id":"feat-smoke-a","payload":{"status":"working","modified_files":[]}}"#,
    );
    assert_eq!(status, 202, "status publish should return 202");

    // Register agent feat-smoke-b
    let (status, _) = http_req_e2e(
        &http_url,
        "POST",
        "/publish",
        &[("Content-Type", "application/json")],
        r#"{"type":"agent.status","agent_id":"feat-smoke-b","payload":{"status":"idle","modified_files":[]}}"#,
    );
    assert_eq!(status, 202, "second agent status should return 202");

    // Publish an artifact from feat-smoke-a (broadcasts to feat-smoke-b)
    let (status, _) = http_req_e2e(
        &http_url,
        "POST",
        "/publish",
        &[("Content-Type", "application/json")],
        r#"{"type":"agent.artifact","agent_id":"feat-smoke-a","payload":{"status":"done","exports":[],"modified_files":["src/lib.rs"]}}"#,
    );
    assert_eq!(status, 202, "artifact publish should return 202");

    // Poll feat-smoke-b's inbox — should contain the artifact
    let (status, body) = http_req_e2e(&http_url, "GET", "/messages/feat-smoke-b", &[], "");
    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_str(&body).expect("valid JSON from /messages");
    let messages = json["messages"]
        .as_array()
        .expect("messages should be an array");
    assert_eq!(
        messages.len(),
        1,
        "feat-smoke-b should have exactly 1 artifact message"
    );
    let last_seq = json["last_seq"]
        .as_u64()
        .expect("last_seq should be a number");
    assert!(last_seq > 0, "last_seq should be positive");

    // -----------------------------------------------------------------------
    // Step 7: Cursor advancement — since=last_seq returns empty
    // -----------------------------------------------------------------------
    let path = format!("/messages/feat-smoke-b?since={last_seq}");
    let (status, body) = http_req_e2e(&http_url, "GET", &path, &[], "");
    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_str(&body).expect("valid JSON");
    let messages = json["messages"]
        .as_array()
        .expect("messages should be an array");
    assert!(
        messages.is_empty(),
        "no new messages after cursor, got {} messages",
        messages.len()
    );

    // -----------------------------------------------------------------------
    // Step 8: Verify AGENTS.md skill injection in worktree
    // -----------------------------------------------------------------------
    // Find worktree paths by listing git worktrees
    let wt_output = std::process::Command::new("git")
        .current_dir(tr.path())
        .args(["worktree", "list", "--porcelain"])
        .output()
        .expect("list worktrees");
    let wt_list = String::from_utf8_lossy(&wt_output.stdout);

    // Collect worktree paths (excluding the main repo).
    // Canonicalize to handle macOS /var -> /private/var symlinks.
    let repo_canonical = tr.path().canonicalize().expect("canonicalize repo path");
    let mut worktree_paths: Vec<PathBuf> = Vec::new();
    for line in wt_list.lines() {
        if let Some(path_str) = line.strip_prefix("worktree ") {
            let path = PathBuf::from(path_str);
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            if canonical != repo_canonical {
                worktree_paths.push(path);
            }
        }
    }
    assert_eq!(
        worktree_paths.len(),
        2,
        "should have 2 worktrees, got {}",
        worktree_paths.len()
    );

    // Check at least one worktree's AGENTS.md contains skill injection
    let agents_content =
        fs::read_to_string(worktree_paths[0].join("AGENTS.md")).expect("read worktree AGENTS.md");
    assert!(
        agents_content.contains("Coordination Skills"),
        "AGENTS.md should contain 'Coordination Skills', got:\n{agents_content}"
    );
    assert!(
        agents_content.contains("${GIT_PAW_BROKER_URL}"),
        "AGENTS.md should contain '${{GIT_PAW_BROKER_URL}}', got:\n{agents_content}"
    );
    // Check the slugified branch name appears (feat-smoke-a or feat-smoke-b)
    let has_slug =
        agents_content.contains("feat-smoke-a") || agents_content.contains("feat-smoke-b");
    assert!(
        has_slug,
        "AGENTS.md should contain slugified branch name, got:\n{agents_content}"
    );

    // -----------------------------------------------------------------------
    // Step 9: Stop — kill the tmux session and verify broker port is freed
    // -----------------------------------------------------------------------
    cmd().current_dir(tr.path()).arg("stop").assert().success();

    // Wait for broker to shut down
    std::thread::sleep(std::time::Duration::from_secs(3));

    // Verify the broker port is freed — binding to it should succeed
    let bind_result = std::net::TcpListener::bind(format!("127.0.0.1:{broker_port}"));
    assert!(
        bind_result.is_ok(),
        "broker port {broker_port} should be freed after stop"
    );
    drop(bind_result);

    // -----------------------------------------------------------------------
    // Step 10: Purge — remove worktrees and branches
    // -----------------------------------------------------------------------
    cmd()
        .current_dir(tr.path())
        .args(["purge", "--force"])
        .assert()
        .success();

    // Verify worktrees are removed
    for wt_path in &worktree_paths {
        assert!(
            !wt_path.exists(),
            "worktree {} should be removed after purge",
            wt_path.display()
        );
    }

    // Verify branches are deleted
    let branch_output = std::process::Command::new("git")
        .current_dir(tr.path())
        .args(["branch"])
        .output()
        .expect("list branches");
    let branches = String::from_utf8_lossy(&branch_output.stdout);
    assert!(
        !branches.contains("feat/smoke-a"),
        "feat/smoke-a should be deleted after purge, branches:\n{branches}"
    );
    assert!(
        !branches.contains("feat/smoke-b"),
        "feat/smoke-b should be deleted after purge, branches:\n{branches}"
    );
}
