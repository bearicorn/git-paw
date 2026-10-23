//! End-to-end tests for the `git paw start --unattended` in-process drive loop
//! (`unattended-operation`), exercised against a real isolated supervisor
//! session with **no real LLM backend and no interactive terminal**.
//!
//! Each test launches `git paw start --unattended` as a detached child on a
//! test-owned tmux socket, with an isolated `HOME` and an OS-assigned ephemeral
//! broker port. The dummy CLI is a bash script that continuously reprints a
//! permission-prompt footer so the drive loop always sees a live prompt in the
//! capture tail. The loop must answer none of them: where a test needs to
//! prove that negative, the dummy records every byte reaching its stdin to a
//! marker file. The test drives the wave to a terminal condition by
//! publishing to the broker over HTTP (the human/completion signal), then
//! observes the child's exit status and printed summary.
//!
// # `supervisor-unattended-operation` scenario → test map
//
// Every scenario in the capability's spec maps to at least one behavioral
// test. `drive::…` names are unit tests in `src/supervisor/drive.rs` (the loop
// exercised in memory with fakes — no tmux, no LLM); `e2e::…` names are tests
// in this file (real tmux session + broker, no LLM, no TTY).
//
// - runs a session to completion + summary → e2e::escalates_live_prompt_then_completes,
//   drive::loop_escalates_live_prompt_then_exits_on_completion
// - polls on a ~15-second cadence → drive::cadence_constants_match_spec
// - without --unattended the launch returns immediately → tests/e2e_supervisor_returns.rs
//   (supervisor-launch), main::dispatch_unattended_routes_to_supervisor_launch_path
// - a live prompt is escalated without blocking the other agent →
//   e2e::escalates_live_prompt_without_blocking_other_agent,
//   drive::loop_escalates_live_prompts_without_blocking_either_agent
// - the escalation reaches the broker, the summary and the decision log →
//   e2e::escalates_live_prompt_without_blocking_other_agent (broker question +
//   summary entry + one manual-approvals.jsonl line)
// - a drive-loop escalation appends a JSONL line →
//   drive::a_drive_loop_escalation_appends_a_jsonl_line,
//   e2e::escalates_live_prompt_without_blocking_other_agent
// - a deduplicated escalation is logged once → drive::a_deduplicated_escalation_records_once
// - non-prompt escalations are not logged →
//   drive::a_cli_unavailable_escalation_records_no_manual_decision,
//   drive::a_nudge_wedged_escalation_records_no_manual_decision,
//   drive::a_correction_loop_signal_records_no_manual_decision
// - the opt-out applies to the drive loop → drive::the_opt_out_applies_to_the_drive_loop,
//   drive::the_opt_out_withholds_the_first_seen_learning
// - a command formerly classified safe is escalated, not approved →
//   drive::formerly_safe_command_is_escalated_not_approved,
//   e2e::escalates_live_prompt_then_completes
// - command text cannot steer the outcome → drive::command_text_cannot_steer_the_outcome
// - a file-edit prompt is escalated by its target path →
//   drive::file_edit_prompt_is_escalated_by_its_target_path
// - a live prompt on the supervisor pane is escalated, not answered →
//   e2e::escalates_supervisor_pane_zero_prompt_without_typing,
//   drive::supervisor_pane_prompt_is_escalated_not_answered,
//   drive::loop_escalates_supervisor_pane_prompt_without_typing
// - pane 0 with no live prompt is left untouched → drive::loop_leaves_supervisor_pane_untouched_without_prompt
// - prompt footer in the capture tail triggers action → drive::loop_escalates_live_prompt_then_exits_on_completion
// - a trailing task-list region does not hide a live modal →
//   e2e::escalates_live_prompt_with_trailing_task_region
// - prompt-like text in scrollback is ignored → drive::scrollback_prompt_is_not_acted_on
// - each pane captured explicitly, never a shell for-loop → drive::sweep_captures_each_pane_exactly_once
// - pane→agent resolution via pane_current_path → drive::resolves_coding_agent_by_path_not_index,
//   drive::resolves_pane_zero_and_one_to_supervisor_and_dashboard
// - send-keys nudges send a follow-up Enter → drive::nudge_sends_text_then_a_separate_enter
// - repeated identical alert is deduped in the window → drive::dedup_emits_once_per_window_for_repeated_prompt
// - two distinct prompts sharing boilerplate not collapsed → drive::dedup_shape_distinguishes_commands_sharing_boilerplate
// - pane with no broker presence is still watched → drive::sweeps_pane_with_no_broker_record
// - N feedback→fix→re-verify cycles are tolerated → drive::loop_exits_on_heartbeat_when_never_completing
// - PASS/FAIL verdict ends the loop → drive::completion_on_supervisor_verdict
// - all-tasks-checked ends the loop → drive::completion_when_all_agents_checked
// - heartbeat fires after a prolonged run without progress →
//   drive::loop_exits_on_heartbeat_when_never_completing, e2e::heartbeat_exits_with_summary
// - a progressing wave is not ended by the heartbeat → drive::branch_tip_movement_defers_the_heartbeat,
//   drive::a_phase_advance_defers_the_heartbeat
// - only a tip move or a phase advance resets the interval → drive::progress_kinds_reset_the_heartbeat,
//   drive::a_tip_moved_by_the_loops_own_branch_refresh_is_not_progress
// - re-escalating an unanswered prompt does not defer the heartbeat →
//   drive::re_escalations_do_not_defer_the_heartbeat
// - a detached run reaches completion → e2e::detached_unattended_run_completes_past_the_heartbeat,
//   drive::a_heartbeat_takes_a_fresh_look_before_exiting
// - exit summary reports outcome + escalations → drive::summary_reports_outcome_states_and_escalations,
//   e2e::escalates_live_prompt_without_blocking_other_agent (summary read from stdout)
// - an escalated prompt is recorded as a learning → drive::formerly_safe_command_is_escalated_not_approved
// - wind-down synthesis records durable learnings → drive::loop_escalates_live_prompts_without_blocking_either_agent
// - the claim guards the loop's own sends → drive::a_pane_claimed_by_another_sender_receives_no_typed_text
// - drive loop is exercised end-to-end without a real LLM → every e2e:: test here

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
use git_paw::config::WorktreePlacement;
use git_paw::git::worktree_path_for;
use git_paw::supervisor::drive::fetch_status_over_http;

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

