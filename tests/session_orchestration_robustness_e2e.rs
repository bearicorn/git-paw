//! E2E regression tests for session-orchestration-robustness (G2/G3) — the
//! tests that would have caught the v0.8.0 dogfood failures.
//!
//! Each test drives the real `git-paw` binary against a per-test isolated tmux
//! socket ([`helpers::TmuxTestEnv`]) and an isolated `HOME`, so the global
//! receipt lands under a temp dir — never the user's real data dir or the live
//! supervisor session. Skips when tmux is unavailable.
//!
//! Coverage:
//! - 6.1/6.2 applied-layout widths (live tmux): a 3-agent row renders equal
//!   thirds (NOT 50/25/25); the top row stays ~50/50; a 5-agent row is ~20%
//!   per pane.
//! - 6.3 pane-integrity on `add`: pane count == JSON-agent count (+supervisor
//!   +dashboard) and every agent's worktree→pane `pane_current_path` mapping
//!   is intact.
//! - 6.4 pane-integrity on `remove`: removing a middle agent kills only that
//!   pane (no collateral loss, no orphan); the removed pane is an `echo`
//!   shell-occupied pane (the G1 condition).
//!
//! The launches use the `echo` CLI, so each pane is a bare shell — the
//! launch-readiness gate falls back after its budget. `GIT_PAW_READINESS_TIMEOUT_MS`
//! is set low so that fall-back is fast.

use std::path::Path;
use std::process::Command as StdCommand;
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

/// Low readiness budget: every `echo` pane is a bare shell that never matches a
/// CLI marker, so the gate would otherwise spend the full default budget per
/// pane. 120ms keeps the fall-back path fast for tests.
const FAST_READINESS_MS: &str = "120";

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn tmux_available() -> bool {
    StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Supervisor-mode config: broker disabled, `echo` CLI, so launches are fast
/// and need no real agent binary.
fn write_supervisor_config(repo: &Path) {
    let paw_dir = repo.join(".git-paw");
    std::fs::create_dir_all(&paw_dir).expect("create .git-paw");
    std::fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"echo\"\n\n[supervisor]\nenabled = true\ncli = \"echo\"\n",
    )
    .expect("write config");
}

/// Start a supervisor session with the given comma-separated branches and
/// return its session name (parsed from stdout).
fn start_session(repo: &Path, home: &Path, tmux_env: &TmuxTestEnv, branches: &str) -> String {
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(repo)
        .env("HOME", home)
        .env("GIT_PAW_READINESS_TIMEOUT_MS", FAST_READINESS_MS)
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

fn kill(tmux_env: &TmuxTestEnv, session: &str) {
    let mut c = StdCommand::new("tmux");
    tmux_env.apply(&mut c);
    let _ = c.args(["kill-session", "-t", session]).status();
}

/// `(pane_index, pane_width)` for every live pane in `session`, sorted by index.
fn pane_widths(tmux_env: &TmuxTestEnv, session: &str) -> Vec<(usize, usize)> {
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
    let text = String::from_utf8_lossy(&out.stdout);
    let mut rows: Vec<(usize, usize)> = text
        .lines()
        .filter_map(|l| {
            let (i, w) = l.split_once(' ')?;
            Some((i.trim().parse().ok()?, w.trim().parse().ok()?))
        })
        .collect();
    rows.sort_by_key(|(i, _)| *i);
    rows
}

/// The `pane_current_path` of every live pane in `session`.
fn pane_paths(tmux_env: &TmuxTestEnv, session: &str) -> Vec<String> {
    let mut c = StdCommand::new("tmux");
    tmux_env.apply(&mut c);
    let out = c
        .args(["list-panes", "-t", session, "-F", "#{pane_current_path}"])
        .output()
        .expect("tmux list-panes paths");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| {
            std::fs::canonicalize(l.trim()).map_or_else(
                |_| l.trim().to_string(),
                |p| p.to_string_lossy().into_owned(),
            )
        })
        .collect()
}

fn canonical(p: &Path) -> String {
    std::fs::canonicalize(p).map_or_else(
        |_| p.to_string_lossy().into_owned(),
        |c| c.to_string_lossy().into_owned(),
    )
}

// --- 6.1 / 6.2 applied-layout widths -------------------------------------

#[test]
#[serial]
fn three_agent_row_renders_equal_thirds_not_50_25_25() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session(tr.path(), home.path(), &tmux_env, "a,b,c");
    let widths = pane_widths(&tmux_env, &session);
    kill(&tmux_env, &session);

    // Agent panes are indices 2,3,4 (supervisor=0, dashboard=1).
    let agents: Vec<usize> = widths
        .iter()
        .filter(|(i, _)| *i >= 2)
        .map(|(_, w)| *w)
        .collect();
    assert_eq!(agents.len(), 3, "expected 3 agent panes; got {widths:?}");
    let max = *agents.iter().max().unwrap();
    let min = *agents.iter().min().unwrap();
    assert!(
        max - min <= 2,
        "the 3 agent panes must be equal width within tolerance; got {agents:?}"
    );
    // 50/25/25 would put one pane at ~2x the others. Equal thirds keeps the
    // widest agent pane well under half the window.
    assert!(
        max < min * 2,
        "row must NOT render as 50/25/25; got {agents:?}"
    );
}

