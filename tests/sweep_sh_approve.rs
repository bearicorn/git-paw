//! E2E tests for the bundled `sweep.sh approve <pane>` subcommand's
//! broker-mediated-approvals re-confirm gate (capability
//! `stuck-prompt-detection`).
//!
//! Each test installs the bundled `assets/scripts/sweep.sh`, stands up a real
//! detached tmux session on a test-owned socket, lays a synthetic prompt into
//! a coding-agent pane, and drives `sweep.sh approve` — asserting it sends
//! keys only on a live prompt, sends nothing when the prompt cleared, and
//! refuses pane 0.
//!
//! The tests are `#[serial]` because they mutate the process env (the tmux
//! socket) via `helpers::tmux_test_env().apply_to_process()`, matching
//! `auto_approve_integration.rs`. tmux is a hard dependency, so they run
//! normally rather than behind `#[ignore]`.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serial_test::serial;
use tempfile::TempDir;

mod helpers;

fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn init_git_repo(dir: &Path) {
    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git command");
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "t@e.st"]);
    run(&["config", "user.name", "Test"]);
    fs::write(dir.join("README.md"), "x").expect("readme");
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "init"]);
}

/// Copies the bundled sweep.sh asset — together with the `_paw_common.sh`
/// preamble it sources — into `<repo>/.git-paw/scripts/`, mirroring the
/// co-deployment `git paw init` guarantees.
fn install_sweep(repo: &Path) -> std::path::PathBuf {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts");
    let dst_dir = repo.join(".git-paw/scripts");
    fs::create_dir_all(&dst_dir).expect("mk scripts dir");
    fs::copy(
        assets.join("_paw_common.sh"),
        dst_dir.join("_paw_common.sh"),
    )
    .expect("copy _paw_common.sh");
    let dst = dst_dir.join("sweep.sh");
    fs::copy(assets.join("sweep.sh"), &dst).expect("copy sweep.sh");
    dst
}

/// Writes the per-repo discovery JSON so sweep.sh resolves `session_name`
/// without needing `$TMUX`.
fn write_session_json(repo: &Path, session_name: &str) {
    let dir = repo.join(".git-paw/sessions");
    fs::create_dir_all(&dir).expect("mk sessions dir");
    let body = format!("{{\n  \"session_name\": \"{session_name}\",\n  \"agents\": []\n}}");
    fs::write(dir.join(format!("{session_name}.json")), body).expect("write session json");
}

fn unique_session_name(tag: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("paw-apr-{tag}-{nanos}")
}

fn tmux(args: &[&str]) -> std::process::Output {
    Command::new("tmux").args(args).output().expect("tmux")
}

fn kill_session(name: &str) {
    let _ = tmux(&["kill-session", "-t", name]);
}

/// Creates a detached session and splits window 0 so panes `0..=2` exist,
/// each running a long-lived `sh`.
fn create_session_with_panes(name: &str) {
    let st = tmux(&[
        "new-session",
        "-d",
        "-s",
        name,
        "-x",
        "200",
        "-y",
        "50",
        "sh",
    ]);
    assert!(st.status.success(), "tmux new-session failed");
    let target = format!("{name}:0");
    for _ in 0..2 {
        let st = tmux(&["split-window", "-t", &target, "sh"]);
        assert!(st.status.success(), "tmux split-window failed");
    }
    std::thread::sleep(Duration::from_millis(200));
}

/// Sends `line` (a shell command) to pane `pane` and waits for it to render.
fn send_line(session: &str, pane: usize, line: &str) {
    let target = format!("{session}:0.{pane}");
    let st = tmux(&["send-keys", "-t", &target, line, "Enter"]);
    assert!(st.status.success(), "send-keys failed");
    std::thread::sleep(Duration::from_millis(200));
}

/// Runs `sweep.sh approve <pane>` from the repo and returns combined
/// stdout+stderr.
///
/// The helper reads the durable-grant marker from `git paw __prompt-profile`,
/// so the freshly built binary is prepended to `PATH` for the child; without
/// it the helper falls back to its built-in marker, which is a different
/// scenario from the ones here.
fn run_approve(repo: &Path, sweep: &Path, pane: &str) -> String {
    run_approve_with_status(repo, sweep, pane).1
}

