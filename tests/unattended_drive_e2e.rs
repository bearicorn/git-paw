//! End-to-end tests for the `git paw start --unattended` in-process drive loop
//! (`unattended-operation`), exercised against a real isolated supervisor
//! session with **no real LLM backend and no interactive terminal**.
//!
//! Each test launches `git paw start --unattended` as a detached child on a
//! test-owned tmux socket, with an isolated `HOME` and an OS-assigned ephemeral
//! broker port. The dummy CLI is a bash script that continuously reprints a
//! permission-prompt footer so the drive loop always sees a live prompt in the
//! capture tail; any approval keystrokes the loop sends land harmlessly on the
//! script's ignored stdin. The test drives the wave to a terminal condition by
//! publishing to the broker over HTTP (the human/completion signal), then
//! observes the child's exit status and printed summary.
//!
// # `unattended-operation` scenario → test map (task 8.4)
//
// Every scenario in `specs/unattended-operation/spec.md` maps to at least one
// behavioral test. `drive::…` names are unit tests in `src/supervisor/drive.rs`
// (the loop exercised in memory with fakes — no tmux, no LLM); `e2e::…` names
// are tests in this file (real tmux session + broker, no LLM, no TTY).
//
// - runs a session to completion + summary → e2e::auto_approves_safe_prompt_then_completes,
//   drive::loop_approves_safe_prompt_then_exits_on_completion
// - polls on a ~15-second cadence → drive::cadence_constants_match_spec
// - without --unattended the launch returns immediately → tests/e2e_supervisor_returns.rs
//   (supervisor-launch), main::dispatch_unattended_routes_to_supervisor_launch_path
// - drive loop is the sole auto-approver / dashboard thread disabled →
//   main::dashboard_auto_approve_disabled_under_unattended
// - classifier-safe prompt is auto-approved → e2e::auto_approves_safe_prompt_then_completes,
//   drive::loop_approves_safe_prompt_then_exits_on_completion
// - two-option Yes/No approved with "1" then Enter → drive::loop_approves_safe_prompt_then_exits_on_completion
// - approval is logged before keystrokes → drive::loop_approves_safe_prompt_then_exits_on_completion,
//   e2e::auto_approves_safe_prompt_then_completes (broker `auto_approved` observed)
// - safe prompt on the supervisor pane is approved → e2e::approves_supervisor_pane_zero_safe_prompt,
//   drive::loop_approves_supervisor_pane_safe_prompt
// - pane 0 with no live prompt is left untouched → drive::loop_leaves_supervisor_pane_untouched_without_prompt
// - pane-0 approval sends only minimal keystrokes → drive::loop_approves_supervisor_pane_safe_prompt
// - no keystrokes when pane 0 not at a prompt → drive::loop_leaves_supervisor_pane_untouched_without_prompt
// - prompt footer in the capture tail triggers action → drive::loop_approves_safe_prompt_then_exits_on_completion
// - prompt-like text in scrollback is ignored → drive::scrollback_prompt_is_not_acted_on
// - each pane captured explicitly, never a shell for-loop → drive::sweep_captures_each_pane_exactly_once
// - pane→agent resolution via pane_current_path → drive::resolves_coding_agent_by_path_not_index,
//   drive::resolves_pane_zero_and_one_to_supervisor_and_dashboard
// - a captured prompt is attributed to the correct agent → e2e::auto_approves_safe_prompt_then_completes
//   (agent id `feat-alpha` observed), drive::resolves_coding_agent_by_path_not_index
// - send-keys nudges send a follow-up Enter → drive::nudge_sends_text_then_a_separate_enter
// - risky prompt escalated without blocking the wave → e2e::escalates_danger_without_blocking_other_agent,
//   drive::loop_escalates_danger_without_blocking_other_agent
// - unknown classification is escalated, not approved → drive::classifies_unknown_when_no_rule_matches
//   (+ the shared escalation path in drive::loop_escalates_danger_without_blocking_other_agent)
// - repeated identical alert is deduped in the window → drive::dedup_emits_once_per_window_for_repeated_prompt
// - two distinct prompts sharing boilerplate not collapsed → drive::dedup_shape_distinguishes_commands_sharing_boilerplate
// - pane with no broker presence is still watched → drive::sweeps_pane_with_no_broker_record
// - N feedback→fix→re-verify cycles are tolerated → drive::loop_exits_on_heartbeat_when_never_completing
// - PASS/FAIL verdict ends the loop → drive::completion_on_supervisor_verdict,
//   e2e::auto_approves_safe_prompt_then_completes
// - all-tasks-checked ends the loop → drive::completion_when_all_agents_checked,
//   e2e::approves_supervisor_pane_zero_safe_prompt (completes via a verified agent)
// - heartbeat fires after a prolonged run → drive::loop_exits_on_heartbeat_when_never_completing,
//   e2e::heartbeat_exits_with_summary
// - exit summary reports outcome + escalations → drive::summary_reports_outcome_states_and_escalations,
//   e2e::escalates_danger_without_blocking_other_agent (summary read from stdout)
// - friction absorbed during the run recorded as a learning → drive::loop_escalates_danger_without_blocking_other_agent
// - wind-down synthesis records durable learnings → drive::loop_escalates_danger_without_blocking_other_agent
// - drive loop exercised end-to-end without a real LLM → every e2e:: test here

