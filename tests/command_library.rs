//! `code-architecture` — "Command entry points are a callable library
//! surface": the commands run in-process through `git_paw::commands` with a
//! capturing sink, write nothing to the process's stdout, and offer only what
//! the CLI can express.
//!
//! A command reads `HOME` (the session receipts) and `TMUX_TMPDIR` (the tmux
//! socket) from the process environment. Rather than mutating this test
//! process's environment, each in-process scenario runs in a child: the parent
//! builds the fixture, re-executes this test binary filtered to one `child_*`
//! test with the fixture's `HOME`, private tmux socket and repository in the
//! child's environment, and then checks the child's exit status and stdout.
//! The `child_*` tests are `#[ignore]`d so they run only that way.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use git_paw::cli_setup::facts::SetupItem;
use git_paw::command_runner::RealCommandRunner;
use git_paw::commands::pause::{PauseOptions, PauseOutcome};
use git_paw::commands::setup_cli::{self, Prepared, SetupRequest};
use git_paw::commands::status::{StatusOptions, StatusOutcome};
use git_paw::commands::stop::StopOptions;
use git_paw::commands::{CaptureOutput, CommandContext};
use git_paw::config::PawConfig;
use git_paw::session::DisplayStatus;
use tempfile::TempDir;

mod helpers;
use helpers::{TestRepo, TmuxTestEnv, setup_test_repo};

/// The child's environment variable naming the fixture repository.
const CHILD_REPO: &str = "GIT_PAW_LIBRARY_TEST_REPO";
/// The child's environment variable naming the session it acts on.
const CHILD_SESSION: &str = "GIT_PAW_LIBRARY_TEST_SESSION";
/// The line a child prints on stderr once its assertions all passed.
const CHILD_PASSED: &str = "library-child-passed";

/// A repository with one worktree, a session receipt in a fake `HOME`, and a
/// live tmux session on a private socket.
struct Fixture {
    _repo: TestRepo,
    root: PathBuf,
    home: TempDir,
    tmux: TmuxTestEnv,
    session: String,
}

impl Fixture {
    fn live(tag: &str) -> Self {
        let repo = setup_test_repo();
        let root = PathBuf::from(git(repo.path(), &["rev-parse", "--show-toplevel"]));
        let wt = root.parent().expect("parent").join("wt-feat-alpha");
        git(
            &root,
            &[
                "worktree",
                "add",
                "-b",
                "feat/alpha",
                wt.to_str().expect("utf8"),
            ],
        );
        let fixture = Self {
            _repo: repo,
            root,
            home: TempDir::new().expect("fake HOME"),
            tmux: TmuxTestEnv::new(),
            session: format!("paw-lib-{tag}-{}", std::process::id()),
        };
        let receipt = serde_json::json!({
            "session_name": fixture.session,
            "repo_path": fixture.root.to_string_lossy(),
            "project_name": "lib",
            "created_at": "2026-05-01T00:00:00Z",
            "status": "active",
            "worktrees": [{
                "branch": "feat/alpha",
                "worktree_path": wt.to_string_lossy(),
                "cli": "sleep",
                "branch_created": true,
            }],
        });
        let dir = fixture.sessions_dir();
        fs::create_dir_all(&dir).expect("sessions dir");
        fs::write(
            dir.join(format!("{}.json", fixture.session)),
            serde_json::to_string_pretty(&receipt).expect("serialize"),
        )
        .expect("write receipt");

        let mut tmux = Command::new("tmux");
        fixture.tmux.apply(&mut tmux).args([
            "new-session",
            "-d",
            "-s",
            &fixture.session,
            "-x",
            "200",
            "-y",
            "50",
            "sleep 3600",
        ]);
        assert!(tmux.status().expect("tmux").success(), "tmux new-session");
        fixture
    }

    fn sessions_dir(&self) -> PathBuf {
        if cfg!(target_os = "macos") {
            self.home
                .path()
                .join("Library/Application Support/git-paw/sessions")
        } else {
            self.home.path().join(".local/share/git-paw/sessions")
        }
    }

    fn receipt_status(&self) -> String {
        let text = fs::read_to_string(self.sessions_dir().join(format!("{}.json", self.session)))
            .expect("receipt");
        let json: serde_json::Value = serde_json::from_str(&text).expect("json");
        json["status"].as_str().expect("status").to_string()
    }

    fn tmux_alive(&self) -> bool {
        let mut cmd = Command::new("tmux");
        self.tmux
            .apply(&mut cmd)
            .args(["has-session", "-t", &self.session]);
        cmd.output().is_ok_and(|o| o.status.success())
    }

