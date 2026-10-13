//! Key-sequence tests for `sweep.sh approve` (capability `approval-auto`,
//! requirement "Approval keystrokes are sent only by an explicit approve
//! action"; capability `broker-agent-helper`, requirement "sweep.sh approve
//! re-confirms a live prompt before sending keys").
//!
//! A recording fake `tmux` on `$PATH` serves a scripted pane capture and logs
//! every invocation, so the tests assert the EXACT keystrokes dispatched:
//! always the digit `1` followed by `Enter`, never a blind `Down` and never a
//! durable "don't ask again" option. A recording `git` shim proves the helper
//! consults no classifier. The suite is hermetic — no real tmux server, no
//! broker.

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
/// without `$TMUX`.
fn write_session_json(repo: &Path) {
    let dir = repo.join(".git-paw/sessions");
    fs::create_dir_all(&dir).expect("mk sessions dir");
    let body = format!("{{\n  \"session_name\": \"{SESSION}\",\n  \"agents\": []\n}}");
    fs::write(dir.join(format!("{SESSION}.json")), body).expect("write session json");
}

/// Makes `path` executable.
fn chmod_exec(path: &Path) {
    let mut perms = fs::metadata(path).expect("stat shim").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod shim");
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
    chmod_exec(&tmux);
    bin
}

/// Installs a recording `git` shim into `fakebin` that appends every
/// invocation's arguments to `$FAKE_GIT_LOG` and then delegates to the real
/// `git`, so the helper still resolves its repository root and prompt profile
/// normally while the test can prove which subcommands it invoked.
fn install_recording_git(fakebin: &Path) {
    let real_git = StdCommand::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|p| !p.is_empty())
        .expect("a real git on PATH");
    let shim = fakebin.join("git");
    fs::write(
        &shim,
        format!(
            "#!/bin/sh\n\
             echo \"$*\" >> \"${{FAKE_GIT_LOG}}\"\n\
             exec {real_git} \"$@\"\n"
        ),
    )
    .expect("write git shim");
    chmod_exec(&shim);
}

