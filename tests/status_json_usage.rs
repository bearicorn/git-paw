//! `worker-usage-budget` — `git paw status --json` exposes each worker's
//! attributed usage and a session total; the human report is unchanged.
//!
//! A session receipt is written to a fake `HOME`, and a Claude-format
//! transcript fixture is placed under a fake `CLAUDE_CONFIG_DIR` for the
//! worker's assigned session id.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::SystemTime;

use assert_cmd::Command;
use git_paw::session::{Session, SessionMode, SessionStatus, WorktreeEntry};
use tempfile::TempDir;

const SESSION_FIXTURE: &str = include_str!("fixtures/claude_usage/session.jsonl");
const SID: &str = "11111111-2222-3333-4444-555555555555";

fn sessions_dir_for_home(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-paw/sessions")
    } else {
        home.join(".local/share/git-paw/sessions")
    }
}

fn canon(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

fn init_git_repo(dir: &Path) {
    let run = |args: &[&str]| {
        StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git command");
    };
    run(&["init", "-q"]);
    run(&["config", "user.email", "t@e.st"]);
    run(&["config", "user.name", "Test"]);
    fs::write(dir.join("README.md"), "x").expect("write readme");
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "init"]);
}

fn entry(branch: &str, cli: &str, worktree: &Path, sid: Option<&str>) -> WorktreeEntry {
    WorktreeEntry {
        branch: branch.to_string(),
        worktree_path: worktree.to_path_buf(),
        cli: cli.to_string(),
        profile: None,
        branch_created: false,
        base_branch: None,
        pending_boot_prompt: None,
        pending_entry_point: None,
        runtime_slot: None,
        cli_session_id: sid.map(String::from),
        spec_id: None,
    }
}

struct Fixture {
    home: TempDir,
    repo: TempDir,
    claude_config: TempDir,
}

impl Fixture {
    /// A session with a `claude` worker on `feat/x` (assigned id `SID`) and
    /// a `generic`-family worker on `feat/y`.
    fn new(tag: &str) -> Self {
        let home = TempDir::new().expect("home");
        let repo = TempDir::new().expect("repo");
        let claude_config = TempDir::new().expect("claude config");
        init_git_repo(repo.path());
        let repo_path = canon(repo.path());
        let session = Session {
            session_name: format!("paw-spend-{tag}-doesnotexist"),
            repo_path: repo_path.clone(),
            project_name: "usage".to_string(),
            created_at: SystemTime::now(),
            status: SessionStatus::Active,
            worktrees: vec![
                entry("feat/x", "claude", &repo_path.join("wt-x"), Some(SID)),
                entry("feat/y", "aider", &repo_path.join("wt-y"), None),
            ],
            broker_port: None,
            broker_bind: None,
            broker_log_path: None,
            mode: SessionMode::Bare,
            dashboard_pane: None,
            layout: None,
            broker_authority: None,
        };
        let sdir = sessions_dir_for_home(home.path());
        fs::create_dir_all(&sdir).expect("sessions dir");
        git_paw::session::save_session_in(&session, &sdir).expect("save receipt");
        Self {
            home,
            repo,
            claude_config,
        }
    }

    fn write_transcript(&self) {
        self.write_transcript_as(SID);
    }

    /// Writes the usage fixture as the transcript of session `sid` in
    /// `feat/x`'s worktree.
    fn write_transcript_as(&self, sid: &str) {
        let slug: String = canon(self.repo.path())
            .join("wt-x")
            .to_string_lossy()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let dir = self.claude_config.path().join("projects").join(slug);
        fs::create_dir_all(&dir).expect("project dir");
        fs::write(dir.join(format!("{sid}.jsonl")), SESSION_FIXTURE).expect("transcript");
    }

    fn status(&self, json: bool) -> String {
        let mut cmd = Command::cargo_bin("git-paw").expect("binary exists");
        cmd.current_dir(self.repo.path())
            .env("HOME", self.home.path())
            .env("CLAUDE_CONFIG_DIR", self.claude_config.path())
            .env_remove("XDG_DATA_HOME")
            .env_remove("XDG_CONFIG_HOME")
            .arg("status");
        if json {
            cmd.arg("--json");
        }
        let out = cmd.output().expect("status");
        assert!(out.status.success(), "status should succeed");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn status_json(&self) -> serde_json::Value {
        serde_json::from_str(self.status(true).trim()).expect("status --json is JSON")
    }
}

fn worktree<'a>(json: &'a serde_json::Value, branch: &str) -> &'a serde_json::Value {
    json["worktrees"]
        .as_array()
        .expect("worktrees array")
        .iter()
        .find(|w| w["branch"] == branch)
        .expect("worktree present")
}

