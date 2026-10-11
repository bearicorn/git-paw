//! E2E observable: a pane launches the CLI's configured
//! `[clis.<name>].command`, while the `[clis.<name>]` KEY stays the identity
//! every per-CLI lookup resolves against.
//!
//! Maps to `cli-resolution` scenarios:
//!
//! - `A custom CLI whose key differs from its command launches the command`
//! - `Saved worktree entry records the key`
//! - `Per-CLI overrides resolve against the key`
//! - `Supervisor pane launches the configured command`
//! - `Recovery relaunches the configured command`
//!
//! and the `boot-block` scenario `Launch path renders the opted-in form`.
//!
//! The CLI under test is always a stub script whose *file name* differs from
//! its `[clis]` key, so a pane that launched the key instead of the command
//! would fail to start at all. The stub records its own argv and every line
//! it is sent on stdin into a log file, which is how these tests observe both
//! the launch command and the injected boot prompt without depending on the
//! pane's column width.
//!
//! Skips if tmux is unavailable.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

/// The stub's binary name. Deliberately unlike every `[clis]` key used below,
/// so "the pane ran the command, not the key" is directly observable.
const STUB_BIN: &str = "paw-launch-probe";

/// Prefix the stub prints for its own argv, as its very first output.
const ARGV_MARKER: &str = "PAW_LAUNCHED_ARGS:";

/// Approval-flag sentinel for the worker-override test, mapped to `auto`.
const OSS_SENTINEL: &str = "--paw-oss-sentinel";

/// Approval-flag sentinel for the supervisor test, mapped to `full-auto`.
const SUP_SENTINEL: &str = "--paw-sup-sentinel";

/// The CLI session id the recovery test's saved receipt carries.
const SESSION_ID: &str = "11111111-2222-3333-4444-555555555555";

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// Atomic counter so each test gets a unique tmux project name.
static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_project_name(tag: &str) -> String {
    let pid = std::process::id();
    let n = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("launchid-{tag}-{pid}-{n}")
}

/// Binds an ephemeral TCP port and returns it, so the broker-enabled config
/// below never collides with another test's port.
fn find_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral")
        .local_addr()
        .expect("local_addr")
        .port()
}

fn skip_if_no_tmux() -> bool {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return true;
    }
    false
}

/// Renames the test repo's basename so `tmux::resolve_session_name` produces
/// a deterministic, unique session name the test can target.
fn rename_repo_basename(tr: &TestRepo, new_basename: &str) -> std::path::PathBuf {
    let original = tr.path().to_path_buf();
    let parent = original.parent().expect("repo has parent").to_path_buf();
    let renamed = parent.join(new_basename);
    fs::rename(&original, &renamed).expect("rename repo dir");
    renamed
}

/// Writes the executable stub at `<bin_dir>/paw-launch-probe`.
///
/// It appends `PAW_LAUNCHED_ARGS:<argv>` to `log_path` the moment it starts —
/// so the log exists *only* if the pane actually spawned this binary — then
/// appends every terminated line it reads on stdin, which is how the injected
/// boot prompt becomes observable.
///
/// `bin_dir` MUST be prepended to the test process's `PATH`: tmux resolves the
/// pane's command against its inherited `PATH`.
fn write_stub_cli(bin_dir: &std::path::Path, log_path: &std::path::Path) {
    let script = format!(
        "#!/bin/sh\nprintf '{ARGV_MARKER}%s\\n' \"$*\" >> '{log}'\n\
         while IFS= read -r line; do\n  printf '%s\\n' \"$line\" >> '{log}'\ndone\n",
        log = log_path.display()
    );
    let script_path = bin_dir.join(STUB_BIN);
    fs::write(&script_path, script).expect("write stub script");
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).expect("chmod stub");
}