/// Writes an executable dummy-CLI script that continuously reprints a live
/// permission prompt FOLLOWED by a persistent six-line task-list region below
/// the footer — mirroring current Claude Code's chrome under its input area,
/// the exact shape `live-prompt-tail-window` fixes detection for. Used by
/// `unattended_escalates_live_prompt_with_trailing_task_region`.
fn write_prompt_dummy_with_trailing_region(dir: &Path, name: &str, shown_cmd: &str) -> PathBuf {
    let path = dir.join(name);
    let script = format!(
        "#!/usr/bin/env bash\n\
         # Test dummy CLI: reprint a live permission prompt with a trailing\n\
         # task-list region below the footer; ignore stdin.\n\
         while true; do\n\
         \x20 printf 'Bash command\\n  {shown_cmd}\\nDo you want to proceed?\\n> 1. Yes\\n  2. No\\n(esc to cancel)\\nTasks\\n[ ] one\\n[ ] two\\n[ ] three\\nmain*\\nSonnet\\n'\n\
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

/// A dummy CLI that just holds its pane open with no prompt.
fn plain_dummy() -> &'static str {
    "cat"
}

/// Writes an executable dummy CLI that commits to its own branch every 0.6s —
/// a cadence shorter than the test's heartbeat — until `stop_file` appears,
/// then touches `stopped_marker` and holds the pane with `cat`. Each commit
/// moves the branch tip, which is the progress `unattended-heartbeat-progress`
/// measures the heartbeat from.
fn write_committing_dummy(
    dir: &Path,
    name: &str,
    stop_file: &Path,
    stopped_marker: &Path,
) -> PathBuf {
    let path = dir.join(name);
    let stop = stop_file.display();
    let stopped = stopped_marker.display();
    let script = format!(
        "#!/usr/bin/env bash\n\
         # Test dummy CLI: commit on a short cadence until told to stop.\n\
         # Drain whatever is typed into the pane (the boot prompt) with echo\n\
         # off, so the pane never shows buffered input the loop would try to\n\
         # recover; this dummy models an agent busy working, not one waiting.\n\
         stty -echo 2>/dev/null\n\
         cat >/dev/null &\n\
         n=0\n\
         while [ ! -e '{stop}' ]; do\n\
         \x20 n=$((n + 1))\n\
         \x20 echo \"$n\" >> progress.txt\n\
         \x20 git add progress.txt >/dev/null 2>&1\n\
         \x20 git commit -q -m \"progress $n\" >/dev/null 2>&1\n\
         \x20 sleep 0.6\n\
         done\n\
         : > '{stopped}'\n\
         exec tail -f /dev/null\n"
    );
    fs::write(&path, script).expect("write committing dummy");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).expect("stat dummy").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("chmod dummy");
    }
    path
}

/// Writes an executable dummy CLI that, on branch `prompt_branch`, reprints a
/// live permission prompt for `shown_cmd` AND appends every byte that arrives
/// on its stdin to `stdin_marker`; on any other branch it just holds the pane.
///
/// The marker file is what proves the negative the retirement turns on: the
/// drive loop must send NO keystroke to a prompting pane, so after a run that
/// swept this pane many times the file must still be empty. A dummy that
/// merely ignored stdin could not tell "nothing was sent" from "something was
/// sent and discarded".
fn write_prompt_recorder_dummy(
    dir: &Path,
    name: &str,
    prompt_branch: &str,
    shown_cmd: &str,
    stdin_marker: &Path,
) -> PathBuf {
    let path = dir.join(name);
    let marker = stdin_marker.display();
    let script = format!(
        "#!/usr/bin/env bash\n\
         # Test dummy CLI: reprint a live permission prompt and RECORD stdin.\n\
         branch=\"$(git rev-parse --abbrev-ref HEAD 2>/dev/null)\"\n\
         if [ \"$branch\" != \"{prompt_branch}\" ]; then exec cat; fi\n\
         : > '{marker}'\n\
         ( while IFS= read -r -n1 ch; do printf '%s' \"$ch\" >> '{marker}'; done ) &\n\
         while true; do\n\
         \x20 printf 'Bash command\\n  {shown_cmd}\\nDo you want to proceed?\\n> 1. Yes\\n  2. No\\n(esc to cancel)\\n'\n\
         \x20 sleep 0.4\n\
         done\n"
    );
    fs::write(&path, script).expect("write prompt-recorder dummy");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).expect("stat dummy").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("chmod dummy");
    }
    path
}

/// Reads the session's broker log out of the test's isolated `HOME`, which is
/// where `session_state_dir()` resolves to. Returns the concatenation of every
/// `*.broker.log` found, or an empty string when none exists yet.
fn read_broker_log(home: &Path) -> String {
    fn walk(dir: &Path, out: &mut String) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".broker.log"))
                && let Ok(text) = fs::read_to_string(&path)
            {
                out.push_str(&text);
            }
        }
    }
    let mut out = String::new();
    walk(home, &mut out);
    out
}

/// Reads the session's manual-decision JSONL log out of the repo under test.
///
/// Globs `.git-paw/sessions/*.manual-approvals.jsonl` rather than rebuilding
/// the session name, so the test does not re-derive a path the production
/// code already owns. Returns the concatenation of every match, or an empty
/// string when none exists.
fn read_manual_decision_log(repo: &Path) -> String {
    let dir = repo.join(".git-paw").join("sessions");
    let Ok(entries) = fs::read_dir(&dir) else {
        return String::new();
    };
    let mut out = String::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".manual-approvals.jsonl"))
            && let Ok(text) = fs::read_to_string(&path)
        {
            out.push_str(&text);
        }
    }
    out
}

/// Polls until the broker log carries a line containing every needle, or the
/// budget elapses.
fn wait_for_broker_log(home: &Path, needles: &[&str], budget: Duration) -> bool {
    let deadline = Instant::now() + budget;
    loop {
        let log = read_broker_log(home);
        if log.lines().any(|l| needles.iter().all(|n| l.contains(n))) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

/// Writes an executable dummy CLI that continuously reprints `marker_line` —
/// plain output, no prompt, no options — simulating a CLI that has stopped
/// serving anything but a rate-limit / login-loss status line
/// (`drive-loop-exit-summary-truth`).
fn write_marker_dummy(dir: &Path, name: &str, marker_line: &str) -> PathBuf {
    let path = dir.join(name);
    // Double-quoted, not single-quoted: `marker_line` (e.g. "You've hit your
    // session limit") carries an apostrophe that would otherwise desync the
    // shell's quoting.
    let script = format!(
        "#!/usr/bin/env bash\n\
         # Test dummy CLI: reprint a plain status line forever; ignore stdin.\n\
         while true; do\n\
         \x20 printf '%s\\n' \"{marker_line}\"\n\
         \x20 sleep 0.4\n\
         done\n"
    );
    fs::write(&path, script).expect("write marker dummy");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).expect("stat dummy").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("chmod dummy");
    }
    path
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

/// [`spawn_unattended`], fully detached: stdin is null as well, so the run has
/// no controlling terminal on any stream — the shape of a `nohup … &` launch.
#[allow(clippy::too_many_arguments)]
fn spawn_unattended_detached(
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
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .expect("spawn detached git paw start --unattended");
    (child, stdout_path)
}

/// Counts the commits on `branch` in `repo`, or 0 when it cannot be read.
fn commit_count(repo: &Path, branch: &str) -> usize {
    StdCommand::new("git")
        .current_dir(repo)
        .args(["rev-list", "--count", branch])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
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

/// Launches a supervised session with a short heartbeat and lets its first
/// drive loop exit, leaving the session live, idle and unbound — the starting
/// point for the Paused and Stopped re-attach tests.
///
/// Returns the fixtures the caller has to keep alive (the repo, the tmux
/// socket and the isolated `HOME`) plus the session name, or `None` when the
/// launch did not produce a session to re-attach to.
fn launched_then_idle_session() -> (TestRepo, TmuxTestEnv, TempDir, String) {
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let home = TempDir::new().expect("home dir");
    let out = TempDir::new().expect("out dir");
    let port = pick_broker_port();
    write_config(tr.path(), port, plain_dummy(), plain_dummy());

    let (mut child, stdout_path) = spawn_unattended_detached(
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
    assert!(
        status.is_some_and(|s| s.success()),
        "the launch should exit 0; stdout:\n{stdout}"
    );
    let session = session_name_in(&stdout)
        .unwrap_or_else(|| panic!("the launch should name its session; stdout:\n{stdout}"));
    (tr, tmux_env, home, session)
}

/// The global session receipts under an isolated `HOME`, concatenated — the
/// durable record a refusal must leave byte-identical.
/// Walks `HOME` rather than rebuilding the data-dir path, because that path is
/// platform-specific (`Library/Application Support` on macOS,
/// `.local/share` elsewhere) and git-paw's own `dirs` module owns it — a
/// hand-written guess silently reads nothing and makes the comparison vacuous.
fn session_records(home: &Path) -> String {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "json")
                && path.parent().is_some_and(|p| p.ends_with("sessions"))
                && let Ok(text) = fs::read_to_string(&path)
            {
                out.push((path, text));
            }
        }
    }
    let mut found = Vec::new();
    walk(home, &mut found);
    found.sort();
    found.into_iter().map(|(_, text)| text).collect()
}

