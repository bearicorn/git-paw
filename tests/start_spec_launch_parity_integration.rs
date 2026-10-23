//! E2E regression tests for `start-spec-launch-parity`.
//!
//! Covers every defect fixed by this change, all on the non-supervisor
//! launch paths (`cmd_start` and `cmd_start_with_specs`):
//!
//! 1. A `--specs` launch without supervisor mode left the boot prompt
//!    pre-filled (never submitted) and never injected the task prompt at
//!    all, so no agent ever started work.
//! 2. `config::resolve_approval_flags` was never consulted outside the
//!    supervisor and `add` paths, so `agent_approval` / `approval_args` had
//!    no effect on a bare or spec-driven launch.
//! 3. `cmd_start_with_specs` never called `write_repo_discovery_file`, so
//!    `.git-paw/sessions/paw-<project>.json` never existed for a spec-driven
//!    launch and `sweep.sh` could not discover the session from the repo
//!    root (folded in during review, task 7).
//! 4. Neither non-supervisor path gated a pane on launch-readiness before
//!    injecting, unlike the supervisor and `add` paths — a blind `Enter`
//!    into a pane still showing an unanswered first-run trust/bypass dialog
//!    could accidentally accept it (folded in during gate review, task 9).
//!
//! **Why a logging probe script, not `sh`, is the stub CLI for the
//! submission test.** A real boot+task prompt is long, multi-paragraph
//! markdown full of inline backticks and fenced code blocks. An earlier
//! iteration of this test used `sh` (matching `boot_block_integration.rs`'s
//! stub) and fed it the real prompt text — `sh`, being a real cooked-mode
//! POSIX shell, tries to *execute* every embedded line, and an unmatched
//! backtick inside a prose sentence opens a multi-line command substitution
//! that swallows and garbles subsequent lines. That's fine for
//! `boot_block_integration.rs` (which only checks that recognizable
//! substrings landed somewhere in the pane), but it makes a precise "was the
//! final line actually terminated" signal both unreliable and, worse,
//! ambiguous even with a non-interpreting stub like `cat`: a pane's local
//! terminal echo and a `cat` readback of the same line are visually
//! indistinguishable in a single `capture-pane` snapshot, so counting
//! occurrences doesn't reliably separate "echoed as typed" from "delivered
//! to the reader".
//!
//! Instead, the stub CLI here is a tiny generated shell script that reads
//! lines from stdin and appends each one it receives to a log *file* (not
//! back to the pty). A POSIX terminal's cooked-mode line discipline only
//! delivers a line to the reading process once it sees a newline — until
//! then the line sits in the kernel's input buffer, invisible to the reading
//! process, even though the terminal's local echo has already *displayed*
//! the typed characters on screen. The task-prompt templates
//! (`assets/boot-task-prompts/*.md`) deliberately end with no trailing
//! newline of their own (so the combined prompt's very last byte is never a
//! `\n`), so the only thing that can terminate that final line is the
//! separate `Enter` keystroke `submit_prompt_to_pane` sends after the settle
//! delay. The probe script's log file is a clean, unambiguous sink: the
//! anchor phrase lands there if and only if that final line was actually
//! delivered — i.e. the prompt was submitted, not left pre-fill-only.
//!
//! Skips if tmux is unavailable.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// Sentinel approval flag mapped to the default `auto` level via the
/// `[clis.echo]` override; unique enough that it can only appear in a pane
/// via the resolved worker command.
const SENTINEL_FLAG: &str = "--paw-spec-launch-parity-sentinel";

/// Atomic counter so each test gets a unique tmux project name.
static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Returns a project name unique to this run.
fn unique_project_name(tag: &str) -> String {
    let pid = std::process::id();
    let n = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("launch-parity-{tag}-{pid}-{n}")
}

/// Binds an ephemeral TCP port and returns it, so broker-enabled configs in
/// this file never collide with another test's fixed port.
fn find_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral")
        .local_addr()
        .expect("local_addr")
        .port()
}

fn skip_if_no_tmux() -> bool {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return true;
    }
    false
}

/// Renames the test repo's basename so `tmux::resolve_session_name` produces
/// a deterministic, unique session name we can target.
fn rename_repo_basename(tr: &TestRepo, new_basename: &str) -> std::path::PathBuf {
    let original = tr.path().to_path_buf();
    let parent = original.parent().expect("repo has parent").to_path_buf();
    let renamed = parent.join(new_basename);
    fs::rename(&original, &renamed).expect("rename repo dir");
    renamed
}

/// Resolves the sessions directory a `git paw` subprocess given `HOME = home`
/// (and `XDG_DATA_HOME` removed) would write into, mirroring
/// `crate::dirs::data_dir()`. Needed because an in-process call to
/// `session::find_session_for_repo` resolves from the *test process's own*
/// environment, not the `home` override passed only to a spawned subprocess —
/// checking written-session-state after such a launch must look here instead.
fn sessions_dir_under(home: &std::path::Path) -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support")
            .join("git-paw")
            .join("sessions")
    }
    #[cfg(not(target_os = "macos"))]
    {
        home.join(".local/share").join("git-paw").join("sessions")
    }
}

/// Writes and commits a pending `OpenSpec` change at `<repo>/specs/<id>/tasks.md`.
fn write_committed_spec(repo: &std::path::Path, id: &str, body: &str) {
    let change_dir = repo.join("specs").join(id);
    fs::create_dir_all(&change_dir).expect("create change dir");
    fs::write(change_dir.join("tasks.md"), body).expect("write tasks.md");
    commit_specs(repo);
}

/// Writes and commits a pending **Markdown**-backend spec at
/// `<repo>/specs/<id>.md`. That backend declares no workflow entry point, so
/// a launch from it must type exactly one message.
fn write_committed_markdown_spec(repo: &std::path::Path, id: &str, body: &str) {
    let specs = repo.join("specs");
    fs::create_dir_all(&specs).expect("create specs dir");
    fs::write(
        specs.join(format!("{id}.md")),
        format!("---\npaw_status: pending\n---\n{body}"),
    )
    .expect("write markdown spec");
    commit_specs(repo);
}

