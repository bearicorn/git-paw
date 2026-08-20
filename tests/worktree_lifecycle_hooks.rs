//! Worktree lifecycle hook integration tests (`session-runtime-isolation`).
//!
//! Covers the spec requirements:
//!
//! - *Worktree lifecycle hooks provision and tear down per-worktree resources* —
//!   `on_create` runs in the new worktree after env/port provisioning and before
//!   the agent launches, `on_remove` runs before the worktree is deleted, and
//!   both substitute `{worktree_id}` / `{worktree_path}`.
//! - *`on_create` stdout merges into the generated `.env.local`* — a `KEY=value`
//!   line becomes environment, coexists with the allocated ports in one managed
//!   block, and non-assignment output is ignored.
//! - *Hook failure policy protects worktree integrity* — a failing `on_create`
//!   is reported with its status and stderr, a failing `on_remove` still lets
//!   the worktree go, and captured values never reach git-paw's own output.
//! - *Lifecycle hooks are opt-in and export-agnostic* — no configured hook runs
//!   no command, and a configured hook is the *only* thing git-paw runs.
//!
//! The hooks here are real shell commands run through the production seam, so
//! what is asserted is what a consumer's script would actually observe: its
//! working directory, the files already provisioned around it, and where its
//! stdout ends up. The two teardown scenarios drive the `git paw` binary
//! end-to-end, because the thing under test is the wiring at the command
//! handlers rather than the hook mechanism itself.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

use git_paw::config::{WorktreeConfig, WorktreeHooksConfig, WorktreePortsConfig};
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

/// A `[worktree]` config declaring only an `on_create` hook.
fn on_create(command: &str) -> WorktreeConfig {
    WorktreeConfig {
        env: None,
        ports: None,
        hooks: Some(WorktreeHooksConfig {
            on_create: Some(command.to_string()),
            on_remove: None,
        }),
    }
}

fn managed_block(contents: &str) -> String {
    contents
        .split_once(git_paw::worktree_provision::MANAGED_BLOCK_START)
        .and_then(|(_, rest)| {
            rest.split_once(git_paw::worktree_provision::MANAGED_BLOCK_END)
                .map(|(block, _)| block.to_string())
        })
        .unwrap_or_else(|| panic!("no managed block in {contents:?}"))
}

// --- Scenario: on_create runs in the worktree, after env/port provisioning ---

#[test]
fn on_create_runs_in_the_worktree_after_port_provisioning() {
    let sb = sandbox_repo();
    let worktree = create_worktree(&sb.repo, "feature/one");

    // The hook reports its own working directory and whether the allocated port
    // block was already on disk when it ran — the two facts the ordering
    // requirement is actually about.
    let mut config = on_create(
        "printf 'HOOK_CWD=%s\\n' \"$PWD\"; printf 'HOOK_SAW_PORT=%s\\n' \
         \"$(grep -c '^PORT=' .env.local)\"",
    );
    config.ports = Some(WorktreePortsConfig {
        base: 3000,
        stride: 10,
        vars: vec!["PORT".to_string()],
    });

    provision_worktree(&sb.repo, &worktree, "feature/one", &config, 1).expect("provision");

    let block = managed_block(&fs::read_to_string(worktree.join(ENV_LOCAL_FILE)).unwrap());
    assert!(
        block.contains(&format!("HOOK_CWD={}", worktree.display())),
        "the hook must run in the worktree; got {block:?}"
    );
    assert!(
        block.contains("HOOK_SAW_PORT=1"),
        "the hook must see the allocated ports already written; got {block:?}"
    );
}

// --- Scenario: Placeholders are substituted ---