    /// Runs `child_test` of this binary in a child process carrying the
    /// fixture's environment.
    fn run_child(&self, child_test: &str) -> Output {
        let mut cmd = Command::new(std::env::current_exe().expect("test binary"));
        self.tmux
            .apply(&mut cmd)
            .args([
                child_test,
                "--exact",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("HOME", self.home.path())
            .env_remove("XDG_DATA_HOME")
            .env(CHILD_REPO, &self.root)
            .env(CHILD_SESSION, &self.session)
            .env("GIT_PAW_ALLOW_LIVE_SESSION", "1");
        cmd.output().expect("run child test")
    }
}

/// Asserts the child passed its own assertions and wrote none of the
/// command's output to stdout.
fn assert_child_passed_silently(out: &Output, fixture: &Fixture) {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success() && stderr.contains(CHILD_PASSED),
        "the in-process child failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stdout.contains(&fixture.session) && !stdout.contains("Session"),
        "the command wrote to the process's stdout:\n{stdout}"
    );
}

/// The child's context, or `None` when this test runs outside a parent.
fn child_context() -> Option<(CommandContext, String)> {
    let repo = std::env::var_os(CHILD_REPO)?;
    let session = std::env::var(CHILD_SESSION).ok()?;
    Some((CommandContext::new(PathBuf::from(repo)), session))
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

// ---------------------------------------------------------------------------
// Scenario: A command runs in-process with a capturing sink
// ---------------------------------------------------------------------------

/// `pause` called in-process with default options and a capturing sink pauses
/// the session exactly as `git paw pause` does, returns an outcome describing
/// it, and writes nothing to stdout.
#[test]
fn pause_runs_in_process_with_a_capturing_sink() {
    let fixture = Fixture::live("pause");
    let out = fixture.run_child("child_pause");
    assert_child_passed_silently(&out, &fixture);
    assert_eq!(fixture.receipt_status(), "paused", "the session is paused");
    assert!(fixture.tmux_alive(), "pause leaves the panes running");
}

#[test]
#[ignore = "run in a child process by pause_runs_in_process_with_a_capturing_sink"]
fn child_pause() {
    let Some((ctx, session)) = child_context() else {
        return;
    };
    let mut sink = CaptureOutput::default();
    let outcome = git_paw::commands::pause::pause(&ctx, PauseOptions::default(), &mut sink)
        .expect("pause succeeds");
    assert_eq!(
        outcome,
        PauseOutcome::Paused {
            session_name: session.clone(),
            cli_panes: 1,
        }
    );
    assert_eq!(
        sink.lines,
        [format!(
            "Session '{session}' paused. 1 CLI pane(s) still running. \
             Run 'git paw start' to resume."
        )]
    );
    assert!(sink.warnings.is_empty(), "{:?}", sink.warnings);
    eprintln!("{CHILD_PASSED}");
}

/// `status` called in-process reports through the sink and returns the
/// session's state, writing nothing to stdout.
#[test]
fn status_runs_in_process_with_a_capturing_sink() {
    let fixture = Fixture::live("status");
    let out = fixture.run_child("child_status");
    assert_child_passed_silently(&out, &fixture);
}

#[test]
#[ignore = "run in a child process by status_runs_in_process_with_a_capturing_sink"]
fn child_status() {
    let Some((ctx, session)) = child_context() else {
        return;
    };
    let mut sink = CaptureOutput::default();
    let outcome = git_paw::commands::status::status(&ctx, StatusOptions::default(), &mut sink)
        .expect("status succeeds");
    assert_eq!(
        outcome,
        StatusOutcome::Session {
            session_name: session.clone(),
            status: DisplayStatus::Active,
            tmux_running: true,
        }
    );
    assert_eq!(sink.lines[0], format!("Session: {session}"));
    assert_eq!(sink.lines[2], "Tmux:    running");
    assert!(
        sink.lines
            .iter()
            .any(|l| l.starts_with("  feat/alpha \u{2192} sleep (")),
        "{:?}",
        sink.lines
    );

    let mut json_sink = CaptureOutput::default();
    git_paw::commands::status::status(&ctx, StatusOptions { json: true }, &mut json_sink)
        .expect("status --json succeeds");
    let body: serde_json::Value =
        serde_json::from_str(&json_sink.lines.concat()).expect("one JSON line");
    assert_eq!(body["session"], session.as_str());
    eprintln!("{CHILD_PASSED}");
}

// ---------------------------------------------------------------------------
// Scenario: Stop has no force option in the library
// ---------------------------------------------------------------------------

/// `StopOptions` carries no force or confirmation field: its only
/// constructor is the empty struct, so the library cannot express a forced or
/// confirmed stop the CLI cannot.
#[test]
fn stop_options_carry_no_force_or_confirmation_field() {
    let opts = StopOptions {};
    let rendered = format!("{opts:?}");
    assert_eq!(
        rendered, "StopOptions",
        "StopOptions has fields: {rendered}"
    );
    let source = include_str!("../src/commands/stop.rs");
    let start = source
        .find("pub struct StopOptions")
        .expect("StopOptions defined in commands/stop.rs");
    let decl = &source[start..start + source[start..].find('}').expect("struct body")];
    for word in ["force", "confirm", "yes"] {
        assert!(
            !decl.to_lowercase().contains(word),
            "StopOptions must not carry a `{word}` option:\n{decl}"
        );
    }
}

// ---------------------------------------------------------------------------
// Scenario: setup_cli writes nothing without the caller's confirmation
// ---------------------------------------------------------------------------

/// Every file under `root` with its bytes.
fn byte_snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.insert(path.clone(), fs::read(&path).expect("read file"));
            }
        }
    }
    out
}

