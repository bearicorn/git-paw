//! E2E observable tests for `git paw add` / `git paw remove` (capabilities
//! `add-branch` / `remove-branch`).
//!
//! Each test drives the real `git-paw` binary against a per-test isolated tmux
//! socket ([`helpers::TmuxTestEnv`]) and an isolated `HOME` so the global
//! session receipt lands under a temp dir — never the user's real data dir or
//! the live supervisor session. Skips when tmux is unavailable.
//!
//! Maps to git-paw-add tasks 8.1, 8.4, 8.6, 8.7, 8.8, 8.9, 8.10.

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

/// Write a supervisor-mode config (broker disabled, `echo` CLI) so launches
/// are fast and need no real agent binary.
fn write_supervisor_config(repo: &Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"echo\"\n\n[supervisor]\nenabled = true\ncli = \"echo\"\n",
    )
    .expect("write config");
}

/// Count agent entries in the per-repo discovery file the session writes
/// (`<repo>/.git-paw/sessions/<name>.json`). Returns `(count, raw_json)`.
fn read_discovery(repo: &Path, session: &str) -> (usize, String) {
    let path = repo
        .join(".git-paw")
        .join("sessions")
        .join(format!("{session}.json"));
    let raw = fs::read_to_string(&path).unwrap_or_default();
    let count = raw.matches("\"branch_id\"").count();
    (count, raw)
}

/// List the pane count for `session` on the test socket.
///
/// Routes the `tmux` invocation through [`TmuxTestEnv::apply`], which both
/// points at the test socket dir AND removes `TMUX` / `TMUX_PANE` from the
/// child env. The latter is essential: the test process itself may run inside
/// a tmux session (e.g. the dogfood supervisor), and a lingering `TMUX` env
/// var makes `tmux` talk to that *default* server, ignoring `TMUX_TMPDIR`.
fn list_pane_count(tmux_env: &TmuxTestEnv, session: &str) -> usize {
    let mut c = StdCommand::new("tmux");
    tmux_env.apply(&mut c);
    let out = c
        .args(["list-panes", "-t", session, "-F", "#{pane_index}"])
        .output()
        .expect("tmux list-panes");
    String::from_utf8_lossy(&out.stdout).lines().count()
}

fn kill(tmux_env: &TmuxTestEnv, session: &str) {
    let mut c = StdCommand::new("tmux");
    tmux_env.apply(&mut c);
    let _ = c.args(["kill-session", "-t", session]).status();
}

/// Start a supervisor session with the given comma-separated branches and
/// return its session name (parsed from stdout).
fn start_session(repo: &Path, home: &Path, tmux_env: &TmuxTestEnv, branches: &str) -> String {
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(repo)
        .env("HOME", home)
        .args(["start", "--supervisor", "--branches", branches])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "start failed; stdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
        .expect("session name in start stdout")
        .to_string()
}

// --- 8.10 / argument-surface errors (no live session needed) ---

#[test]
fn remove_supervisor_is_refused_with_stop_hint() {
    let tr = setup_test_repo();
    let mut c = cmd();
    let out = c
        .current_dir(tr.path())
        .args(["remove", "supervisor"])
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run remove supervisor");
    assert!(!out.status.success(), "remove supervisor must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("git paw stop"),
        "should point at `git paw stop`; stderr:\n{stderr}"
    );
}

#[test]
fn add_without_active_session_errors() {
    let tr = setup_test_repo();
    let home = TempDir::new().unwrap();
    let mut c = cmd();
    let out = c
        .current_dir(tr.path())
        .env("HOME", home.path())
        .args(["add", "feat/x"])
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run add");
    assert!(!out.status.success(), "add with no session must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no active session"),
        "should explain there is no active session; stderr:\n{stderr}"
    );
}

// --- 8.1 add attaches a pane and registers the agent ---