#[test]
fn placeholders_are_substituted_before_the_hook_runs() {
    let sb = sandbox_repo();
    let worktree = create_worktree(&sb.repo, "feature/auth-flow");

    provision_worktree(
        &sb.repo,
        &worktree,
        "feature/auth-flow",
        &on_create("printf 'WT_ID=%s\\nWT_PATH=%s\\n' '{worktree_id}' '{worktree_path}'"),
        0,
    )
    .expect("provision");

    let block = managed_block(&fs::read_to_string(worktree.join(ENV_LOCAL_FILE)).unwrap());
    assert!(
        block.contains("WT_ID=feature-auth-flow"),
        "{{worktree_id}} must expand to the branch-derived slug; got {block:?}"
    );
    assert!(
        block.contains(&format!("WT_PATH={}", worktree.display())),
        "{{worktree_path}} must expand to the absolute path; got {block:?}"
    );
}

// --- Scenario: Hook output becomes environment in .env.local ---

#[test]
fn hook_output_becomes_environment_in_env_local() {
    let sb = sandbox_repo();
    let worktree = create_worktree(&sb.repo, "feature/one");

    provision_worktree(
        &sb.repo,
        &worktree,
        "wt1",
        &on_create("echo DATABASE_URL=postgres://localhost/paw_wt1"),
        0,
    )
    .expect("provision");

    let block = managed_block(&fs::read_to_string(worktree.join(ENV_LOCAL_FILE)).unwrap());
    assert!(
        block.contains("DATABASE_URL=postgres://localhost/paw_wt1"),
        "got {block:?}"
    );
}

// --- Scenario: Hook output coexists with allocated ports ---

#[test]
fn hook_output_coexists_with_allocated_ports() {
    let sb = sandbox_repo();
    let worktree = create_worktree(&sb.repo, "feature/one");
    fs::write(
        worktree.join(ENV_LOCAL_FILE),
        "USER_SETTING=keep-me\nANOTHER=also-keep\n",
    )
    .expect("seed .env.local");

    let mut config = on_create("echo DATABASE_URL=postgres://localhost/paw_wt1");
    config.ports = Some(WorktreePortsConfig {
        base: 3000,
        stride: 10,
        vars: vec!["PORT".to_string(), "VITE_PORT".to_string()],
    });

    provision_worktree(&sb.repo, &worktree, "wt1", &config, 2).expect("provision");

    let contents = fs::read_to_string(worktree.join(ENV_LOCAL_FILE)).unwrap();
    let block = managed_block(&contents);
    for expected in [
        "PORT=3020",
        "VITE_PORT=3021",
        "DATABASE_URL=postgres://localhost/paw_wt1",
    ] {
        assert!(
            block.contains(expected),
            "{expected} missing from {block:?}"
        );
    }
    assert!(
        contents.starts_with("USER_SETTING=keep-me\nANOTHER=also-keep\n"),
        "content outside the managed block must be untouched; got {contents:?}"
    );
}

// --- Scenario: Non-assignment output is ignored ---

#[test]
fn non_assignment_output_is_ignored() {
    let sb = sandbox_repo();
    let worktree = create_worktree(&sb.repo, "feature/one");

    provision_worktree(
        &sb.repo,
        &worktree,
        "wt1",
        &on_create(
            "echo 'Creating database branch...'; echo; \
             echo DATABASE_URL=postgres://localhost/paw_wt1; \
             echo '  done in 1.2s'; echo 'not an assignment'",
        ),
        0,
    )
    .expect("provision");

    let block = managed_block(&fs::read_to_string(worktree.join(ENV_LOCAL_FILE)).unwrap());
    assert!(block.contains("DATABASE_URL=postgres://localhost/paw_wt1"));
    for noise in [
        "Creating database branch",
        "done in 1.2s",
        "not an assignment",
    ] {
        assert!(!block.contains(noise), "{noise:?} leaked into {block:?}");
    }
}

// --- Scenario: on_create failure is reported ---

#[test]
fn on_create_failure_is_reported_with_status_and_stderr() {
    let sb = sandbox_repo();
    let worktree = create_worktree(&sb.repo, "feature/one");

    let err = provision_worktree(
        &sb.repo,
        &worktree,
        "wt1",
        &on_create("echo 'could not reach the database host' >&2; exit 7"),
        0,
    )
    .expect_err("a non-zero on_create must fail provisioning");

    let message = err.to_string();
    assert!(message.contains("on_create"), "got {message}");
    assert!(message.contains("exit 7"), "got {message}");
    assert!(
        message.contains("could not reach the database host"),
        "got {message}"
    );
}

