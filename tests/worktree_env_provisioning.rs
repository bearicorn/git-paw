//! Env-file provisioning integration tests (`session-runtime-isolation`).
//!
//! Covers the spec requirements:
//!
//! - *Env-file provisioning copies declared files into each worktree* — the
//!   declared file lands in the new worktree, the copy is independent of the
//!   source (not a symlink), a missing declared file is skipped without
//!   failing the create, and provisioning precedes the agent launch.
//! - *Runtime provisioning is opt-in and backward compatible* — absent
//!   configuration leaves the worktree exactly as a prior version created it.
//!
//! Each test creates real git worktrees under a `tempfile` sandbox and drives
//! the provisioning seam the add/start command flows call. The launch-ordering
//! scenario drives the CLI end-to-end through tmux, using a stub agent CLI that
//! records what it can see in its worktree the moment it starts.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::{Duration, Instant};

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

use git_paw::config::{WorktreeConfig, WorktreeEnvConfig, WorktreePortsConfig};
use git_paw::git;
use git_paw::worktree_provision::{ENV_LOCAL_FILE, provision_worktree};

mod helpers;

struct Sandbox {
    _dir: TempDir,
    repo: PathBuf,
}

fn run_git(dir: &Path, args: &[&str]) {
    let output = StdCommand::new("git")
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

/// Creates a sandbox holding an initialized repo at `<sandbox>/repo`, so
/// sibling-placement worktrees land inside the sandbox and are cleaned up with
/// it.
fn sandbox_repo() -> Sandbox {
    let dir = TempDir::new().expect("create temp dir");
    let repo = dir.path().join("repo");
    fs::create_dir_all(&repo).expect("create repo dir");

    run_git(&repo, &["init", "-q", "-b", "main"]);
    run_git(&repo, &["config", "user.email", "test@test.com"]);
    run_git(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("README.md"), "# test").expect("write README");
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-q", "-m", "initial"]);

    Sandbox { _dir: dir, repo }
}

/// Creates a worktree for `branch` the way the command flows do.
fn create_worktree(repo: &Path, branch: &str) -> PathBuf {
    git::create_worktree(
        repo,
        branch,
        false,
        git_paw::config::WorktreePlacement::Sibling,
    )
    .expect("create worktree")
    .path
}

fn env_only(copy: &[&str]) -> WorktreeConfig {
    WorktreeConfig {
        env: Some(WorktreeEnvConfig {
            copy: copy.iter().map(|c| (*c).to_string()).collect(),
        }),
        ports: None,
        hooks: None,
    }
}

// --- Scenario: Declared env file is copied into a new worktree ---

#[test]
fn declared_env_file_is_copied_into_a_new_worktree() {
    let sb = sandbox_repo();
    fs::write(sb.repo.join(".env"), "SECRET=abc\nAPI_KEY=xyz\n").unwrap();
    let worktree = create_worktree(&sb.repo, "feature/one");

    let result =
        provision_worktree(&sb.repo, &worktree, "feat-x", &env_only(&[".env"]), 0).unwrap();

    assert!(result.warnings.is_empty(), "got {:?}", result.warnings);
    assert_eq!(
        fs::read(worktree.join(".env")).unwrap(),
        fs::read(sb.repo.join(".env")).unwrap(),
        "the worktree's .env is a byte-for-byte copy of the repository root's"
    );
}

// --- Scenario: Copied env file is independent of the source ---

#[test]
fn copied_env_file_is_independent_of_source_and_siblings() {
    let sb = sandbox_repo();
    fs::write(sb.repo.join(".env"), "SECRET=original\n").unwrap();
    let first = create_worktree(&sb.repo, "feature/one");
    let second = create_worktree(&sb.repo, "feature/two");

    provision_worktree(&sb.repo, &first, "feat-x", &env_only(&[".env"]), 0).unwrap();
    provision_worktree(&sb.repo, &second, "feat-x", &env_only(&[".env"]), 1).unwrap();

    // Edit the first worktree's copy after provisioning.
    fs::write(first.join(".env"), "SECRET=edited-by-agent-one\n").unwrap();

    assert_eq!(
        fs::read_to_string(sb.repo.join(".env")).unwrap(),
        "SECRET=original\n",
        "the repository-root source must be unchanged"
    );
    assert_eq!(
        fs::read_to_string(second.join(".env")).unwrap(),
        "SECRET=original\n",
        "a sibling worktree's copy must be unchanged"
    );
    assert!(
        !fs::symlink_metadata(first.join(".env"))
            .unwrap()
            .file_type()
            .is_symlink(),
        "the propagated file must be a real copy, not a symlink to shared state"
    );
}

// --- Scenario: Missing declared file is skipped without failing the create ---

#[test]
fn missing_declared_file_is_skipped_with_a_warning() {
    let sb = sandbox_repo();
    fs::write(sb.repo.join(".env"), "SECRET=abc\n").unwrap();
    let worktree = create_worktree(&sb.repo, "feature/one");

    let result = provision_worktree(
        &sb.repo,
        &worktree,
        "feat-x",
        &env_only(&[".env", ".env.absent"]),
        0,
    )
    .unwrap();

    assert_eq!(result.warnings.len(), 1, "got {:?}", result.warnings);
    assert!(
        result.warnings[0].contains(".env.absent"),
        "the warning should name the missing file; got {:?}",
        result.warnings[0]
    );
    assert!(
        worktree.join(".env").exists(),
        "the present file is still copied"
    );
    assert!(!worktree.join(".env.absent").exists());
}

// --- Scenario: Absent configuration preserves prior behavior ---

#[test]
fn absent_configuration_leaves_the_worktree_untouched() {
    let sb = sandbox_repo();
    fs::write(sb.repo.join(".env"), "SECRET=abc\n").unwrap();
    let worktree = create_worktree(&sb.repo, "feature/one");
    let before: Vec<String> = listing(&worktree);

    let result =
        provision_worktree(&sb.repo, &worktree, "feat-x", &WorktreeConfig::default(), 0).unwrap();

    assert_eq!(result.runtime_slot, None, "no slot is recorded");
    assert!(result.warnings.is_empty());
    assert!(!worktree.join(".env").exists(), "no files are copied");
    assert!(
        !worktree.join(ENV_LOCAL_FILE).exists(),
        "no .env.local is generated"
    );
    assert_eq!(listing(&worktree), before, "the worktree is unchanged");
}

// --- Scenario: No implicit defaults ---

#[test]
fn empty_vars_list_writes_no_port_lines() {
    let sb = sandbox_repo();
    let worktree = create_worktree(&sb.repo, "feature/one");

    let config = WorktreeConfig {
        env: None,
        ports: Some(WorktreePortsConfig {
            base: 3000,
            stride: 10,
            vars: Vec::new(),
        }),
        hooks: None,
    };
    provision_worktree(&sb.repo, &worktree, "feat-x", &config, 0).unwrap();

    assert!(
        !worktree.join(ENV_LOCAL_FILE).exists(),
        "git-paw supplies no default port variables"
    );
}

/// Sorted names of the entries directly inside `dir`.
fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("read worktree dir")
        .map(|e| e.expect("dir entry").file_name().to_string_lossy().into())
        .collect();
    names.sort();
    names
}