/// Polls until `path` exists, or the budget elapses.
fn wait_for_path(path: &Path, budget: Duration) -> bool {
    let deadline = Instant::now() + budget;
    loop {
        if path.exists() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Commits an empty commit in `worktree` — content-free, since only ancestry
/// (not the diff) matters to the ref-derived merge check
/// (`drive-loop-exit-summary-truth`).
fn commit_ahead(worktree: &Path, message: &str) {
    let status = StdCommand::new("git")
        .current_dir(worktree)
        .args(["commit", "--allow-empty", "-m", message])
        .status()
        .expect("git commit --allow-empty");
    assert!(status.success(), "commit_ahead must succeed");
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

/// The session name from a launch's "Supervisor session 'paw-…' launched"
/// stdout line.
fn session_name_in(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .find(|l| l.contains("Supervisor session 'paw-"))
        .and_then(|l| l.split('\'').nth(1))
        .map(ToString::to_string)
}

/// The session name from a non-attaching launch's "Session '…' started in
/// detached mode." stdout line — what a bare launch prints when stdin is not a
/// terminal.
fn detached_session_name_in(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .find(|l| l.contains("started in detached mode"))
        .and_then(|l| l.split('\'').nth(1))
        .map(ToString::to_string)
}

/// Kills any tmux session named in the child's "launched" stdout line, on the
/// test-owned socket.
fn kill_session(socket_dir: &Path, stdout: &str) {
    if let Some(name) = session_name_in(stdout) {
        kill_session_named(socket_dir, &name);
    }
}

/// Kills `name` on the test-owned socket. A missing session is a no-op.
fn kill_session_named(socket_dir: &Path, name: &str) {
    let _ = StdCommand::new("tmux")
        .env("TMUX_TMPDIR", socket_dir)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .args(["kill-session", "-t", name])
        .status();
}

/// How many broker-log lines contain every needle.
fn broker_log_hits(home: &Path, needles: &[&str]) -> usize {
    read_broker_log(home)
        .lines()
        .filter(|l| needles.iter().all(|n| l.contains(n)))
        .count()
}

/// Polls `cond` until it holds or the budget elapses.
fn wait_until(mut cond: impl FnMut() -> bool, budget: Duration) -> bool {
    let deadline = Instant::now() + budget;
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

/// Every session name on the test-owned socket — diagnostic only.
fn sessions_on(socket_dir: &Path) -> Vec<String> {
    StdCommand::new("tmux")
        .env("TMUX_TMPDIR", socket_dir)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().map(ToString::to_string).collect())
        .unwrap_or_default()
}

/// Whether a tmux session named `name` exists on the test-owned socket.
fn session_exists(socket_dir: &Path, name: &str) -> bool {
    StdCommand::new("tmux")
        .env("TMUX_TMPDIR", socket_dir)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .args(["has-session", "-t", name])
        .status()
        .is_ok_and(|s| s.success())
}

/// `<pane_index> <pane_title>` for every pane of `session`, in tmux order —
/// the shape a re-attach must leave untouched (D6: binding rebuilds, re-titles
/// and injects nothing).
fn pane_roster(socket_dir: &Path, session: &str) -> Vec<String> {
    StdCommand::new("tmux")
        .env("TMUX_TMPDIR", socket_dir)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .args([
            "list-panes",
            // `-s`: every pane in the session, so the roster does not depend
            // on which window tmux made current.
            "-s",
            "-t",
            session,
            "-F",
            "#{pane_index} #{pane_title}",
        ])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().map(ToString::to_string).collect())
        .unwrap_or_default()
}

/// Every worktree path registered in `repo`, sorted — a re-attach must add
/// none.
fn worktree_paths(repo: &Path) -> Vec<String> {
    let mut paths: Vec<String> = StdCommand::new("git")
        .current_dir(repo)
        .args(["worktree", "list", "--porcelain"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| {
            s.lines()
                .filter_map(|l| l.strip_prefix("worktree ").map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    paths
}

/// Runs `git paw <args>` to completion with the isolation env applied and no
/// terminal on any stream, returning `(success, stdout, stderr)`.
///
/// The synchronous counterpart of [`spawn_unattended_detached`], for the paths
/// that are expected to REFUSE: a refusal has to be read off the exit status
/// and stderr of a run that ends on its own.
fn run_paw(repo: &Path, home: &Path, socket_dir: &Path, args: &[&str]) -> (bool, String, String) {
    let out = StdCommand::new(BIN)
        .current_dir(repo)
        .args(args)
        .env("TMUX_TMPDIR", socket_dir)
        .env("HOME", home)
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("GIT_PAW_READINESS_TIMEOUT_MS", "120")
        .env("GIT_PAW_DRIVE_POLL_MS", "300")
        .env("GIT_PAW_DRIVE_HEARTBEAT_MS", "2500")
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .stdin(Stdio::null())
        .output()
        .expect("run git paw");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Spec scenarios "Re-running the launch command after a heartbeat exit
/// re-binds the loop", "A released binding lets the next loop bind at once"
/// (`supervisor-unattended-operation`) and "Non-interactive re-start with
/// --unattended binds a drive loop" (`tmux-orchestration`).
///
/// The same command, re-run with no controlling terminal against the session
/// its first run left Active, drives it again — and leaves the session itself
/// exactly as it found it: same panes, same titles, no new worktree, no
/// `paw-x-2`, and none of the GP-13 non-interactive refusal.
#[test]
#[serial]
fn unattended_rebinds_a_loop_to_a_live_session_after_heartbeat() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let home = TempDir::new().expect("home dir");
    let out = TempDir::new().expect("out dir");
    let out2 = TempDir::new().expect("second out dir");
    let port = pick_broker_port();
    write_config(tr.path(), port, plain_dummy(), plain_dummy());

    // First run: launches the session and exits on its heartbeat, leaving the
    // session Active and the binding released.
    let (mut first, first_stdout_path) = spawn_unattended_detached(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        out.path(),
        "feat/alpha",
        "300",
        "2500",
    );
    let first_status = wait_child(&mut first, Duration::from_secs(45));
    let first_stdout = read_to_string(&first_stdout_path);
    let session = session_name_in(&first_stdout)
        .unwrap_or_else(|| panic!("first run should name its session; stdout:\n{first_stdout}"));

    assert!(
        first_status.is_some_and(|s| s.success()),
        "first run should exit 0; stdout:\n{first_stdout}"
    );
    assert!(
        first_stdout.contains("Unattended drive loop exited: heartbeat"),
        "first run should end on its heartbeat; stdout:\n{first_stdout}"
    );
    assert!(
        session_exists(tmux_env.socket_dir(), &session),
        "the session must survive its drive loop's heartbeat exit; sessions on the \
         test socket: {:?}; stdout:\n{first_stdout}",
        sessions_on(tmux_env.socket_dir())
    );

    let panes_before = pane_roster(tmux_env.socket_dir(), &session);
    let worktrees_before = worktree_paths(tr.path());
    assert!(
        !panes_before.is_empty(),
        "the live session must have panes to compare against"
    );

    // Re-run, binding a loop to the live session rather than attaching,
    // refusing or forking. A DIFFERENT `--branches` than the launch used:
    // selection flags are never applied to a live session (that is what
    // `git paw add` is for), so the pane and worktree assertions below would
    // catch `feat/beta` being launched into this session — and with the same
    // branch they could not tell "ignored" from "applied idempotently".
    let started = Instant::now();
    let (mut second, second_stdout_path) = spawn_unattended_detached(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        out2.path(),
        "feat/beta",
        "300",
        "2500",
    );
    let second_status = wait_child(&mut second, Duration::from_secs(45));
    let elapsed = started.elapsed();
    let second_stdout = read_to_string(&second_stdout_path);
    let second_stderr = read_to_string(&out2.path().join("stderr.log"));

    assert!(
        second_status.is_some_and(|s| s.success()),
        "the re-run should bind a loop and exit 0; stdout:\n{second_stdout}\nstderr:\n{second_stderr}"
    );
    assert!(
        second_stdout.contains("Unattended drive loop exited:"),
        "the re-bound loop must run to an exit summary; stdout:\n{second_stdout}"
    );
    assert!(
        !second_stderr.contains("already running for this repository"),
        "--unattended must not take the GP-13 non-interactive refusal; stderr:\n{second_stderr}"
    );
    // The released binding must not make the next loop wait out the staleness
    // window (a floor of two minutes); finishing a 2.5s-heartbeat run well
    // inside that is only possible if it bound immediately.
    assert!(
        elapsed < Duration::from_secs(60),
        "a released binding must be taken at once, not waited out; took {elapsed:?}"
    );

    assert_eq!(
        pane_roster(tmux_env.socket_dir(), &session),
        panes_before,
        "binding must not create, rebuild or re-title any pane"
    );
    assert_eq!(
        worktree_paths(tr.path()),
        worktrees_before,
        "binding must not create or touch any worktree — in particular none for \
         the `feat/beta` the re-run named"
    );
    assert!(
        !session_exists(tmux_env.socket_dir(), &format!("{session}-2")),
        "--unattended must bind to the live session, never fork a parallel one"
    );
    kill_session_named(tmux_env.socket_dir(), &session);
}

/// Spec scenario "A second loop on a live session is refused": while one loop
/// drives `paw-x`, a second `start --unattended` refuses — naming the session
/// and the holding process — without running a loop or disturbing the first.
#[test]
#[serial]
fn second_unattended_loop_on_a_live_session_is_refused() {
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

    // A long heartbeat keeps the first loop running for the whole test; only
    // the completion published at the end ends it.
    let (mut first, first_stdout_path) = spawn_unattended_detached(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        out.path(),
        "feat/alpha",
        "300",
        "600000",
    );

    let url = format!("http://127.0.0.1:{port}");
    assert!(
        wait_for_broker(&url, Duration::from_secs(30)),
        "the first run's broker should come up"
    );
    // The binding file is the loop's own proof that it is bound and driving.
    let binding = git_paw::supervisor::loop_binding::binding_path(tr.path());
    assert!(
        wait_for_path(&binding, Duration::from_secs(30)),
        "the first loop must hold the drive-loop binding while it runs"
    );
    let held = fs::read_to_string(&binding).expect("read the held binding");
    let session = session_name_in(&read_to_string(&first_stdout_path))
        .unwrap_or_else(|| panic!("the first run should name its session"));
    let panes_before = pane_roster(tmux_env.socket_dir(), &session);
    let worktrees_before = worktree_paths(tr.path());
    assert!(
        !panes_before.is_empty(),
        "the driven session must have panes to compare against"
    );

    // A dry run binds nothing and is unaffected by the live loop, so it must
    // not be refused even while the binding is held.
    let (dry_ok, dry_stdout, dry_stderr) = run_paw(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        &[
            "start",
            "--unattended",
            "--dry-run",
            "--branches",
            "feat/alpha",
        ],
    );
    assert!(
        dry_ok,
        "a dry run binds nothing, so the held binding must not refuse it; \
         stdout:\n{dry_stdout}\nstderr:\n{dry_stderr}"
    );
    assert_eq!(
        fs::read_to_string(&binding).expect("read the binding after the dry run"),
        held,
        "a dry run must not touch the binding"
    );

    let (ok, stdout, stderr) = run_paw(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        &["start", "--unattended", "--branches", "feat/alpha"],
    );

    assert!(
        !ok,
        "a second drive loop must be refused; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains(&session),
        "the refusal must name the session; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains(&first.id().to_string()),
        "the refusal must name the holding process {}; stderr:\n{stderr}",
        first.id()
    );
    assert!(
        stderr.contains("only one drive loop may drive a session"),
        "the refusal must say why; stderr:\n{stderr}"
    );
    assert!(
        !stdout.contains("Unattended drive loop exited:"),
        "the refused caller must run no loop; stdout:\n{stdout}"
    );
    assert_eq!(
        fs::read_to_string(&binding).expect("read the binding after the refusal"),
        held,
        "a refused caller must leave the first loop's binding untouched"
    );
    assert_eq!(
        pane_roster(tmux_env.socket_dir(), &session),
        panes_before,
        "a refused caller must leave the session's panes unchanged"
    );
    assert_eq!(
        worktree_paths(tr.path()),
        worktrees_before,
        "a refused caller must leave the session's worktrees unchanged"
    );
    assert!(
        matches!(first.try_wait(), Ok(None)),
        "the first loop must keep driving the session"
    );

    // Let the first loop finish on a completion signal, then clean up.
    publish_terminal(&url, "feat-alpha", "done");
    let status = wait_child(&mut first, Duration::from_secs(45));
    let first_stdout = read_to_string(&first_stdout_path);
    kill_session(tmux_env.socket_dir(), &first_stdout);
    assert!(
        status.is_some(),
        "the first loop should exit after completion; stdout:\n{first_stdout}"
    );
}

/// Spec scenario "The re-bound loop uses the session's recorded broker": a
/// `[broker] port` edited after launch must not redirect the loop — it
/// publishes to the port the SESSION records, where the live broker actually
/// is.
#[test]
#[serial]
fn unattended_rebind_uses_the_recorded_broker_endpoint() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let home = TempDir::new().expect("home dir");
    let out = TempDir::new().expect("out dir");
    let out2 = TempDir::new().expect("second out dir");
    let scripts = TempDir::new().expect("scripts dir");
    let recorded_port = pick_broker_port();
    // The agent keeps a live permission prompt on screen, so the re-bound
    // loop has something to escalate.
    let agent = write_prompt_dummy(scripts.path(), "agent.sh", "rm -rf /tmp/x");
    write_config(
        tr.path(),
        recorded_port,
        plain_dummy(),
        &agent.to_string_lossy(),
    );

    let (mut first, first_stdout_path) = spawn_unattended_detached(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        out.path(),
        "feat/alpha",
        "300",
        "2500",
    );
    let first_status = wait_child(&mut first, Duration::from_secs(45));
    let first_stdout = read_to_string(&first_stdout_path);
    let session = session_name_in(&first_stdout)
        .unwrap_or_else(|| panic!("first run should name its session; stdout:\n{first_stdout}"));
    assert!(
        first_status.is_some_and(|s| s.success()),
        "first run should exit 0; stdout:\n{first_stdout}"
    );

    // Point the CONFIG at a different port. The live broker stays where the
    // session recorded it.
    let stale_port = pick_broker_port();
    assert_ne!(recorded_port, stale_port, "the two ports must differ");
    write_config(
        tr.path(),
        stale_port,
        plain_dummy(),
        &agent.to_string_lossy(),
    );

    // Listen on the RECONFIGURED port for the whole re-bind. This is the
    // discriminating half: if the loop took its endpoint from the current
    // config instead of the session, it would connect here. Counting broker
    // log lines alone cannot prove that — the first run's own escalations
    // land in the same log, and a late flush can arrive after any sample.
    let decoy = TcpListener::bind(("127.0.0.1", stale_port)).expect("listen on the stale port");
    decoy
        .set_nonblocking(true)
        .expect("decoy listener must not block the test");

    // Sample the log only once the first run's writes have settled, so a
    // trailing flush cannot be mistaken for the re-bound loop's own line.
    let mut before = broker_log_hits(home.path(), &["question:", "rm -rf /tmp/x"]);
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let now = broker_log_hits(home.path(), &["question:", "rm -rf /tmp/x"]);
        if now == before {
            break;
        }
        before = now;
    }

    let (mut second, second_stdout_path) = spawn_unattended_detached(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        out2.path(),
        "feat/alpha",
        "300",
        "2500",
    );

    // The escalation landing in the broker log is the observation: that log is
    // written by the broker the SESSION started, on `recorded_port`. Nothing
    // listens on the reconfigured port at all, so an escalation sent there
    // would simply never arrive.
    let escalated = wait_until(
        || broker_log_hits(home.path(), &["question:", "rm -rf /tmp/x"]) > before,
        Duration::from_secs(40),
    );
    let second_status = wait_child(&mut second, Duration::from_secs(45));
    let second_stdout = read_to_string(&second_stdout_path);

    assert!(
        escalated,
        "the re-bound loop must escalate to the broker the session recorded; \
         stdout:\n{second_stdout}\nbroker log:\n{}",
        read_broker_log(home.path())
    );
    assert!(
        second_status.is_some_and(|s| s.success()),
        "the re-bound run should exit 0; stdout:\n{second_stdout}"
    );
    assert!(
        matches!(
            decoy.accept(),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
        ),
        "nothing may reach the reconfigured port {stale_port}: the loop takes its \
         broker endpoint from the session, not the current config"
    );
    kill_session_named(tmux_env.socket_dir(), &session);
}

/// Spec clause "In every case the system SHALL NOT attach the terminal, even
/// when stdin is interactive, and SHALL print the session name and how to
/// attach from another terminal."
///
/// Every other test here runs with `Stdio::null()` or file-redirected stdio,
/// so none of them can see this: they would all pass a regression that
/// attaches on a TTY *in addition to* running the loop. This one runs the
/// command inside a real tmux pane — a genuine pty on stdin — on the
/// test-owned socket.
///
/// Note the helpers in `tests/helpers/pty.rs` are deliberately NOT used: they
/// set no `TMUX_TMPDIR` and remove no inherited `TMUX`, so they would drive
/// whichever tmux server the test process happens to be running under.
#[test]
#[serial]
fn unattended_rebind_does_not_attach_the_terminal_on_a_tty() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let (tr, tmux_env, home, session) = launched_then_idle_session();
    let out = TempDir::new().expect("out dir");
    let log = out.path().join("tty-run.log");
    let marker = out.path().join("tty-run.done");

    // A shell in a pane of its own session: its stdin is a pty, so
    // `is_terminal()` is true for the git-paw run below — the condition under
    // which an attach would be attempted.
    let shell = format!("{session}-tty");
    let started = StdCommand::new("tmux")
        .env("TMUX_TMPDIR", tmux_env.socket_dir())
        .env("HOME", home.path())
        .env("XDG_DATA_HOME", home.path().join(".local/share"))
        .env("XDG_CONFIG_HOME", home.path().join(".config"))
        .env("GIT_PAW_READINESS_TIMEOUT_MS", "120")
        .env("GIT_PAW_DRIVE_POLL_MS", "300")
        .env("GIT_PAW_DRIVE_HEARTBEAT_MS", "2500")
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .args([
            "new-session",
            "-d",
            "-s",
            &shell,
            "-x",
            "200",
            "-y",
            "50",
            "sh",
        ])
        .status()
        .expect("tmux new-session for the tty shell");
    assert!(started.success(), "could not create the tty shell session");

    let command = format!(
        "cd {} && {BIN} start --unattended --branches feat/alpha > {} 2>&1; touch {}\n",
        tr.path().display(),
        log.display(),
        marker.display()
    );
    let sent = StdCommand::new("tmux")
        .env("TMUX_TMPDIR", tmux_env.socket_dir())
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .args(["send-keys", "-t", &format!("{shell}:0.0"), &command])
        .status()
        .expect("tmux send-keys");
    assert!(sent.success(), "could not type the command into the pane");

    let finished = wait_for_path(&marker, Duration::from_secs(90));
    let text = read_to_string(&log);
    kill_session_named(tmux_env.socket_dir(), &shell);
    kill_session_named(tmux_env.socket_dir(), &session);

    assert!(finished, "the tty run never completed; output:\n{text}");
    assert!(
        text.contains("Unattended drive loop exited:"),
        "the loop must run even from a terminal; output:\n{text}"
    );
    assert!(
        text.contains("git paw attach"),
        "the command must print how to attach from another terminal; output:\n{text}"
    );
    // tmux refuses a nested attach with "sessions should be nested with care";
    // a client that got as far as trying would say so here. Neither message
    // may appear, because no attach may be attempted at all.
    assert!(
        !text.contains("sessions should be nested") && !text.contains("open terminal failed"),
        "no attach may be attempted on a tty; output:\n{text}"
    );
}

/// Spec scenario "A paused session is revived, then driven": the re-attach
/// brings the session back exactly as `git paw start` revives it, and only
/// then runs the loop — without attaching the terminal.
#[test]
#[serial]
fn unattended_revives_a_paused_session_then_drives() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let (tr, tmux_env, home, session) = launched_then_idle_session();
    let out = TempDir::new().expect("out dir");

    let (ok, stdout, stderr) = run_paw(tr.path(), home.path(), tmux_env.socket_dir(), &["pause"]);
    assert!(
        ok,
        "pause should succeed; stdout:\n{stdout}\nstderr:\n{stderr}"
    );

    let (mut child, stdout_path) = spawn_unattended_detached(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        out.path(),
        "feat/alpha",
        "300",
        "2500",
    );
    let status = wait_child(&mut child, Duration::from_secs(60));
    let run_stdout = read_to_string(&stdout_path);
    let run_stderr = read_to_string(&out.path().join("stderr.log"));

    assert!(
        status.is_some_and(|s| s.success()),
        "the re-attach should revive and drive; stdout:\n{run_stdout}\nstderr:\n{run_stderr}"
    );
    assert!(
        run_stdout.contains("Restarting paused session"),
        "the paused session must be revived first; stdout:\n{run_stdout}"
    );
    assert!(
        run_stdout.contains("Unattended drive loop exited:"),
        "a drive loop must then run against the revived session; stdout:\n{run_stdout}"
    );
    assert!(
        session_records(home.path()).contains("\"active\""),
        "the revived session must be recorded active; records:\n{}",
        session_records(home.path())
    );
    kill_session_named(tmux_env.socket_dir(), &session);
}

/// Spec scenario "A stopped session is recovered, then driven": the re-attach
/// rebuilds the session exactly as `git paw start` recovers it, then runs the
/// loop against what it rebuilt.
#[test]
#[serial]
fn unattended_recovers_a_stopped_session_then_drives() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let (tr, tmux_env, home, session) = launched_then_idle_session();
    let out = TempDir::new().expect("out dir");

    let (ok, stdout, stderr) = run_paw(tr.path(), home.path(), tmux_env.socket_dir(), &["stop"]);
    assert!(
        ok,
        "stop should succeed; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !git_paw::supervisor::loop_binding::binding_path(tr.path()).exists(),
        "stop must revoke the drive-loop binding"
    );

    let (mut child, stdout_path) = spawn_unattended_detached(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        out.path(),
        "feat/alpha",
        "300",
        "2500",
    );
    let status = wait_child(&mut child, Duration::from_secs(60));
    let run_stdout = read_to_string(&stdout_path);
    let run_stderr = read_to_string(&out.path().join("stderr.log"));

    assert!(
        status.is_some_and(|s| s.success()),
        "the re-attach should recover and drive; stdout:\n{run_stdout}\nstderr:\n{run_stderr}"
    );
    assert!(
        run_stdout.contains("Recovering session"),
        "the stopped session must be recovered first; stdout:\n{run_stdout}"
    );
    assert!(
        run_stdout.contains("Unattended drive loop exited:"),
        "a drive loop must then run against the recovered session; stdout:\n{run_stdout}"
    );
    assert!(
        session_exists(tmux_env.socket_dir(), &session),
        "the recovered session must be live"
    );
    kill_session_named(tmux_env.socket_dir(), &session);
}

/// Spec scenario "A bare session is refused without being changed": a session
/// launched without supervisor mode deliberately leaves its boot prompts for
/// the human, and the loop submits an idle pane's buffered input — so driving
/// one would send exactly what the human was meant to review.
#[test]
#[serial]
fn unattended_refuses_a_bare_session() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let home = TempDir::new().expect("home dir");
    // No broker, so the bare session is exactly one coding pane: the refusal
    // is decided on the session's recorded mode and needs nothing else, and a
    // one-pane session makes "the panes are unchanged" unambiguous.
    fs::create_dir_all(tr.path().join(".git-paw")).expect("create .git-paw");
    fs::write(
        tr.path().join(".git-paw/config.toml"),
        format!(
            "default_cli = \"{0}\"\n\n[clis.{0}]\ncommand = \"{0}\"\n\n[broker]\nenabled = false\n",
            plain_dummy()
        ),
    )
    .expect("write bare config");

    // A bare launch: --no-supervisor overrides the config's supervisor mode.
    let (ok, stdout, stderr) = run_paw(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        &[
            "start",
            "--no-supervisor",
            "--branches",
            "feat/alpha",
            "--cli",
            plain_dummy(),
        ],
    );
    assert!(
        ok,
        "the bare launch should succeed; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let session = detached_session_name_in(&stdout)
        .unwrap_or_else(|| panic!("the bare launch should name its session; stdout:\n{stdout}"));

    // The session as the refusal must leave it. Liveness is captured
    // alongside the roster rather than asserted: the refusal is decided on
    // the session's RECORDED mode, so it must hold for an Active, a Paused
    // and a Stopped bare session alike (as the scenario's GIVEN says), and
    // this pair is what "the panes are unchanged" means in all three.
    let panes_before = (
        session_exists(tmux_env.socket_dir(), &session),
        pane_roster(tmux_env.socket_dir(), &session),
    );
    let worktrees_before = worktree_paths(tr.path());
    let record_before = session_records(home.path());
    assert!(
        !worktrees_before.is_empty(),
        "the bare launch should have registered a worktree; stdout:\n{stdout}"
    );

    let (ok, stdout, stderr) = run_paw(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        &["start", "--unattended", "--branches", "feat/alpha"],
    );

    assert!(
        !ok,
        "a bare session must not be driven unattended; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains(&session),
        "the refusal must name the session; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("supervisor mode"),
        "the refusal must say only supervised sessions can be driven; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("git paw attach"),
        "the refusal must name how to drive it by hand; stderr:\n{stderr}"
    );
    assert!(
        !stdout.contains("Unattended drive loop exited:"),
        "no drive loop may run against a bare session; stdout:\n{stdout}"
    );
    assert_eq!(
        (
            session_exists(tmux_env.socket_dir(), &session),
            pane_roster(tmux_env.socket_dir(), &session),
        ),
        panes_before,
        "the refusal must leave the session's panes unchanged"
    );
    assert_eq!(
        worktree_paths(tr.path()),
        worktrees_before,
        "the refusal must leave the session's worktrees unchanged"
    );
    assert_eq!(
        session_records(home.path()),
        record_before,
        "the refusal must leave the session record unchanged"
    );
    assert!(
        !git_paw::supervisor::loop_binding::binding_path(tr.path()).exists(),
        "a refused bare session must not even have had a binding taken"
    );
    kill_session_named(tmux_env.socket_dir(), &session);
}

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

/// Spec scenario "A detached run reaches completion"
/// (`unattended-heartbeat-progress`): launched with no terminal on any stream
/// and a 4s heartbeat, a wave whose agent keeps committing for three
/// heartbeat intervals is NOT cut off, because the heartbeat counts from the
/// last branch-tip move rather than from launch. Once the agent stops, its
/// branch is merged and it reports `done`, so the run ends `completed`.
#[test]
#[serial]
fn detached_unattended_run_completes_past_the_heartbeat() {
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
    let stop = out.path().join("stop");
    let stopped = out.path().join("stopped");

    let agent = write_committing_dummy(tr.path(), "dummy_commit.sh", &stop, &stopped);
    write_config(tr.path(), port, plain_dummy(), &agent.to_string_lossy());

    let heartbeat = Duration::from_secs(4);
    let (mut child, stdout_path) = spawn_unattended_detached(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        out.path(),
        "feat/alpha",
        "300",
        "4000",
    );

    assert!(
        wait_for_broker(&url, Duration::from_secs(20)),
        "broker should come up on the ephemeral port"
    );

    // The run must outlive three heartbeat intervals while the agent keeps
    // committing — measured from the broker coming up, which precedes the
    // loop, so the loop has run at least that long by the time this holds.
    let since_broker = Instant::now();
    let main_commits = commit_count(tr.path(), "main");
    let mut exited_early = None;
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            exited_early = Some(status);
            break;
        }
        // The process can outlive its loop briefly while it prints the
        // summary, so the loop's own exit line is the earlier signal.
        if read_to_string(&stdout_path).contains("Unattended drive loop exited") {
            exited_early = child.wait().ok();
            break;
        }
        let committing = commit_count(tr.path(), "feat/alpha") >= main_commits + 5;
        if since_broker.elapsed() >= heartbeat * 3 && committing {
            break;
        }
        assert!(
            since_broker.elapsed() < Duration::from_secs(60),
            "the agent never committed enough to test the heartbeat"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    if let Some(status) = exited_early {
        let stdout = read_to_string(&stdout_path);
        kill_session(tmux_env.socket_dir(), &stdout);
        panic!(
            "a progressing wave was cut off ({status}) before three heartbeat intervals; \
             stdout:\n{stdout}"
        );
    }

    // Stop the agent, wait until it has really stopped committing, then merge
    // its branch and report it done — quickly, well inside one heartbeat.
    fs::write(&stop, "").expect("write stop file");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !stopped.exists() {
        assert!(Instant::now() < deadline, "the agent never stopped");
        std::thread::sleep(Duration::from_millis(50));
    }
    let merge = StdCommand::new("git")
        .current_dir(tr.path())
        .args(["merge", "--ff-only", "-q", "feat/alpha"])
        .output()
        .expect("git merge");
    assert!(
        merge.status.success(),
        "fast-forward main to the agent's branch: {}",
        String::from_utf8_lossy(&merge.stderr)
    );
    publish_terminal(&url, "feat-alpha", "done");

    let status = wait_child(&mut child, Duration::from_secs(30));
    let stdout = read_to_string(&stdout_path);
    kill_session(tmux_env.socket_dir(), &stdout);

    let status = status.unwrap_or_else(|| panic!("child did not exit; stdout:\n{stdout}"));
    assert!(
        status.success(),
        "unattended run should exit 0; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Unattended drive loop exited: completed"),
        "a wave that kept progressing must reach completion, not the heartbeat; stdout:\n{stdout}"
    );
}