/// Polls `log_path` until it contains `needle` or `timeout` elapses. Returns
/// the final contents either way (callers assert).
fn wait_for_log(log_path: &std::path::Path, needle: &str, timeout: Duration) -> String {
    let deadline = Instant::now() + timeout;
    let mut buffer = String::new();
    while Instant::now() < deadline {
        buffer = fs::read_to_string(log_path).unwrap_or_default();
        if buffer.contains(needle) {
            break;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    buffer
}

/// Captures `pane` of `session`, joining soft-wrapped lines (`-J`) so a
/// distinctive substring survives regardless of the pane's column width.
fn capture_pane(env: &TmuxTestEnv, session: &str, pane: usize) -> String {
    let target = format!("{session}:0.{pane}");
    let mut capture_cmd = StdCommand::new("tmux");
    env.apply(&mut capture_cmd);
    let out = capture_cmd
        .args(["capture-pane", "-t", &target, "-p", "-J", "-S", "-2000"])
        .output()
        .expect("tmux capture-pane");
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// Polls `pane` until its capture contains `needle`, or `timeout` elapses.
fn wait_for_pane_content(
    env: &TmuxTestEnv,
    session: &str,
    pane: usize,
    needle: &str,
    timeout: Duration,
) -> String {
    let deadline = Instant::now() + timeout;
    let mut buffer = String::new();
    while Instant::now() < deadline {
        buffer = capture_pane(env, session, pane);
        if buffer.contains(needle) {
            break;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    buffer
}

fn kill_session(env: &TmuxTestEnv, name: &str) {
    let mut kill_cmd = StdCommand::new("tmux");
    env.apply(&mut kill_cmd);
    let _ = kill_cmd.args(["kill-session", "-t", name]).status();
}

/// The sessions directory under a fake `HOME`.
fn sessions_dir_for(home: &std::path::Path) -> std::path::PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-paw/sessions")
    } else {
        home.join(".local/share/git-paw/sessions")
    }
}

/// What `git rev-parse --show-toplevel` reports for `repo` — the form
/// `find_session_for_repo` compares against (NOT `fs::canonicalize`, which
/// resolves `/var` -> `/private/var` on macOS and would never match).
fn toplevel_of(repo: &std::path::Path) -> std::path::PathBuf {
    let out = StdCommand::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .expect("git rev-parse");
    assert!(out.status.success(), "git rev-parse must succeed");
    std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
}

/// A bin dir on PATH holding the stub, plus the log it writes to.
struct Stub {
    _bin_dir: TempDir,
    _log_dir: TempDir,
    log: std::path::PathBuf,
    path_env: String,
}

fn stub() -> Stub {
    let bin_dir = TempDir::new().expect("stub bin dir");
    let log_dir = TempDir::new().expect("stub log dir");
    let log = log_dir.path().join("stub.log");
    write_stub_cli(bin_dir.path(), &log);
    let inherited = std::env::var("PATH").unwrap_or_default();
    let path_env = format!("{}:{inherited}", bin_dir.path().display());
    Stub {
        _bin_dir: bin_dir,
        _log_dir: log_dir,
        log,
        path_env,
    }
}

// ---------------------------------------------------------------------------
// Worker panes
// ---------------------------------------------------------------------------

/// `[clis.claude-variant] command = <stub>`: the worker pane SHALL launch the
/// command. The key is not a binary, so the stub's log existing at all is
/// proof the substitution happened.
#[test]
#[serial]
fn worker_pane_launches_configured_command_not_key() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();
    let project = unique_project_name("worker");
    let repo = rename_repo_basename(&tr, &project);
    let st = stub();

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        format!("[clis.claude-variant]\ncommand = \"{STUB_BIN}\"\n"),
    )
    .expect("write config");

    let home = TempDir::new().expect("home tempdir");
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(&repo)
        .env("HOME", home.path())
        .env("PATH", &st.path_env)
        .env_remove("XDG_DATA_HOME")
        .args([
            "start",
            "--no-supervisor",
            "--cli",
            "claude-variant",
            "--branches",
            "a",
        ])
        .timeout(Duration::from_secs(20))
        .output()
        .expect("run start");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "start failed; stdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let session_name = format!("paw-{project}");
    let log = wait_for_log(&st.log, ARGV_MARKER, Duration::from_secs(10));
    let pane0 = capture_pane(&tmux_env, &session_name, 0);
    kill_session(&tmux_env, &session_name);

    assert!(
        log.contains(ARGV_MARKER),
        "the configured command must have actually been spawned; log:\n{log}\npane:\n{pane0}"
    );
    assert!(
        pane0.contains(STUB_BIN),
        "pane must show the configured command; capture:\n{pane0}"
    );
    assert!(
        !pane0.contains("claude-variant"),
        "the [clis] key is not a binary and must never be launched; capture:\n{pane0}"
    );
}

/// The saved session records the CLI *identity* (the key), not the command it
/// resolved to — so recovery can resolve the same per-CLI overrides again.
#[test]
#[serial]
fn saved_session_records_cli_key() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();
    let project = unique_project_name("saved");
    let repo = rename_repo_basename(&tr, &project);
    let st = stub();

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        format!("[clis.claude-oss]\ncommand = \"{STUB_BIN}\"\n"),
    )
    .expect("write config");

    let home = TempDir::new().expect("home tempdir");
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(&repo)
        .env("HOME", home.path())
        .env("PATH", &st.path_env)
        .env_remove("XDG_DATA_HOME")
        .args([
            "start",
            "--no-supervisor",
            "--cli",
            "claude-oss",
            "--branches",
            "a",
        ])
        .timeout(Duration::from_secs(20))
        .output()
        .expect("run start");
    assert!(
        out.status.success(),
        "start failed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let session_name = format!("paw-{project}");
    kill_session(&tmux_env, &session_name);

    let receipt = sessions_dir_for(home.path()).join(format!("{session_name}.json"));
    let saved = fs::read_to_string(&receipt)
        .unwrap_or_else(|e| panic!("read session receipt {}: {e}", receipt.display()));
    let parsed: serde_json::Value = serde_json::from_str(&saved).expect("parse session receipt");
    let entry_cli = parsed["worktrees"][0]["cli"]
        .as_str()
        .expect("worktree entry has a cli");

    assert_eq!(
        entry_cli, "claude-oss",
        "the saved entry must carry the identity, not the resolved command; receipt:\n{saved}"
    );
}

