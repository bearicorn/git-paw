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

/// Copies the bundled sweep.sh asset into `<repo>/.git-paw/scripts/`.
fn install_sweep(repo: &Path) -> std::path::PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts/sweep.sh");
    let dst_dir = repo.join(".git-paw/scripts");
    fs::create_dir_all(&dst_dir).expect("mk scripts dir");
    let dst = dst_dir.join("sweep.sh");
    fs::copy(&src, &dst).expect("copy sweep.sh");
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
fn run_approve(repo: &Path, sweep: &Path, pane: &str) -> String {
    let out = Command::new("bash")
        .arg(sweep)
        .args(["approve", pane])
        .current_dir(repo)
        .output()
        .expect("run sweep.sh approve");
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
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

    // A live prompt: the permission-prompt footer lands in the pane tail.
    send_line(
        &session,
        2,
        "printf 'Bash command\\n  cargo test\\nDo you want to proceed? '",
    );

    let out = run_approve(repo.path(), &sweep, "2");
    assert!(
        out.contains("approved pane 2"),
        "live prompt must be approved, got: {out}"
    );
    assert!(
        !out.contains("prompt cleared"),
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
        out.contains("prompt cleared, no keys sent"),
        "cleared prompt must report no keys sent, got: {out}"
    );
    assert!(
        !out.contains("approved pane"),
        "cleared prompt must not report approved, got: {out}"
    );

    kill_session(&session);
}

/// Scenario "approve 0 is rejected": pane 0 (the supervisor's own pane) is
/// refused with no keystrokes.
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
