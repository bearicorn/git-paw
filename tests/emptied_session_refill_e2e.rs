//! An emptied session — its last agent removed with `git paw remove` while its
//! tmux session, dashboard and broker kept running — can be refilled with
//! `git paw add`, and `git paw start` with a launch selection never silently
//! ignores that selection on it.
//!
//! Each session is backdated past the launch grace period before the check,
//! because a just-launched session is reported alive regardless of its panes.

use std::fs;
use std::path::Path;
use std::process::{Command as StdCommand, Output};
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

/// Supervisor-mode config with `cat` as the stand-in CLI: it stays alive and
/// reads the boot block like a real CLI instead of executing it. It is
/// declared under `[clis]` because an emptied session has no agent left for
/// `add` to take the CLI from, so `add` resolves `default_cli` itself.
fn write_supervisor_config(repo: &Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"cat\"\n\n[clis.cat]\ncommand = \"cat\"\n\n\
         [supervisor]\nenabled = true\ncli = \"cat\"\n",
    )
    .expect("write config");
}

fn paw(repo: &Path, home: &Path, tmux_env: &TmuxTestEnv, args: &[&str]) -> Output {
    let mut c = Command::cargo_bin("git-paw").expect("binary exists");
    tmux_env.apply_assert(&mut c);
    c.current_dir(repo)
        .env("HOME", home)
        .env_remove("XDG_DATA_HOME")
        .args(args)
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run git-paw")
}

fn text(out: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Starts a supervisor session with `branches` and returns its name.
fn start(repo: &Path, home: &Path, tmux_env: &TmuxTestEnv, branches: &str) -> String {
    let out = paw(
        repo,
        home,
        tmux_env,
        &["start", "--supervisor", "--branches", branches],
    );
    assert!(out.status.success(), "start failed; {}", text(&out));
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
        .expect("session name in start stdout")
        .to_string()
}

fn receipt_path(home: &Path, session: &str) -> std::path::PathBuf {
    let dir = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-paw/sessions")
    } else {
        home.join(".local/share/git-paw/sessions")
    };
    dir.join(format!("{session}.json"))
}

/// Moves the receipt's `created_at` far back, past the launch grace period,
/// so liveness is judged from the panes.
fn backdate(home: &Path, session: &str) {
    let path = receipt_path(home, session);
    let raw = fs::read_to_string(&path).expect("read receipt");
    let mut value: serde_json::Value = serde_json::from_str(&raw).expect("parse receipt");
    assert!(value["created_at"].is_string(), "created_at: {raw}");
    value["created_at"] = serde_json::json!("2020-01-01T00:00:00Z");
    fs::write(
        &path,
        serde_json::to_string_pretty(&value).expect("serialize"),
    )
    .expect("write receipt");
}

fn pane_count(tmux_env: &TmuxTestEnv, session: &str) -> usize {
    let mut c = StdCommand::new("tmux");
    tmux_env.apply(&mut c);
    let out = c
        .args([
            "list-panes",
            "-t",
            &format!("{session}:0"),
            "-F",
            "#{pane_index}",
        ])
        .output()
        .expect("tmux list-panes");
    String::from_utf8_lossy(&out.stdout).lines().count()
}

fn kill(tmux_env: &TmuxTestEnv, session: &str) {
    let mut c = StdCommand::new("tmux");
    tmux_env.apply(&mut c);
    let _ = c.args(["kill-session", "-t", session]).status();
}

/// A live supervisor session whose only agent was removed, backdated.
fn emptied_session(repo: &Path, home: &Path, tmux_env: &TmuxTestEnv) -> String {
    let session = start(repo, home, tmux_env, "a");
    let out = paw(repo, home, tmux_env, &["remove", "a"]);
    assert!(out.status.success(), "remove failed; {}", text(&out));
    backdate(home, &session);
    session
}

/// git-add-remove-branch scenario "add after removing the last agent attaches
/// the new agent", with session-state "A live session emptied by remove stays
/// active".
#[test]
#[serial]
fn add_refills_a_live_session_emptied_by_remove() {
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().expect("home");
    let tmux_env = tmux_test_env();
    let session = emptied_session(tr.path(), home.path(), &tmux_env);

    let status = paw(tr.path(), home.path(), &tmux_env, &["status"]);
    let add = paw(tr.path(), home.path(), &tmux_env, &["add", "b"]);
    let panes = pane_count(&tmux_env, &session);
    kill(&tmux_env, &session);

    assert!(
        !String::from_utf8_lossy(&status.stdout).contains("stopped"),
        "an emptied live session must not read as stopped; {}",
        text(&status)
    );
    assert!(
        add.status.success(),
        "add must refill the session; {}",
        text(&add)
    );
    assert!(!text(&add).contains("is stopped"), "{}", text(&add));
    assert_eq!(panes, 2, "dashboard + the added agent");

    // core-init scenario "Each worker's sidecar disclaims the orchestrator
    // section", on a real launch.
    let sidecar = tr
        .path()
        .parent()
        .expect("repo parent")
        .join("repo-b/.git-paw/AGENTS.local.md");
    let sidecar = fs::read_to_string(&sidecar).expect("read the added worker's sidecar");
    assert!(
        sidecar.contains("ignore the 'git-paw orchestrator' section of AGENTS.md"),
        "{sidecar}"
    );

    // core-init scenario "The root session's context carries no worker role
    // line": the supervised start wrote the orchestrator's own context at the
    // repository root.
    let root_context = fs::read_to_string(tr.path().join(".git-paw/AGENTS.local.md"))
        .expect("read the orchestrator's context");
    assert!(
        !root_context.contains("ignore the 'git-paw orchestrator' section"),
        "{root_context}"
    );
}

