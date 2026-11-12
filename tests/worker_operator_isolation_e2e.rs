//! E2E: claude-family workers launch with the operator isolation, on every
//! worker path, and git-paw touches no agent-CLI settings file doing it.
//!
//! Maps to:
//!
//! - `cli-provider` "Start and add stay identical" and "Recovery keeps the
//!   isolation" (through `git paw start` on a stopped session).
//! - `core-memory-isolation` "A worker does not run the operator's user
//!   plugins or hooks", "A worker does not write the shared auto memory" and
//!   "Repository settings still apply to an isolated worker". What Claude Code
//!   does with the flag and the variables is the recorded fact gate
//!   (`worker-operator-isolation` design D5); these tests observe that a real
//!   launch hands them to the CLI.
//!
//! The CLI is a stub named `claude` on `PATH` that records its argv and the
//! two isolation variables, distinguishing unset from empty, then reads stdin
//! so the pane stays alive. The launching environment sets
//! `CLAUDE_CODE_PLUGIN_DIRS`, as an operator's would.
//!
//! Skips if tmux is unavailable.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

/// Prefix of each line the stub records.
const MARKER: &str = "PAW_ISO:";

/// The plugin directory list the operator's environment carries.
const OPERATOR_PLUGIN_DIRS: &str = "/operator/plugins";

/// The CLI session id the recovery test's saved receipt carries.
const SESSION_ID: &str = "11111111-2222-3333-4444-555555555555";

/// What every isolated launch hands the CLI, after its argv.
const ISOLATED_ENV: &str = "|MEM=1|PLUG=";

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_project_name(tag: &str) -> String {
    let n = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("opiso-{tag}-{}-{n}", std::process::id())
}

fn find_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral")
        .local_addr()
        .expect("local_addr")
        .port()
}

fn tmux_available() -> bool {
    which::which("tmux").is_ok()
}

fn rename_repo_basename(tr: &TestRepo, new_basename: &str) -> PathBuf {
    let original = tr.path().to_path_buf();
    let renamed = original
        .parent()
        .expect("repo has parent")
        .join(new_basename);
    fs::rename(&original, &renamed).expect("rename repo dir");
    renamed
}

/// A bin dir holding the `claude` stub, its log, and a `PATH` with it first.
struct Stub {
    _bin_dir: TempDir,
    _log_dir: TempDir,
    log: PathBuf,
    path_env: String,
}

fn stub() -> Stub {
    let bin_dir = TempDir::new().expect("stub bin dir");
    let log_dir = TempDir::new().expect("stub log dir");
    let log = log_dir.path().join("stub.log");
    let script = format!(
        "#!/bin/sh\nprintf '{MARKER}%s|MEM=%s|PLUG=%s\\n' \"$*\" \
         \"${{CLAUDE_CODE_DISABLE_AUTO_MEMORY-unset}}\" \
         \"${{CLAUDE_CODE_PLUGIN_DIRS-unset}}\" >> '{log}'\n\
         while IFS= read -r line; do :; done\n",
        log = log.display()
    );
    let path = bin_dir.path().join("claude");
    fs::write(&path, script).expect("write stub");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod stub");
    let path_env = format!(
        "{}:{}",
        bin_dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    Stub {
        _bin_dir: bin_dir,
        _log_dir: log_dir,
        log,
        path_env,
    }
}

/// The stub's recorded launches, once `count` of them exist or `timeout`
/// elapses.
fn wait_for_launches(log: &Path, count: usize, timeout: Duration) -> Vec<String> {
    let deadline = Instant::now() + timeout;
    loop {
        let lines: Vec<String> = fs::read_to_string(log)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.strip_prefix(MARKER).map(str::to_string))
            .collect();
        if lines.len() >= count || Instant::now() >= deadline {
            return lines;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

fn kill_session(env: &TmuxTestEnv, name: &str) {
    let mut kill = StdCommand::new("tmux");
    env.apply(&mut kill);
    let _ = kill.args(["kill-session", "-t", name]).status();
}

fn sessions_dir_for(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-paw/sessions")
    } else {
        home.join(".local/share/git-paw/sessions")
    }
}

fn toplevel_of(repo: &Path) -> PathBuf {
    let out = StdCommand::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .expect("git rev-parse");
    PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
}

/// Every `.claude/settings*.json` under `root`, so a test can assert git-paw
/// created none.
fn claude_settings_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != ".git") {
                    stack.push(path);
                }
            } else if dir.file_name().is_some_and(|n| n == ".claude")
                && path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("settings"))
                && path.extension().is_some_and(|e| e == "json")
            {
                found.push(path);
            }
        }
    }
    found
}

/// Runs `git paw <args>` in `repo` with the stub on `PATH` and the operator's
/// plugin directories in the environment.
fn paw(
    repo: &Path,
    home: &Path,
    tmux_env: &TmuxTestEnv,
    st: &Stub,
    args: &[&str],
) -> std::process::Output {
    let mut c = cmd();
    tmux_env.apply_assert(&mut c);
    c.current_dir(repo)
        .env("HOME", home)
        .env("PATH", &st.path_env)
        .env("CLAUDE_CODE_PLUGIN_DIRS", OPERATOR_PLUGIN_DIRS)
        .env_remove("XDG_DATA_HOME")
        .args(args)
        .timeout(Duration::from_secs(60))
        .output()
        .expect("run git paw")
}