struct ApproveRun {
    /// Combined stdout+stderr of `sweep.sh approve`.
    output: String,
    /// One line per fake-tmux invocation (its space-joined arguments).
    tmux_log: Vec<String>,
    /// One line per recording-`git` invocation, when the shim was installed.
    git_log: Vec<String>,
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
/// With `record_git`, a recording `git` shim also precedes the real one on
/// `$PATH`.
fn run_approve_inner(capture: &str, pane: &str, record_git: bool) -> ApproveRun {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    write_session_json(repo.path());
    let fakebin = install_fake_tmux(repo.path());
    if record_git {
        install_recording_git(&fakebin);
    }

    let capture_file = repo.path().join("capture.txt");
    fs::write(&capture_file, capture).expect("write capture fixture");
    let log_file = repo.path().join("tmux.log");
    fs::write(&log_file, "").expect("create tmux log");
    let git_log_file = repo.path().join("git.log");
    fs::write(&git_log_file, "").expect("create git log");

    // Fake tmux (and the recording git shim) first, then the freshly built
    // binary: `sweep.sh approve` resolves the durable-grant marker through
    // `git paw __prompt-profile`, so THIS build must be the `git-paw` that
    // `git paw` finds.
    let path = format!(
        "{}:{}:{}",
        fakebin.display(),
        assert_cmd::cargo::cargo_bin("git-paw")
            .parent()
            .expect("built binary has a parent dir")
            .display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = StdCommand::new("bash")
        .arg(&sweep)
        .args(["approve", pane])
        .current_dir(repo.path())
        .env("PATH", path)
        .env("FAKE_TMUX_LOG", &log_file)
        .env("FAKE_TMUX_CAPTURE", &capture_file)
        .env("FAKE_GIT_LOG", &git_log_file)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .output()
        .expect("run sweep.sh approve");
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let read_lines = |p: &Path| -> Vec<String> {
        fs::read_to_string(p)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    };
    ApproveRun {
        output,
        tmux_log: read_lines(&log_file),
        git_log: read_lines(&git_log_file),
    }
}

fn run_approve(capture: &str, pane: &str) -> ApproveRun {
    run_approve_inner(capture, pane, false)
}

/// Spec scenario "approve sends 1 then Enter when the prompt is still live":
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

/// Spec scenario "approve answers once on a three-option prompt": a prompt
/// offering a durable "don't ask again" option is answered with the one-time
/// `1`, never the durable option `2`.
#[test]
fn three_option_prompt_answers_once_with_option_one() {
    let capture = "Bash command\n  git status\nDo you want to proceed?\n❯ 1. Yes\n  2. Yes, and don't ask again for git status in this project\n  3. No, and tell Claude what to do differently (esc)\n  Esc to cancel";
    let run = run_approve(capture, "2");
    assert!(
        run.output.contains("approved pane 2 (option 1)"),
        "a three-option prompt must still be answered once, got: {}",
        run.output
    );
    assert_eq!(
        run.sent_keys(),
        vec![
            format!("send-keys -t {SESSION}:0.2 1"),
            format!("send-keys -t {SESSION}:0.2 Enter"),
        ],
        "option 1 (one-time Yes), never the durable grant"
    );
    assert!(
        !run.sent_keys().iter().any(|l| l.ends_with(" 2")),
        "the helper must never dispatch the durable-grant digit, log: {:?}",
        run.tmux_log
    );
}

/// Spec scenario "approve does not judge the command": a command the retired
/// classifier would have escalated is answered exactly like any other, and no
/// `git paw __classify` subprocess is invoked.
#[test]
fn approve_does_not_judge_the_command() {
    let capture = "Bash command\n  git push --force origin main\nDo you want to proceed?\n❯ 1. Yes\n  2. No\n  Esc to cancel";
    let run = run_approve_inner(capture, "2", true);
    assert!(
        run.output.contains("approved pane 2 (option 1)"),
        "approve must not judge the command, got: {}",
        run.output
    );
    assert_eq!(
        run.sent_keys(),
        vec![
            format!("send-keys -t {SESSION}:0.2 1"),
            format!("send-keys -t {SESSION}:0.2 Enter"),
        ],
    );
    assert!(
        !run.git_log.is_empty(),
        "the recording git shim must have been exercised at all, log: {:?}",
        run.git_log
    );
    assert!(
        !run.git_log.iter().any(|l| l.contains("__classify")),
        "approve must consult no classifier, git invocations: {:?}",
        run.git_log
    );
}

/// Spec scenario "approve refuses an option 1 that reads as a durable grant":
/// when the first numbered option itself carries the broad-grant marker,
/// pressing `1` would write a durable permission rule — the helper sends
/// nothing and says why.
#[test]
fn broad_grant_in_option_one_sends_nothing() {
    let capture = "Bash command\n  git status\nDo you want to proceed?\n❯ 1. Yes, and don't ask again for git status in this project\n  2. No, and tell Claude what to do differently (esc)\n  Esc to cancel";
    let run = run_approve(capture, "2");
    assert!(
        run.sent_keys().is_empty(),
        "an option 1 that reads as a durable grant must dispatch zero keystrokes, log: {:?}",
        run.tmux_log
    );
    assert!(
        run.output.contains("durable grant"),
        "the helper must report why it refused, got: {}",
        run.output
    );
    assert!(
        !run.output.contains("approved pane"),
        "nothing must be reported as approved, got: {}",
        run.output
    );
}

/// Spec scenario "approve sends nothing when the prompt has cleared" (shell
/// half): a capture with no live prompt yields zero `send-keys` invocations.
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

/// Spec scenario "No classifier subcommand exists" (shell half,
/// `approval-command-safety`): `sweep.sh classify` is GONE, not merely inert.
/// It must be rejected as an unknown subcommand with exit 2, so a script that
/// still calls it breaks loudly.
#[test]
fn classify_subcommand_is_rejected_as_unknown() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    write_session_json(repo.path());

    let out = StdCommand::new("bash")
        .arg(&sweep)
        .arg("classify")
        .current_dir(repo.path())
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .output()
        .expect("run sweep.sh classify");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "sweep.sh classify must exit 2 as an unknown subcommand, got: {combined}"
    );
    assert!(
        combined.contains("unknown subcommand"),
        "the helper must name it as unknown, got: {combined}"
    );
}