// --- Scenario: Captured secret values are not logged ---

#[test]
fn a_captured_secret_reaches_env_local_but_not_git_paws_own_output() {
    let sb = sandbox_repo();
    let worktree = create_worktree(&sb.repo, "feature/one");
    let secret = "postgres://admin:hunter2@db.internal/paw_wt1";

    let result = provision_worktree(
        &sb.repo,
        &worktree,
        "wt1",
        &on_create(&format!("echo DATABASE_URL={secret}")),
        0,
    )
    .expect("provision");

    // Everything git-paw could print about the hook, in one string.
    let reportable = format!(
        "{result:?} {} {}",
        result.hook_summary().unwrap_or_default(),
        result.warnings.join(" ")
    );
    assert!(
        !reportable.contains("hunter2"),
        "the secret leaked into git-paw's own output: {reportable}"
    );
    assert!(
        reportable.contains("DATABASE_URL"),
        "the merged key should still be reported; got {reportable}"
    );
    assert!(
        fs::read_to_string(worktree.join(ENV_LOCAL_FILE))
            .unwrap()
            .contains(secret),
        "the value must still reach .env.local"
    );
}

// --- Scenario: Absent hooks configuration runs nothing ---

#[test]
fn absent_hooks_configuration_runs_no_command() {
    let sb = sandbox_repo();
    let worktree = create_worktree(&sb.repo, "feature/one");

    // A hook that would be unmistakable if it ran, declared nowhere.
    let result = provision_worktree(
        &sb.repo,
        &worktree,
        "feature/one",
        &WorktreeConfig::default(),
        0,
    )
    .expect("provision");

    assert!(result.hook_env_keys.is_empty());
    assert!(result.hook_summary().is_none());
    assert!(
        !worktree.join(ENV_LOCAL_FILE).exists(),
        "no hook and no ports must generate no .env.local"
    );
}

// --- Scenario: No implied database policy ---

#[test]
fn only_the_declared_command_runs_and_nothing_is_provisioned_for_it() {
    let sb = sandbox_repo();
    let worktree = create_worktree(&sb.repo, "feature/one");
    let before = listing(&worktree);

    // A hook that provisions nothing and says nothing: git-paw must add no
    // resource, no file, and no environment of its own around it.
    let result =
        provision_worktree(&sb.repo, &worktree, "wt1", &on_create("true"), 0).expect("provision");

    assert!(result.hook_env_keys.is_empty());
    assert_eq!(
        listing(&worktree),
        before,
        "git-paw must create nothing on the hook's behalf"
    );
}

/// Sorted names of the entries directly under `dir`.
fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("read worktree")
        .map(|e| e.expect("entry").file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    names
}

// --- Scenario: on_remove runs at worktree teardown ---

/// Writes an executable stub agent CLI that idles, so a session can be started
/// without a real AI CLI on PATH.
fn write_idle_cli(bin_dir: &Path, name: &str) {
    let script = bin_dir.join(name);
    fs::write(&script, "#!/bin/sh\nexec sleep 60\n").expect("write stub cli");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("chmod stub cli");
    }
}