/// Scenario "A worker with a usage report shows figures".
#[test]
fn a_worker_with_a_usage_report_shows_its_token_totals() {
    let fx = Fixture::new("known");
    fx.write_transcript();
    let json = fx.status_json();
    let usage = &worktree(&json, "feat/x")["usage"];
    assert_eq!(usage["input_tokens"], 5, "got {usage}");
    assert_eq!(usage["output_tokens"], 150, "got {usage}");
    assert_eq!(usage["cache_read_tokens"], 2500, "got {usage}");
    assert_eq!(usage["cache_write_tokens"], 520, "got {usage}");
    assert!(usage.get("cost").is_none(), "no cost is reported: {usage}");
    assert_eq!(json["usage_total"]["output_tokens"], 150);
    assert_eq!(
        json["usage_total"]["tokens"],
        5 + 150 + 2500 + 520,
        "the capped figure counts every token kind: {json}"
    );
}

/// Scenario "A CLI session git-paw did not launch is excluded": a second
/// transcript in the same worktree's history, under a session id the launch
/// did not record, adds nothing.
#[test]
fn a_session_git_paw_did_not_launch_is_excluded() {
    let fx = Fixture::new("foreign");
    fx.write_transcript();
    fx.write_transcript_as("0b1c2d3e-0000-4000-8000-00000000f00d");
    let json = fx.status_json();
    assert_eq!(
        worktree(&json, "feat/x")["usage"]["output_tokens"],
        150,
        "{json}"
    );
    assert_eq!(json["usage_total"]["output_tokens"], 150, "{json}");
}

/// Scenario "Unknown usage is null and counted".
#[test]
fn an_unknown_worker_is_null_and_counted_in_the_total() {
    let fx = Fixture::new("unknown");
    fx.write_transcript();
    let json = fx.status_json();
    assert!(worktree(&json, "feat/y")["usage"].is_null(), "got {json}");
    assert_eq!(json["usage_total"]["unknown_workers"], 1, "got {json}");
}

#[test]
fn existing_worktree_fields_are_unchanged() {
    let fx = Fixture::new("fields");
    let json = fx.status_json();
    let x = worktree(&json, "feat/x");
    assert_eq!(x["cli"], "claude");
    assert_eq!(x["cli_session_id"], SID);
    assert!(x["worktree_path"].is_string());
    // No transcript yet: the claude worker's usage is unknown too.
    assert!(x["usage"].is_null(), "got {x}");
    assert_eq!(json["usage_total"]["unknown_workers"], 2);
}

/// Scenario "Human-readable status is unchanged by usage".
#[test]
fn human_status_is_byte_identical_with_and_without_usage() {
    let fx = Fixture::new("human");
    let before = fx.status(false);
    fx.write_transcript();
    let after = fx.status(false);
    assert_eq!(before, after);
    assert!(
        !after.contains("usage") && !after.contains("tokens"),
        "got {after}"
    );
}

// ---------------------------------------------------------------------------
// agent-profiles: `usage_by_profile` (session-state).
// ---------------------------------------------------------------------------

const SID2: &str = "22222222-3333-4444-5555-666666666666";
const SID3: &str = "33333333-4444-5555-6666-777777777777";

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