/// Spec scenario "A command formerly classified safe is escalated, not
/// approved", end-to-end: `--unattended` escalates a live prompt for `ls -la`
/// on a coding-agent pane rather than answering it, publishes no
/// `auto_approved` status, still detects the completion signal, and exits
/// `escalated-for-review` — with no real LLM.
#[test]
#[serial]
fn unattended_escalates_live_prompt_then_completes() {
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

    // Agent "a" sits on a live prompt for `ls -la` — a command the retired
    // classifier called safe and auto-approved. The supervisor pane holds.
    let prompt = write_prompt_dummy(tr.path(), "dummy_prompt.sh", "ls -la");
    write_config(tr.path(), port, plain_dummy(), &prompt.to_string_lossy());

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
    // The loop escalates it rather than answering it: an `agent.question`
    // naming the agent and the command reaches the broker.
    let escalated = wait_for_broker_log(
        home.path(),
        &["question:", "feat-alpha is waiting on", "ls -la"],
        Duration::from_secs(25),
    );
    // Signal completion via a supervisor terminal verdict, then let the loop exit.
    publish_terminal(&url, "supervisor", "done");

    let status = wait_child(&mut child, Duration::from_secs(30));
    let stdout = read_to_string(&stdout_path);
    let broker_log = read_broker_log(home.path());
    kill_session(tmux_env.socket_dir(), &stdout);

    assert!(
        escalated,
        "the prompt should have been escalated, not approved; stdout:\n{stdout}\nbroker log:\n{broker_log}"
    );
    assert!(
        !broker_log.contains("auto_approved"),
        "no auto_approved status may ever be published; broker log:\n{broker_log}"
    );
    let status = status.unwrap_or_else(|| panic!("child did not exit; stdout:\n{stdout}"));
    assert!(
        status.success(),
        "unattended run should exit 0; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Unattended drive loop exited: escalated-for-review"),
        "an unanswered prompt ends the run for review; stdout:\n{stdout}"
    );
}

