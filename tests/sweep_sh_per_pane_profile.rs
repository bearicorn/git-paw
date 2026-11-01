//! `sweep.sh` reads each pane with the profile of the CLI the roster records
//! for it (`cli-prompt-profiles`, requirement *The helper script resolves the
//! profile per pane*).
//!
//! A two-CLI roster — `claude` on pane 1, `mycli` on pane 2 with a
//! `prompt_profile` override — and a recording fake `tmux` that serves the
//! same capture for every pane. Whatever differs between the two panes'
//! outcomes can only come from the per-pane profile. Hermetic: no tmux
//! server, no broker.

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command as StdCommand;

use git_paw::agent_cli::CliProfile;
use git_paw::config::{CliPromptProfileOverride, CustomCli};
use git_paw::supervisor::permission_prompt::{has_approval_marker, is_live_prompt};
use tempfile::TempDir;

const SESSION: &str = "paw-fake-per-pane";

/// The `mycli` override: its own live-prompt and approval wording and
/// durable-grant marker.
const MYCLI_PROFILE: &str = "[clis.mycli]\ncommand = \"mycli\"\n\n\
     [clis.mycli.prompt_profile]\n\
     live_prompt_markers = [\"approve this?\"]\n\
     approval_markers = [\"waiting for go-ahead\"]\n\
     broad_grant_marker = \"remember this\"\n";

fn git(dir: &Path, args: &[&str]) {
    StdCommand::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git command");
}

/// A repo with the helper installed, the two-CLI roster, the `mycli`
/// override and a recording fake `tmux`; returns the repo and `sweep.sh`.
fn two_cli_repo() -> (TempDir, std::path::PathBuf) {
    let repo = TempDir::new().expect("repo");
    let root = repo.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@e.st"]);
    git(root, &["config", "user.name", "Test"]);
    fs::write(root.join("README.md"), "x").expect("readme");
    git(root, &["add", "."]);
    git(root, &["commit", "-q", "-m", "init"]);

    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts");
    let scripts = root.join(".git-paw/scripts");
    fs::create_dir_all(&scripts).expect("mk scripts");
    for name in ["_paw_common.sh", "sweep.sh"] {
        fs::copy(assets.join(name), scripts.join(name)).expect("copy helper");
    }
    fs::write(root.join(".git-paw/config.toml"), MYCLI_PROFILE).expect("config");

    let sessions = root.join(".git-paw/sessions");
    fs::create_dir_all(&sessions).expect("mk sessions");
    let roster = serde_json::json!({
        "session_name": SESSION,
        "agents": [
            { "branch_id": "feat-claude", "worktree_path": root.join("wt-1"),
              "pane_index": 1, "cli": "claude" },
            { "branch_id": "feat-mycli", "worktree_path": root.join("wt-2"),
              "pane_index": 2, "cli": "mycli" },
        ],
    });
    fs::write(
        sessions.join(format!("{SESSION}.json")),
        serde_json::to_string_pretty(&roster).expect("roster json"),
    )
    .expect("write roster");

    // Fake tmux: panes 1 and 2 are coding panes whose current path is the
    // roster worktree `wt-<pane>`; every pane shows `$FAKE_TMUX_CAPTURE`.
    // Fake curl: an empty broker, so no agent has a stale heartbeat.
    let bin = root.join("fakebin");
    fs::create_dir_all(&bin).expect("mk fakebin");
    let tmux = format!(
        "#!/bin/sh\n\
         echo \"$*\" >> \"${{FAKE_TMUX_LOG}}\"\n\
         case \"$1\" in\n\
           capture-pane) cat \"${{FAKE_TMUX_CAPTURE}}\" ;;\n\
           list-panes) printf '1 claude agent\\n2 mycli agent\\n' ;;\n\
           display-message) echo \"{root}/wt-${{4##*.}}\" ;;\n\
         esac\n\
         exit 0\n",
        root = root.display()
    );
    for (name, body) in [("tmux", tmux.as_str()), ("curl", "#!/bin/sh\necho '{}'\n")] {
        let path = bin.join(name);
        fs::write(&path, body).expect("write fake");
        let mut perms = fs::metadata(&path).expect("stat").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("chmod");
    }
    (repo, scripts.join("sweep.sh"))
}

