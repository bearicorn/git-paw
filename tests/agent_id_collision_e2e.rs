//! A session never launches two agents that would share a broker `agent_id`.
//!
//! `agent-id-slug-safety` (`broker-protocol`, "Agent ids derived for one
//! session are distinct"): `slugify_branch` maps `_` to `-`, so `feat/a_b` and
//! `feat/a-b` are distinct refs that derive one id — two agents sharing an
//! inbox, where a verdict or feedback meant for one reaches the other. Every
//! launch path refuses such a pair before creating anything.
//!
//! Each test drives the real binary against a per-test isolated tmux socket
//! ([`helpers::TmuxTestEnv`]) and an isolated `HOME`, so a receipt never lands
//! in the user's real data dir or touches the live supervisor session.

use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn tmux_available() -> bool {
    StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Config using `cat` as the stand-in CLI — see `tests/add_remove_e2e.rs` for
/// why `cat` and not `echo`.
///
/// The broker is enabled explicitly (it is off by default): agent ids exist to
/// address agents on the broker, so the duplicate-id check only runs when the
/// broker does.
fn write_config(repo: &Path, supervisor: bool, broker: bool) {
    use std::fmt::Write as _;

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    // `[clis.cat]` registers `cat` as a known CLI identity; the spec-driven
    // path resolves against the configured set, not bare PATH lookup.
    let mut toml = String::from("default_cli = \"cat\"\n\n[clis.cat]\ncommand = \"cat\"\n");
    if supervisor {
        toml.push_str("\n[supervisor]\nenabled = true\ncli = \"cat\"\n");
    }
    if broker {
        // An OS-assigned free port, released for the launch to bind. Port 0
        // itself cannot be used: the launcher waits for the broker host on the
        // configured port, and agents are handed that port's URL.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("bind ephemeral port")
            .local_addr()
            .expect("local addr")
            .port();
        let _ = write!(toml, "\n[broker]\nenabled = true\nport = {port}\n");
    }
    fs::write(paw_dir.join("config.toml"), toml).expect("write config");
}

fn write_supervisor_config(repo: &Path) {
    write_config(repo, true, true);
}

/// Writes a Spec Kit feature directory with a single `[P]` task.
///
/// A `[P]` task's branch is `task/<task-id>-<description>`, which does NOT
/// include the feature directory — so two features carrying the same task id
/// are what produce two branches differing only in their description, the
/// shape that collides on the derived id.
fn write_speckit_feature(repo: &Path, feature_dir: &str, task_id: &str, description: &str) {
    let feature = repo.join(".specify/specs").join(feature_dir);
    fs::create_dir_all(&feature).expect("create feature dir");
    fs::write(
        feature.join("tasks.md"),
        format!("## Phase 1: Setup\n- [ ] {task_id} [P] {description}\n"),
    )
    .expect("write tasks.md");
}

/// Number of worktrees git knows about, including the main one. A refusal
/// that created nothing leaves this at 1 whatever the configured placement is.
fn worktree_count(repo: &Path) -> usize {
    let out = StdCommand::new("git")
        .current_dir(repo)
        .args(["worktree", "list", "--porcelain"])
        .output()
        .expect("git worktree list");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.starts_with("worktree "))
        .count()
}

fn list_pane_count(tmux_env: &TmuxTestEnv, session: &str) -> usize {
    let mut c = StdCommand::new("tmux");
    tmux_env.apply(&mut c);
    let out = c
        .args(["list-panes", "-t", session, "-F", "#{pane_index}"])
        .output()
        .expect("tmux list-panes");
    String::from_utf8_lossy(&out.stdout).lines().count()
}

fn assert_names_the_clash(stderr: &str) {
    for needle in ["feat/a_b", "feat/a-b", "feat-a-b"] {
        assert!(
            stderr.contains(needle),
            "refusal should name {needle}; stderr:\n{stderr}"
        );
    }
}

/// Bare `start`: two branches differing only by `_` vs `-` are refused, and
/// the refusal names both branches and the id they share.
#[test]
#[serial]
fn bare_start_refuses_branches_that_derive_one_agent_id() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_config(tr.path(), false, true);
    let home = TempDir::new().expect("home tempdir");
    let tmux_env = tmux_test_env();

    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--cli", "cat", "--branches", "feat/a_b,feat/a-b"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start");

    assert!(
        !out.status.success(),
        "start must refuse colliding ids; stdout:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_names_the_clash(&String::from_utf8_lossy(&out.stderr));
    assert_eq!(
        worktree_count(tr.path()),
        1,
        "a refused launch must create no worktree"
    );
}

