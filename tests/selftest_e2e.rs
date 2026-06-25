//! E2E tests for `git paw selftest` (capability `selftest`).
//!
//! Each test drives the real `git-paw` binary's `selftest` subcommand from a
//! throwaway git repository and asserts the observable verdict, output, and
//! cleanup. The harness isolates its own tmux socket, broker port, `HOME`, and
//! throwaway repo internally, so these tests need no socket plumbing of their
//! own — they only observe `git paw selftest`'s stdout/stderr, exit code, and
//! filesystem after-effects. They skip when tmux is unavailable.
//!
//! Maps to selftest-harness tasks 4.1–4.5 and the `selftest` spec scenarios.

use std::process::Command as StdCommand;
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;

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

/// Lists sessions on the user's **default** tmux socket, with `TMUX_TMPDIR`,
/// `TMUX`, and `TMUX_PANE` stripped so the probe inspects the real default
/// socket rather than any inherited isolated one (mirrors
/// `helpers::guard_against_live_session`).
fn default_socket_sessions() -> String {
    let out = StdCommand::new("tmux")
        .arg("ls")
        .env_remove("TMUX_TMPDIR")
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .output();
    out.map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

/// Parses the session name from the harness's
/// `selftest: session '<name>' booted on its private tmux socket` line.
fn parse_session_name(stdout: &str) -> Option<String> {
    let line = stdout
        .lines()
        .find(|l| l.contains("booted on its private tmux socket"))?;
    let start = line.find('\'')? + 1;
    let end = line[start..].find('\'')? + start;
    Some(line[start..end].to_string())
}

// --- 4.1 healthy build: exit 0 + pass indication ---

#[test]
#[serial]
fn selftest_passes_and_exits_zero_on_healthy_build() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();

    let out = cmd()
        .current_dir(tr.path())
        .arg("selftest")
        .timeout(Duration::from_secs(90))
        .output()
        .expect("run selftest");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    assert!(
        out.status.success(),
        "selftest should exit 0 on a healthy build; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("selftest passed"),
        "stdout should report a pass; stdout:\n{stdout}"
    );
}

// --- 4.2 session lands on the private socket, never the default socket ---

#[test]
#[serial]
fn selftest_session_lands_on_private_socket_not_default() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();

    let out = cmd()
        .current_dir(tr.path())
        .arg("selftest")
        .timeout(Duration::from_secs(90))
        .output()
        .expect("run selftest");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "selftest should pass; stdout:\n{stdout}"
    );

    // The harness fails the `start` step unless the session is live on its
    // private socket, then prints this line — so its presence is the evidence
    // the session booted on the private socket (not the default one).
    assert!(
        stdout.contains("booted on its private tmux socket"),
        "stdout should confirm private-socket placement; stdout:\n{stdout}"
    );

    // And the session must never have appeared on the user's default socket.
    let session = parse_session_name(&stdout).expect("session name in stdout");
    let default_sessions = default_socket_sessions();
    assert!(
        !default_sessions.contains(&session),
        "selftest session '{session}' must NOT appear on the default tmux socket; \
         default socket sessions:\n{default_sessions}"
    );
}

// --- 4.3 dummy CLI + throwaway repo under .git-paw/tmp/ removed afterward ---

#[test]
#[serial]
fn selftest_throwaway_repo_lives_under_git_paw_tmp_and_is_removed() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    let tmp_dir = tr.path().join(".git-paw").join("tmp");

    let out = cmd()
        .current_dir(tr.path())
        .arg("selftest")
        .timeout(Duration::from_secs(90))
        .output()
        .expect("run selftest");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();

    // A healthy pass requires the throwaway session to have booted with the
    // dummy CLI and no real AI CLI — the test environment has no claude/codex/
    // gemini on PATH, so a passing run proves the lifecycle needs no LLM.
    assert!(
        out.status.success(),
        "selftest should pass; stdout:\n{stdout}"
    );

    // The harness namespaces its throwaway repo under .git-paw/tmp/ and removes
    // that directory after the run completes.
    assert!(
        !tmp_dir.exists(),
        "the throwaway .git-paw/tmp/ dir should be removed after the run; \
         it still exists at {}",
        tmp_dir.display()
    );
}