use std::fs;
use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command as StdCommand, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use serial_test::serial;
use tempfile::TempDir;

use git_paw::broker::messages::{ArtifactPayload, BrokerMessage};
use git_paw::broker::publish::publish_to_broker_http;
use git_paw::supervisor::poll::fetch_status_over_http;

mod helpers;
use helpers::*;

/// The `git-paw` binary under test (cargo exports its path to integration
/// tests).
const BIN: &str = env!("CARGO_BIN_EXE_git-paw");

fn tmux_available() -> bool {
    StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Allocates an OS-assigned ephemeral loopback port and releases it for the
/// broker to claim (mirrors the selftest harness / `pick_broker_port`).
fn pick_broker_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    listener.local_addr().expect("local addr").port()
    // listener drops here, releasing the port
}

/// Writes an executable dummy-CLI script that continuously reprints a
/// permission prompt whose command line is `shown_cmd`. The footer carries both
/// the live-prompt marker (`esc to cancel`) and the approval marker (`Do you
/// want to proceed`) so the drive loop treats it as a live, actionable prompt.
fn write_prompt_dummy(dir: &Path, name: &str, shown_cmd: &str) -> PathBuf {
    let path = dir.join(name);
    let script = format!(
        "#!/usr/bin/env bash\n\
         # Test dummy CLI: reprint a live permission prompt forever; ignore stdin.\n\
         while true; do\n\
         \x20 printf 'Bash command\\n  {shown_cmd}\\nDo you want to proceed?\\n> 1. Yes\\n  2. No\\n(esc to cancel)\\n'\n\
         \x20 sleep 0.4\n\
         done\n"
    );
    fs::write(&path, script).expect("write dummy script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).expect("stat dummy").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("chmod dummy");
    }
    path
}

/// Writes an executable dummy CLI that reprints a *branch-aware* prompt: agent
/// panes on branch `danger_branch` show `danger_cmd`, every other agent shows
/// `safe_cmd`. The pane runs in the agent's worktree, so the current branch
/// selects the prompt — letting one shared CLI drive a danger prompt on one
/// agent and a safe prompt on another in the same session.
fn write_branch_aware_dummy(
    dir: &Path,
    name: &str,
    danger_branch: &str,
    danger_cmd: &str,
    safe_cmd: &str,
) -> PathBuf {
    let path = dir.join(name);
    let script = format!(
        "#!/usr/bin/env bash\n\
         # Test dummy CLI: reprint a branch-aware live permission prompt.\n\
         branch=\"$(git rev-parse --abbrev-ref HEAD 2>/dev/null)\"\n\
         if [ \"$branch\" = \"{danger_branch}\" ]; then cmd='{danger_cmd}'; else cmd='{safe_cmd}'; fi\n\
         while true; do\n\
         \x20 printf 'Bash command\\n  %s\\nDo you want to proceed?\\n> 1. Yes\\n  2. No\\n(esc to cancel)\\n' \"$cmd\"\n\
         \x20 sleep 0.4\n\
         done\n"
    );
    fs::write(&path, script).expect("write branch-aware dummy");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).expect("stat dummy").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("chmod dummy");
    }
    path
}