// --- Scenario: Provisioning precedes agent launch ---

/// Writes an executable stub agent CLI at `<bin_dir>/<name>` that records the
/// contents of its working directory into `.agent-saw.txt` before idling.
///
/// The recording is what makes the ordering assertion real: the file lists what
/// the agent process could actually see the moment it started, so a `.env` in
/// it proves provisioning ran *before* the launch rather than merely at some
/// point during `git paw start`.
fn write_probe_cli(bin_dir: &Path, name: &str) -> PathBuf {
    let script = bin_dir.join(name);
    fs::write(
        &script,
        "#!/bin/sh\nls -a > .agent-saw.txt\nexec sleep 60\n",
    )
    .expect("write probe cli");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("chmod probe cli");
    }
    script
}

fn tmux_available() -> bool {
    StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Waits up to `timeout` for `path` to appear, returning its contents.
fn wait_for_file(path: &Path, timeout: Duration) -> Option<String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        // The probe writes with `ls -a > .agent-saw.txt`, which TRUNCATES the
        // file before `ls` writes into it. Reading in that window yields an
        // empty string — a partial read, not the real listing (`ls -a` always
        // emits at least `.`/`..`). Keep polling until the content is non-empty
        // so a load-widened truncate window cannot flake the assertion.
        if let Ok(contents) = fs::read_to_string(path)
            && !contents.trim().is_empty()
        {
            return Some(contents);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

#[test]
#[serial]
fn provisioned_files_are_present_before_the_agent_cli_starts() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = helpers::setup_test_repo();
    let tmux_env = helpers::tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    // A stub agent CLI on PATH, named so git-paw's custom-CLI resolution finds
    // it and the pane command executes it.
    let bin_dir = TempDir::new().expect("bin tempdir");
    write_probe_cli(bin_dir.path(), "envprobe");
    let path_var = format!(
        "{}:{}",
        bin_dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    fs::write(tr.path().join(".env"), "SECRET=abc\n").expect("write .env");
    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"envprobe\"\n\
         [clis.envprobe]\n\
         command = \"envprobe\"\n\
         [worktree.env]\n\
         copy = [\".env\"]\n\
         [worktree.ports]\n\
         base = 3000\n\
         stride = 10\n\
         vars = [\"PORT\"]\n",
    )
    .expect("write config");

    let mut start = Command::cargo_bin("git-paw").expect("binary exists");
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(tr.path())
        .env("PATH", &path_var)
        .args(["start", "--branches", "probe-a", "--cli", "envprobe"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run start");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();

    let session_name = stdout
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
        .map(str::to_string);

    let worktree = tr
        .path()
        .parent()
        .expect("repo parent")
        .join(git::worktree_dir_name(
            &git::project_name(tr.path()),
            "probe-a",
        ));
    // Generous wait: the idleprobe CLI runs in a tmux pane and, under the CPU
    // load of the full `just check` suite, can be slow to be scheduled and write
    // its marker. The operation itself completes in ~2s on a quiet machine; the
    // 45s ceiling only absorbs load so this e2e does not flake under it.
    let saw = wait_for_file(&worktree.join(".agent-saw.txt"), Duration::from_secs(45));

    // Tear down before asserting so a failure never leaks a tmux server.
    if let Some(name) = session_name {
        let _ = StdCommand::new("tmux")
            .env("TMUX_TMPDIR", tmux_env.socket_dir())
            .args(["kill-session", "-t", &name])
            .status();
    }

    assert!(
        out.status.success(),
        "start failed; stdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let saw = saw.unwrap_or_else(|| {
        panic!(
            "the agent CLI never recorded its worktree listing at {}",
            worktree.display()
        )
    });
    assert!(
        saw.lines().any(|l| l.trim() == ".env"),
        "the copied .env must exist before the agent CLI starts; agent saw:\n{saw}"
    );
    assert!(
        saw.lines().any(|l| l.trim() == ENV_LOCAL_FILE),
        "the generated .env.local must exist before the agent CLI starts; agent saw:\n{saw}"
    );
}