/// Runs `sweep.sh detect-stuck` with every pane showing `capture`.
fn detect_stuck(repo: &Path, sweep: &Path, capture: &str) -> String {
    let capture_file = repo.join("capture.txt");
    fs::write(&capture_file, capture).expect("capture");
    let log = repo.join("tmux-stuck.log");
    let out = StdCommand::new("bash")
        .arg(sweep)
        .arg("detect-stuck")
        .current_dir(repo)
        .env("PATH", helper_path(repo))
        .env("FAKE_TMUX_LOG", &log)
        .env("FAKE_TMUX_CAPTURE", &capture_file)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .output()
        .expect("run sweep.sh detect-stuck");
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// The fakes first, then this build of `git-paw`, then the real `PATH`.
fn helper_path(repo: &Path) -> String {
    format!(
        "{}:{}:{}",
        repo.join("fakebin").display(),
        assert_cmd::cargo::cargo_bin("git-paw")
            .parent()
            .expect("binary dir")
            .display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

/// Spec scenario "The helper and the loop read a pane the same way"
/// (`approval_markers`): the stuck detector matches the `mycli` pane against the
/// override's `approval_markers`, and the `claude` pane against the default,
/// exactly as the compiled marker scan does.
#[test]
fn the_helper_and_the_loop_read_each_pane_approval_markers_the_same_way() {
    let (repo, sweep) = two_cli_repo();
    let capture = "mycli is waiting for go-ahead on: make deploy";

    let mycli = CliProfile::resolve("mycli", &mycli_clis())
        .compiled_prompt_profile()
        .expect("compiles");
    let claude = CliProfile::resolve("claude", &HashMap::new())
        .compiled_prompt_profile()
        .expect("compiles");
    assert!(has_approval_marker(capture, &mycli));
    assert!(!has_approval_marker(capture, &claude));

    // A matched marker with a fresh heartbeat reports `not-stuck ... fresh
    // heartbeat`; only a pane whose markers matched can say that.
    let out = detect_stuck(repo.path(), &sweep, capture);
    assert!(
        out.contains("not-stuck: feat-mycli (fresh heartbeat"),
        "the mycli pane matches its override: {out}"
    );
    assert!(
        !out.contains("not-stuck: feat-claude (fresh heartbeat"),
        "the claude pane keeps the default markers: {out}"
    );
}

/// Runs `sweep.sh approve <pane>` against `capture`, returning the helper
/// output and the keystrokes it sent.
fn approve(repo: &Path, sweep: &Path, capture: &str, pane: &str) -> (String, Vec<String>) {
    let capture_file = repo.join("capture.txt");
    fs::write(&capture_file, capture).expect("capture");
    let log = repo.join(format!("tmux-{pane}.log"));
    fs::write(&log, "").expect("log");
    let path = format!(
        "{}:{}:{}",
        repo.join("fakebin").display(),
        assert_cmd::cargo::cargo_bin("git-paw")
            .parent()
            .expect("binary dir")
            .display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = StdCommand::new("bash")
        .arg(sweep)
        .args(["approve", pane])
        .current_dir(repo)
        .env("PATH", path)
        .env("FAKE_TMUX_LOG", &log)
        .env("FAKE_TMUX_CAPTURE", &capture_file)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .output()
        .expect("run sweep.sh approve");
    let keys = fs::read_to_string(&log)
        .expect("read log")
        .lines()
        .filter(|l| l.starts_with("send-keys"))
        .map(str::to_string)
        .collect();
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
        keys,
    )
}

fn mycli_clis() -> HashMap<String, CustomCli> {
    HashMap::from([(
        "mycli".to_string(),
        CustomCli {
            command: "mycli".to_string(),
            display_name: None,
            submit_delay_ms: None,
            settings_path: None,
            approval_args: HashMap::new(),
            prompt_profile: Some(CliPromptProfileOverride {
                live_prompt_markers: Some(vec!["approve this?".to_string()]),
                approval_markers: Some(vec!["waiting for go-ahead".to_string()]),
                broad_grant_marker: Some("remember this".to_string()),
                ..Default::default()
            }),
            session_id_args: None,
            resume_args: None,
            mcp: None,
            headless_args: None,
            family: None,
            args: None,
        },
    )])
}

/// Spec scenario "The helper re-confirm reads a pane's own live-prompt
/// markers": a `mycli`-worded prompt is live on the `mycli` pane and not on the `claude`
/// pane — for the helper's re-confirm and for the compiled gate alike.
#[test]
fn the_helper_and_the_loop_read_each_pane_the_same_way() {
    let (repo, sweep) = two_cli_repo();
    let capture = "mycli wants to run: make deploy\napprove this?\n1. Yes\n2. No";

    let mycli = CliProfile::resolve("mycli", &mycli_clis())
        .compiled_prompt_profile()
        .expect("compiles");
    let claude = CliProfile::resolve("claude", &HashMap::new())
        .compiled_prompt_profile()
        .expect("compiles");
    assert!(is_live_prompt(capture, &mycli), "the loop reads mycli live");
    assert!(
        !is_live_prompt(capture, &claude),
        "the loop reads claude not live"
    );

    let (out, keys) = approve(repo.path(), &sweep, capture, "2");
    assert_eq!(keys.len(), 2, "mycli pane approved: {out}");
    let (out, keys) = approve(repo.path(), &sweep, capture, "1");
    assert!(keys.is_empty(), "claude pane keys: {keys:?}");
    assert!(out.contains("cleared before send"), "{out}");
}

/// Spec scenario "The approve guard reads the pane's own broad-grant marker".
#[test]
fn the_approve_guard_reads_the_pane_own_broad_grant_marker() {
    let (repo, sweep) = two_cli_repo();
    // Live under both wordings: Claude reads `Do you want to`, mycli reads
    // `approve this?`.
    let capture = "Bash command\n  git status\nDo you want to proceed? approve this?\n\u{276f} 1. Yes, remember this\n  2. No\n  Esc to cancel";

    let (out, keys) = approve(repo.path(), &sweep, capture, "2");
    assert!(
        keys.is_empty(),
        "mycli option 1 is a durable grant: {keys:?}"
    );
    assert!(out.contains("durable grant"), "{out}");

    let (out, keys) = approve(repo.path(), &sweep, capture, "1");
    assert_eq!(
        keys.len(),
        2,
        "claude keeps the embedded marker, which this option does not carry: {out}"
    );
    assert!(out.contains("approved pane 1"), "{out}");
}
