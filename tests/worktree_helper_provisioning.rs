//! E2E coverage for `broker-agent-helper`'s *Helpers provisioned into agent
//! worktrees* requirement: EVERY launch path that creates an agent worktree
//! SHALL leave an executable `<worktree>/.git-paw/scripts/broker.sh` behind
//! before the agent boots.
//!
//! One test per path — the supervisor launch and `git paw add` (both already
//! satisfied the requirement through `attach_agent`), the bare
//! `git paw start --branches …` launch and the spec-driven
//! `git paw start --from-specs` launch (neither did: they call
//! `worktree_provision::provision_worktree`, which writes no scripts, and
//! never called `init::provision_worktree_helpers`). A fifth test pins the
//! idempotency half of the requirement on the newly-covered bare path: a
//! re-attach to a surviving worktree REFRESHES the helper from the running
//! binary rather than failing or leaving a stale copy.
//!
//! Each test drives the real binary against an isolated tmux socket and an
//! isolated `HOME`, and uses `cat` as the stand-in CLI so a pane consumes the
//! injected boot block as stdin instead of executing its markdown. Skips when
//! tmux is unavailable.

use std::fs;
use std::path::{Path, PathBuf};
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
    let available = StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success());
    if !available {
        eprintln!("skipping: tmux not available");
    }
    !available
}

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Returns a project name unique to this run so sessions never collide.
fn unique_project_name(tag: &str) -> String {
    let pid = std::process::id();
    let n = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("helperprov-{tag}-{pid}-{n}")
}

fn find_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

/// Renames the sandbox repo directory to `new_basename` so the derived
/// project (and therefore tmux session) name is unique per test.
fn rename_repo_basename(tr: &TestRepo, new_basename: &str) -> PathBuf {
    let old = tr.path().to_path_buf();
    let renamed = old
        .parent()
        .expect("repo has a parent sandbox dir")
        .join(new_basename);
    fs::rename(&old, &renamed).expect("rename repo dir");
    renamed
}

/// Writes a `.git-paw/config.toml` with the broker enabled on a free port and
/// `cat` as the CLI, plus any `extra` sections appended verbatim.
fn write_config(repo: &Path, extra: &str) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let port = find_free_port();
    let config = format!(
        r#"
default_cli = "cat"

[broker]
enabled = true
port = {port}

[clis.cat]
command = "cat"
{extra}
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
}

/// Writes and commits a pending `OpenSpec` change at
/// `<repo>/specs/<id>/tasks.md` so `--from-specs` has something to launch.
fn write_committed_spec(repo: &Path, id: &str, body: &str) {
    let dir = repo.join("specs").join(id);
    fs::create_dir_all(&dir).expect("create spec dir");
    fs::write(dir.join("tasks.md"), body).expect("write tasks.md");
    StdCommand::new("git")
        .current_dir(repo)
        .args(["add", "."])
        .output()
        .expect("git add");
    StdCommand::new("git")
        .current_dir(repo)
        .args(["commit", "-m", "add spec"])
        .output()
        .expect("git commit");
}

fn kill_session(env: &TmuxTestEnv, name: &str) {
    let mut kill_cmd = StdCommand::new("tmux");
    env.apply(&mut kill_cmd);
    let _ = kill_cmd.args(["kill-session", "-t", name]).status();
}

/// The agent worktree paths a launch recorded in the per-repo discovery file.
fn agent_worktrees(repo: &Path, session: &str) -> Vec<PathBuf> {
    let path = repo
        .join(".git-paw")
        .join("sessions")
        .join(format!("{session}.json"));
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read discovery file {}: {e}", path.display()));
    let file: git_paw::session::RepoSessionFile =
        serde_json::from_str(&raw).expect("parse discovery file");
    file.agents.into_iter().map(|a| a.worktree_path).collect()
}

/// Asserts `<worktree>/.git-paw/scripts/broker.sh` exists, is executable, and
/// carries the bundled helper's content (not a stale or empty placeholder).
fn assert_broker_helper_provisioned(worktree: &Path) {
    let helper = worktree.join(".git-paw").join("scripts").join("broker.sh");
    assert!(
        helper.exists(),
        "agent worktree must carry the bundled broker helper at {}",
        helper.display()
    );
    let contents = fs::read_to_string(&helper).expect("read provisioned broker.sh");
    assert!(
        contents.contains("Agent-side broker helper"),
        "provisioned helper must be the bundled broker.sh; got:\n{contents}"
    );
    // The shared preamble each helper sources must land alongside it.
    let common = worktree
        .join(".git-paw")
        .join("scripts")
        .join("_paw_common.sh");
    assert!(
        common.exists(),
        "the shared preamble broker.sh sources must be provisioned at {}",
        common.display()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&helper)
            .expect("stat provisioned broker.sh")
            .permissions()
            .mode();
        assert!(
            mode & 0o111 != 0,
            "provisioned broker.sh must be executable; mode is {mode:o}"
        );
    }
}