/// Per-CLI overrides resolve against the KEY, and the resolved flag is
/// appended after the substituted COMMAND.
#[test]
#[serial]
fn worker_overrides_resolve_by_key_and_launch_command() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();
    let project = unique_project_name("override");
    let repo = rename_repo_basename(&tr, &project);
    let st = stub();

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    // The override is keyed on `claude-oss`; it would not be found under the
    // stub's own binary name.
    fs::write(
        paw_dir.join("config.toml"),
        format!(
            "[supervisor]\nenabled = false\nagent_approval = \"auto\"\n\n\
             [clis.claude-oss]\ncommand = \"{STUB_BIN}\"\n\
             approval_args = {{ auto = \"{OSS_SENTINEL}\" }}\n"
        ),
    )
    .expect("write config");

    let home = TempDir::new().expect("home tempdir");
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(&repo)
        .env("HOME", home.path())
        .env("PATH", &st.path_env)
        .env_remove("XDG_DATA_HOME")
        .args([
            "start",
            "--no-supervisor",
            "--cli",
            "claude-oss",
            "--branches",
            "a",
        ])
        .timeout(Duration::from_secs(20))
        .output()
        .expect("run start");
    assert!(
        out.status.success(),
        "start failed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let session_name = format!("paw-{project}");
    let log = wait_for_log(&st.log, ARGV_MARKER, Duration::from_secs(10));
    let pane0 = capture_pane(&tmux_env, &session_name, 0);
    kill_session(&tmux_env, &session_name);

    // The stub reports its own argv: the sentinel reached the spawned
    // process, which proves the flag was both resolved under the key and
    // appended after the substituted command.
    assert!(
        log.contains(&format!("{ARGV_MARKER}{OSS_SENTINEL}")),
        "the override flag must be resolved by key and appended after the command; \
         log:\n{log}\npane:\n{pane0}"
    );
    assert!(
        !pane0.contains("claude-oss"),
        "the key must not be launched; capture:\n{pane0}"
    );
}

