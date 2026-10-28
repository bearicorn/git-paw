//! A hostile CLI prompt profile cannot widen what git-paw will do.
//!
//! Spec scenarios "A profile cannot widen the live-prompt window", "A profile
//! cannot make approve select another option" and "The pane-0 refusal ignores
//! profile content" (`cli-prompt-profiles`, requirement *Security decisions
//! are never profile-sourced*).
//!
//! `[clis.<name>].prompt_profile` is operator-supplied data that describes
//! how a CLI's terminal surface LOOKS. After the classifier's retirement
//! there is no command classification for it to subvert, so the compiled set
//! it must not reach is now small and sharp:
//!
//! 1. the live-prompt gate's window sizes and structural rules — a profile
//!    may change which strings count as markers, but not how far from the
//!    end of a capture a prompt may sit and still be live;
//! 2. `sweep.sh approve`'s rule that it presses option `1` and nothing else;
//! 3. the pane-0 refusal.
//!
//! Each test drives the real surface with a deliberately adversarial
//! override rather than asserting on the type's shape, so a future change
//! that starts consulting the profile at one of these points fails here.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command as StdCommand;

use git_paw::config::{CliPromptProfileOverride, CustomCli, resolve_prompt_profile};
use git_paw::supervisor::permission_prompt::is_live_prompt;
use tempfile::TempDir;

const SESSION: &str = "paw-fake-boundary";

/// An override authored to be maximally adversarial: every marker and pattern
/// field is set to something that would, if consulted at a security point,
/// make git-paw act where it should not.
fn adversarial_override() -> CliPromptProfileOverride {
    CliPromptProfileOverride {
        // Matches essentially any line, so a capture with no modal at all
        // would read "live" if the gate trusted markers alone.
        live_prompt_markers: Some(vec![String::new(), " ".to_string(), "e".to_string()]),
        approval_markers: Some(vec![String::new()]),
        // Claims EVERY line is a numbered option, so an option-1 line could
        // be "found" anywhere.
        option_line_pattern: Some("^.*$".to_string()),
        // Claims nothing is ever a durable grant, trying to disarm the
        // approve guard.
        broad_grant_marker: Some("\u{0}never-matches\u{0}".to_string()),
        ..Default::default()
    }
}

fn clis_with_adversarial_profile() -> std::collections::HashMap<String, CustomCli> {
    let mut clis = std::collections::HashMap::new();
    clis.insert(
        "hostile".to_string(),
        CustomCli {
            command: "cat".to_string(),
            display_name: None,
            submit_delay_ms: None,
            settings_path: None,
            approval_args: std::collections::HashMap::new(),
            prompt_profile: Some(adversarial_override()),
            session_id_args: None,
            resume_args: None,
            mcp: None,
            headless_args: None,
        },
    );
    clis
}

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

fn write_session_json(repo: &Path) {
    let dir = repo.join(".git-paw/sessions");
    fs::create_dir_all(&dir).expect("mk sessions dir");
    let body = format!("{{\n  \"session_name\": \"{SESSION}\",\n  \"agents\": []\n}}");
    fs::write(dir.join(format!("{SESSION}.json")), body).expect("write session json");
}

/// Writes a `.git-paw/config.toml` whose default CLI carries the adversarial
/// profile, so `sweep.sh`'s `git paw __prompt-profile` lookups resolve it.
fn write_hostile_config(repo: &Path) {
    let paw = repo.join(".git-paw");
    fs::create_dir_all(&paw).expect("create .git-paw");
    fs::write(
        paw.join("config.toml"),
        "default_cli = \"hostile\"\n\n\
         [clis.hostile]\n\
         command = \"cat\"\n\n\
         [clis.hostile.prompt_profile]\n\
         option_line_pattern = \"^.*$\"\n\
         broad_grant_marker = \"zzz-never-matches-zzz\"\n\
         live_prompt_markers = [\"e\"]\n",
    )
    .expect("write config");
}

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
    output: String,
    tmux_log: Vec<String>,
}

impl ApproveRun {
    fn sent_keys(&self) -> Vec<&str> {
        self.tmux_log
            .iter()
            .filter(|l| l.starts_with("send-keys"))
            .map(String::as_str)
            .collect()
    }
}

/// Runs `sweep.sh approve <pane>` on `capture` under the hostile profile.
fn approve_under_hostile_profile(capture: &str, pane: &str) -> ApproveRun {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    write_session_json(repo.path());
    write_hostile_config(repo.path());
    let fakebin = install_fake_tmux(repo.path());

    let capture_file = repo.path().join("capture.txt");
    fs::write(&capture_file, capture).expect("write capture fixture");
    let log_file = repo.path().join("tmux.log");
    fs::write(&log_file, "").expect("create tmux log");

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
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .output()
        .expect("run sweep.sh approve");
    ApproveRun {
        output: format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
        tmux_log: fs::read_to_string(&log_file)
            .expect("read tmux log")
            .lines()
            .map(str::to_string)
            .collect(),
    }
}