// --- 4.4 roster grows on add, shrinks on remove ---

#[test]
#[serial]
fn selftest_observes_roster_grow_on_add_and_shrink_on_remove() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();

    let out = cmd()
        .current_dir(tr.path())
        .arg("selftest")
        .timeout(Duration::from_secs(90))
        .output()
        .expect("run selftest");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "selftest should pass; stdout:\n{stdout}"
    );

    // The roster grows on add (both agents present) ...
    assert!(
        stdout.contains("roster after add: selftest-a, selftest-b"),
        "roster should include both agents after add; stdout:\n{stdout}"
    );
    // ... and shrinks on remove (only the original agent remains).
    assert!(
        stdout.contains("roster after remove: selftest-a"),
        "roster should hold only the original agent after remove; stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("roster after remove: selftest-a, selftest-b"),
        "the removed agent must be gone from the roster; stdout:\n{stdout}"
    );
}

// --- 4.5 forced failure: non-zero exit + named step, cleanup still runs ---

#[test]
#[serial]
fn selftest_forced_failure_exits_nonzero_and_names_step() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    let tmp_dir = tr.path().join(".git-paw").join("tmp");

    // Inject a forced failure at a real lifecycle step: start and add run for
    // real, then the post-add roster check is forced to fail.
    let out = cmd()
        .current_dir(tr.path())
        .arg("selftest")
        .env("GIT_PAW_SELFTEST_FORCE_FAIL", "roster-after-add")
        .timeout(Duration::from_secs(90))
        .output()
        .expect("run selftest");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    assert!(
        !out.status.success(),
        "selftest must exit non-zero when a lifecycle step fails; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("roster-after-add"),
        "stderr must name the failing step; stderr:\n{stderr}"
    );
    // Cleanup runs on the failure path too.
    assert!(
        !tmp_dir.exists(),
        "the throwaway .git-paw/tmp/ dir should be removed even on the failure path; \
         it still exists at {}",
        tmp_dir.display()
    );
}

// --- scenario: TMUX / TMUX_PANE stripped from child env ---

/// `git paw selftest` is invoked with bogus `TMUX` / `TMUX_PANE` set (as if run
/// from inside an existing tmux session). The harness must strip them from its
/// child processes; otherwise the spawned tmux would try to reach the bogus
/// parent server and the lifecycle would fail. A passing run is the evidence
/// the variables were stripped.
#[test]
#[serial]
fn selftest_strips_tmux_env_from_children() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();

    let out = cmd()
        .current_dir(tr.path())
        .arg("selftest")
        .env("TMUX", "/nonexistent/tmux-socket,99999,0")
        .env("TMUX_PANE", "%999")
        .timeout(Duration::from_secs(90))
        .output()
        .expect("run selftest");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    assert!(
        out.status.success(),
        "selftest must pass even when launched with bogus TMUX/TMUX_PANE set, \
         proving it strips them from its children; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

// --- Mapping note ---
//
// Task 4.6 (the ephemeral broker-port helper returns a free, immediately
// bindable port and two concurrent calls yield distinct ports) is covered by
// the unit tests in `src/selftest.rs`:
// `pick_broker_port_returns_an_immediately_bindable_port` and
// `two_helper_calls_yield_distinct_ports_under_concurrency`, plus the canonical
// helper exercised by every migrated broker test (selftest-harness §3).
//
// Task 4.7 (run two `cargo test` shards concurrently / under `cargo llvm-cov`
// and confirm no "address already in use" broker-port failure) is a
// whole-suite verification step rather than a single test; it is performed
// during the change's verification.
