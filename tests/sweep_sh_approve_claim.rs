//! Cross-process exclusion tests for `sweep.sh approve <pane>` (capability
//! `supervisor-unattended-operation`, requirement "Approval on a pane is
//! guarded by an exclusive per-pane claim").
//!
//! The claim is what makes the exclusion structural rather than conventional,
//! and it only works if the shell helper and the binary agree on the claim path
//! **byte for byte** — they run in different processes and share nothing else.
//! So these tests place the claim using the Rust-side
//! [`git_paw::supervisor::claim::claim_path`] and assert the *shell* observes
//! it: the pane is skipped and zero keystrokes go out. A drift in either side's
//! formula makes the shell miss the claim and approve anyway, failing here.
//!
//! A recording fake `tmux` on `$PATH` serves a scripted pane capture and logs
//! every invocation, so "sent nothing" is asserted against the real dispatch
//! log. The suite is hermetic — no real tmux server, no broker.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use tempfile::TempDir;

use git_paw::supervisor::claim::{CLAIM_TTL, claim_path};

const SESSION: &str = "paw-fake-claim";

/// A live 2-option prompt: approved with option `1` unless something stops it.
const LIVE_PROMPT: &str =
    "Bash command\n  git status\nDo you want to proceed?\n❯ 1. Yes\n  2. No\n  Esc to cancel";

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
fn install_sweep(repo: &Path) -> PathBuf {
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

/// Installs the recording fake `tmux` into `<repo>/fakebin/`: every invocation
/// appends its arguments to `$FAKE_TMUX_LOG`, and `capture-pane` prints the
/// scripted capture from `$FAKE_TMUX_CAPTURE`.
fn install_fake_tmux(repo: &Path) -> PathBuf {
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

/// A repository with sweep.sh, the session JSON and the fake tmux installed.
struct Repo {
    dir: TempDir,
    sweep: PathBuf,
    /// The repository root as `git rev-parse --show-toplevel` reports it —
    /// the exact root BOTH the helper and the binary build claim paths from.
    /// On macOS this differs from `dir.path()` (`/var` vs `/private/var`), so
    /// resolving it the same way both sides do is part of the contract.
    root: PathBuf,
}

impl Repo {
    fn new() -> Self {
        let dir = TempDir::new().expect("repo");
        init_git_repo(dir.path());
        let sweep = install_sweep(dir.path());
        write_session_json(dir.path());
        install_fake_tmux(dir.path());
        let out = StdCommand::new("git")
            .current_dir(dir.path())
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .expect("git rev-parse");
        let root = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string());
        Self { dir, sweep, root }
    }

    /// The claim path for `pane`, computed by the **Rust** side.
    fn claim(&self, pane: usize) -> PathBuf {
        claim_path(&self.root, pane)
    }

    /// Lays down a claim file at the Rust-computed path, standing in for
    /// another approver (the drive loop, or a second `sweep.sh`) holding it.
    fn plant_claim(&self, pane: usize) -> PathBuf {
        let path = self.claim(pane);
        fs::create_dir_all(path.parent().expect("claim parent")).expect("mk tmp dir");
        fs::write(&path, "").expect("write claim");
        path
    }

    /// Runs `sweep.sh approve <pane>` with `capture` behind the fake tmux.
    fn approve(&self, capture: &str, pane: &str) -> ApproveRun {
        let capture_file = self.dir.path().join("capture.txt");
        fs::write(&capture_file, capture).expect("write capture fixture");
        let log_file = self.dir.path().join("tmux.log");
        fs::write(&log_file, "").expect("create tmux log");

        // Fake tmux first, then the freshly built binary: `sweep.sh approve`
        // resolves its option index by delegating to `git paw __classify`, so
        // THIS build must be the `git-paw` that `git paw` finds.
        let path = format!(
            "{}:{}:{}",
            self.dir.path().join("fakebin").display(),
            assert_cmd::cargo::cargo_bin("git-paw")
                .parent()
                .expect("built binary has a parent dir")
                .display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let out = StdCommand::new("bash")
            .arg(&self.sweep)
            .args(["approve", pane])
            .current_dir(self.dir.path())
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

/// Spec scenario "The Rust loop and the shell helper are mutually exclusive":
/// a claim taken at the path the **binary** computes is seen as held by the
/// **shell** helper, which sends no keystroke. This is simultaneously the
/// path-formula contract — the two sides only meet through this one string.
#[test]
fn a_claim_taken_at_the_rust_path_blocks_the_shell_approver() {
    let repo = Repo::new();
    let claim = repo.plant_claim(2);

    let run = repo.approve(LIVE_PROMPT, "2");

    assert!(
        run.sent_keys().is_empty(),
        "a claimed pane must receive zero keystrokes, log: {:?}",
        run.tmux_log
    );
    assert!(
        !run.output.contains("approved pane"),
        "a claimed pane must not report an approval, got: {}",
        run.output
    );
    assert!(
        run.output.contains(&claim.display().to_string()),
        "the helper must report the SAME claim path the binary computes \
         ({}), got: {}",
        claim.display(),
        run.output
    );
    assert!(
        claim.exists(),
        "the helper must leave the other approver's claim in place"
    );
}

/// The complement: with no claim held the same prompt IS approved, and the
/// claim the helper took for itself is released once it returns (spec scenario
/// "The claim is released after the send and on error") — so a later approver
/// is never wedged out by a completed one.
#[test]
fn an_unclaimed_pane_is_approved_and_the_claim_is_released() {
    let repo = Repo::new();

    let run = repo.approve(LIVE_PROMPT, "2");

    assert_eq!(
        run.sent_keys(),
        vec![
            format!("send-keys -t {SESSION}:0.2 1"),
            format!("send-keys -t {SESSION}:0.2 Enter"),
        ],
        "an unclaimed pane is approved with the digit then a separate Enter"
    );
    assert!(
        !repo.claim(2).exists(),
        "the helper must release its own claim on exit"
    );
}

/// A claim whose approver was hard-killed leaves a file no `trap` will ever
/// remove. Once it is older than the TTL the next approver steals it, so a
/// crash wedges the pane for one TTL rather than forever.
#[test]
fn a_claim_older_than_the_ttl_is_stolen_and_the_pane_is_approved() {
    let repo = Repo::new();
    let claim = repo.plant_claim(2);
    // Backdate past the TTL, as an abandoned claim would be.
    let stale = std::time::SystemTime::now() - (CLAIM_TTL + std::time::Duration::from_secs(5));
    let file = fs::OpenOptions::new()
        .write(true)
        .open(&claim)
        .expect("open claim");
    file.set_times(fs::FileTimes::new().set_accessed(stale).set_modified(stale))
        .expect("backdate claim");
    drop(file);

    let run = repo.approve(LIVE_PROMPT, "2");

    assert!(
        run.output.contains("approved pane 2"),
        "an abandoned claim must be stolen and the pane approved, got: {}",
        run.output
    );
    assert!(
        !run.sent_keys().is_empty(),
        "stealing the claim must let the keystrokes through, log: {:?}",
        run.tmux_log
    );
}

/// Pane 0 stays refused before the claim is ever considered: the supervisor's
/// own pane is excluded from the blind send-keys path, and refusing it must not
/// leave a claim file behind for a later approver to trip over.
#[test]
fn pane_zero_is_still_refused_and_takes_no_claim() {
    let repo = Repo::new();

    let run = repo.approve(LIVE_PROMPT, "0");

    assert!(
        run.output.contains("pane 0 excluded"),
        "pane 0 must still be refused, got: {}",
        run.output
    );
    assert!(
        run.sent_keys().is_empty(),
        "pane 0 must receive zero keystrokes, log: {:?}",
        run.tmux_log
    );
    assert!(
        !repo.claim(0).exists(),
        "a refused pane must not leave a claim behind"
    );
}