// ---------------------------------------------------------------------------
// Supervisor pane
// ---------------------------------------------------------------------------

/// `[supervisor] cli = "claude-oss"` with `[clis.claude-oss] command = <stub>`:
/// pane 0 launches the command followed by the supervisor's approval flag.
#[test]
#[serial]
fn supervisor_pane_launches_configured_command() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();
    let project = unique_project_name("super");
    let repo = rename_repo_basename(&tr, &project);
    let st = stub();

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        format!(
            "[supervisor]\nenabled = true\ncli = \"claude-oss\"\n\
             approval = \"full-auto\"\nagent_approval = \"auto\"\n\n\
             [clis.claude-oss]\ncommand = \"{STUB_BIN}\"\n\
             approval_args = {{ \"full-auto\" = \"{SUP_SENTINEL}\" }}\n"
        ),
    )
    .expect("write config");

    let home = TempDir::new().expect("home tempdir");
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(&repo)
        .env("HOME", home.path())
        .env("PATH", &st.path_env)
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--supervisor", "--cli", "echo", "--branches", "a"])
        .timeout(Duration::from_secs(25))
        .output()
        .expect("run start");
    assert!(
        out.status.success(),
        "supervisor start failed; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let session_name = format!("paw-{project}");
    // Pane 0 = supervisor. The agent uses `echo`, so only the supervisor can
    // have produced the stub's log.
    let pane0 = wait_for_pane_content(
        &tmux_env,
        &session_name,
        0,
        SUP_SENTINEL,
        Duration::from_secs(15),
    );
    let log = wait_for_log(&st.log, ARGV_MARKER, Duration::from_secs(5));
    kill_session(&tmux_env, &session_name);

    assert!(
        pane0.contains(STUB_BIN) && pane0.contains(SUP_SENTINEL),
        "supervisor pane 0 must launch the configured command plus its flag; capture:\n{pane0}"
    );
    assert!(
        !pane0.contains("claude-oss"),
        "the supervisor [clis] key must not be launched; capture:\n{pane0}"
    );
    assert!(
        log.contains(&format!("{ARGV_MARKER}{SUP_SENTINEL}")),
        "the supervisor's configured command must have actually been spawned; log:\n{log}"
    );
}

// ---------------------------------------------------------------------------
// Recovery
// ---------------------------------------------------------------------------