#[test]
#[serial]
fn add_attaches_pane_and_registers_agent() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session(tr.path(), home.path(), &tmux_env, "a,b");
    // 2 agents -> supervisor + dashboard + 2 = 4 panes.
    assert_eq!(
        list_pane_count(&tmux_env, &session),
        4,
        "expected 4 panes after a 2-agent start"
    );

    let mut add = cmd();
    tmux_env.apply_assert(&mut add);
    let out = add
        .current_dir(tr.path())
        .env("HOME", home.path())
        .args(["add", "c"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run add");
    let add_stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let add_stderr = String::from_utf8_lossy(&out.stderr).to_string();

    let panes = list_pane_count(&tmux_env, &session);
    let (agent_count, raw) = read_discovery(tr.path(), &session);
    kill(&tmux_env, &session);

    assert!(
        out.status.success(),
        "add failed; stdout:\n{add_stdout}\nstderr:\n{add_stderr}"
    );
    assert_eq!(panes, 5, "add should splice one pane (4 -> 5)");
    assert_eq!(agent_count, 3, "discovery file should list 3 agents");
    assert!(
        raw.contains("\"branch_id\": \"c\""),
        "discovery file should register the added branch 'c'; raw:\n{raw}"
    );
}

// --- 8.6 remove a clean agent: pane closes, worktree gone, session updated ---

#[test]
#[serial]
fn remove_clean_agent_detaches_and_updates_session() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session(tr.path(), home.path(), &tmux_env, "a,b,c");
    assert_eq!(list_pane_count(&tmux_env, &session), 5);

    // Worktree for 'b' lives as a sibling of the repo.
    let wt_b = tr.path().parent().unwrap().join("repo-b");
    assert!(wt_b.exists(), "worktree for 'b' should exist after start");

    let mut rm = cmd();
    tmux_env.apply_assert(&mut rm);
    let out = rm
        .current_dir(tr.path())
        .env("HOME", home.path())
        .args(["remove", "b"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run remove");

    let panes = list_pane_count(&tmux_env, &session);
    let (agent_count, raw) = read_discovery(tr.path(), &session);
    let wt_gone = !wt_b.exists();
    kill(&tmux_env, &session);

    assert!(
        out.status.success(),
        "remove failed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(panes, 4, "remove should close one pane (5 -> 4)");
    assert_eq!(agent_count, 2, "discovery file should list 2 agents");
    assert!(
        !raw.contains("\"branch_id\": \"b\""),
        "removed branch 'b' should be gone from the discovery file; raw:\n{raw}"
    );
    assert!(wt_gone, "worktree for 'b' should be removed from disk");
}

// --- 8.7 / 8.8 uncommitted-work safety + --keep-worktree ---

#[test]
#[serial]
fn remove_dirty_refuses_then_keep_worktree_succeeds() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session(tr.path(), home.path(), &tmux_env, "a");
    let wt_a = tr.path().parent().unwrap().join("repo-a");
    assert!(wt_a.exists(), "worktree for 'a' should exist");
    // Dirty the worktree.
    fs::write(wt_a.join("dirty.txt"), "uncommitted").expect("write dirty file");

    // Plain remove must refuse and name the dirty file.
    let mut rm = cmd();
    tmux_env.apply_assert(&mut rm);
    let refused = rm
        .current_dir(tr.path())
        .env("HOME", home.path())
        .args(["remove", "a"])
        .timeout(Duration::from_secs(20))
        .output()
        .expect("run remove (dirty)");
    let refused_stderr = String::from_utf8_lossy(&refused.stderr).to_string();
    let still_there = wt_a.exists();

    // --keep-worktree bypasses the check and leaves the worktree on disk.
    let mut rmk = cmd();
    tmux_env.apply_assert(&mut rmk);
    let kept = rmk
        .current_dir(tr.path())
        .env("HOME", home.path())
        .args(["remove", "a", "--keep-worktree"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run remove --keep-worktree");
    let kept_ok = kept.status.success();
    let wt_kept = wt_a.exists();
    kill(&tmux_env, &session);

    assert!(!refused.status.success(), "dirty remove must refuse");
    assert!(
        refused_stderr.contains("dirty.txt"),
        "refusal should name the uncommitted file; stderr:\n{refused_stderr}"
    );
    assert!(still_there, "worktree must survive a refused remove");
    assert!(
        kept_ok,
        "remove --keep-worktree should succeed; stderr:\n{}",
        String::from_utf8_lossy(&kept.stderr)
    );
    assert!(
        wt_kept,
        "--keep-worktree must leave the worktree (with its uncommitted file) on disk"
    );
}

// --- 8.9 remove a non-existent branch lists the live agents ---

#[test]
#[serial]
fn remove_nonexistent_branch_lists_live_agents() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session(tr.path(), home.path(), &tmux_env, "a");

    let mut rm = cmd();
    tmux_env.apply_assert(&mut rm);
    let out = rm
        .current_dir(tr.path())
        .env("HOME", home.path())
        .args(["remove", "ghost"])
        .timeout(Duration::from_secs(20))
        .output()
        .expect("run remove ghost");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    kill(&tmux_env, &session);

    assert!(!out.status.success(), "removing a non-agent must fail");
    assert!(
        stderr.contains("not an agent") && stderr.contains('a'),
        "error should explain and list live agents; stderr:\n{stderr}"
    );
}

// --- 8.4 add to a paused session holds the prompt for resume ---

#[test]
#[serial]
fn add_to_paused_session_holds_prompt_for_resume() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session(tr.path(), home.path(), &tmux_env, "a");

    // Pause the session (broker disabled: this flips the receipt to paused
    // and detaches; the tmux session + panes stay alive).
    let mut pause = cmd();
    tmux_env.apply_assert(&mut pause);
    let _ = pause
        .current_dir(tr.path())
        .env("HOME", home.path())
        .args(["pause"])
        .timeout(Duration::from_secs(20))
        .output()
        .expect("run pause");

    let mut add = cmd();
    tmux_env.apply_assert(&mut add);
    let out = add
        .current_dir(tr.path())
        .env("HOME", home.path())
        .args(["add", "b"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run add (paused)");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let panes = list_pane_count(&tmux_env, &session);
    let (agent_count, _) = read_discovery(tr.path(), &session);
    kill(&tmux_env, &session);

    assert!(
        out.status.success(),
        "add to paused session should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("git paw resume") || stdout.contains("paused"),
        "add to a paused session should report it will start on resume; stdout:\n{stdout}"
    );
    // 1 agent -> supervisor + dashboard + a = 3 panes; the held add makes 4.
    assert_eq!(panes, 4, "paused add still creates the pane (3 -> 4)");
    assert_eq!(
        agent_count, 2,
        "the held agent is registered in the session"
    );
}

/// Collect `(pane_index, pane_current_path)` for every pane on the test socket.
fn pane_paths(tmux_env: &TmuxTestEnv, session: &str) -> Vec<(String, String)> {
    let mut c = StdCommand::new("tmux");
    tmux_env.apply(&mut c);
    let out = c
        .args([
            "list-panes",
            "-t",
            session,
            "-F",
            "#{pane_index}\t#{pane_current_path}",
        ])
        .output()
        .expect("tmux list-panes paths");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let (i, p) = l.split_once('\t')?;
            Some((i.to_string(), p.to_string()))
        })
        .collect()
}

// --- 8.2 existing panes retain their pane_current_path mapping after add ---

#[test]
#[serial]
fn add_preserves_existing_pane_current_paths() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session(tr.path(), home.path(), &tmux_env, "a,b");
    // Match on the worktree dir-name suffix: pane_current_path is canonicalised
    // (on macOS /tmp -> /private/tmp), so an exact path compare is brittle.
    let has =
        |paths: &[(String, String)], suffix: &str| paths.iter().any(|(_, p)| p.ends_with(suffix));
    let before = pane_paths(&tmux_env, &session);
    assert!(
        has(&before, "repo-a") && has(&before, "repo-b"),
        "both agent worktree paths should be present before add; before:\n{before:?}"
    );

    let mut add = cmd();
    tmux_env.apply_assert(&mut add);
    let out = add
        .current_dir(tr.path())
        .env("HOME", home.path())
        .args(["add", "c"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run add");
    let after = pane_paths(&tmux_env, &session);
    kill(&tmux_env, &session);

    assert!(out.status.success(), "add should succeed");
    // The original agents' worktree paths are still mapped to live panes —
    // the re-tile did not relocate them (existing send-keys targeting holds).
    assert!(
        has(&after, "repo-a"),
        "agent a's worktree mapping should survive the add; after:\n{after:?}"
    );
    assert!(
        has(&after, "repo-b"),
        "agent b's worktree mapping should survive the add; after:\n{after:?}"
    );
}

// --- 8.5 add --from-spec resolves a spec and attaches the derived agent ---

#[test]
#[serial]
fn add_from_spec_resolves_and_attaches() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    // Config enables supervisor AND points spec discovery at OpenSpec changes
    // under specs/. (The cross-backend resolution itself is covered by
    // cross_format_spec_selection.rs; this exercises the cmd_add wiring.)
    let paw_dir = tr.path().join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"echo\"\n\n[specs]\ntype = \"openspec\"\ndir = \"specs\"\n\n\
         [supervisor]\nenabled = true\ncli = \"echo\"\n",
    )
    .expect("write config");
    // A discoverable OpenSpec change.
    let change_dir = tr.path().join("specs").join("add-export");
    fs::create_dir_all(&change_dir).expect("create change dir");
    fs::write(change_dir.join("tasks.md"), "Implement export.").expect("write tasks.md");
    StdCommand::new("git")
        .current_dir(tr.path())
        .args(["add", "."])
        .output()
        .expect("git add");
    StdCommand::new("git")
        .current_dir(tr.path())
        .args(["commit", "-m", "add spec"])
        .output()
        .expect("git commit");

    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();
    let session = start_session(tr.path(), home.path(), &tmux_env, "a");
    assert_eq!(list_pane_count(&tmux_env, &session), 3);

    let mut add = cmd();
    tmux_env.apply_assert(&mut add);
    let out = add
        .current_dir(tr.path())
        .env("HOME", home.path())
        .args(["add", "--from-spec", "add-export"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run add --from-spec");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let panes = list_pane_count(&tmux_env, &session);
    let (agent_count, _) = read_discovery(tr.path(), &session);
    kill(&tmux_env, &session);

    assert!(
        out.status.success(),
        "add --from-spec should succeed; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(panes, 4, "add --from-spec should splice one pane (3 -> 4)");
    assert_eq!(
        agent_count, 2,
        "the spec-derived agent should be registered in the session"
    );
}
