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

/// Write a supervisor-mode config (broker disabled) using `cat` as the
/// stand-in CLI so launches are fast and need no real agent binary.
///
/// `cat` (not `echo`) is deliberate: `echo` exits immediately, leaving the
/// pane a bare login shell that then EXECUTES the boot block git-paw injects
/// via `send-keys -l` — its backtick/redirect-bearing markdown creates junk
/// files, dirtying the worktree and flakily blocking a non-force `remove`
/// under load. `cat` stays alive and consumes the boot block as stdin (like a
/// real CLI, which reads the prompt rather than executing it), so the worktree
/// keeps only git-paw's managed files — exactly the clean post-start state the
/// non-force remove tests here assert.
fn write_supervisor_config(repo: &Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"cat\"\n\n[supervisor]\nenabled = true\ncli = \"cat\"\n",
    )
    .expect("write config");
}

/// An ephemeral free port, so a broker-enabled test never collides with a
/// lingering broker from another run.
fn find_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral")
        .local_addr()
        .expect("local_addr")
        .port()
}

/// Write a BARE config (no `[supervisor]` section) with the broker ENABLED,
/// using the same `cat` stand-in CLI. The broker gives the session a dashboard
/// at pane 0 and its coding agents from pane 1 — the same grid a supervised
/// session has (`git-add-remove-branch`: "Hot-attach works on every grid
/// session").
fn write_bare_config(repo: &Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let port = find_free_port();
    fs::write(
        paw_dir.join("config.toml"),
        format!("default_cli = \"cat\"\n\n[clis.cat]\ncommand = \"cat\"\n\n[broker]\nenabled = true\nport = {port}\n"),
    )
    .expect("write config");
}

/// A bare config with the broker OFF: no dashboard pane, so the coding agents
/// start at pane 0 and the agent rows share the full window height.
fn write_brokerless_bare_config(repo: &Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"cat\"\n\n[clis.cat]\ncommand = \"cat\"\n\n[broker]\nenabled = false\n",
    )
    .expect("write config");
}

/// Path of the global session receipt under an isolated `HOME`.
fn session_receipt_path(home: &Path, session: &str) -> std::path::PathBuf {
    let dir = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-paw/sessions")
    } else {
        home.join(".local/share/git-paw/sessions")
    };
    dir.join(format!("{session}.json"))
}

/// Rewrite the saved session so it looks like one an earlier binary wrote:
/// drop the recorded `layout`, and optionally force `mode`. A bare session
/// without a recorded layout is the earlier whole-window tiled layout.
fn strip_recorded_layout(home: &Path, session: &str) {
    let path = session_receipt_path(home, session);
    let raw = fs::read_to_string(&path).expect("read session receipt");
    let mut value: serde_json::Value = serde_json::from_str(&raw).expect("parse session receipt");
    assert!(
        value.as_object().is_some_and(|o| o.contains_key("layout")),
        "this binary must record a layout for the test to strip; got:\n{raw}"
    );
    value
        .as_object_mut()
        .expect("session receipt is an object")
        .remove("layout");
    fs::write(
        &path,
        serde_json::to_string_pretty(&value).expect("serialize session receipt"),
    )
    .expect("write session receipt");
}