/// A saved entry with `cli = "claude-oss"` and a `cli_session_id` relaunches
/// the configured command followed by the resume arguments rendered from the
/// identity's template.
#[test]
#[serial]
fn recovery_launches_configured_command_with_resume_args() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();
    let project = unique_project_name("recover");
    let repo = rename_repo_basename(&tr, &project);
    let canonical_repo = toplevel_of(&repo);
    let st = stub();

    // `recover_session` expects the saved worktree to already exist on disk.
    let branch = "feat/alpha";
    let wt_path = {
        let parent = repo.parent().expect("repo has parent");
        let wt = parent.join(format!("{project}-feat-alpha"));
        let status = StdCommand::new("git")
            .current_dir(&repo)
            .args(["branch", branch])
            .status()
            .expect("git branch");
        assert!(status.success(), "git branch must succeed");
        let status = StdCommand::new("git")
            .current_dir(&repo)
            .args(["worktree", "add", &wt.to_string_lossy(), branch])
            .status()
            .expect("git worktree add");
        assert!(status.success(), "git worktree add must succeed");
        wt
    };

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        format!(
            "[clis.claude-oss]\ncommand = \"{STUB_BIN}\"\n\
             resume_args = \"--resume {{{{SESSION_ID}}}}\"\n"
        ),
    )
    .expect("write config");

    let fake_home = TempDir::new().expect("create temp HOME");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    let session_name = format!("paw-{project}");
    let session_json = serde_json::json!({
        "session_name": session_name,
        "repo_path": canonical_repo.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-01-01T00:00:00Z",
        "status": "stopped",
        "mode": "bare",
        "worktrees": [{
            "branch": branch,
            "worktree_path": wt_path.to_string_lossy(),
            "cli": "claude-oss",
            "branch_created": true,
            "cli_session_id": SESSION_ID,
        }],
    });
    fs::write(
        sessions_dir.join(format!("{session_name}.json")),
        serde_json::to_string_pretty(&session_json).expect("serialize session"),
    )
    .expect("write session json");

    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(&repo)
        .env("HOME", fake_home.path())
        .env("PATH", &st.path_env)
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--no-supervisor"])
        .timeout(Duration::from_secs(20))
        .output()
        .expect("run start");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        stdout.contains("Recovering session"),
        "start must take the recovery path; stdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let resume = format!("--resume {SESSION_ID}");
    let pane0 = wait_for_pane_content(
        &tmux_env,
        &session_name,
        0,
        &resume,
        Duration::from_secs(15),
    );
    let log = wait_for_log(&st.log, ARGV_MARKER, Duration::from_secs(5));
    kill_session(&tmux_env, &session_name);

    assert!(
        pane0.contains(STUB_BIN) && pane0.contains(&resume),
        "the recovered pane must launch the configured command plus the rendered \
         resume args; capture:\n{pane0}"
    );
    assert!(
        !pane0.contains("claude-oss"),
        "the saved key must not be launched; capture:\n{pane0}"
    );
    assert!(
        log.contains(&format!("{ARGV_MARKER}{resume}")),
        "the configured command must have actually been spawned with the resume \
         args; log:\n{log}"
    );
}

// ---------------------------------------------------------------------------
// MCP opt-in
// ---------------------------------------------------------------------------

/// `[clis.<name>] mcp = true` on a CLI the built-in table does not know makes
/// the launch path deliver the MCP tool form of the boot block, not the
/// `broker.sh` helper form.
#[test]
#[serial]
fn mcp_opt_in_reaches_worker_boot_prompt() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();
    let project = unique_project_name("mcp");
    let repo = rename_repo_basename(&tr, &project);
    let st = stub();
    let broker_port = find_free_port();

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        format!(
            "[broker]\nenabled = true\nport = {broker_port}\n\n\
             [clis.paw-stub]\ncommand = \"{STUB_BIN}\"\nmcp = true\n"
        ),
    )
    .expect("write config");

    let home = TempDir::new().expect("home tempdir");
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(&repo)
        .env("HOME", home.path())
        .env("PATH", &st.path_env)
        .env_remove("XDG_DATA_HOME")
        .args([
            "start",
            "--no-supervisor",
            "--cli",
            "paw-stub",
            "--branches",
            "a",
        ])
        .timeout(Duration::from_secs(25))
        .output()
        .expect("run start");
    assert!(
        out.status.success(),
        "start failed; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let session_name = format!("paw-{project}");
    // The stub logs every line it is sent on stdin, so the delivered boot
    // prompt is observable without depending on pane wrapping.
    let log = wait_for_log(&st.log, "publish_status", Duration::from_secs(20));
    kill_session(&tmux_env, &session_name);

    assert!(
        log.contains("publish_status"),
        "an mcp = true CLI must receive the MCP tool form of the boot block; log:\n{log}"
    );
    // The two forms differ in how the four events are expressed, so compare
    // the REGISTER step specifically. (The MCP template still names
    // `broker.sh` once, for the poll the MCP surface has no tool for, so a
    // bare `broker.sh` search would not discriminate.)
    assert!(
        !log.contains("broker.sh --agent a status booting"),
        "the helper-script publish form must NOT be delivered to an opted-in CLI; log:\n{log}"
    );
}