/// `live-prompt-tail-window` task 5.2 — the same safe prompt, but the pane's
/// chrome under the input area (a persistent task list) pushes the modal six
/// non-blank lines above the end of the capture, exactly as a dogfooded
/// current Claude Code renders it. An `--unattended` run must still
/// auto-approve: the trailing chrome must not hide a live modal.
#[test]
#[serial]
fn unattended_escalates_live_prompt_with_trailing_task_region() {
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

    let prompt =
        write_prompt_dummy_with_trailing_region(tr.path(), "dummy_prompt_chrome.sh", "ls -la");
    write_config(tr.path(), port, plain_dummy(), &prompt.to_string_lossy());

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
    let escalated = wait_for_broker_log(
        home.path(),
        &["question:", "feat-alpha is waiting on", "ls -la"],
        Duration::from_secs(25),
    );
    publish_terminal(&url, "supervisor", "done");

    let status = wait_child(&mut child, Duration::from_secs(30));
    let stdout = read_to_string(&stdout_path);
    let broker_log = read_broker_log(home.path());
    kill_session(tmux_env.socket_dir(), &stdout);

    assert!(
        escalated,
        "the prompt must still be DETECTED and escalated with trailing task-list chrome below it; stdout:\n{stdout}\nbroker log:\n{broker_log}"
    );
    let status = status.unwrap_or_else(|| panic!("child did not exit; stdout:\n{stdout}"));
    assert!(
        status.success(),
        "unattended run should exit 0; stdout:\n{stdout}"
    );
}