/// Spec scenario "start provisions the broker helper into each worktree" —
/// the supervisor launch (via `attach_agent`).
#[test]
#[serial]
fn supervisor_launch_provisions_broker_helper_into_each_worktree() {
    if skip_if_no_tmux() {
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("sup");
    let repo = rename_repo_basename(&tr, &project);
    write_config(&repo, "\n[supervisor]\nenabled = true\ncli = \"cat\"\n");

    let session_name = format!("paw-{project}");
    let home = TempDir::new().expect("home tempdir");
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--supervisor", "--branches", "feat/alpha"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start --supervisor");
    kill_session(&tmux_env, &session_name);
    assert!(
        out.status.success(),
        "supervisor launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let worktrees = agent_worktrees(&repo, &session_name);
    assert_eq!(worktrees.len(), 1, "expected one agent worktree");
    for wt in &worktrees {
        assert_broker_helper_provisioned(wt);
    }
}

/// Spec scenario "add provisions the helper into a mid-session worktree".
#[test]
#[serial]
fn add_provisions_broker_helper_into_the_new_worktree() {
    if skip_if_no_tmux() {
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("add");
    let repo = rename_repo_basename(&tr, &project);
    write_config(&repo, "\n[supervisor]\nenabled = true\ncli = \"cat\"\n");

    let session_name = format!("paw-{project}");
    let home = TempDir::new().expect("home tempdir");
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--supervisor", "--branches", "feat/alpha"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start --supervisor");
    assert!(
        out.status.success(),
        "supervisor launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let mut add = cmd();
    tmux_env.apply_assert(&mut add);
    let add_out = add
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["add", "feat/beta"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run add");
    kill_session(&tmux_env, &session_name);
    assert!(
        add_out.status.success(),
        "add should succeed; stderr:\n{}",
        String::from_utf8_lossy(&add_out.stderr)
    );

    let worktrees = agent_worktrees(&repo, &session_name);
    assert_eq!(worktrees.len(), 2, "expected the added agent in the roster");
    for wt in &worktrees {
        assert_broker_helper_provisioned(wt);
    }
}

/// Spec scenario "A bare launch provisions the helper" — `git paw start
/// --branches <branch>` with no supervisor mode and no specs. This path never
/// called `init::provision_worktree_helpers` before this change.
#[test]
#[serial]
fn bare_launch_provisions_broker_helper_into_each_worktree() {
    if skip_if_no_tmux() {
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("bare");
    let repo = rename_repo_basename(&tr, &project);
    write_config(&repo, "");

    let session_name = format!("paw-{project}");
    let home = TempDir::new().expect("home tempdir");
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args([
            "start",
            "--branches",
            "feat/alpha,feat/beta",
            "--cli",
            "cat",
        ])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start --branches");
    kill_session(&tmux_env, &session_name);
    assert!(
        out.status.success(),
        "bare launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let worktrees = agent_worktrees(&repo, &session_name);
    assert_eq!(worktrees.len(), 2, "expected both agent worktrees");
    for wt in &worktrees {
        assert_broker_helper_provisioned(wt);
    }
}

/// Spec scenario "A spec-driven launch without supervisor mode provisions the
/// helper" — `git paw start --from-specs`. This path never called
/// `init::provision_worktree_helpers` before this change.
#[test]
#[serial]
fn spec_driven_launch_provisions_broker_helper_into_each_worktree() {
    if skip_if_no_tmux() {
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("specs");
    let repo = rename_repo_basename(&tr, &project);
    write_config(&repo, "\n[specs]\ntype = \"openspec\"\ndir = \"specs\"\n");
    write_committed_spec(&repo, "feature-helper", "Implement the helper.\n");

    let session_name = format!("paw-{project}");
    let home = TempDir::new().expect("home tempdir");
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--from-specs", "--cli", "cat"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start --from-specs");
    kill_session(&tmux_env, &session_name);
    assert!(
        out.status.success(),
        "spec-driven launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let worktrees = agent_worktrees(&repo, &session_name);
    assert_eq!(
        worktrees.len(),
        1,
        "expected the spec-driven agent worktree"
    );
    for wt in &worktrees {
        assert_broker_helper_provisioned(wt);
    }
}

/// Spec scenario "provisioning is idempotent and version-matched", pinned on
/// the newly-covered bare path: a worktree that survives into a second launch
/// has its helper REWRITTEN from the running binary's bundled assets, so a
/// locally-clobbered copy is restored rather than left stale.
#[test]
#[serial]
fn re_attaching_a_surviving_worktree_refreshes_the_helper() {
    if skip_if_no_tmux() {
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("refresh");
    let repo = rename_repo_basename(&tr, &project);
    write_config(&repo, "");

    let session_name = format!("paw-{project}");
    let home = TempDir::new().expect("home tempdir");
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--branches", "feat/alpha", "--cli", "cat"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start --branches");
    kill_session(&tmux_env, &session_name);
    assert!(
        out.status.success(),
        "first bare launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let worktree = agent_worktrees(&repo, &session_name)
        .into_iter()
        .next()
        .expect("one agent worktree");
    let helper = worktree.join(".git-paw").join("scripts").join("broker.sh");
    fs::write(&helper, "# clobbered\n").expect("clobber provisioned helper");

    // A second launch against the surviving worktree: a fresh HOME means no
    // saved session receipt, so this takes the same bare launch path again.
    let home2 = TempDir::new().expect("second home tempdir");
    let mut restart = cmd();
    tmux_env.apply_assert(&mut restart);
    let out2 = restart
        .current_dir(&repo)
        .env("HOME", home2.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--branches", "feat/alpha", "--cli", "cat"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("re-run start --branches");
    kill_session(&tmux_env, &session_name);
    assert!(
        out2.status.success(),
        "re-attach launch should succeed, not fail on an existing helper; stderr:\n{}",
        String::from_utf8_lossy(&out2.stderr)
    );

    assert_broker_helper_provisioned(&worktree);
}