fn tmux_available() -> bool {
    StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Starts a one-agent session in `repo` whose config declares `on_remove`, then
/// purges it, returning the purge command's stderr and the worktree path.
///
/// Callers point the hook's markers outside the worktree — which is about to be
/// deleted — so what it recorded survives the teardown it ran during. Returns
/// `None` when tmux is unavailable and the scenario cannot be driven.
fn start_then_purge(repo: &Path, on_remove: &str) -> Option<(String, PathBuf)> {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return None;
    }
    let tmux_env = helpers::tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    let bin_dir = TempDir::new().expect("bin tempdir");
    write_idle_cli(bin_dir.path(), "idleprobe");
    let path_var = format!(
        "{}:{}",
        bin_dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        format!(
            "default_cli = \"idleprobe\"\n\
             [clis.idleprobe]\n\
             command = \"idleprobe\"\n\
             [worktree.hooks]\n\
             on_remove = \"{on_remove}\"\n"
        ),
    )
    .expect("write config");

    let mut start = Command::cargo_bin("git-paw").expect("binary exists");
    tmux_env.apply_assert(&mut start);
    let started = start
        .current_dir(repo)
        .env("PATH", &path_var)
        .args(["start", "--branches", "teardown-a", "--cli", "idleprobe"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run start");
    assert!(
        started.status.success(),
        "start failed: {}",
        String::from_utf8_lossy(&started.stderr)
    );

    let worktree = repo
        .parent()
        .expect("repo parent")
        .join(git::worktree_dir_name(
            &git::project_name(repo),
            "teardown-a",
        ));
    assert!(worktree.is_dir(), "the worktree should exist before purge");
    // Resolve the worktree to its physical path while it still exists, so the
    // cwd comparison below is not defeated by the macOS `/var` -> `/private/var`
    // symlink (the hook's `pwd` records the physical path). No-op on Linux.
    let worktree = fs::canonicalize(&worktree).expect("canonicalize worktree before purge");

    let mut purge = Command::cargo_bin("git-paw").expect("binary exists");
    tmux_env.apply_assert(&mut purge);
    let purged = purge
        .current_dir(repo)
        .env("PATH", &path_var)
        .args(["purge", "--force"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run purge");
    assert!(
        purged.status.success(),
        "purge failed: {}",
        String::from_utf8_lossy(&purged.stderr)
    );

    Some((
        String::from_utf8_lossy(&purged.stderr).to_string(),
        worktree,
    ))
}

#[test]
#[serial]
fn on_remove_runs_in_the_worktree_before_it_is_deleted() {
    let tr = helpers::setup_test_repo();
    let markers = TempDir::new().expect("marker tempdir");
    // Records the hook's cwd and the substituted id, and proves the checkout was
    // still readable when it ran.
    let hook = format!(
        "pwd > {dir}/cwd-{{worktree_id}}.txt; ls README.md > {dir}/saw-{{worktree_id}}.txt",
        dir = markers.path().display()
    );

    let Some((_stderr, worktree)) = start_then_purge(tr.path(), &hook) else {
        return;
    };

    let cwd_marker = markers.path().join("cwd-teardown-a.txt");
    assert!(
        cwd_marker.is_file(),
        "on_remove must run, and {{worktree_id}} must expand to the branch slug"
    );
    assert_eq!(
        fs::read_to_string(&cwd_marker).unwrap().trim(),
        worktree.display().to_string(),
        "on_remove must run with the worktree as its working directory"
    );
    assert!(
        markers.path().join("saw-teardown-a.txt").is_file(),
        "on_remove must run while the checkout still exists"
    );
    assert!(
        !worktree.exists(),
        "the worktree should be gone after purge; {} still exists",
        worktree.display()
    );
}

// --- Scenario: on_remove failure does not block removal ---

#[test]
#[serial]
fn a_failing_on_remove_warns_but_still_removes_the_worktree() {
    let tr = helpers::setup_test_repo();
    let markers = TempDir::new().expect("marker tempdir");
    let hook = format!(
        "echo ran > {dir}/ran.txt; echo 'could not drop the branch' >&2; exit 3",
        dir = markers.path().display()
    );

    let Some((stderr, worktree)) = start_then_purge(tr.path(), &hook) else {
        return;
    };

    assert!(markers.path().join("ran.txt").is_file(), "the hook ran");
    assert!(
        !worktree.exists(),
        "a failed teardown must never strand a worktree; {} still exists",
        worktree.display()
    );
    assert!(
        stderr.contains("on_remove") && stderr.contains("exit 3"),
        "the failure must be warned about; got {stderr}"
    );
}