/// Spec scenario "A live prompt is escalated without blocking the other
/// agent" (`supervisor-unattended-operation`): agent `feat/alpha` sits on a
/// live prompt for `ls -la` and RECORDS every byte reaching its stdin, while
/// `feat/beta` finishes normally. The run must escalate alpha's prompt to the
/// broker, the summary and the manual-decision log, send alpha not one
/// keystroke, and still exit 0.
#[test]
#[serial]
fn unattended_escalates_live_prompt_without_blocking_other_agent() {
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

    // One shared branch-aware CLI: the launcher assigns the same default CLI
    // to every agent pane, and the pane's current branch decides what it does.
    // `feat/alpha` prompts and records its stdin; `feat/beta` just holds.
    let stdin_marker = out.path().join("alpha-stdin.bin");
    let dummy = write_prompt_recorder_dummy(
        tr.path(),
        "dummy_recorder.sh",
        "feat/alpha",
        "ls -la",
        &stdin_marker,
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
    // Alpha's prompt is escalated to the broker …
    let escalated = wait_for_broker_log(
        home.path(),
        &["question:", "feat-alpha is waiting on", "ls -la"],
        Duration::from_secs(25),
    );
    // … while beta is free to finish: the wave is not blocked on alpha.
    publish_terminal(&url, "feat-beta", "done");
    let beta_done = wait_for_status(&url, "feat-beta", "done", Duration::from_secs(25));
    publish_terminal(&url, "supervisor", "done");

    let status = wait_child(&mut child, Duration::from_secs(30));
    let stdout = read_to_string(&stdout_path);
    let broker_log = read_broker_log(home.path());
    let alpha_stdin = fs::read(&stdin_marker).unwrap_or_default();
    kill_session(tmux_env.socket_dir(), &stdout);

    assert!(
        escalated,
        "alpha's prompt should be escalated; stdout:\n{stdout}\nbroker log:\n{broker_log}"
    );
    assert!(
        beta_done,
        "the other agent must still reach done (non-blocking); stdout:\n{stdout}"
    );
    // The whole point: across every sweep of a permanently prompting pane,
    // not one byte was typed into it.
    assert!(
        alpha_stdin.is_empty(),
        "no keystroke may reach the prompting pane; its stdin recorded {:?}; stdout:\n{stdout}",
        String::from_utf8_lossy(&alpha_stdin)
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
        stdout.contains("[permission-prompt]") && stdout.contains("ls -la"),
        "the prompt should be listed as an escalation under the one verdict; stdout:\n{stdout}"
    );

    // Spec scenario "A drive-loop escalation appends a JSONL line": the same
    // escalation also lands in the session's manual-decision log, exactly
    // once, so `git paw approvals` can surface the command later. Alpha's
    // pane prompted on every sweep of the run, so "exactly one" is also the
    // dedup window doing its job end to end.
    let decisions = read_manual_decision_log(tr.path());
    let lines: Vec<&str> = decisions.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        lines.len(),
        1,
        "exactly one manual-decision line expected; log was:\n{decisions}"
    );
    let entry: serde_json::Value =
        serde_json::from_str(lines[0]).expect("the log line must be JSON");
    assert_eq!(entry["agent_id"], "feat-alpha", "{entry}");
    assert_eq!(entry["pattern"], "ls -la", "{entry}");
    assert_eq!(entry["first_seen"], true, "{entry}");
}