/// [`run_approve`], also returning the helper's exit code.
fn run_approve_with_status(repo: &Path, sweep: &Path, pane: &str) -> (Option<i32>, String) {
    let bin = assert_cmd::cargo::cargo_bin("git-paw");
    let bin_dir = bin.parent().expect("built binary has a parent dir");
    let path = std::env::var("PATH").unwrap_or_default();
    let out = Command::new("bash")
        .arg(sweep)
        .args(["approve", pane])
        .current_dir(repo)
        .env("PATH", format!("{}:{path}", bin_dir.display()))
        .output()
        .expect("run sweep.sh approve");
    (
        out.status.code(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// Scenario "approve sends keys only when the prompt is still live": a
/// coding-agent pane whose tail shows a permission-prompt marker is approved.
#[test]
#[serial]
fn approve_sends_keys_on_live_prompt() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let env = helpers::tmux_test_env();
    let _proc = env.apply_to_process();

    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    let session = unique_session_name("live");
    write_session_json(repo.path(), &session);
    create_session_with_panes(&session);

    // A live prompt: the permission-prompt footer AND the numbered option
    // list land in the pane tail. The option list is part of the fixture
    // because `approve` now confirms option 1 is really on screen before
    // pressing it — a question with no options is not a modal it can answer.
    send_line(
        &session,
        2,
        "printf 'Bash command\\n  cargo test\\nDo you want to proceed?\\n 1. Yes\\n 2. No\\n Esc to cancel '",
    );

    let out = run_approve(repo.path(), &sweep, "2");
    assert!(
        out.contains("approved pane 2 (option 1)"),
        "live prompt must be answered once with option 1, got: {out}"
    );
    assert!(
        !out.contains("cleared before send"),
        "live prompt must not report cleared, got: {out}"
    );

    kill_session(&session);
}

/// Scenario "approve sends nothing when the prompt has cleared": a pane whose
/// tail carries no permission-prompt marker receives no keystrokes.
#[test]
#[serial]
fn approve_sends_nothing_when_prompt_cleared() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let env = helpers::tmux_test_env();
    let _proc = env.apply_to_process();

    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    let session = unique_session_name("cleared");
    write_session_json(repo.path(), &session);
    create_session_with_panes(&session);

    // No permission-prompt marker in the tail — the prompt has cleared.
    send_line(&session, 2, "printf 'build finished ok\\n'");

    let out = run_approve(repo.path(), &sweep, "2");
    assert!(
        out.contains("cleared before send, no keys sent"),
        "cleared prompt must report cleared before send, got: {out}"
    );
    assert!(
        !out.contains("approved pane"),
        "cleared prompt must not report approved, got: {out}"
    );

    kill_session(&session);
}

/// Scenario "Without a roster pane 0 is still refused": with no roster entry
/// to check against, pane 0 is refused with no keystrokes.
#[test]
#[serial]
fn approve_refuses_pane_zero() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let env = helpers::tmux_test_env();
    let _proc = env.apply_to_process();

    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    let session = unique_session_name("pane0");
    write_session_json(repo.path(), &session);
    create_session_with_panes(&session);

    // Even with a live prompt in pane 0, approve must refuse it.
    send_line(&session, 0, "printf 'Do you want to proceed? '");

    let out = run_approve(repo.path(), &sweep, "0");
    assert!(
        out.contains("pane 0 excluded from blind send-keys"),
        "approve 0 must report pane 0 excluded, got: {out}"
    );
    assert!(
        !out.contains("approved pane"),
        "approve 0 must not approve, got: {out}"
    );

    kill_session(&session);
}

/// Writes a discovery JSON whose roster lists one coding agent per entry of
/// `panes`, at that `pane_index`.
fn write_session_json_with_roster(repo: &Path, session_name: &str, panes: &[usize]) {
    let dir = repo.join(".git-paw/sessions");
    fs::create_dir_all(&dir).expect("mk sessions dir");
    let agents: Vec<String> = panes
        .iter()
        .map(|p| {
            format!(
                "{{\"branch_id\": \"feat-a{p}\", \"worktree_path\": \"/tmp/wt{p}\", \
                 \"cli\": \"claude\", \"pane_index\": {p}}}"
            )
        })
        .collect();
    let body = format!(
        "{{\"session_name\": \"{session_name}\", \"agents\": [{}]}}",
        agents.join(", ")
    );
    fs::write(dir.join(format!("{session_name}.json")), body).expect("write session json");
}

/// A live one-time-yes permission prompt, as `sh` renders it from `printf`.
const LIVE_PROMPT_LINE: &str = "printf 'Bash command\\n  cargo test\\nDo you want to proceed?\\n 1. Yes\\n 2. No\\n Esc to cancel '";

