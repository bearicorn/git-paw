//! Key-sequence tests for `sweep.sh approve`'s option-index selection
//! (capability `automatic-approval`, requirement "Option-index selection for
//! Yes/No prompts", change `approve-send-gate-hardening`).
//!
//! A recording fake `tmux` on `$PATH` serves a scripted pane capture and logs
//! every invocation, so the tests assert the EXACT keystrokes dispatched:
//! the resolved option digit followed by `Enter`, never a blind `Down`. The
//! suite is hermetic — no real tmux server, no broker.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command as StdCommand;

use tempfile::TempDir;

const SESSION: &str = "paw-fake-approve";

fn init_git_repo(dir: &Path) {
    let run = |args: &[&str]| {
        StdCommand::new("git")
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
/// without `$TMUX`.
fn write_session_json(repo: &Path) {
    let dir = repo.join(".git-paw/sessions");
    fs::create_dir_all(&dir).expect("mk sessions dir");
    let body = format!("{{\n  \"session_name\": \"{SESSION}\",\n  \"agents\": []\n}}");
    fs::write(dir.join(format!("{SESSION}.json")), body).expect("write session json");
}

/// Installs the recording fake `tmux` into `<repo>/fakebin/`: every
/// invocation appends its arguments to `$FAKE_TMUX_LOG`, and `capture-pane`
/// prints the scripted capture from `$FAKE_TMUX_CAPTURE`.
fn install_fake_tmux(repo: &Path) -> std::path::PathBuf {
    let bin = repo.join("fakebin");
    fs::create_dir_all(&bin).expect("mk fakebin");
    let tmux = bin.join("tmux");
    fs::write(
        &tmux,
        "#!/bin/sh\n\
         echo \"$*\" >> \"${FAKE_TMUX_LOG}\"\n\
         case \"$1\" in\n\
           capture-pane) cat \"${FAKE_TMUX_CAPTURE}\" ;;\n\
         esac\n\
         exit 0\n",
    )
    .expect("write fake tmux");
    let mut perms = fs::metadata(&tmux).expect("stat fake tmux").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&tmux, perms).expect("chmod fake tmux");
    bin
}

struct ApproveRun {
    /// Combined stdout+stderr of `sweep.sh approve`.
    output: String,
    /// One line per fake-tmux invocation (its space-joined arguments).
    tmux_log: Vec<String>,
}

impl ApproveRun {
    /// The `send-keys` log lines, i.e. the keystrokes actually dispatched.
    fn sent_keys(&self) -> Vec<&str> {
        self.tmux_log
            .iter()
            .filter(|l| l.starts_with("send-keys"))
            .map(String::as_str)
            .collect()
    }
}

/// Lays `capture` behind the fake tmux and runs `sweep.sh approve <pane>`.
fn run_approve(capture: &str, pane: &str) -> ApproveRun {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    write_session_json(repo.path());
    let fakebin = install_fake_tmux(repo.path());

    let capture_file = repo.path().join("capture.txt");
    fs::write(&capture_file, capture).expect("write capture fixture");
    let log_file = repo.path().join("tmux.log");
    fs::write(&log_file, "").expect("create tmux log");

    let path = format!(
        "{}:{}",
        fakebin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = StdCommand::new("bash")
        .arg(&sweep)
        .args(["approve", pane])
        .current_dir(repo.path())
        .env("PATH", path)
        .env("FAKE_TMUX_LOG", &log_file)
        .env("FAKE_TMUX_CAPTURE", &capture_file)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .output()
        .expect("run sweep.sh approve");
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let tmux_log = fs::read_to_string(&log_file)
        .expect("read tmux log")
        .lines()
        .map(str::to_string)
        .collect();
    ApproveRun { output, tmux_log }
}

/// Spec scenario "Sweep helper approves a 2-option prompt affirmatively":
/// on a live 2-option Yes/No prompt the helper sends the digit `1` then
/// `Enter` — and never a blind `Down`.
#[test]
fn two_option_prompt_sends_digit_one_then_enter() {
    let capture =
        "Bash command\n  git status\nDo you want to proceed?\n❯ 1. Yes\n  2. No\n  Esc to cancel";
    let run = run_approve(capture, "2");
    assert!(
        run.output.contains("approved pane 2 (option 1)"),
        "2-option prompt must approve with option 1, got: {}",
        run.output
    );
    assert_eq!(
        run.sent_keys(),
        vec![
            format!("send-keys -t {SESSION}:0.2 1"),
            format!("send-keys -t {SESSION}:0.2 Enter"),
        ],
        "the helper must dispatch the digit and Enter as two separate keystrokes"
    );
    assert!(
        !run.tmux_log.iter().any(|l| l.contains(" Down")),
        "the helper must never dispatch a blind Down, log: {:?}",
        run.tmux_log
    );
}

/// Spec scenario "Sweep helper respects the broad-grant rule on 3-option
/// prompts": an arbitrary-code runner takes the one-time Yes (option 1),
/// never the permanent broad grant (option 2).
#[test]
fn three_option_arbitrary_code_prompt_sends_one_time_yes() {
    let capture = "Bash command\n  bash -c \"do-thing\"\nDo you want to proceed?\n❯ 1. Yes\n  2. Yes, and don't ask again for bash commands in this project\n  3. No, and tell Claude what to do differently (esc)";
    let run = run_approve(capture, "3");
    assert!(
        run.output.contains("approved pane 3 (option 1)"),
        "an arbitrary-code runner must take the one-time Yes, got: {}",
        run.output
    );
    assert_eq!(
        run.sent_keys(),
        vec![
            format!("send-keys -t {SESSION}:0.3 1"),
            format!("send-keys -t {SESSION}:0.3 Enter"),
        ],
        "option 1 (one-time Yes), never the broad grant"
    );
}

/// Complement to the broad-grant rule: an allowlisted read-mostly verb on a
/// 3-option prompt takes option 2 (Yes, and don't ask again), agreeing with
/// the in-tool auto-approver's `select_option_index`.
#[test]
fn three_option_allowlisted_verb_takes_broad_grant() {
    let capture = "Bash command\n  git status\nDo you want to proceed?\n❯ 1. Yes\n  2. Yes, and don't ask again for git status in this project\n  3. No, and tell Claude what to do differently (esc)";
    let run = run_approve(capture, "2");
    assert!(
        run.output.contains("approved pane 2 (option 2)"),
        "an allowlisted verb must take the broad grant, got: {}",
        run.output
    );
    assert_eq!(
        run.sent_keys(),
        vec![
            format!("send-keys -t {SESSION}:0.2 2"),
            format!("send-keys -t {SESSION}:0.2 Enter"),
        ]
    );
}

/// Spec scenario "Prompt cleared before send sends nothing" (shell half): a
/// capture with no live prompt yields zero `send-keys` invocations.
#[test]
fn cleared_prompt_sends_zero_keystrokes() {
    let capture = "$ git status\nnothing to commit, working tree clean\n$ ";
    let run = run_approve(capture, "2");
    assert!(
        run.output
            .contains("cleared before send, no keys sent (pane 2)"),
        "cleared prompt must report cleared before send, got: {}",
        run.output
    );
    assert!(
        run.sent_keys().is_empty(),
        "cleared prompt must dispatch zero keystrokes, log: {:?}",
        run.tmux_log
    );
}
