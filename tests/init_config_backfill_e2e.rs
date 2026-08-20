//! End-to-end integration tests for `init-config-backfill`.
//!
//! Exercises the production binary so the cross-module flow — existing
//! `config.toml` → top-level default-key backfill → session-state worktree
//! enumeration → relocation warning on stderr — is covered behaviourally, not
//! only at the unit level (`migrate_existing_config`,
//! `placement_backfill_warning`).
//!
//! Covers the `core-init` spec scenarios:
//!
//! - "Init backfills a missing top-level default key"
//! - "Backfilling child placement into a repo with existing sibling worktrees
//!   warns"
//! - "Backfilling placement with no existing worktrees does not warn"
//!
//! These tests deliberately do NOT call `setup_test_repo()` (the live-session
//! guard helper): the seeded session name is guaranteed-absent from tmux and
//! `git paw init` never probes or creates a tmux session, so the run is
//! socket-isolated.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Output};
use std::time::SystemTime;

use assert_cmd::Command;
use git_paw::session::{Session, SessionMode, SessionStatus, WorktreeEntry};
use tempfile::TempDir;

/// Canonicalises a path so it matches what `git rev-parse --show-toplevel`
/// returns — on macOS `/var/...` resolves to `/private/var/...`, and a receipt
/// keyed on the un-canonicalised path would never match the running `init`
/// invocation's repo root.
fn canon(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

fn git(dir: &Path, args: &[&str]) -> Output {
    StdCommand::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git command")
}

/// A repo at `<outer>/proj` whose config omits `worktree_placement`, plus an
/// isolated `HOME` so the binary resolves this fixture's session receipts and
/// global config instead of the developer's.
struct Fixture {
    home: TempDir,
    outer: TempDir,
    repo: PathBuf,
    config_path: PathBuf,
}

impl Fixture {
    /// Builds a committed git repo with a `.git-paw/config.toml` that omits
    /// `worktree_placement` (so the repo resolves to the sibling layout) but
    /// already carries `[supervisor]`, so the migration has no section to
    /// append and only the key backfill is exercised.
    fn new() -> Self {
        let home = TempDir::new().expect("home");
        let outer = TempDir::new().expect("outer");
        let repo = outer.path().join("proj");
        fs::create_dir_all(&repo).expect("create repo dir");

        git(&repo, &["init", "-q"]);
        git(&repo, &["config", "user.email", "t@e.st"]);
        git(&repo, &["config", "user.name", "Test"]);
        fs::write(repo.join("README.md"), "x").expect("write readme");
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "init"]);

        let paw_dir = repo.join(".git-paw");
        fs::create_dir_all(&paw_dir).expect("create .git-paw");
        let config_path = paw_dir.join("config.toml");
        fs::write(&config_path, "[supervisor]\nenabled = false\n").expect("write config");

        Self {
            home,
            outer,
            repo,
            config_path,
        }
    }

    /// The sessions directory the binary resolves for this fixture's `HOME`,
    /// mirroring `git_paw::dirs::data_dir()`.
    fn sessions_dir(&self) -> PathBuf {
        if cfg!(target_os = "macos") {
            self.home
                .path()
                .join("Library/Application Support/git-paw/sessions")
        } else {
            self.home.path().join(".local/share/git-paw/sessions")
        }
    }

    /// Adds a real sibling-layout worktree beside the repo, records it in a
    /// session receipt (the mechanism `status`/`purge` read), and returns its
    /// canonical path.
    fn with_sibling_worktree(&self, branch: &str, dir_name: &str) -> PathBuf {
        let sibling = self.outer.path().join(dir_name);
        let add = git(
            &self.repo,
            &["worktree", "add", "-b", branch, &sibling.to_string_lossy()],
        );
        assert!(
            add.status.success(),
            "git worktree add failed: {}",
            String::from_utf8_lossy(&add.stderr)
        );
        fs::write(sibling.join("agent-work.txt"), "in progress").expect("write work");

        let sdir = self.sessions_dir();
        fs::create_dir_all(&sdir).expect("create sessions dir");
        let receipt = Session {
            // Guaranteed absent from tmux, so no liveness probe touches a real
            // session.
            session_name: "paw-init-backfill-doesnotexist".to_string(),
            repo_path: canon(&self.repo),
            project_name: "proj".to_string(),
            created_at: SystemTime::now(),
            status: SessionStatus::Active,
            worktrees: vec![WorktreeEntry {
                branch: branch.to_string(),
                worktree_path: canon(&sibling),
                cli: "claude".to_string(),
                branch_created: true,
                pending_boot_prompt: None,
                runtime_slot: None,
            }],
            broker_port: None,
            broker_bind: None,
            broker_log_path: None,
            mode: SessionMode::Bare,
            dashboard_pane: None,
        };
        git_paw::session::save_session_in(&receipt, &sdir).expect("save receipt");
        canon(&sibling)
    }

    /// Runs `git paw init` in the fixture repo with the isolated `HOME`.
    fn run_init(&self) -> Output {
        let out = Command::cargo_bin("git-paw")
            .expect("binary exists")
            .current_dir(&self.repo)
            .env("HOME", self.home.path())
            .env_remove("XDG_DATA_HOME")
            .env_remove("XDG_CONFIG_HOME")
            .arg("init")
            .output()
            .expect("init runs");
        assert!(
            out.status.success(),
            "init should succeed; stderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }

    /// Parses the repo config as it stands on disk, returning the raw text too.
    fn parsed_config(&self) -> (String, git_paw::config::PawConfig) {
        let text = fs::read_to_string(&self.config_path).expect("read config");
        let parsed =
            toml::from_str(&text).unwrap_or_else(|e| panic!("config must parse: {e}\n{text}"));
        (text, parsed)
    }
}

/// Scenario: Init backfills a missing top-level default key + Backfilling child
/// placement into a repo with existing sibling worktrees warns.
#[test]
fn init_backfills_placement_and_warns_about_existing_sibling_worktree() {
    let fx = Fixture::new();
    let sibling = fx.with_sibling_worktree("feat/a", "proj-feat-a");

    let out = fx.run_init();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // The backfill is reported in init's action summary.
    assert!(
        stdout.contains("added missing keys: worktree_placement"),
        "init should report the backfilled key; stdout:\n{stdout}"
    );

    // The key is written with its generated-default value, and the existing
    // section survives.
    let (after, parsed) = fx.parsed_config();
    assert_eq!(
        parsed.worktree_placement(),
        git_paw::config::WorktreePlacement::Child,
        "init backfills the generated default; got:\n{after}"
    );
    assert!(
        !parsed.supervisor.expect("supervisor preserved").enabled,
        "the pre-existing [supervisor] section must be preserved"
    );

    // The warning names the previous location, says nothing was moved, and
    // carries the remediation.
    for needle in [
        "previous (sibling) location",
        "did not move them",
        "worktree_placement = \"sibling\"",
        &sibling.display().to_string(),
    ] {
        assert!(
            stderr.contains(needle),
            "warning must contain `{needle}`; stderr:\n{stderr}"
        );
    }

    // Nothing moved, deleted, or re-registered.
    assert_eq!(
        fs::read_to_string(sibling.join("agent-work.txt")).expect("work survives"),
        "in progress",
        "init must not touch an existing worktree's contents"
    );
    let listing =
        String::from_utf8_lossy(&git(&fx.repo, &["worktree", "list", "--porcelain"]).stdout)
            .into_owned();
    assert!(
        listing.contains(&sibling.display().to_string()),
        "the existing worktree must stay registered with git; listing:\n{listing}"
    );
    let recorded = git_paw::session::find_session_for_repo_in(&canon(&fx.repo), &fx.sessions_dir())
        .expect("read receipt")
        .expect("receipt present");
    assert_eq!(
        recorded.worktrees.len(),
        1,
        "init must not re-register worktrees in session state"
    );
    assert_eq!(recorded.worktrees[0].worktree_path, sibling);
}

/// Scenario: Backfilling placement with no existing worktrees does not warn.
#[test]
fn init_backfills_placement_without_warning_when_no_worktrees_exist() {
    let fx = Fixture::new();

    let out = fx.run_init();
    let stderr = String::from_utf8_lossy(&out.stderr);

    let (after, parsed) = fx.parsed_config();
    assert_eq!(
        parsed.worktree_placement(),
        git_paw::config::WorktreePlacement::Child,
        "the key is written even with no worktrees; got:\n{after}"
    );
    assert!(
        !stderr.contains("previous (sibling) location"),
        "no relocation warning when the repo has no worktrees; stderr:\n{stderr}"
    );
}

/// A second `git paw init` must not re-add or duplicate the backfilled key —
/// once every section and top-level key is present, init is a no-op.
#[test]
fn second_init_does_not_duplicate_the_backfilled_key() {
    let fx = Fixture::new();

    fx.run_init();
    let (first, _) = fx.parsed_config();
    fx.run_init();
    let (second, _) = fx.parsed_config();

    assert_eq!(first, second, "a second init must not change the config");
    assert_eq!(
        second.matches("worktree_placement").count(),
        1,
        "the backfilled key must appear exactly once; got:\n{second}"
    );
}