/// Scenarios "approve of the dashboard pane is rejected"
/// (`broker-agent-helper`) and "Dashboard surface is refused in a bare
/// session" (`approval-auto`): with a roster listing coding agents at panes
/// 1 and 2, pane 0 is refused by role — even showing a live prompt.
#[test]
#[serial]
fn approve_refuses_the_dashboard_pane_by_roster() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let env = helpers::tmux_test_env();
    let _proc = env.apply_to_process();

    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    let session = unique_session_name("dash");
    write_session_json_with_roster(repo.path(), &session, &[1, 2]);
    create_session_with_panes(&session);
    send_line(&session, 0, LIVE_PROMPT_LINE);

    let out = run_approve(repo.path(), &sweep, "0");
    assert!(
        out.contains("pane 0 excluded from blind send-keys (not a coding agent)"),
        "the dashboard must be refused as not a coding agent, got: {out}"
    );
    assert!(!out.contains("approved pane"), "got: {out}");

    kill_session(&session);
}

/// Scenario "approve of a pane no roster entry claims is rejected".
#[test]
#[serial]
fn approve_refuses_a_pane_no_roster_entry_claims() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let env = helpers::tmux_test_env();
    let _proc = env.apply_to_process();

    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    let session = unique_session_name("unclaimed");
    write_session_json_with_roster(repo.path(), &session, &[1]);
    create_session_with_panes(&session);
    send_line(&session, 2, LIVE_PROMPT_LINE);

    let out = run_approve(repo.path(), &sweep, "2");
    assert!(
        out.contains("pane 2 excluded from blind send-keys (not a coding agent)"),
        "a pane no roster entry claims must be refused, got: {out}"
    );
    assert!(!out.contains("approved pane"), "got: {out}");

    kill_session(&session);
}

/// Scenario "a non-numeric pane argument is refused": `.` and `[0-2]` would
/// match a roster index as a pattern and resolve to the active pane in tmux,
/// so both are refused with a non-zero exit and every pane — each showing a
/// live prompt — is left untouched.
#[test]
#[serial]
fn approve_refuses_a_non_numeric_pane() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let env = helpers::tmux_test_env();
    let _proc = env.apply_to_process();

    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    let session = unique_session_name("nonnum");
    write_session_json_with_roster(repo.path(), &session, &[1, 2]);
    create_session_with_panes(&session);
    for pane in 0..=2 {
        send_line(&session, pane, LIVE_PROMPT_LINE);
    }
    let capture = |pane: usize| {
        let target = format!("{session}:0.{pane}");
        String::from_utf8_lossy(&tmux(&["capture-pane", "-p", "-t", &target]).stdout).into_owned()
    };
    let before: Vec<String> = (0..=2).map(capture).collect();

    for arg in [".", "[0-2]"] {
        let (code, out) = run_approve_with_status(repo.path(), &sweep, arg);
        assert_ne!(
            code,
            Some(0),
            "approve {arg} must exit non-zero, got: {out}"
        );
        assert!(
            out.contains("approve takes a numeric pane index, no keys sent"),
            "approve {arg} must be refused as non-numeric, got: {out}"
        );
        assert!(!out.contains("approved pane"), "got: {out}");
    }

    std::thread::sleep(Duration::from_millis(200));
    let after: Vec<String> = (0..=2).map(capture).collect();
    kill_session(&session);
    assert_eq!(before, after, "no pane may receive a keystroke");
}

/// Scenario "approve of a roster agent's pane proceeds": a roster agent at
/// pane 1 showing a live one-time-yes prompt is answered with option 1.
#[test]
#[serial]
fn approve_of_a_roster_agents_pane_proceeds() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let env = helpers::tmux_test_env();
    let _proc = env.apply_to_process();

    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    let session = unique_session_name("roster");
    write_session_json_with_roster(repo.path(), &session, &[1, 2]);
    create_session_with_panes(&session);
    send_line(&session, 1, LIVE_PROMPT_LINE);

    let out = run_approve(repo.path(), &sweep, "1");
    assert!(
        out.contains("approved pane 1 (option 1)"),
        "a roster agent's live prompt must be answered, got: {out}"
    );

    kill_session(&session);
}

/// Scenario "The first agent of a broker-less session is approvable"
/// (`approval-auto`): with no dashboard, the roster lists the first coding
/// agent at pane 0, and a human's `approve 0` reaches it — eligibility is
/// the role, never the index.
#[test]
#[serial]
fn approve_reaches_the_first_agent_of_a_broker_less_session() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let env = helpers::tmux_test_env();
    let _proc = env.apply_to_process();

    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    let session = unique_session_name("nobroker");
    write_session_json_with_roster(repo.path(), &session, &[0, 1]);
    create_session_with_panes(&session);
    send_line(&session, 0, LIVE_PROMPT_LINE);

    let out = run_approve(repo.path(), &sweep, "0");
    assert!(
        out.contains("approved pane 0 (option 1)"),
        "the broker-less first agent at pane 0 must be approvable, got: {out}"
    );

    kill_session(&session);
}