/// session-state scenario "A session whose agents' panes were all closed still
/// reports Stopped": the receipt still records an agent, so a window with no
/// agent pane means it exited, and `start` recovers it instead of reattaching
/// to an empty grid.
#[test]
#[serial]
fn closed_agent_panes_with_agents_recorded_still_recover() {
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().expect("home");
    let tmux_env = tmux_test_env();
    let session = start(tr.path(), home.path(), &tmux_env, "a");

    // Close the agent's pane behind git-paw's back: the receipt keeps agent `a`.
    let mut c = StdCommand::new("tmux");
    tmux_env.apply(&mut c);
    let closed = c
        .args(["kill-pane", "-t", &format!("{session}:0.1")])
        .status()
        .expect("tmux kill-pane");
    assert!(closed.success(), "closing the agent pane failed");
    backdate(home.path(), &session);

    let status = paw(tr.path(), home.path(), &tmux_env, &["status"]);
    let restart = paw(
        tr.path(),
        home.path(),
        &tmux_env,
        &["start", "--supervisor"],
    );
    let panes = pane_count(&tmux_env, &session);
    kill(&tmux_env, &session);

    assert!(
        String::from_utf8_lossy(&status.stdout).contains("stopped"),
        "a session recording agents with no agent pane is stopped; {}",
        text(&status)
    );
    assert!(restart.status.success(), "start failed; {}", text(&restart));
    assert!(
        text(&restart).contains("Recovering session"),
        "start must recover, not reattach; {}",
        text(&restart)
    );
    assert_eq!(panes, 2, "recovery rebuilds dashboard + the recorded agent");
}

/// tmux-orchestration scenario "A live empty session refuses and names add".
#[test]
#[serial]
fn start_with_a_selection_on_a_live_empty_session_refuses_naming_add() {
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().expect("home");
    let tmux_env = tmux_test_env();
    let session = emptied_session(tr.path(), home.path(), &tmux_env);

    let out = paw(
        tr.path(),
        home.path(),
        &tmux_env,
        &["start", "--supervisor", "--branches", "b"],
    );
    let panes = pane_count(&tmux_env, &session);
    kill(&tmux_env, &session);

    assert!(!out.status.success(), "start must refuse; {}", text(&out));
    assert!(text(&out).contains("git paw add"), "{}", text(&out));
    assert_eq!(
        panes, 1,
        "the session must be left unchanged (dashboard only)"
    );
}

/// tmux-orchestration scenarios "A stopped empty session is replaced by the
/// selection" and "start without selection flags keeps its behaviour".
#[test]
#[serial]
fn a_stopped_empty_session_is_replaced_by_the_selection_and_otherwise_recovered() {
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().expect("home");
    let tmux_env = tmux_test_env();
    let session = emptied_session(tr.path(), home.path(), &tmux_env);
    let stop = paw(tr.path(), home.path(), &tmux_env, &["stop"]);
    assert!(stop.status.success(), "stop failed; {}", text(&stop));

    // git-add-remove-branch scenario "add on a genuinely stopped session is
    // still refused".
    let add = paw(tr.path(), home.path(), &tmux_env, &["add", "b"]);
    assert!(
        !add.status.success(),
        "add on a stopped session must refuse"
    );
    assert!(text(&add).contains("git paw start"), "{}", text(&add));

    // Without a selection, start keeps recovering the stopped session.
    let recover = paw(
        tr.path(),
        home.path(),
        &tmux_env,
        &["start", "--supervisor"],
    );
    let recovered = text(&recover);
    kill(&tmux_env, &session);
    let stop = paw(tr.path(), home.path(), &tmux_env, &["stop"]);
    assert!(stop.status.success(), "second stop failed; {}", text(&stop));

    // With a selection, the empty session is replaced and the selection runs.
    let replace = paw(
        tr.path(),
        home.path(),
        &tmux_env,
        &["start", "--supervisor", "--branches", "b"],
    );
    let replaced = text(&replace);
    let new_session = String::from_utf8_lossy(&replace.stdout)
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
        .map(str::to_string);
    let panes = new_session
        .as_deref()
        .map(|s| pane_count(&tmux_env, s))
        .unwrap_or_default();
    if let Some(s) = &new_session {
        kill(&tmux_env, s);
    }

    assert!(recover.status.success(), "recover failed; {recovered}");
    assert!(recovered.contains("Recovering session"), "{recovered}");
    assert!(replace.status.success(), "replace failed; {replaced}");
    assert!(
        replaced.contains("had no agents; replacing it"),
        "{replaced}"
    );
    assert!(!replaced.contains("Recovering session"), "{replaced}");
    assert_eq!(panes, 2, "dashboard + the selected agent");
}

/// tmux-orchestration scenario "A session with agents keeps its behaviour":
/// a selection on a session that still has agents does not replace it.
#[test]
#[serial]
fn a_selection_on_a_session_with_agents_keeps_the_old_behaviour() {
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().expect("home");
    let tmux_env = tmux_test_env();
    let session = start(tr.path(), home.path(), &tmux_env, "a");

    let out = paw(
        tr.path(),
        home.path(),
        &tmux_env,
        &["start", "--supervisor", "--branches", "b"],
    );
    let panes = pane_count(&tmux_env, &session);
    kill(&tmux_env, &session);

    assert!(!text(&out).contains("had no agents"), "{}", text(&out));
    // tmux-orchestration scenario "A selection given to a session with agents
    // is reported as not applied".
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("the launch selection was not applied")
            && text(&out).contains("git paw add <branch>"),
        "the ignored selection must be reported, naming git paw add; {}",
        text(&out)
    );
    assert_eq!(panes, 2, "the running session keeps its one agent");
}