/// A dummy CLI that just holds its pane open with no prompt.
fn plain_dummy() -> &'static str {
    "cat"
}

/// Writes a `.git-paw/config.toml` wiring the broker port and the supervisor +
/// default CLIs. `supervisor_cli` / `agent_cli` are command strings (absolute
/// script paths or `cat`).
fn write_config(repo: &Path, port: u16, supervisor_cli: &str, agent_cli: &str) {
    let paw = repo.join(".git-paw");
    fs::create_dir_all(&paw).expect("create .git-paw");
    let config = format!(
        "default_cli = \"{agent_cli}\"\n\n\
         [broker]\n\
         enabled = true\n\
         port = {port}\n\n\
         [supervisor]\n\
         enabled = true\n\
         cli = \"{supervisor_cli}\"\n"
    );
    fs::write(paw.join("config.toml"), config).expect("write config");
}

/// Spawns `git paw start --unattended` as a detached child with the isolation
/// env applied, its stdout/stderr redirected to files under `out_dir`.
#[allow(clippy::too_many_arguments)]
fn spawn_unattended(
    repo: &Path,
    home: &Path,
    socket_dir: &Path,
    out_dir: &Path,
    branches: &str,
    poll_ms: &str,
    heartbeat_ms: &str,
) -> (Child, PathBuf) {
    let stdout_path = out_dir.join("stdout.log");
    let stdout = fs::File::create(&stdout_path).expect("create stdout log");
    let stderr = fs::File::create(out_dir.join("stderr.log")).expect("create stderr log");

    let child = StdCommand::new(BIN)
        .current_dir(repo)
        .args(["start", "--unattended", "--branches", branches])
        .env("TMUX_TMPDIR", socket_dir)
        .env("HOME", home)
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("GIT_PAW_READINESS_TIMEOUT_MS", "120")
        .env("GIT_PAW_DRIVE_POLL_MS", poll_ms)
        .env("GIT_PAW_DRIVE_HEARTBEAT_MS", heartbeat_ms)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .expect("spawn git paw start --unattended");
    (child, stdout_path)
}