/// A session with two `routine` workers on `claude` (feat/x, feat/z) and one
/// plain `aider` worker (feat/y), plus the given ledger lines.
fn profiled_fixture(tag: &str, ledger: &[serde_json::Value]) -> Fixture {
    let fx = Fixture::new(tag);
    let repo_path = canon(fx.repo.path());
    let mut x = entry("feat/x", "claude", &repo_path.join("wt-x"), Some(SID));
    x.profile = Some("routine".to_string());
    let mut z = entry("feat/z", "claude", &repo_path.join("wt-z"), Some(SID2));
    z.profile = Some("routine".to_string());
    let y = entry("feat/y", "aider", &repo_path.join("wt-y"), None);
    let session = Session {
        session_name: format!("paw-spend-{tag}-doesnotexist"),
        repo_path: repo_path.clone(),
        project_name: "usage".to_string(),
        created_at: SystemTime::now(),
        status: SessionStatus::Active,
        worktrees: vec![x, y, z],
        broker_port: None,
        broker_bind: None,
        broker_log_path: None,
        mode: SessionMode::Bare,
        dashboard_pane: None,
        layout: None,
        broker_authority: None,
    };
    git_paw::session::save_session_in(&session, &sessions_dir_for_home(fx.home.path()))
        .expect("save receipt");
    let dir = repo_path.join(".git-paw/logs/delegates");
    fs::create_dir_all(&dir).expect("ledger dir");
    let lines: Vec<String> = ledger.iter().map(ToString::to_string).collect();
    fs::write(dir.join("runs.jsonl"), lines.join("\n") + "\n").expect("ledger");
    fx
}

/// Writes the usage fixture as session `sid`'s transcript for a CLI run in
/// `cwd`.
fn write_transcript_in(fx: &Fixture, cwd: &Path, sid: &str) {
    let slug: String = cwd
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let dir = fx.claude_config.path().join("projects").join(slug);
    fs::create_dir_all(&dir).expect("project dir");
    fs::write(dir.join(format!("{sid}.jsonl")), SESSION_FIXTURE).expect("transcript");
}

fn run(profile: &str, cli: &str, sid: Option<&str>, started_at: u64) -> serde_json::Value {
    serde_json::json!({
        "profile": profile, "cli": cli, "session_id": sid,
        "started_at": started_at, "duration_seconds": 1.0, "outcome": "ok"
    })
}

/// Scenario "Worker and delegate usage are grouped by profile".
#[test]
fn worker_and_delegate_usage_are_grouped_by_profile() {
    let later = unix_now() + 5;
    let fx = profiled_fixture(
        "byprofile",
        &[
            run("spec-audit", "claude", Some(SID3), later),
            run("explorer", "aider", None, later),
        ],
    );
    let root = canon(fx.repo.path());
    write_transcript_in(&fx, &root.join("wt-x"), SID);
    write_transcript_in(&fx, &root.join("wt-z"), SID2);
    write_transcript_in(&fx, &root, SID3);
    let json = fx.status_json();
    let by = &json["usage_by_profile"];

    assert_eq!(by["routine"]["kind"], "worker", "{by}");
    assert_eq!(by["routine"]["workers"], 2, "{by}");
    assert_eq!(by["routine"]["usage"]["output_tokens"], 300, "{by}");
    assert_eq!(by["routine"]["unknown"], 0, "{by}");

    assert_eq!(by["spec-audit"]["kind"], "delegate", "{by}");
    assert_eq!(by["spec-audit"]["delegate_runs"], 1, "{by}");
    assert_eq!(by["spec-audit"]["usage"]["output_tokens"], 150, "{by}");

    assert_eq!(by["explorer"]["delegate_runs"], 1, "{by}");
    assert_eq!(by["explorer"]["unknown"], 1, "{by}");
    assert!(by["explorer"]["usage"].is_null(), "{by}");

    // usage_total counts the workers only, as before.
    assert_eq!(json["usage_total"]["output_tokens"], 300, "{json}");
    assert_eq!(json["usage_total"]["unknown_workers"], 1, "{json}");
    assert!(by.get("feat/y").is_none() && by.as_object().unwrap().len() == 3);
}

/// Scenario "Runs before the session are excluded".
#[test]
fn runs_before_the_session_are_excluded() {
    let fx = profiled_fixture("before", &[run("spec-audit", "claude", Some(SID3), 1)]);
    let json = fx.status_json();
    assert!(
        json["usage_by_profile"].get("spec-audit").is_none(),
        "{json}"
    );
}

/// Scenario "No profiles yields an empty object".
#[test]
fn no_profiles_yields_an_empty_object() {
    let fx = Fixture::new("noprofiles");
    let json = fx.status_json();
    assert_eq!(json["usage_by_profile"], serde_json::json!({}), "{json}");
    assert!(
        worktree(&json, "feat/x")["profile"].is_null(),
        "each worktree carries profile null: {json}"
    );
}