/// Pad the saved session's worktree list out to `total` entries by appending
/// synthetic ones, so a cap that counts `session.worktrees` can be exercised
/// without standing up that many live panes.
///
/// The entries never need to exist on disk: `cmd_add` enforces the cap before
/// it creates anything, which is exactly the property under test — a run that
/// got far enough to touch these paths would fail loudly instead.
fn pad_session_worktrees(home: &Path, session: &str, total: usize) {
    let path = session_receipt_path(home, session);
    let raw = fs::read_to_string(&path).expect("read session receipt");
    let mut value: serde_json::Value = serde_json::from_str(&raw).expect("parse session receipt");
    let worktrees = value
        .get_mut("worktrees")
        .and_then(serde_json::Value::as_array_mut)
        .expect("session receipt lists worktrees");
    assert!(
        worktrees.len() <= total,
        "the session already has more than {total} agents"
    );
    for i in worktrees.len()..total {
        worktrees.push(serde_json::json!({
            "branch": format!("feat/pad-{i}"),
            "worktree_path": format!("/nonexistent/pad-{i}"),
            "cli": "cat",
            "branch_created": false,
        }));
    }
    fs::write(
        &path,
        serde_json::to_string_pretty(&value).expect("serialize session receipt"),
    )
    .expect("write session receipt");
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

/// Capture every live pane's visible content, keyed by the worktree it runs
/// in. Used to prove `add` / `remove` type into no surface but the one they
/// are creating: a pane that received a keystroke would render new text.
///
/// Captures the FULL scrollback (`-S -`), not the visible region, and strips
/// whitespace: the re-tile legitimately changes each pane's width and height,
/// so tmux both re-wraps the text already on it and scrolls a different part
/// of it into view. Neither is a keystroke. What this compares is the
/// characters the pane has ever shown, which no re-tile can add to.
///
/// Only AGENT panes are returned (their worktrees are `<repo>-<branch>`
/// siblings). The dashboard pane is excluded because it is a live TUI that
/// redraws itself about once a second, so its content changes with no input
/// at all and cannot be compared this way. That the dashboard is never typed
/// into is pinned instead by the role resolution — `SurfaceRole::Dashboard`
/// is never acted on — and by the drive-loop tests.
fn capture_panes_by_worktree(
    tmux_env: &TmuxTestEnv,
    session: &str,
) -> std::collections::HashMap<String, String> {
    pane_paths(tmux_env, session)
        .into_iter()
        .filter(|(_, path)| path.contains("repo-"))
        .map(|(idx, path)| {
            let mut c = StdCommand::new("tmux");
            tmux_env.apply(&mut c);
            let out = c
                .args([
                    "capture-pane",
                    "-p",
                    "-S",
                    "-",
                    "-t",
                    &format!("{session}:0.{idx}"),
                ])
                .output()
                .expect("tmux capture-pane");
            let text = String::from_utf8_lossy(&out.stdout)
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            (path, text)
        })
        .collect()
}

fn kill(tmux_env: &TmuxTestEnv, session: &str) {
    let mut c = StdCommand::new("tmux");
    tmux_env.apply(&mut c);
    let _ = c.args(["kill-session", "-t", session]).status();
}

/// Start a supervisor session with the given comma-separated branches and
/// return its session name (parsed from stdout).
fn start_session(repo: &Path, home: &Path, tmux_env: &TmuxTestEnv, branches: &str) -> String {
    start_session_with(repo, home, tmux_env, branches, true)
}

/// [`start_session`], with `--supervisor` optional so the bare launch path can
/// be driven too.
fn start_session_with(
    repo: &Path,
    home: &Path,
    tmux_env: &TmuxTestEnv,
    branches: &str,
    supervisor: bool,
) -> String {
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let mut args: Vec<&str> = vec!["start"];
    if supervisor {
        args.push("--supervisor");
    } else {
        // The bare path prompts for the CLI when none is named on the command
        // line; naming it keeps the launch non-interactive.
        args.extend(["--cli", "cat"]);
    }
    args.extend(["--branches", branches]);
    let out = start
        .current_dir(repo)
        .env("HOME", home)
        .env_remove("XDG_DATA_HOME")
        .args(&args)
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
    let home = TempDir::new().expect("home tempdir");
    let mut c = cmd();
    let out = c
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
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
        .env_remove("XDG_DATA_HOME")
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
    // 2 agents -> dashboard + 2 = 3 panes.
    assert_eq!(
        list_pane_count(&tmux_env, &session),
        3,
        "expected 3 panes after a 2-agent start"
    );

    let mut add = cmd();
    tmux_env.apply_assert(&mut add);
    let out = add
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
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
    assert_eq!(panes, 4, "add should splice one pane (3 -> 4)");
    assert_eq!(agent_count, 3, "discovery file should list 3 agents");
    // `agent-c`, not a bare `c`: a single-word branch's id takes the `agent-`
    // prefix, because the broker's `/publish` validator refuses a bare word
    // (`agent-id-slug-safety`). Before that prefix this registered as `c`, an
    // id every publish from the agent would have been answered 400 for.
    assert!(
        raw.contains("\"branch_id\": \"agent-c\""),
        "discovery file should register the added branch 'c' as agent-c; raw:\n{raw}"
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
    assert_eq!(list_pane_count(&tmux_env, &session), 4);

    // Worktree for 'b' lives as a sibling of the repo.
    let wt_b = tr.path().parent().unwrap().join("repo-b");
    assert!(wt_b.exists(), "worktree for 'b' should exist after start");

    let mut rm = cmd();
    tmux_env.apply_assert(&mut rm);
    let out = rm
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
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
    assert_eq!(panes, 3, "remove should close one pane (4 -> 3)");
    assert_eq!(agent_count, 2, "discovery file should list 2 agents");
    assert!(
        !raw.contains("\"branch_id\": \"b\""),
        "removed branch 'b' should be gone from the discovery file; raw:\n{raw}"
    );
    assert!(wt_gone, "worktree for 'b' should be removed from disk");
}

// --- branch retention on teardown (`git-add-remove-branch`) ---

/// The tip SHA of `branch` in `repo`, or `None` when it does not exist.
fn branch_tip(repo: &Path, branch: &str) -> Option<String> {
    let out = StdCommand::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--verify", "--quiet", branch])
        .output()
        .expect("git rev-parse");
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn commit_in(worktree: &Path, file: &str, body: &str, message: &str) {
    fs::write(worktree.join(file), body).expect("write file");
    for args in [vec!["add", file], vec!["commit", "-q", "-m", message]] {
        let out = StdCommand::new("git")
            .current_dir(worktree)
            .args(&args)
            .output()
            .expect("git spawn");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// Scenario "remove keeps a created branch with unmerged commits": the
/// worktree goes, the branch stays at the same tip, and the output names it
/// with its SHA and the recovery hint.
#[test]
#[serial]
fn remove_keeps_a_created_branch_that_has_unmerged_commits() {
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
    commit_in(&wt_a, "work.txt", "agent work", "agent work");
    let tip_before = branch_tip(tr.path(), "a").expect("branch 'a' exists before remove");

    let mut rm = cmd();
    tmux_env.apply_assert(&mut rm);
    let out = rm
        .current_dir(tr.path())
        .env("HOME", home.path())
        .args(["remove", "a"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run remove");

    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let wt_gone = !wt_a.exists();
    let tip_after = branch_tip(tr.path(), "a");
    kill(&tmux_env, &session);

    assert!(out.status.success(), "remove failed; stderr:\n{stderr}");
    assert!(wt_gone, "worktree for 'a' should be removed from disk");
    assert_eq!(
        tip_after.as_deref(),
        Some(tip_before.as_str()),
        "the unmerged branch must survive at the same tip; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("Kept branch a at") && stderr.contains("1 commit(s) not in main"),
        "retention must report the branch, its tip, and the count; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("git worktree add") && stderr.contains("git branch -D a"),
        "retention must print the resume and discard hints; stderr:\n{stderr}"
    );
}

/// Scenario "remove deletes a created branch that is fully merged".
#[test]
#[serial]
fn remove_deletes_a_created_branch_that_is_fully_merged() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session(tr.path(), home.path(), &tmux_env, "a");
    let tip_before = branch_tip(tr.path(), "a").expect("branch 'a' exists before remove");
    let short_tip = tip_before[..7].to_string();

    let mut rm = cmd();
    tmux_env.apply_assert(&mut rm);
    let out = rm
        .current_dir(tr.path())
        .env("HOME", home.path())
        .args(["remove", "a"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run remove");

    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let tip_after = branch_tip(tr.path(), "a");
    kill(&tmux_env, &session);

    assert!(out.status.success(), "remove failed; stderr:\n{stderr}");
    assert!(
        tip_after.is_none(),
        "a fully merged created branch should be deleted; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains(&format!("Deleted branch a (was {short_tip}")),
        "deletion must print the branch and the SHA it was at; stderr:\n{stderr}"
    );
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
        .env_remove("XDG_DATA_HOME")
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
        .env_remove("XDG_DATA_HOME")
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

// --- remove-dirty-check-ignores-managed-files regression coverage ---

/// Scenario "Clean just-started worktree with only git-paw-injected files is
/// removed": a freshly provisioned agent whose only uncommitted entry is
/// git-paw's injected sidecar is removed by a plain `remove` (no `--force`),
/// and the command output never mentions the sidecar. This is the v0.8.0
/// regression made deterministic by the exclude-before-write reorder and the
/// managed-path filter.
#[test]
#[serial]
fn remove_just_started_clean_agent_succeeds_without_force() {
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
    assert!(wt_a.exists(), "worktree for 'a' should exist after start");
    // The injected sidecar is present — its mere presence must NOT block remove.
    assert!(
        wt_a.join(".git-paw/AGENTS.local.md").exists(),
        "start should have injected the sidecar"
    );

    let panes_before = list_pane_count(&tmux_env, &session);

    let mut rm = cmd();
    tmux_env.apply_assert(&mut rm);
    let out = rm
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["remove", "a"]) // deliberately no --force
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run remove");

    let panes_after = list_pane_count(&tmux_env, &session);
    let (agent_count, raw) = read_discovery(tr.path(), &session);
    let wt_gone = !wt_a.exists();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    kill(&tmux_env, &session);

    assert!(
        out.status.success(),
        "plain remove of a clean just-started agent must succeed; output:\n{combined}"
    );
    assert_eq!(
        panes_after,
        panes_before - 1,
        "remove should close exactly the agent's pane"
    );
    assert!(wt_gone, "worktree for 'a' should be removed from disk");
    assert_eq!(agent_count, 0, "session JSON should list no agents");
    assert!(
        !raw.contains("\"branch_id\": \"a\""),
        "removed branch 'a' should be gone from the discovery file; raw:\n{raw}"
    );
    assert!(
        !combined.contains(".git-paw/AGENTS.local.md"),
        "remove output must NOT mention the injected sidecar; output:\n{combined}"
    );
}

/// Scenario "Genuine user edit still refuses, and managed files are not
/// listed": a worktree containing a real user-authored change (`src/foo.rs`)
/// alongside the injected sidecar is still refused without `--force`; the
/// refusal names `src/foo.rs` but never the sidecar.
#[test]
#[serial]
fn remove_refuses_genuine_edit_and_hides_managed_files() {
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
    assert!(wt_a.exists(), "worktree for 'a' should exist after start");
    assert!(
        wt_a.join(".git-paw/AGENTS.local.md").exists(),
        "start should have injected the sidecar"
    );

    // Genuine user work: a new source file, staged so `git status` reports it
    // by path (an untracked file in a brand-new directory would collapse to
    // the `src/` directory entry).
    fs::create_dir_all(wt_a.join("src")).expect("mkdir src");
    fs::write(wt_a.join("src/foo.rs"), "fn foo() {}\n").expect("write foo.rs");
    let add = StdCommand::new("git")
        .current_dir(&wt_a)
        .args(["add", "src/foo.rs"])
        .output()
        .expect("git add foo.rs");
    assert!(
        add.status.success(),
        "git add of the user file should succeed"
    );

    let mut rm = cmd();
    tmux_env.apply_assert(&mut rm);
    let out = rm
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["remove", "a"]) // deliberately no --force
        .timeout(Duration::from_secs(20))
        .output()
        .expect("run remove");

    let still_there = wt_a.exists();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    kill(&tmux_env, &session);

    assert!(
        !out.status.success(),
        "remove must refuse while a genuine user edit is uncommitted; stderr:\n{stderr}"
    );
    assert!(still_there, "worktree must survive a refused remove");
    assert!(
        stderr.contains("src/foo.rs"),
        "refusal must name the user file; stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains(".git-paw/AGENTS.local.md"),
        "refusal must NOT list git-paw's injected sidecar; stderr:\n{stderr}"
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
        .env_remove("XDG_DATA_HOME")
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
        .env_remove("XDG_DATA_HOME")
        .args(["pause"])
        .timeout(Duration::from_secs(20))
        .output()
        .expect("run pause");

    let mut add = cmd();
    tmux_env.apply_assert(&mut add);
    let out = add
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
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
        stdout.contains("paused"),
        "add to a paused session should report it will start on `git paw start`; stdout:\n{stdout}"
    );
    // 1 agent -> dashboard + a = 2 panes; the held add makes 3.
    assert_eq!(panes, 3, "paused add still creates the pane (2 -> 3)");
    assert_eq!(
        agent_count, 2,
        "the held agent is registered in the session"
    );
}

/// Collect `(pane_index, pane_current_path)` for every pane on the test socket.
///
/// The index/path pair is space-delimited, NOT tab-delimited: a literal tab in
/// a tmux `-F` format string does not survive to the output on every tmux build
/// (Ubuntu noble's tmux 3.4 emits a space where macOS's tmux emits the tab), so
/// a `split_once('\t')` silently drops every line and the caller sees an empty
/// list even though the panes are healthy. A space is safe to split on because
/// `#{pane_index}` is always a bare integer, so the first space is unambiguously
/// the boundary — even if `#{pane_current_path}` itself contains spaces.
fn pane_paths(tmux_env: &TmuxTestEnv, session: &str) -> Vec<(String, String)> {
    let mut c = StdCommand::new("tmux");
    tmux_env.apply(&mut c);
    let out = c
        .args([
            "list-panes",
            "-t",
            session,
            "-F",
            "#{pane_index} #{pane_current_path}",
        ])
        .output()
        .expect("tmux list-panes paths");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let (i, p) = l.split_once(' ')?;
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
        .env_remove("XDG_DATA_HOME")
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
    assert_eq!(list_pane_count(&tmux_env, &session), 2);

    let mut add = cmd();
    tmux_env.apply_assert(&mut add);
    let out = add
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
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
    assert_eq!(panes, 3, "add --from-spec should splice one pane (2 -> 3)");
    assert_eq!(
        agent_count, 2,
        "the spec-derived agent should be registered in the session"
    );
}

// ---------------------------------------------------------------------------
// `git-add-remove-branch` — "Hot-attach works on every grid session".
//
// `add` / `remove` no longer refuse a session for being bare: every session
// this binary builds is the agent grid, which the incremental re-tile builders
// can extend. Only a live session an earlier binary built with the tiled
// layout is refused.
// ---------------------------------------------------------------------------

/// Scenario "Add to a bare grid session": a bare session started with two
/// branches and the broker enabled accepts a third agent, landing at pane 3
/// (dashboard 0, agents 1-3) — the grid a 3-agent start would produce.
#[test]
#[serial]
fn add_attaches_to_a_bare_grid_session() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_bare_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session_with(tr.path(), home.path(), &tmux_env, "a,b", false);
    assert_eq!(
        list_pane_count(&tmux_env, &session),
        3,
        "a 2-agent bare start is dashboard + 2 agents"
    );
    let before = capture_panes_by_worktree(&tmux_env, &session);

    let mut add = cmd();
    tmux_env.apply_assert(&mut add);
    let out = add
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["add", "c"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run add");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    let panes = list_pane_count(&tmux_env, &session);
    let paths = pane_paths(&tmux_env, &session);
    let after = capture_panes_by_worktree(&tmux_env, &session);
    let (agent_count, raw) = read_discovery(tr.path(), &session);
    kill(&tmux_env, &session);

    assert!(
        out.status.success(),
        "add on a bare grid session must not be refused; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    // Scenario "The new agent appears in the roster sources": no keystroke is
    // sent to any surface other than the new agent's pane — every
    // pre-existing agent pane has shown exactly what it had shown before.
    assert!(
        !before.is_empty(),
        "the start must leave agent panes to compare"
    );
    for (worktree, content) in &before {
        assert_eq!(
            after.get(worktree),
            Some(content),
            "add typed into {worktree}, a surface other than the new agent's pane"
        );
    }
    assert_eq!(panes, 4, "add should splice one pane (3 -> 4)");
    assert_eq!(agent_count, 3, "discovery file should list 3 agents");
    assert!(
        raw.contains("\"branch_id\": \"agent-c\""),
        "the added branch should be registered; raw:\n{raw}"
    );
    // The new agent is the last pane of the agent row, matching a 3-agent
    // start's grid.
    let new_pane = paths
        .iter()
        .find(|(_, path)| path.ends_with("repo-c"))
        .map(|(idx, _)| idx.clone())
        .expect("the added worktree should own a pane");
    assert_eq!(
        new_pane, "3",
        "the third agent takes pane 3 below the dashboard; panes:\n{paths:?}"
    );
}

/// Scenario "Remove from a bare grid session": only the named agent's pane
/// closes and the grid re-flows for the smaller count.
#[test]
#[serial]
fn remove_detaches_from_a_bare_grid_session() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_bare_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session_with(tr.path(), home.path(), &tmux_env, "a,b,c", false);
    assert_eq!(list_pane_count(&tmux_env, &session), 4);

    let wt_b = tr.path().parent().unwrap().join("repo-b");
    assert!(wt_b.exists(), "worktree for 'b' should exist after start");
    let before = capture_panes_by_worktree(&tmux_env, &session);

    let mut rm = cmd();
    tmux_env.apply_assert(&mut rm);
    let out = rm
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["remove", "b"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run remove");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    let panes = list_pane_count(&tmux_env, &session);
    let paths = pane_paths(&tmux_env, &session);
    let after = capture_panes_by_worktree(&tmux_env, &session);
    let wt_gone = !wt_b.exists();
    kill(&tmux_env, &session);

    assert!(
        out.status.success(),
        "remove on a bare grid session must not be refused; stderr:\n{stderr}"
    );
    // Scenario "The removed agent leaves the roster sources": no keystroke is
    // sent to any surface as part of the removal — every surviving agent pane
    // has shown exactly what it had shown before.
    assert!(
        !after.is_empty(),
        "agents must survive the removal to compare"
    );
    for (worktree, content) in &after {
        assert_eq!(
            before.get(worktree),
            Some(content),
            "remove typed into the surviving surface {worktree}"
        );
    }
    assert_eq!(panes, 3, "remove should close one pane (4 -> 3)");
    assert!(wt_gone, "worktree for 'b' should be removed from disk");
    // Only 'b' went: 'a' and 'c' each still own exactly one pane.
    for survivor in ["repo-a", "repo-c"] {
        assert_eq!(
            paths
                .iter()
                .filter(|(_, path)| path.ends_with(survivor))
                .count(),
            1,
            "{survivor} should still own exactly one pane; panes:\n{paths:?}"
        );
    }
    assert!(
        !paths.iter().any(|(_, path)| path.ends_with("repo-b")),
        "the removed agent should own no pane; panes:\n{paths:?}"
    );
}

/// Scenario "Add to a broker-less grid session": with no dashboard the agents
/// start at pane 0, and the new agent takes the next index after them.
#[test]
#[serial]
fn add_attaches_to_a_brokerless_grid_session() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_brokerless_bare_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session_with(tr.path(), home.path(), &tmux_env, "a,b", false);
    assert_eq!(
        list_pane_count(&tmux_env, &session),
        2,
        "with the broker off there is no dashboard pane"
    );

    let mut add = cmd();
    tmux_env.apply_assert(&mut add);
    let out = add
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["add", "c"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run add");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    let panes = list_pane_count(&tmux_env, &session);
    let paths = pane_paths(&tmux_env, &session);
    kill(&tmux_env, &session);

    assert!(
        out.status.success(),
        "add on a broker-less grid session must not be refused; \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(panes, 3, "add should splice one pane (2 -> 3)");
    let new_pane = paths
        .iter()
        .find(|(_, path)| path.ends_with("repo-c"))
        .map(|(idx, _)| idx.clone())
        .expect("the added worktree should own a pane");
    assert_eq!(
        new_pane, "2",
        "agents start at pane 0, so the third takes pane 2; panes:\n{paths:?}"
    );
}

/// Scenario "A supervised session saved earlier stays hot-attachable": a
/// supervised session whose receipt records no layout is the grid already, so
/// it is NOT refused — the refusal keys on the layout, not the mode. (The
/// offset half of the scenario, a receipt whose dashboard sits at pane 1 with
/// agents from 2, is pinned by the unit test
/// `add_agent_to_a_pre_relocation_session_uses_its_recorded_offset`.)
#[test]
#[serial]
fn add_still_works_on_a_supervised_session_saved_without_a_layout() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session(tr.path(), home.path(), &tmux_env, "a,b");
    strip_recorded_layout(home.path(), &session);

    let mut add = cmd();
    tmux_env.apply_assert(&mut add);
    let out = add
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["add", "c"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run add");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    let panes = list_pane_count(&tmux_env, &session);
    kill(&tmux_env, &session);

    assert!(
        out.status.success(),
        "a supervised session saved by an earlier binary stays hot-attachable; \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("tiled"),
        "a supervised session was never the tiled layout; stderr:\n{stderr}"
    );
    assert_eq!(panes, 4, "add should splice one pane (3 -> 4)");
}

/// Scenario "Adding past the agent cap is rejected": a session already at the
/// 25-agent cap refuses the next `add` with the same actionable error `start`
/// uses, and creates no worktree or pane doing so.
#[test]
#[serial]
fn adding_past_the_agent_cap_is_rejected_without_creating_anything() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_brokerless_bare_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session_with(tr.path(), home.path(), &tmux_env, "a,b", false);
    pad_session_worktrees(home.path(), &session, 25);

    let panes_before = list_pane_count(&tmux_env, &session);

    let mut add = cmd();
    tmux_env.apply_assert(&mut add);
    let out = add
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["add", "twenty-six"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run add");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    let panes_after = list_pane_count(&tmux_env, &session);
    let wt_created = tr.path().parent().unwrap().join("repo-twenty-six").exists();
    kill(&tmux_env, &session);

    assert!(
        !out.status.success(),
        "the 26th agent must be refused; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("maximum is 25") && stderr.contains("Split into multiple sessions"),
        "add must surface the same cap error `start` uses; stderr:\n{stderr}"
    );
    assert_eq!(panes_after, panes_before, "no pane may be created");
    assert!(
        !wt_created,
        "no worktree may be created for the refused branch"
    );
}

/// Scenario "An earlier tiled session is refused explicitly": a live BARE
/// session whose receipt records no layout is the earlier tiled layout. Both
/// verbs refuse it, naming the layout and the fix, citing no version — and
/// touching no worktree, pane or session entry.
#[test]
#[serial]
fn an_earlier_tiled_bare_session_is_refused_without_mutating_anything() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_bare_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session_with(tr.path(), home.path(), &tmux_env, "a,b", false);
    strip_recorded_layout(home.path(), &session);

    let panes_before = list_pane_count(&tmux_env, &session);
    let (agents_before, _) = read_discovery(tr.path(), &session);
    let receipt_before =
        fs::read_to_string(session_receipt_path(home.path(), &session)).expect("read receipt");
    let wt_a = tr.path().parent().unwrap().join("repo-a");

    let mut results = Vec::new();
    for args in [["add", "x"], ["remove", "a"]] {
        let mut c = cmd();
        tmux_env.apply_assert(&mut c);
        let out = c
            .current_dir(tr.path())
            .env("HOME", home.path())
            .env_remove("XDG_DATA_HOME")
            .args(args)
            .timeout(Duration::from_secs(40))
            .output()
            .expect("run command");
        results.push((
            args[0],
            out.status.success(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        ));
    }

    let panes_after = list_pane_count(&tmux_env, &session);
    let (agents_after, _) = read_discovery(tr.path(), &session);
    let receipt_after =
        fs::read_to_string(session_receipt_path(home.path(), &session)).expect("read receipt");
    let wt_a_alive = wt_a.exists();
    let wt_x_absent = !tr.path().parent().unwrap().join("repo-x").exists();
    kill(&tmux_env, &session);

    for (verb, success, stderr) in &results {
        assert!(
            !success,
            "`git paw {verb}` must exit non-zero on a tiled session; stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("tiled"),
            "`{verb}` error must name the tiled layout; stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("git paw start"),
            "`{verb}` error must tell the user to start the session again; stderr:\n{stderr}"
        );
        assert!(
            !stderr.contains("v0.6.0") && !stderr.contains("v1.0.0"),
            "`{verb}` error must not cite a release version; stderr:\n{stderr}"
        );
    }

    // Refused BEFORE any mutation: no pane, worktree or session entry moved.
    assert_eq!(
        panes_after, panes_before,
        "no pane may be created or killed"
    );
    assert_eq!(
        agents_after, agents_before,
        "no session entry may be added or removed"
    );
    assert_eq!(
        receipt_after, receipt_before,
        "the session receipt must be untouched"
    );
    assert!(wt_a_alive, "the removal target's worktree must survive");
    assert!(wt_x_absent, "no worktree may be created for the add target");
}

// --- GP-81: every agent's pane index holds that agent, from the 6th on ---

/// `(pane_index, pane_current_command)` for every pane on the test socket.
fn pane_commands(tmux_env: &TmuxTestEnv, session: &str) -> Vec<(String, String)> {
    let mut c = StdCommand::new("tmux");
    tmux_env.apply(&mut c);
    let out = c
        .args([
            "list-panes",
            "-t",
            session,
            "-F",
            "#{pane_index} #{pane_current_command}",
        ])
        .output()
        .expect("tmux list-panes commands");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let (i, cmd) = l.split_once(' ')?;
            Some((i.to_string(), cmd.to_string()))
        })
        .collect()
}

/// `(pane_index, pane_width)` for every pane on the test socket.
fn pane_widths(tmux_env: &TmuxTestEnv, session: &str) -> Vec<(usize, u32)> {
    let mut c = StdCommand::new("tmux");
    tmux_env.apply(&mut c);
    let out = c
        .args([
            "list-panes",
            "-t",
            session,
            "-F",
            "#{pane_index} #{pane_width}",
        ])
        .output()
        .expect("tmux list-panes widths");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let (i, w) = l.split_once(' ')?;
            Some((i.parse().ok()?, w.parse().ok()?))
        })
        .collect()
}

/// Assert pane `first_agent + i` runs in `branches[i]`'s worktree AND runs the
/// stand-in CLI there. A misrouted grid fails both halves: an agent's
/// worktree lands at another index, and the launch command typed by computed
/// index runs in the wrong pane, leaving one agent pane a bare shell (GP-81).
///
/// Polls briefly: a pane's current command reads as the shell until it has
/// exec'd the CLI typed into it.
fn assert_grid_order(tmux_env: &TmuxTestEnv, session: &str, first_agent: usize, branches: &[&str]) {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        let paths = pane_paths(tmux_env, session);
        let commands = pane_commands(tmux_env, session);
        let mismatches: Vec<String> = branches
            .iter()
            .enumerate()
            .filter_map(|(i, branch)| {
                let idx = (first_agent + i).to_string();
                let path = paths.iter().find(|(p, _)| *p == idx).map(|(_, p)| p);
                let command = commands.iter().find(|(p, _)| *p == idx).map(|(_, c)| c);
                let in_worktree = path.is_some_and(|p| p.ends_with(&format!("repo-{branch}")));
                let runs_cli = command.is_some_and(|c| c == "cat");
                (!in_worktree || !runs_cli).then(|| {
                    format!("pane {idx}: expected repo-{branch} running cat, got {path:?} running {command:?}")
                })
            })
            .collect();
        if mismatches.is_empty() {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "every agent pane must hold its own worktree and CLI:\n{}\npanes: {paths:?}\ncommands: {commands:?}",
            mismatches.join("\n")
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Scenarios "Bare start builds the grid" and "Every agent's pane index holds
/// that agent": a seven-agent bare start with the broker on puts the dashboard
/// at pane 0 and agent i at pane 1 + i, the CLI running in each.
#[test]
#[serial]
fn a_seven_agent_bare_start_puts_every_agent_in_its_own_pane() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_bare_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let branches = ["a", "b", "c", "d", "e", "f", "g"];
    let session = start_session_with(
        tr.path(),
        home.path(),
        &tmux_env,
        &branches.join(","),
        false,
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_eq!(
            list_pane_count(&tmux_env, &session),
            8,
            "dashboard + 7 agents"
        );
        assert_grid_order(&tmux_env, &session, 1, &branches);
    }));
    kill(&tmux_env, &session);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

/// Scenario "Broker-less start uses the full height", for more than one row:
/// with no dashboard agent i is pane i, the CLI running in each.
#[test]
#[serial]
fn a_seven_agent_brokerless_start_puts_every_agent_in_its_own_pane() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_brokerless_bare_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let branches = ["a", "b", "c", "d", "e", "f", "g"];
    let session = start_session_with(
        tr.path(),
        home.path(),
        &tmux_env,
        &branches.join(","),
        false,
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_eq!(
            list_pane_count(&tmux_env, &session),
            7,
            "7 agents, no dashboard"
        );
        assert_grid_order(&tmux_env, &session, 0, &branches);
    }));
    kill(&tmux_env, &session);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

/// Scenario "Bare start builds the grid" (equal-width rows): a bare start
/// rebalances its row, so three agents under the dashboard get equal widths
/// within tmux's one-column rounding, not the 50/25/25 the raw splits give.
#[test]
#[serial]
fn a_bare_start_gives_its_agent_row_equal_widths() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_bare_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session_with(tr.path(), home.path(), &tmux_env, "a,b,c", false);
    let widths = pane_widths(&tmux_env, &session);
    kill(&tmux_env, &session);

    let agent_widths: Vec<u32> = widths
        .iter()
        .filter(|(idx, _)| (1..=3).contains(idx))
        .map(|(_, w)| *w)
        .collect();
    assert_eq!(
        agent_widths.len(),
        3,
        "three agent panes; widths: {widths:?}"
    );
    let (min, max) = (
        agent_widths.iter().min().copied().unwrap_or(0),
        agent_widths.iter().max().copied().unwrap_or(0),
    );
    assert!(
        max - min <= 1,
        "the agent row must be equal-width within one column; widths: {widths:?}"
    );
}

/// Scenarios "Added grid matches a start-time grid" and "The new agent appears
/// in the roster sources", at the row boundary: adding a sixth agent to a
/// five-agent bare session opens a second row, the new agent lands at pane 6
/// running its CLI, every existing agent keeps its pane, and no existing
/// agent pane receives a keystroke.
#[test]
#[serial]
fn adding_a_sixth_agent_opens_a_row_and_types_into_no_other_pane() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_bare_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session_with(tr.path(), home.path(), &tmux_env, "a,b,c,d,e", false);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_grid_order(&tmux_env, &session, 1, &["a", "b", "c", "d", "e"]);
        let before = capture_panes_by_worktree(&tmux_env, &session);

        let mut add = cmd();
        tmux_env.apply_assert(&mut add);
        let out = add
            .current_dir(tr.path())
            .env("HOME", home.path())
            .env_remove("XDG_DATA_HOME")
            .args(["add", "f"])
            .timeout(Duration::from_secs(40))
            .output()
            .expect("run add");
        assert!(
            out.status.success(),
            "add of a sixth agent failed; stderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );

        assert_grid_order(&tmux_env, &session, 1, &["a", "b", "c", "d", "e", "f"]);
        let after = capture_panes_by_worktree(&tmux_env, &session);
        for (worktree, content) in &before {
            assert_eq!(
                after.get(worktree),
                Some(content),
                "add typed into {worktree}, a surface other than the new agent's pane"
            );
        }
    }));
    kill(&tmux_env, &session);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

/// Scenario "A supervised session saved earlier stays hot-attachable", the
/// offset half, end to end: a live session an earlier binary built (its
/// dashboard at pane 1, agents from pane 2) takes a sixth agent at the next
/// index after its own last agent, opening a new row without renumbering any
/// existing pane or typing into one.
#[test]
#[serial]
fn adding_a_sixth_agent_to_a_pre_relocation_session_keeps_its_offset() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session(tr.path(), home.path(), &tmux_env, "a,b,c,d,e");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // Recreate the pre-relocation shape: a pane ahead of the dashboard (the
        // old supervisor pane), so the dashboard is pane 1 and the agents run
        // from pane 2, and a receipt that records it that way.
        let mut split = StdCommand::new("tmux");
        tmux_env.apply(&mut split);
        let status = split
            .args([
                "split-window",
                "-b",
                "-v",
                "-t",
                &format!("{session}:0.0"),
                "-c",
                &tr.path().to_string_lossy(),
            ])
            .status()
            .expect("tmux split-window -b");
        assert!(status.success(), "could not insert the legacy pane 0");
        strip_recorded_layout(home.path(), &session);
        let path = session_receipt_path(home.path(), &session);
        let raw = fs::read_to_string(&path).expect("read session receipt");
        let mut value: serde_json::Value = serde_json::from_str(&raw).expect("parse receipt");
        value["dashboard_pane"] = serde_json::json!(1);
        fs::write(
            &path,
            serde_json::to_string_pretty(&value).expect("serialize"),
        )
        .expect("write session receipt");

        assert_grid_order(&tmux_env, &session, 2, &["a", "b", "c", "d", "e"]);
        let before = capture_panes_by_worktree(&tmux_env, &session);

        let mut add = cmd();
        tmux_env.apply_assert(&mut add);
        let out = add
            .current_dir(tr.path())
            .env("HOME", home.path())
            .env_remove("XDG_DATA_HOME")
            .args(["add", "f"])
            .timeout(Duration::from_secs(40))
            .output()
            .expect("run add");
        assert!(
            out.status.success(),
            "add to a pre-relocation session failed; stderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );

        assert_grid_order(&tmux_env, &session, 2, &["a", "b", "c", "d", "e", "f"]);
        let after = capture_panes_by_worktree(&tmux_env, &session);
        for (worktree, content) in &before {
            assert_eq!(
                after.get(worktree),
                Some(content),
                "add typed into {worktree}, a surface other than the new agent's pane"
            );
        }
    }));
    kill(&tmux_env, &session);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