/// Spec scenarios "A permission-prompt escalation is never typed into a
/// surface" (`supervisor-unattended-operation`) and "Escalation is recorded
/// as a broker review item only" (`supervisor-autonomous-orchestrator`): a
/// coding agent's live prompt reaches the broker as a review item from
/// `supervisor`, the prompting pane's stdin records no keystroke, and the
/// configured `[supervisor].cli` never runs in any pane — its own recorder
/// is never created.
#[test]
#[serial]
fn unattended_records_the_escalation_on_the_broker_and_types_nothing() {
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

    // The coding agent shows a live `ls -la` prompt and records its stdin.
    // `[supervisor].cli` is a recorder too: it would create its marker the
    // moment any pane ran it, on any branch.
    let agent_stdin = out.path().join("agent-stdin.bin");
    let agent = write_prompt_recorder_dummy(
        tr.path(),
        "dummy_agent.sh",
        "feat/alpha",
        "ls -la",
        &agent_stdin,
    );
    let orchestrator_marker = out.path().join("orchestrator-ran.bin");
    let orchestrator = write_prompt_recorder_dummy(
        tr.path(),
        "dummy_orch.sh",
        "main",
        "never shown",
        &orchestrator_marker,
    );
    write_config(
        tr.path(),
        port,
        &orchestrator.to_string_lossy(),
        &agent.to_string_lossy(),
    );

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
    let escalated = wait_for_broker_log(
        home.path(),
        &["question:", "feat-alpha is waiting on", "ls -la"],
        Duration::from_secs(25),
    );
    publish_terminal(&url, "feat-alpha", "verified");

    let status = wait_child(&mut child, Duration::from_secs(30));
    let stdout = read_to_string(&stdout_path);
    let broker_log = read_broker_log(home.path());
    let agent_keys = fs::read(&agent_stdin).unwrap_or_default();
    kill_session(tmux_env.socket_dir(), &stdout);

    assert!(
        escalated,
        "the prompt should be a broker review item; stdout:\n{stdout}\nbroker log:\n{broker_log}"
    );
    assert!(
        agent_keys.is_empty(),
        "no keystroke may reach the prompting pane; its stdin recorded {:?}; stdout:\n{stdout}",
        String::from_utf8_lossy(&agent_keys)
    );
    assert!(
        !orchestrator_marker.exists(),
        "no pane may run the orchestrator CLI; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("its CLI is not monitored by the loop"),
        "the summary disclaims orchestrator coverage; stdout:\n{stdout}"
    );
    let status = status.unwrap_or_else(|| panic!("child did not exit; stdout:\n{stdout}"));
    assert!(
        status.success(),
        "unattended run should exit 0; stdout:\n{stdout}"
    );
}