/// Spec scenario "A profile cannot widen the live-prompt window": the tail,
/// block and trailing-bound windows are compiled constants. A profile whose
/// markers match everything still cannot make a prompt that has scrolled far
/// past the end of the capture read as live.
#[test]
fn a_profile_cannot_widen_the_live_prompt_window() {
    // The profile resolves — the override IS applied, so this is not a
    // vacuous test about an ignored field.
    let resolved = resolve_prompt_profile("hostile", &clis_with_adversarial_profile());
    assert_eq!(
        resolved.option_line_pattern, "^.*$",
        "the adversarial override must actually resolve"
    );

    // A real modal, then far more trailing output than the bounded search
    // tolerates. 30 non-blank lines is well past the effective ceiling.
    let mut capture = String::from(
        "Bash command\n  cargo test\nDo you want to proceed?\n\u{276f} 1. Yes\n  2. No\n(esc to cancel)\n",
    );
    for i in 0..30 {
        use std::fmt::Write as _;
        let _ = writeln!(capture, "output line {i}");
    }
    assert!(
        !is_live_prompt(&capture),
        "a prompt scrolled past the compiled trailing bound must not read live"
    );
}

/// Spec scenario "A profile cannot make approve select another option":
/// whatever the profile says, the helper sends `1` + `Enter` or nothing. It
/// has no code path that presses any other digit.
#[test]
fn a_profile_cannot_make_approve_select_another_option() {
    let capture = "Bash command\n  git status\nDo you want to proceed?\n\u{276f} 1. Yes\n  2. Yes, and don't ask again for git status\n  3. No\n  Esc to cancel";
    let run = approve_under_hostile_profile(capture, "2");

    for key in run.sent_keys() {
        assert!(
            key.ends_with(" 1") || key.ends_with(" Enter"),
            "only the digit 1 and Enter may ever be dispatched, got: {key}"
        );
    }
    assert!(
        !run.sent_keys().iter().any(|k| k.ends_with(" 2")),
        "the durable-grant digit must never be pressed; log: {:?}",
        run.tmux_log
    );
    assert!(
        !run.tmux_log.iter().any(|l| l.contains(" Down")),
        "no blind cursor movement; log: {:?}",
        run.tmux_log
    );
}

/// A profile that claims nothing is ever a durable grant must NOT be able to
/// disarm the option-1 guard.
///
/// This test previously asserted only that the keys sent were `1`/`Enter` —
/// which the unsafe behaviour also satisfies, since pressing the durable
/// grant IS pressing `1`. It therefore passed while the guard was disarmed,
/// under a name claiming the opposite. It now asserts what the name says:
/// the helper must REFUSE.
///
/// The guard holds because a compiled fallback marker is always checked
/// alongside the profile-sourced one, so a profile can widen what counts as a
/// durable grant but never shrink it.
#[test]
fn a_profile_cannot_disarm_the_durable_grant_guard() {
    let capture = "Bash command\n  git status\nDo you want to proceed?\n\u{276f} 1. Yes, and don't ask again for git status\n  2. No\n  Esc to cancel";
    let run = approve_under_hostile_profile(capture, "2");
    assert!(
        run.sent_keys().is_empty(),
        "a narrowed broad_grant_marker must not disarm the guard; keys sent: {:?}",
        run.tmux_log
    );
    assert!(
        run.output.contains("durable grant"),
        "the helper must report why it refused, got: {}",
        run.output
    );
}

/// Regression: a wrapped multi-option modal followed by trailing chrome can
/// push option 1 beyond any fixed-size tail window, while the live gate still
/// (correctly) reports the prompt LIVE — it tolerates up to 13 trailing
/// non-blank lines.
///
/// The first implementation of the guard searched a fixed `tail -15` window
/// for the first line matching the ANY-digit option pattern. On this capture
/// that window begins below option 1, so the guard inspected `3. No`, found
/// no durable-grant wording, and pressed `1` — which here IS the durable
/// grant. Exactly the outcome the guard exists to prevent.
#[test]
fn a_long_modal_does_not_push_option_one_out_of_the_guards_view() {
    let mut capture = String::from(
        "Bash command\n  git status\nDo you want to proceed?\n         1. Yes, and don't ask again for\n   git status in this project\n         2. Yes\n3. No\n(esc to cancel)\n",
    );
    for i in 0..13 {
        use std::fmt::Write as _;
        let _ = writeln!(capture, "chrome line {i}");
    }
    let run = approve_under_hostile_profile(&capture, "2");
    assert!(
        run.sent_keys().is_empty(),
        "option 1 is the durable grant and must be seen however long the modal is; \
         keys sent: {:?}",
        run.tmux_log
    );
    assert!(
        run.output.contains("durable grant"),
        "the helper must report why it refused, got: {}",
        run.output
    );
}

/// Spec scenario "The pane-0 refusal ignores profile content": pane 0 is
/// refused before any capture is taken, so no profile field is even consulted.
#[test]
fn the_pane_zero_refusal_ignores_profile_content() {
    let capture = "Bash command\n  git status\nDo you want to proceed?\n\u{276f} 1. Yes\n  2. No\n  Esc to cancel";
    let run = approve_under_hostile_profile(capture, "0");
    assert!(
        run.sent_keys().is_empty(),
        "pane 0 must receive no keystroke whatever the profile says; log: {:?}",
        run.tmux_log
    );
    assert!(
        run.output.contains("pane 0 excluded"),
        "the helper must report the pane-0 exclusion, got: {}",
        run.output
    );
}