fn commit_specs(repo: &std::path::Path) {
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

/// The tail of the bundled worktree rule (`assets/worktree-root-rule.md`),
/// which every launch prompt now ends with. Because it is the prompt's LAST
/// line and the asset carries no trailing newline, its arrival in the probe's
/// log is the submission signal — and its absence there, with the text
/// visible in the pane, is the pre-fill-only signal.
const WORKTREE_RULE_TAIL: &str = "not by a relative path that climbs out of it";

/// Polls `path` until its contents contain `needle`, or `timeout` elapses.
/// Returns the final contents either way (callers assert).
fn wait_for_log(path: &std::path::Path, needle: &str, timeout: Duration) -> String {
    let deadline = Instant::now() + timeout;
    let mut contents = String::new();
    while Instant::now() < deadline {
        contents = fs::read_to_string(path).unwrap_or_default();
        if contents.contains(needle) {
            break;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    contents
}

/// Writes an executable shell script at `<bin_dir>/paw-test-dialog` that
/// immediately prints a recognised Claude Code first-run acceptance-dialog
/// heading (`DIALOG_MARKERS` in `src/tmux/readiness.rs`) with no numbered
/// option line following it, then blocks forever — simulating a pane stuck
/// on an unanswered dialog whose affirmative digit the gate cannot
/// determine (`PaneReadiness::Dialog(None)` -> immediate `DialogStuck`).
fn write_dialog_stub_cli(bin_dir: &std::path::Path) -> &'static str {
    let script = "#!/bin/sh\necho 'Bypass Permissions mode'\nwhile true; do sleep 5; done\n";
    let script_path = bin_dir.join("paw-test-dialog");
    fs::write(&script_path, script).expect("write dialog stub script");
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
        .expect("chmod dialog stub script");
    "paw-test-dialog"
}

/// Writes an executable shell script at `<bin_dir>/paw-test-slow-ready` that
/// sleeps for `delay` before printing `marker`, then blocks reading stdin —
/// simulating a CLI whose interactive UI takes a moment to come up. Pair with
/// a `[clis.<name>.prompt_profile] readiness_markers = ["<marker>"]` override
/// so the gate recognises `marker` as this CLI's own readiness signal.
fn write_slow_ready_cli(bin_dir: &std::path::Path, marker: &str, delay: Duration) -> &'static str {
    let script = format!(
        "#!/bin/sh\nsleep {}\necho '{marker}'\ncat\n",
        delay.as_secs_f64()
    );
    let script_path = bin_dir.join("paw-test-slow-ready");
    fs::write(&script_path, script).expect("write slow-ready script");
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
        .expect("chmod slow-ready script");
    "paw-test-slow-ready"
}

/// Spawns `git paw <args>` directly (not via `assert_cmd`, so the caller can
/// poll the live pane while the process is still running), isolated to
/// `env`'s tmux socket and `path`.
fn spawn_start(
    env: &TmuxTestEnv,
    repo: &std::path::Path,
    home: &std::path::Path,
    path: &str,
    readiness_timeout_ms: &str,
    args: &[&str],
) -> std::process::Child {
    let mut command = StdCommand::new(env!("CARGO_BIN_EXE_git-paw"));
    env.apply(&mut command);
    command
        .current_dir(repo)
        .env("HOME", home)
        .env_remove("XDG_DATA_HOME")
        .env("PATH", path)
        // Override the suite-wide short readiness timeout (set by
        // `TmuxTestEnv::apply`) for tests that need the gate to genuinely
        // wait out a slow-but-real readiness transition rather than falling
        // back immediately.
        .env("GIT_PAW_READINESS_TIMEOUT_MS", readiness_timeout_ms)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::null());
    command.spawn().expect("spawn git paw start")
}

/// Polls until `tmux has-session` succeeds for `session`, or `timeout` elapses.
fn wait_for_session(env: &TmuxTestEnv, session: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let mut has_session_cmd = StdCommand::new("tmux");
        env.apply(&mut has_session_cmd);
        if has_session_cmd
            .args(["has-session", "-t", session])
            .status()
            .is_ok_and(|s| s.success())
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// Writes an executable shell script at `<bin_dir>/paw-test-probe` that reads
/// lines from stdin and appends each terminated line it receives to
/// `log_path`. Returns the script's `binary_name` to pass as `--cli`.
///
/// `bin_dir` MUST already be prepended to the test process's `PATH` (tmux
/// resolves the pane's command — a bare binary name, no absolute path —
/// against its inherited `PATH`, same as it already does for the `echo` /
/// `sh` stub CLIs other tests in this suite use).
/// The script `exec`s `tee` rather than running a `while read` loop, so the
/// pane's `#{pane_current_command}` is `tee` and not `sh`. That matters
/// because the entry-point submit refuses to type into a pane whose
/// foreground process is a shell (a pane whose CLI exited is a command
/// interpreter, and free text there would be executed). A shell-loop stub
/// is indistinguishable from exactly that case, so it would be skipped —
/// correctly, but it would stop the stub from standing in for a CLI.
///
/// The submission signal is unchanged: a terminal's cooked-mode line
/// discipline still delivers a line to the reading process only once a
/// newline arrives, whether that reader is `read` or `tee`.
fn write_probe_cli(bin_dir: &std::path::Path, log_path: &std::path::Path) -> &'static str {
    let script = format!(
        "#!/bin/sh\nexec tee -a '{}' > /dev/null\n",
        log_path.display()
    );
    let script_path = bin_dir.join("paw-test-probe");
    fs::write(&script_path, script).expect("write probe script");
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
        .expect("chmod probe script");
    "paw-test-probe"
}