/// `drive-loop-exit-summary-truth`, GP-05/GP-20 fix: a worker publishes `done`
/// with a commit that never made it into the default branch — the exact
/// 0.16.0 dogfood shape. The summary must read `escalated-for-review` /
/// `0/1 branches merged` with an `[unmerged]` entry, and the process still
/// exits `0` (an unmerged branch is a review item, not a process failure).
#[test]
#[serial]
fn unattended_reports_escalated_for_review_when_branch_unmerged() {
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

    // Neither pane needs a live prompt: completion comes from the worker's
    // own broker status, and the merge truth comes from git.
    write_config(tr.path(), port, plain_dummy(), plain_dummy());

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
    let worktree = worktree_path_for(tr.path(), "feat/alpha", WorktreePlacement::Sibling)
        .expect("derive the sibling worktree path");
    assert!(
        wait_for_path(&worktree, Duration::from_secs(20)),
        "the agent's worktree should be created"
    );
    // The worker's branch gets a commit that never reaches the default
    // branch — nothing in this test ever merges it.
    commit_ahead(&worktree, "unmerged work");
    publish_terminal(&url, "feat-alpha", "done");

    let status = wait_child(&mut child, Duration::from_secs(30));
    let stdout = read_to_string(&stdout_path);
    kill_session(tmux_env.socket_dir(), &stdout);

    let status = status.unwrap_or_else(|| panic!("child did not exit; stdout:\n{stdout}"));
    assert!(
        status.success(),
        "an unmerged branch is a review item, not a process failure; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Unattended drive loop exited: escalated-for-review"),
        "a finished-but-unmerged wave must never read completed; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("0/1 branches merged"),
        "the roll-up must be ref-derived, not phase-derived; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("[unmerged]"),
        "the unmerged branch must be listed as an escalation; stdout:\n{stdout}"
    );
}

/// The all-merged counterpart: the same worker commit, fast-forwarded into
/// the default branch before completion, reports `completed` / `1/1 branches
/// merged`.
#[test]
#[serial]
fn unattended_reports_completed_when_branch_is_merged() {
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

    write_config(tr.path(), port, plain_dummy(), plain_dummy());

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
    let worktree = worktree_path_for(tr.path(), "feat/alpha", WorktreePlacement::Sibling)
        .expect("derive the sibling worktree path");
    assert!(
        wait_for_path(&worktree, Duration::from_secs(20)),
        "the agent's worktree should be created"
    );
    commit_ahead(&worktree, "work that gets merged");
    // Fast-forward the default branch to include it, at the repo root (main
    // stays checked out there; the worktree owns `feat/alpha` separately).
    let ff = StdCommand::new("git")
        .current_dir(tr.path())
        .args(["merge", "--ff-only", "feat/alpha"])
        .status()
        .expect("fast-forward main onto feat/alpha");
    assert!(ff.success(), "fast-forward must succeed");
    publish_terminal(&url, "feat-alpha", "done");

    let status = wait_child(&mut child, Duration::from_secs(30));
    let stdout = read_to_string(&stdout_path);
    kill_session(tmux_env.socket_dir(), &stdout);

    let status = status.unwrap_or_else(|| panic!("child did not exit; stdout:\n{stdout}"));
    assert!(
        status.success(),
        "unattended run should exit 0; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Unattended drive loop exited: completed"),
        "stdout:\n{stdout}"
    );
    assert!(stdout.contains("1/1 branches merged"), "stdout:\n{stdout}");
}

/// Spec scenario "A rate-limited worker is escalated while the wave keeps
/// moving": a coding agent whose pane shows a session-limit banner becomes a
/// `cli-unavailable` escalation quoting the line, the run still reaches its
/// completion signal, and the wave is reported `escalated-for-review` — not
/// `completed`, and not a process failure.
#[test]
#[serial]
fn unattended_escalates_a_rate_limited_worker_and_keeps_going() {
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

    let marker = write_marker_dummy(tr.path(), "dummy_limit.sh", "You've hit your session limit");
    write_config(tr.path(), port, plain_dummy(), &marker.to_string_lossy());

    let (mut child, stdout_path) = spawn_unattended(
        tr.path(),
        home.path(),
        tmux_env.socket_dir(),
        out.path(),
        "feat/alpha",
        "400",
        "120000", // long heartbeat: the completion signal ends the run
    );

    assert!(
        wait_for_broker(&url, Duration::from_secs(20)),
        "broker should come up on the ephemeral port"
    );
    let escalated = wait_for_broker_log(
        home.path(),
        &["question:", "feat-alpha"],
        Duration::from_secs(25),
    );
    publish_terminal(&url, "feat-alpha", "verified");

    let status = wait_child(&mut child, Duration::from_secs(30));
    let stdout = read_to_string(&stdout_path);
    kill_session(tmux_env.socket_dir(), &stdout);

    assert!(escalated, "the banner is escalated; stdout:\n{stdout}");
    let status = status.unwrap_or_else(|| panic!("child did not exit; stdout:\n{stdout}"));
    assert!(
        status.success(),
        "a worker's banner is a review item, not a process failure; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Unattended drive loop exited: escalated-for-review"),
        "stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("[cli-unavailable] feat-alpha")
            && stdout.contains("You've hit your session limit"),
        "the summary lists the escalation quoting the matching line; stdout:\n{stdout}"
    );
}