/// The supervised launch path makes the same check, equally before anything
/// is created.
#[test]
#[serial]
fn supervised_start_refuses_branches_that_derive_one_agent_id() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().expect("home tempdir");
    let tmux_env = tmux_test_env();

    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--supervisor", "--branches", "feat/a_b,feat/a-b"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start --supervisor");

    assert!(
        !out.status.success(),
        "supervised start must refuse colliding ids; stdout:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_names_the_clash(&String::from_utf8_lossy(&out.stderr));
    assert_eq!(
        worktree_count(tr.path()),
        1,
        "a refused launch must create no worktree"
    );
}

/// The SPEC-DRIVEN launch path makes the check too — and this is where the
/// collision arises from spec TEXT rather than from what a user typed, so
/// nobody chose the two names.
///
/// Two features carrying the same task id, whose descriptions differ only
/// where the branch slug keeps `_` and the agent id maps it to `-`, decompose
/// to `task/t001-create-room_setup-md` and `task/t001-create-room-setup-md`:
/// different branches, both legal refs, one derived id.
#[test]
#[serial]
fn spec_driven_start_refuses_branches_that_derive_one_agent_id() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_config(tr.path(), false, true);
    write_speckit_feature(tr.path(), "001-rooms", "T001", "Create ROOM_SETUP.md");
    write_speckit_feature(tr.path(), "002-lobby", "T001", "Create ROOM SETUP.md");
    let home = TempDir::new().expect("home tempdir");
    let tmux_env = tmux_test_env();

    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args([
            "start",
            "--from-specs",
            "--specs-format",
            "speckit",
            "--cli",
            "cat",
        ])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start --from-specs");

    assert!(
        !out.status.success(),
        "spec-driven start must refuse colliding ids; stdout:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    for needle in [
        "task/t001-create-room_setup-md",
        "task/t001-create-room-setup-md",
        "task-t001-create-room-setup-md",
    ] {
        assert!(
            stderr.contains(needle),
            "refusal should name {needle}; stderr:\n{stderr}"
        );
    }
    assert_eq!(
        worktree_count(tr.path()),
        1,
        "a refused launch must create no worktree"
    );
}

/// With the broker disabled nothing derives, publishes or routes by agent id,
/// so a launch that would collide is accepted exactly as it was before this
/// check existed — a disabled feature leaves behaviour unchanged.
///
/// `hotfix` and `agent-hotfix` are the pair that isolates the id check: both
/// derive `agent-hotfix` (a single word takes the prefix), but their worktree
/// directories differ, so nothing ELSE stops them coexisting. The spec's
/// `feat/a_b` / `feat/a-b` pair would be refused here anyway, by the worktree
/// directory name they also share.
#[test]
#[serial]
fn broker_disabled_launches_colliding_branches_as_before() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_config(tr.path(), false, false);
    let home = TempDir::new().expect("home tempdir");
    let tmux_env = tmux_test_env();

    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--cli", "cat", "--branches", "hotfix,agent-hotfix"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "a broker-less launch must not be refused; stdout:\n{}\nstderr:\n{stderr}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        !stderr.contains("both derive the broker agent id"),
        "no duplicate-id refusal should be reported; stderr:\n{stderr}"
    );

    let mut stop = cmd();
    tmux_env.apply_assert(&mut stop);
    let _ = stop
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["stop", "--force"])
        .timeout(Duration::from_secs(30))
        .output();
}

/// `git paw add` compares the new branch against the ids of the agents
/// already in the live session, and leaves that session untouched on a clash.
#[test]
#[serial]
fn add_refuses_a_branch_colliding_with_a_live_agent() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().expect("home tempdir");
    let tmux_env = tmux_test_env();

    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let start_out = start
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--supervisor", "--branches", "feat/x"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start");
    assert!(
        start_out.status.success(),
        "start failed; stderr:\n{}",
        String::from_utf8_lossy(&start_out.stderr)
    );
    let session = String::from_utf8_lossy(&start_out.stdout)
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
        .expect("session name in start stdout")
        .to_string();

    let worktrees_before = worktree_count(tr.path());
    let panes_before = list_pane_count(&tmux_env, &session);

    // `feat-x` is a different ref from `feat/x`, but both derive `feat-x`.
    let mut add = cmd();
    tmux_env.apply_assert(&mut add);
    let out = add
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["add", "feat-x"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run add");

    assert!(
        !out.status.success(),
        "add must refuse a branch colliding with a live agent; stdout:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    for needle in ["feat/x", "feat-x"] {
        assert!(
            stderr.contains(needle),
            "refusal should name {needle}; stderr:\n{stderr}"
        );
    }
    assert_eq!(
        worktree_count(tr.path()),
        worktrees_before,
        "a refused add must leave the session's worktrees unchanged"
    );
    assert_eq!(
        list_pane_count(&tmux_env, &session),
        panes_before,
        "a refused add must attach no pane"
    );

    let mut stop = cmd();
    tmux_env.apply_assert(&mut stop);
    let _ = stop
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["stop", "--force"])
        .timeout(Duration::from_secs(30))
        .output();
}