/// Scenarios "Start and add stay identical", "A worker does not run the
/// operator's user plugins or hooks", "A worker does not write the shared
/// auto memory" and "Repository settings still apply to an isolated worker".
#[test]
#[serial]
fn start_and_add_launch_claude_workers_with_identical_isolation() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available on PATH");
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();
    let project = unique_project_name("startadd");
    let repo = rename_repo_basename(&tr, &project);
    let st = stub();
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        format!(
            "default_cli = \"opus\"\n\n[clis.opus]\ncommand = \"claude\"\nfamily = \"claude\"\n\
             args = \"--model opus\"\n\n[broker]\nenabled = true\nport = {}\n",
            find_free_port()
        ),
    )
    .expect("write config");
    let home = TempDir::new().expect("home tempdir");
    let session = format!("paw-{project}");

    let start = paw(
        &repo,
        home.path(),
        &tmux_env,
        &st,
        &[
            "start",
            "--no-supervisor",
            "--cli",
            "opus",
            "--branches",
            "a",
        ],
    );
    let started = wait_for_launches(&st.log, 1, Duration::from_secs(20));
    let add = paw(
        &repo,
        home.path(),
        &tmux_env,
        &st,
        &["add", "b", "--cli", "opus"],
    );
    let launches = wait_for_launches(&st.log, 2, Duration::from_secs(20));
    let settings = claude_settings_files(&repo);
    kill_session(&tmux_env, &session);

    assert!(
        start.status.success(),
        "start failed; stderr:\n{}",
        String::from_utf8_lossy(&start.stderr)
    );
    assert_eq!(started.len(), 1, "start launches one worker: {started:?}");
    assert!(
        add.status.success(),
        "add failed; stderr:\n{}",
        String::from_utf8_lossy(&add.stderr)
    );
    assert_eq!(launches.len(), 2, "start + add: {launches:?}");
    for launch in &launches {
        assert!(
            launch.starts_with("--model opus --setting-sources project,local"),
            "the user setting source is excluded: {launch}"
        );
        assert!(
            launch.ends_with(ISOLATED_ENV),
            "auto memory off and the operator's plugin dirs emptied: {launch}"
        );
    }
    let isolation = |l: &str| {
        let (args, env) = l.split_once('|').expect("argv|env");
        let without_session = args.split(" --session-id").next().unwrap_or(args);
        (without_session.to_string(), env.to_string())
    };
    assert_eq!(
        isolation(&launches[0]),
        isolation(&launches[1]),
        "start and add carry the same isolation"
    );
    assert!(
        settings.is_empty(),
        "git-paw must create no Claude Code settings file: {settings:?}"
    );
}

/// Scenario "Recovery keeps the isolation", through `git paw start` on a
/// stopped session whose entry is the built-in `claude`.
#[test]
#[serial]
fn recovery_through_start_keeps_the_isolation() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available on PATH");
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();
    let project = unique_project_name("recover");
    let repo = rename_repo_basename(&tr, &project);
    let st = stub();

    let branch = "feat/alpha";
    let wt = repo
        .parent()
        .expect("repo has parent")
        .join(format!("{project}-feat-alpha"));
    for args in [
        vec!["branch", branch],
        vec!["worktree", "add", &*wt.to_string_lossy(), branch],
    ] {
        let status = StdCommand::new("git")
            .current_dir(&repo)
            .args(&args)
            .status()
            .expect("git");
        assert!(status.success(), "git {args:?}");
    }

    let home = TempDir::new().expect("home tempdir");
    let sessions_dir = sessions_dir_for(home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");
    let session = format!("paw-{project}");
    let receipt = serde_json::json!({
        "session_name": session,
        "repo_path": toplevel_of(&repo).to_string_lossy(),
        "project_name": project,
        "created_at": "2026-01-01T00:00:00Z",
        "status": "stopped",
        "mode": "bare",
        "worktrees": [{
            "branch": branch,
            "worktree_path": wt.to_string_lossy(),
            "cli": "claude",
            "branch_created": true,
            "cli_session_id": SESSION_ID,
        }],
    });
    fs::write(
        sessions_dir.join(format!("{session}.json")),
        serde_json::to_string_pretty(&receipt).expect("serialize receipt"),
    )
    .expect("write receipt");

    let out = paw(
        &repo,
        home.path(),
        &tmux_env,
        &st,
        &["start", "--no-supervisor"],
    );
    let launches = wait_for_launches(&st.log, 1, Duration::from_secs(20));
    kill_session(&tmux_env, &session);

    assert!(
        String::from_utf8_lossy(&out.stdout).contains("Recovering session"),
        "start must take the recovery path; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(launches.len(), 1, "one recovered worker: {launches:?}");
    let launch = &launches[0];
    let resume = format!("--resume {SESSION_ID}{ISOLATED_ENV}");
    assert!(
        launch.starts_with("--setting-sources project,local ") && launch.ends_with(&resume),
        "the recovered worker resumes isolated, the setting sources before --resume: {launch}"
    );
}