/// `setup_cli::prepare` for `claude` with the verified allowlist item plans a
/// write but performs none: without an `apply_setup` call, no target file and
/// no setup record is created or modified.
#[test]
fn setup_cli_prepare_writes_nothing() {
    let repo = setup_test_repo();
    let root = PathBuf::from(git(repo.path(), &["rev-parse", "--show-toplevel"]));
    assert!(
        !root.join(".claude/settings.local.json").exists(),
        "fixture starts without the target file"
    );
    let before = byte_snapshot(&root);

    let request = SetupRequest {
        cli: "claude".to_string(),
        items: vec![SetupItem::Allowlist],
        undo: false,
    };
    let prepared = setup_cli::prepare(&RealCommandRunner, &root, &PawConfig::default(), &request)
        .expect("prepare succeeds");
    assert!(
        matches!(prepared, Prepared::Setup { .. }),
        "the verified allowlist item yields a setup plan: {prepared:?}"
    );

    assert!(!root.join(".claude/settings.local.json").exists());
    assert_eq!(
        byte_snapshot(&root),
        before,
        "prepare created or modified a file under the repository"
    );
}

// ---------------------------------------------------------------------------
// Requirement: Command entry points are a callable library surface
// ---------------------------------------------------------------------------

/// One `pub fn` per command family is reachable from the library, each taking
/// explicit inputs and an output sink. Naming each as a function item makes
/// this a compile-time check: a family that left the library, or went back to
/// reading its inputs from the process, fails to build.
#[test]
fn every_command_family_is_a_library_function() {
    use git_paw::commands::{
        Output, add, approvals, attach, clis, pause, purge, recover, remove, replay, start, status,
        stop, supervisor,
    };
    type Ctx<'a> = &'a CommandContext;
    type Sink<'a> = &'a mut dyn Output;

    let _: fn(Ctx, start::StartOptions, Sink) -> _ = start::start;
    let _: fn(Ctx, start::StartWithSpecsOptions, Sink) -> _ = start::start_with_specs;
    let _: fn(Ctx, supervisor::SupervisorOptions, Sink) -> _ = supervisor::supervisor;
    let _: fn(Ctx, add::AddOptions, Sink) -> _ = add::add;
    let _: fn(Ctx, remove::RemoveOptions, Sink) -> _ = remove::remove;
    let _: fn(Ctx, pause::PauseOptions, Sink) -> _ = pause::pause;
    let _: fn(Ctx, StopOptions, Sink) -> _ = stop::stop;
    let _: fn(Ctx, purge::PurgeOptions, Sink) -> _ = purge::purge;
    let _: fn(purge::PurgeStaleOptions, Sink) -> _ = purge::purge_stale;
    let _: fn(Ctx, status::StatusOptions, Sink) -> _ = status::status;
    let _: fn(Ctx, attach::AttachOptions, Sink) -> _ = attach::attach;
    let _: fn(Ctx, recover::RecoverOptions, Sink) -> _ = recover::recover;
    let _: fn(Ctx, approvals::ApprovalsOptions, Sink) -> _ = approvals::approvals;
    let _: fn(Ctx, replay::ReplayOptions, Sink) -> _ = replay::replay;
    let _: fn(Ctx, Sink) -> _ = clis::list_clis;
}