/// Captures `pane` of `session`, joining soft-wrapped lines (`-J`) so a
/// distinctive substring survives regardless of the pane's column width.
fn capture_pane(env: &TmuxTestEnv, session: &str, pane: usize) -> String {
    let target = format!("{session}:0.{pane}");
    let mut capture_cmd = StdCommand::new("tmux");
    env.apply(&mut capture_cmd);
    let out = capture_cmd
        .args(["capture-pane", "-t", &target, "-p", "-J", "-S", "-2000"])
        .output()
        .expect("tmux capture-pane");
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// Polls `pane` until its capture contains `needle`, or `timeout` elapses.
/// Returns the final capture either way (callers assert).
fn wait_for_pane_content(
    env: &TmuxTestEnv,
    session: &str,
    pane: usize,
    needle: &str,
    timeout: Duration,
) -> String {
    let deadline = Instant::now() + timeout;
    let mut buffer = String::new();
    while Instant::now() < deadline {
        buffer = capture_pane(env, session, pane);
        if buffer.contains(needle) {
            break;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    buffer
}

fn kill_session(env: &TmuxTestEnv, name: &str) {
    let mut kill_cmd = StdCommand::new("tmux");
    env.apply(&mut kill_cmd);
    let _ = kill_cmd.args(["kill-session", "-t", name]).status();
}

// ---------------------------------------------------------------------------
// Task 1.1 / 1.2 — spec-driven launch submits the boot prompt, which carries
// the task prompt
// ---------------------------------------------------------------------------

/// `git paw start --from-specs` without supervisor mode SHALL submit the
/// combined boot+task prompt (no residual unsubmitted line), and that prompt
/// SHALL carry the task prompt naming the spec assignment and apply
/// invocation.
///
/// Task 1.2 (content was injected at all) is checked via the pane's own
/// capture (local echo shows typed characters immediately, submitted or
/// not). Task 1.1 (the prompt was actually SUBMITTED, not left pre-fill-only)
/// is checked via the probe script's log file: the task-prompt templates end
/// with no trailing newline, so the final line only reaches the probe's
/// `read` loop — and thus its log file — if the separate `Enter`
/// `submit_prompt_to_pane` sends after the settle delay actually arrives.
#[test]
#[serial]
fn spec_driven_launch_submits_boot_prompt_with_task_prompt() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("submit");
    let repo = rename_repo_basename(&tr, &project);

    let bin_dir = TempDir::new().expect("probe bin dir");
    let log_dir = TempDir::new().expect("probe log dir");
    let log_path = log_dir.path().join("probe.log");
    let probe_cli = write_probe_cli(bin_dir.path(), &log_path);
    let inherited_path = std::env::var("PATH").unwrap_or_default();
    let test_path = format!("{}:{inherited_path}", bin_dir.path().display());

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let broker_port = find_free_port();
    let config = format!(
        r#"
default_cli = "{probe_cli}"

[specs]
type = "openspec"
dir = "specs"

[broker]
enabled = true
port = {broker_port}

[clis.{probe_cli}]
command = "{probe_cli}"
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
    let spec_id = "feature-parity";
    write_committed_spec(&repo, spec_id, "Implement feature parity.\n");

    let session_name = format!("paw-{project}");
    let entry_point = format!("/opsx:apply {spec_id}");

    let home = TempDir::new().expect("home tempdir");
    let mut start_cmd = cmd();
    tmux_env.apply_assert(&mut start_cmd);
    let out = start_cmd
        .current_dir(&repo)
        .env("PATH", &test_path)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .env("GIT_PAW_ENTRY_POINT_BUDGET_MS", "8000")
        .args(["start", "--from-specs", "--cli", probe_cli])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start --from-specs");
    assert!(
        out.status.success(),
        "spec-driven launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Pane 0 is the dashboard (broker enabled), pane 1 is the sole agent.
    let buffer = wait_for_pane_content(
        &tmux_env,
        &session_name,
        1,
        WORKTREE_RULE_TAIL,
        Duration::from_secs(8),
    );

    // Task 1.2: the task prompt must have been injected at all — the
    // pre-fix code never appended it to the boot block in the first place.
    assert!(
        buffer.contains(".git-paw/AGENTS.local.md"),
        "task prompt must point the agent at the sidecar; got:\n{buffer}"
    );

    // Task 1.1: the prompt must have been fully SUBMITTED — the probe only
    // logs a line once its terminating newline (an explicit Enter, since the
    // composed prompt itself carries none) actually arrives. The last line is
    // now the worktree rule, so its arrival proves BOTH that the prompt was
    // submitted and that the rule is where the prompt ends.
    let log_contents = wait_for_log(&log_path, WORKTREE_RULE_TAIL, Duration::from_secs(8));
    kill_session(&tmux_env, &session_name);

    assert!(
        log_contents.contains(WORKTREE_RULE_TAIL),
        "spec-driven launch must submit its composed prompt (trailing Enter) — \
         the probe's log never received the final line, meaning it is still \
         sitting unterminated in the pane's input buffer; \
         probe log:\n{log_contents}\npane buffer:\n{buffer}"
    );
    assert!(
        log_contents.contains(&repo.display().to_string()),
        "the worktree rule must name this agent's own absolute worktree root, \
         which lies under {}; probe log:\n{log_contents}",
        repo.display()
    );

    // OVERRIDE (launch-prompt-entry-point): this test used to anchor on
    // `opsx:apply <id>.` INSIDE the prose. The command left the prose (a
    // weaker model narrated it, GP-54) and is now the declared entry point,
    // submitted as its own composer line — checked by the test below. Here we
    // pin the prose free of it, since a command still in the sentence is
    // exactly the regression this change exists to prevent.
    let prose_lines: Vec<&str> = log_contents
        .lines()
        .filter(|line| line.contains(".git-paw/AGENTS.local.md"))
        .collect();
    assert!(
        !prose_lines.is_empty(),
        "the sidecar pointer must have been submitted; probe log:\n{log_contents}"
    );
    assert!(
        prose_lines.iter().all(|line| !line.contains(&entry_point)),
        "the workflow command must not ride along inside the prose; got: {prose_lines:?}"
    );
}

// ---------------------------------------------------------------------------
// launch-prompt-entry-point task 3.1 — the declared entry point arrives as a
// SECOND submitted line, and only for a format that declares one
// ---------------------------------------------------------------------------

/// Maps to `boot-block` scenario "The `OpenSpec` entry point is submitted after
/// the first message": once the pane is idle, `/opsx:apply <id>` is typed
/// literally and submitted with its own `Enter` — after the composed prompt,
/// never inside it.
///
/// Both lines are proven submitted the same way: neither the worktree rule
/// nor the entry-point template carries a trailing newline, so each reaches
/// the probe's log only if its own separate `Enter` arrived.
#[test]
#[serial]
fn openspec_launch_submits_the_entry_point_as_a_second_line() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("entrypoint");
    let repo = rename_repo_basename(&tr, &project);

    let bin_dir = TempDir::new().expect("probe bin dir");
    let log_dir = TempDir::new().expect("probe log dir");
    let log_path = log_dir.path().join("probe.log");
    let probe_cli = write_probe_cli(bin_dir.path(), &log_path);
    let inherited_path = std::env::var("PATH").unwrap_or_default();
    let test_path = format!("{}:{inherited_path}", bin_dir.path().display());

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let broker_port = find_free_port();
    let config = format!(
        r#"
default_cli = "{probe_cli}"

[specs]
type = "openspec"
dir = "specs"

[broker]
enabled = true
port = {broker_port}

[clis.{probe_cli}]
command = "{probe_cli}"
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
    let spec_id = "entry-point-change";
    write_committed_spec(&repo, spec_id, "Implement the entry point.\n");

    let session_name = format!("paw-{project}");
    let entry_point = format!("/opsx:apply {spec_id}");

    let home = TempDir::new().expect("home tempdir");
    let mut start_cmd = cmd();
    tmux_env.apply_assert(&mut start_cmd);
    let out = start_cmd
        .current_dir(&repo)
        .env("PATH", &test_path)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .env("GIT_PAW_ENTRY_POINT_BUDGET_MS", "8000")
        .args(["start", "--from-specs", "--cli", probe_cli])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start --from-specs");
    assert!(
        out.status.success(),
        "spec-driven launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let log_contents = wait_for_log(&log_path, &entry_point, Duration::from_secs(15));
    let buffer = capture_pane(&tmux_env, &session_name, 1);
    kill_session(&tmux_env, &session_name);

    assert!(
        log_contents.contains(WORKTREE_RULE_TAIL),
        "the composed prompt must be submitted first; probe log:\n{log_contents}"
    );
    assert!(
        log_contents.contains(&entry_point),
        "the entry point must be submitted as its own line (its own `Enter`); \
         stderr:\n{}\nprobe log:\n{log_contents}\npane:\n{buffer}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Order matters: the entry point is what the agent runs after reading the
    // sidecar the first message pointed it at.
    let rule_at = log_contents
        .find(WORKTREE_RULE_TAIL)
        .expect("rule line present");
    let entry_at = log_contents.find(&entry_point).expect("entry line present");
    assert!(
        rule_at < entry_at,
        "the entry point must follow the composed prompt, not precede it; \
         probe log:\n{log_contents}"
    );

    // It is its OWN line — not appended to the rule, not wrapped in prose.
    assert!(
        log_contents
            .lines()
            .any(|line| line.trim() == entry_point.as_str()),
        "the entry point must occupy a line of its own; probe log:\n{log_contents}"
    );
}

/// Maps to `boot-block` scenario "A pane whose CLI has exited is not typed
/// into".
///
/// The stub exits immediately, so after the readiness gate gives up the pane
/// is a bare login shell — which shows no input box, no permission prompt
/// and no mid-response marker, and therefore satisfies every content-based
/// idle check. Only reading the pane's foreground process distinguishes it
/// from a CLI waiting at its composer. Without that check the rendered
/// command would be handed to a command interpreter.
#[test]
#[serial]
fn a_pane_whose_cli_exited_gets_no_entry_point_typed() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("deadcli");
    let repo = rename_repo_basename(&tr, &project);

    let bin_dir = TempDir::new().expect("bin dir");
    let cli = "paw-test-exits";
    let script_path = bin_dir.path().join(cli);
    fs::write(&script_path, "#!/bin/sh\nexit 0\n").expect("write exiting stub");
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).expect("chmod stub");
    let inherited_path = std::env::var("PATH").unwrap_or_default();
    let test_path = format!("{}:{inherited_path}", bin_dir.path().display());

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let broker_port = find_free_port();
    let config = format!(
        r#"
default_cli = "{cli}"

[specs]
type = "openspec"
dir = "specs"

[broker]
enabled = true
port = {broker_port}

[clis.{cli}]
command = "{cli}"
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
    let spec_id = "dead-cli-change";
    write_committed_spec(&repo, spec_id, "Implement it.\n");

    let session_name = format!("paw-{project}");

    let home = TempDir::new().expect("home tempdir");
    let mut start_cmd = cmd();
    tmux_env.apply_assert(&mut start_cmd);
    let out = start_cmd
        .current_dir(&repo)
        .env("PATH", &test_path)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .env("GIT_PAW_ENTRY_POINT_BUDGET_MS", "8000")
        .args(["start", "--from-specs", "--cli", cli])
        .timeout(Duration::from_secs(60))
        .output()
        .expect("run start --from-specs");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    kill_session(&tmux_env, &session_name);

    assert!(
        out.status.success(),
        "a dead CLI degrades the launch, it does not fail it; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("is running a shell"),
        "the launch must report that it refused to type into a shell; \
         stderr:\n{stderr}"
    );
    assert!(
        stderr.contains(&format!("/opsx:apply {spec_id}")),
        "and still name the exact command; stderr:\n{stderr}"
    );
}

/// Maps to `boot-block` scenario "A backend without an entry point is
/// unchanged": a Markdown-backed launch declares none, so nothing further is
/// typed into its pane and the launch does not wait for it to go idle.
#[test]
#[serial]
fn markdown_launch_types_no_second_line() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("noentry");
    let repo = rename_repo_basename(&tr, &project);

    let bin_dir = TempDir::new().expect("probe bin dir");
    let log_dir = TempDir::new().expect("probe log dir");
    let log_path = log_dir.path().join("probe.log");
    let probe_cli = write_probe_cli(bin_dir.path(), &log_path);
    let inherited_path = std::env::var("PATH").unwrap_or_default();
    let test_path = format!("{}:{inherited_path}", bin_dir.path().display());

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let broker_port = find_free_port();
    let config = format!(
        r#"
default_cli = "{probe_cli}"

[specs]
type = "markdown"
dir = "specs"

[broker]
enabled = true
port = {broker_port}

[clis.{probe_cli}]
command = "{probe_cli}"
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
    write_committed_markdown_spec(&repo, "plain-feature", "Implement a plain feature.\n");

    let session_name = format!("paw-{project}");

    let home = TempDir::new().expect("home tempdir");
    let mut start_cmd = cmd();
    tmux_env.apply_assert(&mut start_cmd);
    let out = start_cmd
        .current_dir(&repo)
        .env("PATH", &test_path)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        // Deliberately short: a backend with no entry point must not wait at
        // all, so even an 800 ms budget cannot be reached by this launch.
        .env("GIT_PAW_ENTRY_POINT_BUDGET_MS", "800")
        .args(["start", "--from-specs", "--cli", probe_cli])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start --from-specs");
    assert!(
        out.status.success(),
        "markdown-backed launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let log_contents = wait_for_log(&log_path, WORKTREE_RULE_TAIL, Duration::from_secs(10));
    kill_session(&tmux_env, &session_name);

    assert!(
        log_contents.contains(WORKTREE_RULE_TAIL),
        "the composed prompt must still be submitted; probe log:\n{log_contents}"
    );
    assert!(
        !log_contents.contains("/opsx:apply"),
        "a Markdown-backed launch declares no entry point, so no second line \
         may be typed; probe log:\n{log_contents}"
    );
    assert!(
        !String::from_utf8_lossy(&out.stderr).contains("did not go idle"),
        "a pane with no entry point is never waited for, so it can never \
         expire; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---------------------------------------------------------------------------
// launch-prompt-entry-point task 3.3 — the bare pre-fill ends with the
// worktree rule and is still NOT submitted
// ---------------------------------------------------------------------------

/// Maps to `boot-block` scenario "Boot block pre-filled without Enter": bare
/// `git paw start` still leaves the prompt sitting in the composer for the
/// user to append their task to — but it now ends with the worktree rule
/// naming that pane's recorded worktree root.
///
/// The two claims are read from two different sinks, which is the whole point
/// of the probe: the rule is VISIBLE in the pane (local terminal echo shows
/// typed characters whether or not an `Enter` followed) and ABSENT from the
/// probe's log (cooked-mode line discipline delivers a line only once its
/// newline arrives, and the rule asset carries none).
#[test]
#[serial]
fn bare_pre_fill_ends_with_the_worktree_rule_and_is_not_submitted() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("prefillrule");
    let repo = rename_repo_basename(&tr, &project);

    let bin_dir = TempDir::new().expect("probe bin dir");
    let log_dir = TempDir::new().expect("probe log dir");
    let log_path = log_dir.path().join("probe.log");
    let probe_cli = write_probe_cli(bin_dir.path(), &log_path);
    let inherited_path = std::env::var("PATH").unwrap_or_default();
    let test_path = format!("{}:{inherited_path}", bin_dir.path().display());

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let broker_port = find_free_port();
    let config = format!(
        r#"
default_cli = "{probe_cli}"

[broker]
enabled = true
port = {broker_port}

[clis.{probe_cli}]
command = "{probe_cli}"
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");

    let session_name = format!("paw-{project}");

    let home = TempDir::new().expect("home tempdir");
    let mut start_cmd = cmd();
    tmux_env.apply_assert(&mut start_cmd);
    let out = start_cmd
        .current_dir(&repo)
        .env("PATH", &test_path)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args([
            "start",
            "--cli",
            probe_cli,
            "--branches",
            "feat/prefill-rule",
        ])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run bare start");
    assert!(
        out.status.success(),
        "bare launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Broker enabled -> dashboard at pane 0, the sole agent at pane 1.
    let buffer = wait_for_pane_content(
        &tmux_env,
        &session_name,
        1,
        WORKTREE_RULE_TAIL,
        Duration::from_secs(8),
    );
    // Give the probe the same grace the submission tests give it, so "absent"
    // means absent rather than not-yet-flushed.
    let log_contents = wait_for_log(&log_path, WORKTREE_RULE_TAIL, Duration::from_secs(3));
    kill_session(&tmux_env, &session_name);

    assert!(
        buffer.contains("Your worktree root is"),
        "the bare pre-fill must end with the worktree rule; pane:\n{buffer}"
    );
    assert!(
        buffer.contains(&repo.display().to_string()),
        "the rule must name this pane's own recorded worktree path, under {}; \
         pane:\n{buffer}",
        repo.display()
    );
    assert!(
        !log_contents.contains(WORKTREE_RULE_TAIL),
        "the bare pre-fill must stay UNSUBMITTED so the user can append their \
         task — the rule reaching the probe's log means an `Enter` was sent; \
         probe log:\n{log_contents}"
    );
}

// ---------------------------------------------------------------------------
// Task 1.3 — approval flags reach the worker pane command on non-supervisor
// launch paths
// ---------------------------------------------------------------------------

/// A spec-driven launch without supervisor mode SHALL build the worker
/// pane's command from the resolved approval flags.
#[test]
#[serial]
fn spec_driven_launch_applies_resolved_approval_flags() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("specflags");
    let repo = rename_repo_basename(&tr, &project);

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = format!(
        r#"
default_cli = "echo"

[specs]
type = "openspec"
dir = "specs"

[clis.echo]
command = "echo"
approval_args = {{ "auto" = "{SENTINEL_FLAG}" }}
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
    write_committed_spec(&repo, "feature-flags", "Implement feature flags.\n");

    let session_name = format!("paw-{project}");

    let home = TempDir::new().expect("home tempdir");
    let mut start_cmd = cmd();
    tmux_env.apply_assert(&mut start_cmd);
    let out = start_cmd
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--from-specs", "--cli", "echo"])
        .timeout(Duration::from_secs(15))
        .output()
        .expect("run start --from-specs");
    assert!(
        out.status.success(),
        "spec-driven launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // No broker configured (default disabled) -> agent pane is pane 0.
    let buffer = wait_for_pane_content(
        &tmux_env,
        &session_name,
        0,
        SENTINEL_FLAG,
        Duration::from_secs(8),
    );
    kill_session(&tmux_env, &session_name);

    assert!(
        buffer.contains(SENTINEL_FLAG),
        "agent pane command must carry the resolved approval flag; capture:\n{buffer}"
    );
}

/// Bare `git paw start` (no spec selection, no supervisor) SHALL also build
/// the worker pane's command from the resolved approval flags.
#[test]
#[serial]
fn bare_launch_applies_resolved_approval_flags() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("bareflags");
    let repo = rename_repo_basename(&tr, &project);

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = format!(
        r#"
[clis.echo]
command = "echo"
approval_args = {{ "auto" = "{SENTINEL_FLAG}" }}
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");

    let session_name = format!("paw-{project}");

    let home = TempDir::new().expect("home tempdir");
    let mut start_cmd = cmd();
    tmux_env.apply_assert(&mut start_cmd);
    let out = start_cmd
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--cli", "echo", "--branches", "feat/x"])
        .timeout(Duration::from_secs(15))
        .output()
        .expect("run start");
    assert!(
        out.status.success(),
        "bare launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // No broker configured -> agent pane is pane 0.
    let buffer = wait_for_pane_content(
        &tmux_env,
        &session_name,
        0,
        SENTINEL_FLAG,
        Duration::from_secs(8),
    );
    kill_session(&tmux_env, &session_name);

    assert!(
        buffer.contains(SENTINEL_FLAG),
        "agent pane command must carry the resolved approval flag; capture:\n{buffer}"
    );
}

// ---------------------------------------------------------------------------
// Task 1.4 — bare `git paw start` stays pre-fill-only (guard / non-regression)
// ---------------------------------------------------------------------------
//
// This change does not touch `cmd_start`'s boot-block injection code at all
// (only `cmd_start_with_specs` gains the submit mechanism; `cmd_start` only
// gains approval-flag resolution, exercised above). The pre-fill-only
// contract for bare `git paw start` is unit-proven at the call-shape level by
// `boot_inject_args_use_literal_flag_with_no_enter` (asserts `send-keys -l`
// with NO trailing `Enter` argument) and at the e2e content-landing level by
// `manual_mode_boot_block_lands_in_agent_pane`, both in
// `tests/boot_block_integration.rs` — neither test changed by this diff, and
// both remain green, which is the guard: had `cmd_start` been accidentally
// routed through the new submit path, the argv-shape assertion would fail
// immediately (it explicitly asserts no `"Enter"` element in the args).
//
// A live pane-content probe analogous to the spec-driven test above is not
// meaningful here: `assets/boot-block-template.md` (unlike the task-prompt
// templates) ends with its own trailing newline, so every line of the bare
// boot block — including its last — is already terminated by the time
// `tmux send-keys -l` delivers it, with or without a following `Enter`
// keystroke. A cooked-mode stub reader (`sh`, the logging probe script) cannot
// distinguish the two cases for this content; only a raw-mode paste-aware CLI
// (the real target of `cli-resolution`'s split-send contract) treats the
// embedded newlines as data rather than submission, which is exactly why
// production code never relies on a trailing newline to submit and always
// sends a separate `Enter` — see the module-level doc comment for the fuller
// explanation of why the spec-driven test above uses the logging probe
// script with task-prompt content instead.

// ---------------------------------------------------------------------------
// Task 7.2 — spec-driven launch writes the per-repo discovery file
// ---------------------------------------------------------------------------

/// `cmd_start_with_specs` was the only launch path that never called
/// `write_repo_discovery_file` (bare `cmd_start`, `cmd_supervisor`, `add`,
/// and `remove` all do), so `.git-paw/sessions/paw-<project>.json` was never
/// written for a spec-driven launch and `sweep.sh` could not discover the
/// session from the repo root. `git paw start --specs <names>` without
/// supervisor mode SHALL write that file, describing the session name and
/// agent list.
#[test]
#[serial]
fn spec_driven_launch_writes_per_repo_discovery_file() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("discovery");
    let repo = rename_repo_basename(&tr, &project);

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = r#"
default_cli = "echo"

[specs]
type = "openspec"
dir = "specs"

[clis.echo]
command = "echo"
"#;
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
    let spec_id = "feature-discovery";
    write_committed_spec(&repo, spec_id, "Implement feature discovery.\n");

    let session_name = format!("paw-{project}");

    let home = TempDir::new().expect("home tempdir");
    let mut start_cmd = cmd();
    tmux_env.apply_assert(&mut start_cmd);
    let out = start_cmd
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--from-specs", "--cli", "echo"])
        .timeout(Duration::from_secs(15))
        .output()
        .expect("run start --from-specs");
    kill_session(&tmux_env, &session_name);
    assert!(
        out.status.success(),
        "spec-driven launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let discovery_path = repo
        .join(".git-paw")
        .join("sessions")
        .join(format!("{session_name}.json"));
    assert!(
        discovery_path.exists(),
        "per-repo discovery file must exist at {discovery_path:?} after a spec-driven launch"
    );

    let contents = fs::read_to_string(&discovery_path).expect("read discovery file");
    let file: git_paw::session::RepoSessionFile =
        serde_json::from_str(&contents).expect("parse discovery file");
    assert_eq!(file.session_name, session_name);
    assert_eq!(
        file.agents.len(),
        1,
        "expected one agent; file:\n{contents}"
    );
    let agent_branch = format!("spec-{spec_id}");
    assert_eq!(
        file.agents[0].branch_id, agent_branch,
        "discovery file must name the spec-driven agent; file:\n{contents}"
    );
    assert!(
        file.orchestrator.is_none(),
        "bare-mode (non-supervisor) session must carry no orchestrator entry; file:\n{contents}"
    );
}

// ---------------------------------------------------------------------------
// `boot-block` spec — "Task prompt is injected even with the broker
// disabled" / "No boot-block injection when broker is disabled"
// ---------------------------------------------------------------------------

/// `[broker] enabled = false` with a spec-driven launch: no broker boot block
/// is injected (it is broker-specific content, correctly suppressed), but
/// the task prompt — the agent's assignment, independent of the broker —
/// SHALL still be injected and submitted.
#[test]
#[serial]
fn spec_driven_launch_without_broker_submits_task_prompt_but_no_boot_block() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("nobroker");
    let repo = rename_repo_basename(&tr, &project);

    let bin_dir = TempDir::new().expect("probe bin dir");
    let log_dir = TempDir::new().expect("probe log dir");
    let log_path = log_dir.path().join("probe.log");
    let probe_cli = write_probe_cli(bin_dir.path(), &log_path);
    let inherited_path = std::env::var("PATH").unwrap_or_default();
    let test_path = format!("{}:{inherited_path}", bin_dir.path().display());

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    // No [broker] section at all -> BrokerConfig defaults to disabled.
    let config = format!(
        r#"
default_cli = "{probe_cli}"

[specs]
type = "openspec"
dir = "specs"

[clis.{probe_cli}]
command = "{probe_cli}"
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
    let spec_id = "feature-no-broker";
    write_committed_spec(&repo, spec_id, "Implement feature no-broker.\n");

    let session_name = format!("paw-{project}");
    // OVERRIDE (launch-prompt-entry-point): the apply invocation left the
    // prose and is now the declared entry point, submitted as its own line.
    // The sidecar pointer is what identifies the task prompt in the pane.
    let anchor = ".git-paw/AGENTS.local.md".to_string();

    let home = TempDir::new().expect("home tempdir");
    let mut start_cmd = cmd();
    tmux_env.apply_assert(&mut start_cmd);
    let out = start_cmd
        .current_dir(&repo)
        .env("PATH", &test_path)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--from-specs", "--cli", probe_cli])
        .timeout(Duration::from_secs(15))
        .output()
        .expect("run start --from-specs");
    assert!(
        out.status.success(),
        "spec-driven launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // No broker -> no dashboard pane; the sole agent is pane 0.
    let buffer =
        wait_for_pane_content(&tmux_env, &session_name, 0, &anchor, Duration::from_secs(8));

    assert!(
        buffer.contains(&anchor),
        "task prompt must still be injected without a broker; got:\n{buffer}"
    );
    assert!(
        !buffer.contains("BOOT INSTRUCTIONS"),
        "no broker boot block should be injected when the broker is disabled; got:\n{buffer}"
    );
    assert!(
        !buffer.contains("REGISTER"),
        "no broker-specific REGISTER section should appear when the broker is disabled; got:\n{buffer}"
    );

    // And task 1.1's submission check applies here too: the probe's log only
    // receives the final (unterminated) line once an explicit Enter arrives.
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut log_contents = String::new();
    while Instant::now() < deadline {
        log_contents = fs::read_to_string(&log_path).unwrap_or_default();
        if log_contents.contains(&anchor) {
            break;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    kill_session(&tmux_env, &session_name);

    assert!(
        log_contents.contains(&anchor),
        "task prompt must be SUBMITTED (trailing Enter) even without a broker; \
         probe log:\n{log_contents}\npane buffer:\n{buffer}"
    );
}

// ---------------------------------------------------------------------------
// Task 9 (gate review, b387ff7) — launch-readiness gate on the start-time path
// ---------------------------------------------------------------------------
//
// Before this fix, `cmd_start_with_specs` and `cmd_start`'s boot-block
// injection ran immediately after `tmux_session.execute()`, with no call to
// `gate_pane_or_fail_on_dialog` — the readiness gate the supervisor and `add`
// paths already use. `tmux-orchestration`'s "Launch-readiness gate" / "Launch
// gate handles a first-run permission/trust acceptance dialog" requirements
// apply uniformly to every start-time pane; the tests below prove both halves
// of that: injection waits for a genuine readiness transition rather than
// firing blind, and a pane stuck on an unanswered dialog fails the launch
// loudly instead of receiving a (potentially dialog-accepting) blind `Enter`.

/// A spec-driven launch SHALL NOT inject the task prompt into a pane before
/// that pane reports readiness — proven by a stub CLI that takes a visible
/// moment to come up: a capture taken shortly after the tmux session exists
/// (but before the stub's delay elapses) must show nothing, while the
/// eventually-successful launch's final pane state shows the full prompt.
#[test]
#[serial]
fn spec_driven_launch_withholds_injection_until_pane_is_ready() {
    const MARKER: &str = "PAW_SLOW_READY_MARKER";

    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("waitready");
    let repo = rename_repo_basename(&tr, &project);

    let bin_dir = TempDir::new().expect("bin dir");
    let home = TempDir::new().expect("home tempdir");
    let ready_delay = Duration::from_secs(1);
    let probe_cli = write_slow_ready_cli(bin_dir.path(), MARKER, ready_delay);
    let inherited_path = std::env::var("PATH").unwrap_or_default();
    let test_path = format!("{}:{inherited_path}", bin_dir.path().display());

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let broker_port = find_free_port();
    let config = format!(
        r#"
default_cli = "{probe_cli}"

[specs]
type = "openspec"
dir = "specs"

[broker]
enabled = true
port = {broker_port}

[clis.{probe_cli}]
command = "{probe_cli}"

[clis.{probe_cli}.prompt_profile]
readiness_markers = ["{MARKER}"]
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
    let spec_id = "feature-wait-ready";
    write_committed_spec(&repo, spec_id, "Implement feature wait ready.\n");

    let session_name = format!("paw-{project}");
    // OVERRIDE (launch-prompt-entry-point): the apply invocation left the
    // prose and is now the declared entry point, submitted as its own line.
    // The sidecar pointer is what identifies the task prompt in the pane.
    let anchor = ".git-paw/AGENTS.local.md".to_string();

    // Generous custom budget (the suite-wide default is compressed to 120ms
    // so unrecognised-CLI tests fall back fast) so the gate genuinely waits
    // out the 1s stub delay instead of giving up and injecting blind.
    let child = spawn_start(
        &tmux_env,
        &repo,
        home.path(),
        &test_path,
        "3000",
        &["start", "--from-specs", "--cli", probe_cli],
    );

    assert!(
        wait_for_session(&tmux_env, &session_name, Duration::from_secs(5)),
        "tmux session must be created promptly"
    );

    // Poll continuously (rather than a single fixed-delay snapshot) and
    // measure wall-clock elapsed time against this test's OWN detection of
    // the session, so the assertion holds under full-suite load: if
    // everything (including our own polling) runs slower than usual, the
    // *relative* gap between "session exists" and "anchor appears" still
    // has to span a meaningful fraction of the stub's 1s pre-marker delay —
    // a fixed absolute sleep would either race ahead of a loaded scheduler
    // (false negative — never catches premature injection) or be too
    // conservative to notice it was early at all.
    let session_ready_at = Instant::now();
    let poll_deadline = session_ready_at + ready_delay + Duration::from_secs(5);
    let mut anchor_seen_at = None;
    while Instant::now() < poll_deadline {
        if capture_pane(&tmux_env, &session_name, 1).contains(&anchor) {
            anchor_seen_at = Some(Instant::now());
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let output = child.wait_with_output().expect("wait for git paw start");
    kill_session(&tmux_env, &session_name);

    match anchor_seen_at {
        Some(seen_at) => {
            let elapsed = seen_at.duration_since(session_ready_at);
            assert!(
                elapsed >= ready_delay / 2,
                "task prompt appeared only {elapsed:?} after the session was created — \
                 well under half the stub's {ready_delay:?} pre-marker delay — implying \
                 injection fired before the pane reported readiness"
            );
        }
        None => panic!(
            "task prompt never appeared within the poll window; launch output:\n{}",
            String::from_utf8_lossy(&output.stdout)
        ),
    }
    assert!(
        output.status.success(),
        "launch should succeed once the pane reports readiness; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A pane stuck on an unanswered first-run acceptance/trust dialog SHALL fail
/// the spec-driven launch loudly — the same `DialogStuck` contract the
/// supervisor path already enforces — rather than receiving a blind `Enter`
/// that could accidentally accept the dialog's default option.
#[test]
#[serial]
fn spec_driven_launch_fails_loudly_on_stuck_dialog() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("dialogspec");
    let repo = rename_repo_basename(&tr, &project);

    let bin_dir = TempDir::new().expect("bin dir");
    let dialog_cli = write_dialog_stub_cli(bin_dir.path());
    let inherited_path = std::env::var("PATH").unwrap_or_default();
    let test_path = format!("{}:{inherited_path}", bin_dir.path().display());

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let broker_port = find_free_port();
    let config = format!(
        r#"
default_cli = "{dialog_cli}"

[specs]
type = "openspec"
dir = "specs"

[broker]
enabled = true
port = {broker_port}

[clis.{dialog_cli}]
command = "{dialog_cli}"
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
    write_committed_spec(&repo, "feature-dialog", "Implement feature dialog.\n");

    let session_name = format!("paw-{project}");

    let home = TempDir::new().expect("home tempdir");
    let mut start_cmd = cmd();
    tmux_env.apply_assert(&mut start_cmd);
    let out = start_cmd
        .current_dir(&repo)
        .env("PATH", &test_path)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        // Override the suite-wide compressed timeout: a custom CLI's pane
        // can transiently look like a bare shell during startup (shell
        // init overhead) before the gate relaunches into it and the stub's
        // dialog-heading line actually renders. A short budget races that
        // relaunch cycle and can fall back to injection before the dialog
        // line ever appears; a generous one lets the gate's own retry logic
        // do its job and reliably observe the stuck dialog.
        .env("GIT_PAW_READINESS_TIMEOUT_MS", "3000")
        .args(["start", "--from-specs", "--cli", dialog_cli])
        .timeout(Duration::from_secs(15))
        .output()
        .expect("run start --from-specs");
    kill_session(&tmux_env, &session_name);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "launch must fail loudly when a pane is stuck on an unanswered dialog; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("acceptance dialog"),
        "error must name the acceptance-dialog condition; stderr:\n{stderr}"
    );

    // The session and worktrees were already created before the gate failed
    // — they must still be discoverable (`git paw stop`/`purge` need the
    // saved session; `sweep.sh` needs the discovery file), or a retry forks
    // a parallel `paw-<project>-2` session instead of reattaching.
    assert!(
        matches!(
            git_paw::session::find_session_for_repo_in(&repo, &sessions_dir_under(home.path())),
            Ok(Some(_))
        ),
        "a stuck-dialog launch must still save session state for {repo:?}"
    );
    let discovery_path = repo
        .join(".git-paw")
        .join("sessions")
        .join(format!("{session_name}.json"));
    assert!(
        discovery_path.exists(),
        "a stuck-dialog launch must still write the per-repo discovery file at {discovery_path:?}"
    );
}

/// The same `DialogStuck` contract applies to bare `git paw start` (no spec
/// selection) — it must also fail loudly rather than pre-filling a boot
/// block into a pane stuck on an unanswered dialog.
#[test]
#[serial]
fn bare_launch_fails_loudly_on_stuck_dialog() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("dialogbare");
    let repo = rename_repo_basename(&tr, &project);

    let bin_dir = TempDir::new().expect("bin dir");
    let dialog_cli = write_dialog_stub_cli(bin_dir.path());
    let inherited_path = std::env::var("PATH").unwrap_or_default();
    let test_path = format!("{}:{inherited_path}", bin_dir.path().display());

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let broker_port = find_free_port();
    let config = format!(
        r#"
[broker]
enabled = true
port = {broker_port}

[clis.{dialog_cli}]
command = "{dialog_cli}"
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");

    let session_name = format!("paw-{project}");

    let home = TempDir::new().expect("home tempdir");
    let mut start_cmd = cmd();
    tmux_env.apply_assert(&mut start_cmd);
    let out = start_cmd
        .current_dir(&repo)
        .env("PATH", &test_path)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        // See the spec-driven dialog test above for why this needs a
        // generous override rather than the suite-wide compressed default.
        .env("GIT_PAW_READINESS_TIMEOUT_MS", "3000")
        .args(["start", "--cli", dialog_cli, "--branches", "feat/x"])
        .timeout(Duration::from_secs(15))
        .output()
        .expect("run start");
    kill_session(&tmux_env, &session_name);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "bare launch must fail loudly when a pane is stuck on an unanswered dialog; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("acceptance dialog"),
        "error must name the acceptance-dialog condition; stderr:\n{stderr}"
    );

    // Same as the spec-driven test above: the session and worktree were
    // already created before the gate failed — both must still be
    // discoverable for stop/purge/sweep.sh and so a retry reattaches
    // instead of forking a parallel session.
    assert!(
        matches!(
            git_paw::session::find_session_for_repo_in(&repo, &sessions_dir_under(home.path())),
            Ok(Some(_))
        ),
        "a stuck-dialog launch must still save session state for {repo:?}"
    );
    let discovery_path = repo
        .join(".git-paw")
        .join("sessions")
        .join(format!("{session_name}.json"));
    assert!(
        discovery_path.exists(),
        "a stuck-dialog launch must still write the per-repo discovery file at {discovery_path:?}"
    );
}

// ---------------------------------------------------------------------------
// Task 11 (gate review, b387ff7) — real-launch-path coverage for scenarios
// previously asserted only against the now-unused `build_boot_inject_args`
// pure helper in `tests/from_specs_launch_fixes_integration.rs`
// ---------------------------------------------------------------------------

/// `boot-block`'s "Boot block is injected per agent pane in spec-mode-with-
/// broker" and "Pane offset accounts for dashboard" scenarios, against the
/// real launch path: three discovered specs each get their OWN boot+task
/// prompt, landing in their own pane (1, 2, 3 — dashboard at 0), not just
/// somewhere in a shared log.
#[test]
#[serial]
fn spec_driven_launch_injects_per_pane_for_multiple_specs() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("multispec");
    let repo = rename_repo_basename(&tr, &project);

    let bin_dir = TempDir::new().expect("probe bin dir");
    let log_dir = TempDir::new().expect("probe log dir");
    let log_path = log_dir.path().join("probe.log");
    let probe_cli = write_probe_cli(bin_dir.path(), &log_path);
    let inherited_path = std::env::var("PATH").unwrap_or_default();
    let test_path = format!("{}:{inherited_path}", bin_dir.path().display());

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let broker_port = find_free_port();
    let config = format!(
        r#"
default_cli = "{probe_cli}"

[specs]
type = "openspec"
dir = "specs"

[broker]
enabled = true
port = {broker_port}

[clis.{probe_cli}]
command = "{probe_cli}"
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
    let spec_ids = ["multi-a", "multi-b", "multi-c"];
    for id in spec_ids {
        write_committed_spec(&repo, id, &format!("Implement {id}.\n"));
    }

    let session_name = format!("paw-{project}");

    let home = TempDir::new().expect("home tempdir");
    let mut start_cmd = cmd();
    tmux_env.apply_assert(&mut start_cmd);
    let out = start_cmd
        .current_dir(&repo)
        .env("PATH", &test_path)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .env("GIT_PAW_ENTRY_POINT_BUDGET_MS", "10000")
        .args(["start", "--from-all-specs", "--cli", probe_cli])
        .timeout(Duration::from_secs(60))
        .output()
        .expect("run start --from-all-specs");
    assert!(
        out.status.success(),
        "multi-spec launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Dashboard at pane 0; panes 1..=3 each carry exactly one spec's own
    // task prompt. Spec-to-pane ORDER is not asserted here — discovery
    // order is not guaranteed to match the names' sort order — only that
    // each pane carries exactly one spec's content and every spec is
    // covered by exactly one pane (a bijection).
    //
    // OVERRIDE (launch-prompt-entry-point): the per-pane anchor used to be
    // the spec id inside the prose. The OpenSpec prose now names no spec at
    // all — it points at the sidecar and says the workflow command follows —
    // so every pane's prose is byte-identical and cannot distinguish them.
    // The worktree rule can: it names this agent's own worktree root, whose
    // slug is derived from the branch the spec produced.
    //
    // The anchor deliberately carries text only the worktree rule emits. A
    // bare `spec-<id>` would also match the boot block's `--agent spec-<id>`
    // and the pane's own shell prompt, both of which predate this change —
    // so it would hold with the rule removed, and prove nothing. Anchoring on
    // the path's slug IMMEDIATELY followed by the rule's next sentence pins
    // both facts at once, under either worktree placement.
    let anchor_for = |id: &str| format!("spec-{id}. Every file you create");
    let anchors: Vec<String> = spec_ids.iter().map(|id| anchor_for(id)).collect();
    let mut covered: Vec<&str> = Vec::new();
    for pane_idx in 1..=3 {
        // Wait for *some* anchor to land in this pane before reading it.
        let mut buffer = String::new();
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            buffer = capture_pane(&tmux_env, &session_name, pane_idx);
            if anchors.iter().any(|a| buffer.contains(a)) {
                break;
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        let matches: Vec<&str> = spec_ids
            .iter()
            .copied()
            .filter(|id| buffer.contains(&anchor_for(id)))
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "pane {pane_idx} must carry exactly one spec's task prompt, got {matches:?}; buffer:\n{buffer}"
        );
        covered.push(matches[0]);
    }
    covered.sort_unstable();
    let mut expected: Vec<&str> = spec_ids.to_vec();
    expected.sort_unstable();
    assert_eq!(
        covered, expected,
        "every discovered spec must land in exactly one pane"
    );

    // And each pane actually submitted (not just pre-filled) — same
    // probe-log technique as the single-spec submission test, now proving
    // it for every pane sharing one log file.
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut log_contents = String::new();
    while Instant::now() < deadline {
        log_contents = fs::read_to_string(&log_path).unwrap_or_default();
        if spec_ids
            .iter()
            .all(|id| log_contents.contains(&format!("/opsx:apply {id}")))
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    kill_session(&tmux_env, &session_name);

    for id in spec_ids {
        assert!(
            log_contents.contains(&anchor_for(id)),
            "spec '{id}'s composed prompt must have been submitted, ending in its \
             own worktree rule (trailing Enter); probe log:\n{log_contents}"
        );
        // And its entry point too: design D3 waits for every pane under ONE
        // budget, concurrently. A sequential wait would spend three budgets
        // and leave the later panes unserved.
        //
        // On a LINE OF ITS OWN — a `contains` would be satisfied by the old
        // prose, which ended `… then run /opsx:apply <id>.`
        assert!(
            log_contents
                .lines()
                .any(|line| line.trim() == format!("/opsx:apply {id}")),
            "spec '{id}'s entry point must have been submitted as its own \
             line; probe log:\n{log_contents}"
        );
    }

    // `session-state`'s "A spec-driven launch records each agent's spec id":
    // the saved session carries, per agent, the id of the spec it was launched
    // from — a bijection onto the discovered spec set. Branch naming is not
    // asserted here (it is the spec backend's concern), only that every entry
    // records a spec and that together they cover each spec exactly once.
    let session_json =
        fs::read_to_string(sessions_dir_under(home.path()).join(format!("{session_name}.json")))
            .expect("saved session file");
    let saved: serde_json::Value =
        serde_json::from_str(&session_json).expect("session file is valid JSON");
    let worktrees = saved["worktrees"]
        .as_array()
        .expect("session records a worktrees array");
    let mut recorded: Vec<&str> = worktrees
        .iter()
        .map(|wt| {
            wt["spec_id"].as_str().unwrap_or_else(|| {
                panic!("every spec-driven agent must record a spec_id; entry: {wt}")
            })
        })
        .collect();
    recorded.sort_unstable();
    assert_eq!(
        recorded, expected,
        "each agent's worktree entry must carry the spec_id it was launched from; \
         session file:\n{session_json}"
    );
}

/// `boot-block`'s "Boot-block injection failure is non-fatal" scenario: an
/// underlying `tmux` failure during injection (here, the pane itself
/// disappearing moments before the gate/submit calls reach it) SHALL NOT
/// abort the launch — the session is still saved, the discovery file is
/// still written, and the user is still guided to attach.
#[test]
#[serial]
fn spec_driven_launch_survives_a_send_keys_failure() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("sendkeysfail");
    let repo = rename_repo_basename(&tr, &project);
    let session_name = format!("paw-{project}");

    // A stub CLI that immediately kills its OWN pane (it knows the session
    // name and its own pane index — 1, broker enabled, single spec — ahead
    // of time) before the readiness gate or `submit_prompt_to_pane` ever
    // reach it, forcing every subsequent `tmux` call targeting that pane
    // (capture-pane, send-keys) to fail.
    let bin_dir = TempDir::new().expect("bin dir");
    let script =
        format!("#!/bin/sh\ntmux kill-pane -t '{session_name}:0.1' >/dev/null 2>&1 &\nsleep 3\n");
    let script_path = bin_dir.path().join("paw-test-selfkill");
    fs::write(&script_path, script).expect("write self-kill script");
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
        .expect("chmod self-kill script");
    let selfkill_cli = "paw-test-selfkill";
    let inherited_path = std::env::var("PATH").unwrap_or_default();
    let test_path = format!("{}:{inherited_path}", bin_dir.path().display());

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let broker_port = find_free_port();
    let config = format!(
        r#"
default_cli = "{selfkill_cli}"

[specs]
type = "openspec"
dir = "specs"

[broker]
enabled = true
port = {broker_port}

[clis.{selfkill_cli}]
command = "{selfkill_cli}"
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
    write_committed_spec(&repo, "feature-selfkill", "Implement feature selfkill.\n");

    let home = TempDir::new().expect("home tempdir");
    let mut start_cmd = cmd();
    tmux_env.apply_assert(&mut start_cmd);
    let out = start_cmd
        .current_dir(&repo)
        .env("PATH", &test_path)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        // The pane this stub kills can never be captured again, so it can
        // never read idle — correctly, since a capture that fails is not
        // evidence of idleness. The launch therefore waits out the whole
        // entry-point budget before reporting and moving on; bound it so
        // this test measures injection-failure tolerance rather than the
        // default 120s budget.
        .env("GIT_PAW_ENTRY_POINT_BUDGET_MS", "1000")
        .args(["start", "--from-specs", "--cli", selfkill_cli])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run start --from-specs");
    kill_session(&tmux_env, &session_name);

    assert!(
        out.status.success(),
        "a tmux failure during injection must not abort the launch; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("detached mode") || stdout.contains("Attach with"),
        "the user must still be guided to attach; stdout:\n{stdout}"
    );

    let discovery_path = repo
        .join(".git-paw")
        .join("sessions")
        .join(format!("{session_name}.json"));
    assert!(
        discovery_path.exists(),
        "the per-repo discovery file must still be written at {discovery_path:?}"
    );
}