#[test]
#[serial]
fn top_row_stays_fifty_fifty_and_five_agent_row_is_one_fifth_each() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session(tr.path(), home.path(), &tmux_env, "a,b,c,d,e");
    let widths = pane_widths(&tmux_env, &session);
    kill(&tmux_env, &session);

    let width_of = |idx: usize| widths.iter().find(|(i, _)| *i == idx).map(|(_, w)| *w);
    let sup = width_of(0).expect("supervisor pane");
    let dash = width_of(1).expect("dashboard pane");
    assert!(
        sup.abs_diff(dash) <= 2,
        "top row must stay ~50/50; supervisor={sup} dashboard={dash}"
    );

    let agents: Vec<usize> = widths
        .iter()
        .filter(|(i, _)| *i >= 2)
        .map(|(_, w)| *w)
        .collect();
    assert_eq!(agents.len(), 5, "expected 5 agent panes; got {widths:?}");
    let max = *agents.iter().max().unwrap();
    let min = *agents.iter().min().unwrap();
    assert!(
        max - min <= 2,
        "the 5 agent panes must each be ~20% (equal within tolerance); got {agents:?}"
    );
}

// --- 6.3 pane-integrity on add -------------------------------------------

#[test]
#[serial]
fn add_preserves_every_agent_pane_mapping() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session(tr.path(), home.path(), &tmux_env, "a,b,c");
    let sandbox = tr.path().parent().unwrap();

    let mut add = cmd();
    tmux_env.apply_assert(&mut add);
    let out = add
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env("GIT_PAW_READINESS_TIMEOUT_MS", FAST_READINESS_MS)
        .args(["add", "d"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run add");

    let paths = pane_paths(&tmux_env, &session);
    let widths = pane_widths(&tmux_env, &session);
    kill(&tmux_env, &session);

    assert!(
        out.status.success(),
        "add failed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // supervisor + dashboard + 4 agents = 6 panes.
    assert_eq!(
        widths.len(),
        6,
        "expected 6 panes after add; got {widths:?}"
    );
    // Every original AND new agent worktree maps to a live pane (no agent left
    // without a pane — the G2 dropped-pane regression).
    for branch in ["a", "b", "c", "d"] {
        let wt = canonical(&sandbox.join(format!("repo-{branch}")));
        assert!(
            paths.contains(&wt),
            "agent '{branch}' worktree must map to a live pane; wt={wt} paths={paths:?}"
        );
    }
}

// --- 6.4 pane-integrity on remove (shell-occupied pane) ------------------

#[test]
#[serial]
fn remove_middle_agent_kills_only_that_pane() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    write_supervisor_config(tr.path());
    let home = TempDir::new().unwrap();
    let tmux_env = tmux_test_env();

    let session = start_session(tr.path(), home.path(), &tmux_env, "a,b,c");
    let sandbox = tr.path().parent().unwrap();

    // The `echo` panes are bare shells (the CLI runs and exits), so removing
    // 'b' exercises the G1 shell-occupied-pane case: the pane is resolved via
    // pane_current_path and killed by id regardless of the running process.
    //
    // --force is deliberate: `git paw start` submits the boot block into that
    // bare shell, which executes its backtick/redirect-bearing markdown and
    // dirties the worktree — an artifact of the `echo` stand-in CLI (a real
    // CLI consumes the prompt as input, never executing it as shell commands).
    // Under load that surfaced as a flaky phantom `**WARNING:` dirty entry.
    // This test verifies pane-kill/re-tile integrity, not the uncommitted-work
    // gate (covered by the dedicated remove-dirty tests), so --force isolates
    // the behavior under test and removes the load-dependent flake.
    let mut rm = cmd();
    tmux_env.apply_assert(&mut rm);
    let out = rm
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env("GIT_PAW_READINESS_TIMEOUT_MS", FAST_READINESS_MS)
        .args(["remove", "b", "--force"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run remove");

    let paths = pane_paths(&tmux_env, &session);
    let widths = pane_widths(&tmux_env, &session);
    kill(&tmux_env, &session);

    assert!(
        out.status.success(),
        "remove failed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // supervisor + dashboard + 2 surviving agents = 4 panes.
    assert_eq!(
        widths.len(),
        4,
        "expected 4 panes after remove; got {widths:?}"
    );
    // 'a' and 'c' still each have a live pane; 'b' is gone (no orphan).
    let wt_a = canonical(&sandbox.join("repo-a"));
    let wt_b = canonical(&sandbox.join("repo-b"));
    let wt_c = canonical(&sandbox.join("repo-c"));
    assert!(
        paths.contains(&wt_a),
        "'a' must still map to a live pane; paths={paths:?}"
    );
    assert!(
        paths.contains(&wt_c),
        "'c' must still map to a live pane; paths={paths:?}"
    );
    assert!(
        !paths.contains(&wt_b),
        "'b' pane must be gone (no orphan); paths={paths:?}"
    );
}