/// Polls until the broker at `url` accepts a `/status` request, or the budget
/// elapses.
fn wait_for_broker(url: &str, budget: Duration) -> bool {
    let deadline = Instant::now() + budget;
    loop {
        if fetch_status_over_http(url).is_ok() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Polls the broker `/status` until `agent` reports `want_status`, or the budget
/// elapses. Returns whether it was observed.
fn wait_for_status(url: &str, agent: &str, want_status: &str, budget: Duration) -> bool {
    let deadline = Instant::now() + budget;
    loop {
        if let Ok(rows) = fetch_status_over_http(url)
            && rows
                .iter()
                .any(|r| r.agent_id == agent && r.status == want_status)
        {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

/// Publishes a terminal `agent.artifact { status }` for `agent` — the broker
/// mints/mutates the agent's record so `/status` reports the terminal status
/// the drive loop treats as a completion signal.
fn publish_terminal(url: &str, agent: &str, status: &str) {
    let msg = BrokerMessage::Artifact {
        agent_id: agent.to_string(),
        payload: ArtifactPayload {
            status: status.to_string(),
            exports: Vec::new(),
            modified_files: Vec::new(),
        },
    };
    let _ = publish_to_broker_http(url, &msg);
}

/// Waits up to `budget` for `child` to exit, returning its status (or `None` on
/// timeout, after killing it).
fn wait_child(child: &mut Child, budget: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + budget;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {}
            Err(_) => return None,
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn read_to_string(path: &Path) -> String {
    let mut s = String::new();
    if let Ok(mut f) = fs::File::open(path) {
        let _ = f.read_to_string(&mut s);
    }
    s
}

/// Kills any tmux session named in the child's "launched" stdout line, on the
/// test-owned socket.
fn kill_session(socket_dir: &Path, stdout: &str) {
    if let Some(name) = stdout
        .lines()
        .find(|l| l.contains("Supervisor session 'paw-"))
        .and_then(|l| l.split('\'').nth(1))
    {
        let _ = StdCommand::new("tmux")
            .env("TMUX_TMPDIR", socket_dir)
            .args(["kill-session", "-t", name])
            .status();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Heartbeat exit — the simplest proof the `--unattended` launch runs the loop
/// in-process (not the return-with-attach-hint path) and exits with a summary,
/// with no completion signal and no real LLM.
#[test]
#[serial]
fn unattended_heartbeat_exits_with_summary() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let home = TempDir::new().expect("home dir");
    let out = TempDir::new().expect("out dir");
    let port = pick_broker_port();
    write_config(tr.path(), port, plain_dummy(), plain_dummy());

    let (mut child, stdout_path) = spawn_unattended(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        out.path(),
        "feat/alpha",
        "300",
        "2500",
    );

    let status = wait_child(&mut child, Duration::from_secs(45));
    let stdout = read_to_string(&stdout_path);
    kill_session(tmux_env.socket_dir(), &stdout);

    let status = status.unwrap_or_else(|| panic!("child did not exit; stdout:\n{stdout}"));
    assert!(
        status.success(),
        "unattended run should exit 0; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Unattended drive loop exited: heartbeat"),
        "summary should report the heartbeat outcome; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Per-agent final state"),
        "summary should include the per-agent state section; stdout:\n{stdout}"
    );
}

/// 8.1 — `--unattended` auto-approves a classifier-safe prompt on a coding-agent
/// pane, detects the completion signal, and exits with a `completed` summary,
/// with no real LLM.
#[test]
#[serial]
fn unattended_auto_approves_safe_prompt_then_completes() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let home = TempDir::new().expect("home dir");
    let out = TempDir::new().expect("out dir");
    let port = pick_broker_port();
    let url = format!("http://127.0.0.1:{port}");

    // Agent "a" shows a safe (`ls`) prompt; the supervisor pane just holds.
    let safe = write_prompt_dummy(tr.path(), "dummy_safe.sh", "ls -la");
    write_config(tr.path(), port, plain_dummy(), &safe.to_string_lossy());

    let (mut child, stdout_path) = spawn_unattended(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        out.path(),
        "feat/alpha",
        "400",
        "120000", // long heartbeat: completion, not heartbeat, ends the run
    );

    assert!(
        wait_for_broker(&url, Duration::from_secs(20)),
        "broker should come up on the ephemeral port"
    );
    // The loop must actually auto-approve the safe prompt: it publishes
    // `auto_approved` for the agent before the keystrokes.
    let approved = wait_for_status(&url, "feat-alpha", "auto_approved", Duration::from_secs(25));
    // Signal completion via a supervisor terminal verdict, then let the loop exit.
    publish_terminal(&url, "supervisor", "done");

    let status = wait_child(&mut child, Duration::from_secs(30));
    let stdout = read_to_string(&stdout_path);
    kill_session(tmux_env.socket_dir(), &stdout);

    assert!(
        approved,
        "the safe prompt should have been auto-approved; stdout:\n{stdout}"
    );
    let status = status.unwrap_or_else(|| panic!("child did not exit; stdout:\n{stdout}"));
    assert!(
        status.success(),
        "unattended run should exit 0; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Unattended drive loop exited: completed"),
        "summary should report completion; stdout:\n{stdout}"
    );
}

/// 8.2 — a `danger` prompt on one agent is escalated for review WITHOUT blocking
/// the wave: the other agent's safe prompt is still approved, and the run
/// completes with the escalation listed in the summary.
#[test]
#[serial]
fn unattended_escalates_danger_without_blocking_other_agent() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let home = TempDir::new().expect("home dir");
    let out = TempDir::new().expect("out dir");
    let port = pick_broker_port();
    let url = format!("http://127.0.0.1:{port}");

    // One shared branch-aware CLI: agent "a" shows a danger prompt (git push
    // --force), agent "b" shows a safe prompt (ls). The launcher assigns the
    // same default CLI to every agent pane, and the pane's current branch
    // selects which prompt it reprints.
    let dummy = write_branch_aware_dummy(
        tr.path(),
        "dummy_branch.sh",
        "feat/alpha",
        "git push --force origin main",
        "ls -la",
    );
    write_config(tr.path(), port, plain_dummy(), &dummy.to_string_lossy());

    let (mut child, stdout_path) = spawn_unattended(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        out.path(),
        "feat/alpha,feat/beta",
        "400",
        "120000",
    );

    assert!(
        wait_for_broker(&url, Duration::from_secs(20)),
        "broker should come up on the ephemeral port"
    );
    // Non-blocking: while agent "a" sits on an un-approvable danger prompt,
    // agent "b"'s safe prompt is still approved and the wave keeps moving.
    let b_progressed = wait_for_status(&url, "feat-beta", "auto_approved", Duration::from_secs(25));
    publish_terminal(&url, "supervisor", "done");

    let status = wait_child(&mut child, Duration::from_secs(30));
    let stdout = read_to_string(&stdout_path);
    kill_session(tmux_env.socket_dir(), &stdout);

    assert!(
        b_progressed,
        "the other agent's safe prompt should still be approved (non-blocking); stdout:\n{stdout}"
    );
    let status = status.unwrap_or_else(|| panic!("child did not exit; stdout:\n{stdout}"));
    assert!(
        status.success(),
        "unattended run should exit 0; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Unattended drive loop exited: escalated-for-review"),
        "summary should report the escalated outcome; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("[danger]") && stdout.contains("git push"),
        "the danger prompt should be listed as an escalation; stdout:\n{stdout}"
    );
}

/// 8.3 — the drive loop covers the supervisor's OWN pane 0: a classifier-safe
/// prompt on the supervisor pane is auto-approved (verified via the broker
/// audit status for `supervisor`), with the wave completing via the coding
/// agent.
#[test]
#[serial]
fn unattended_approves_supervisor_pane_zero_safe_prompt() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let home = TempDir::new().expect("home dir");
    let out = TempDir::new().expect("out dir");
    let port = pick_broker_port();
    let url = format!("http://127.0.0.1:{port}");

    // The SUPERVISOR pane shows a safe (`cargo test`) prompt; the coding agent
    // just holds.
    let safe = write_prompt_dummy(tr.path(), "dummy_sup.sh", "cargo test");
    write_config(tr.path(), port, &safe.to_string_lossy(), plain_dummy());

    let (mut child, stdout_path) = spawn_unattended(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        out.path(),
        "feat/alpha",
        "400",
        "120000",
    );

    assert!(
        wait_for_broker(&url, Duration::from_secs(20)),
        "broker should come up on the ephemeral port"
    );
    // Pane 0 (supervisor) is covered by the sweep: its safe prompt is approved,
    // logged as `auto_approved` for the `supervisor` agent id.
    let approved = wait_for_status(&url, "supervisor", "auto_approved", Duration::from_secs(25));
    // Complete via the coding agent so the run exits.
    publish_terminal(&url, "feat-alpha", "verified");

    let status = wait_child(&mut child, Duration::from_secs(30));
    let stdout = read_to_string(&stdout_path);
    kill_session(tmux_env.socket_dir(), &stdout);

    assert!(
        approved,
        "the supervisor pane-0 safe prompt should have been auto-approved; stdout:\n{stdout}"
    );
    let status = status.unwrap_or_else(|| panic!("child did not exit; stdout:\n{stdout}"));
    assert!(
        status.success(),
        "unattended run should exit 0; stdout:\n{stdout}"
    );
}
