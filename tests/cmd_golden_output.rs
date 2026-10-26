//! Golden-output checks for `git paw status`, `pause`, `stop` and
//! `purge --force` (`code-architecture`, "CLI behavior is byte-identical").
//!
//! The expectations below were captured from the binary BEFORE the command
//! bodies moved into the library (`cmd-library-surface`) and are not edited
//! afterwards: each test pins the exit code, the full stdout and the full
//! stderr of one invocation. Run-specific values — the sandbox paths, the
//! session name, branch tips and elapsed times — are replaced by placeholders
//! before comparison, so the check is exact for everything the command itself
//! decides to print.
//!
//! Every tmux call (the commands' own, and the fixtures') goes through a
//! private socket via [`TmuxTestEnv`]; nothing touches the default server.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use tempfile::TempDir;

mod helpers;
use helpers::{TestRepo, TmuxTestEnv, setup_test_repo};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// One isolated fixture: a git repo, a fake `HOME` holding the session
/// receipts, and a private tmux socket.
struct Fixture {
    _repo: TestRepo,
    root: PathBuf,
    home: TempDir,
    tmux: TmuxTestEnv,
    session: String,
}

impl Fixture {
    fn new() -> Self {
        let repo = setup_test_repo();
        let root = git_out(repo.path(), &["rev-parse", "--show-toplevel"]).into();
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let session = format!("paw-golden-{}-{n}", std::process::id());
        Self {
            _repo: repo,
            root,
            home: TempDir::new().expect("fake HOME"),
            tmux: TmuxTestEnv::new(),
            session,
        }
    }

    fn sessions_dir(&self) -> PathBuf {
        let dir = if cfg!(target_os = "macos") {
            self.home
                .path()
                .join("Library/Application Support/git-paw/sessions")
        } else {
            self.home.path().join(".local/share/git-paw/sessions")
        };
        fs::create_dir_all(&dir).expect("sessions dir");
        dir
    }

    /// Creates a worktree for `branch` beside the repo; `commit` adds one
    /// commit on it so it is not merged into `main`.
    fn worktree(&self, branch: &str, commit: bool) -> PathBuf {
        let slug = branch.replace('/', "-");
        let path = self
            .root
            .parent()
            .expect("parent")
            .join(format!("wt-{slug}"));
        git_out(
            &self.root,
            &[
                "worktree",
                "add",
                "-b",
                branch,
                path.to_str().expect("utf8"),
            ],
        );
        if commit {
            fs::write(path.join("work.txt"), "work").expect("write");
            git_out(&path, &["add", "."]);
            git_out(&path, &["commit", "-m", "work"]);
        }
        path
    }

    /// Writes the session receipt with one entry per `(branch, path)`.
    fn receipt(&self, status: &str, worktrees: &[(&str, &Path)], broker_port: Option<u16>) {
        let wts: Vec<_> = worktrees
            .iter()
            .map(|(branch, path)| {
                serde_json::json!({
                    "branch": branch,
                    "worktree_path": path.to_string_lossy(),
                    "cli": "sleep",
                    "branch_created": true,
                    "base_branch": "main",
                })
            })
            .collect();
        let mut json = serde_json::json!({
            "session_name": self.session,
            "repo_path": self.root.to_string_lossy(),
            "project_name": "golden",
            "created_at": "2026-05-01T00:00:00Z",
            "status": status,
            "worktrees": wts,
        });
        if let Some(port) = broker_port {
            let obj = json.as_object_mut().expect("object");
            obj.insert("broker_port".into(), port.into());
            obj.insert("broker_bind".into(), "127.0.0.1".into());
        }
        fs::write(
            self.sessions_dir().join(format!("{}.json", self.session)),
            serde_json::to_string_pretty(&json).expect("serialize"),
        )
        .expect("write receipt");
    }

    /// Starts the session's tmux server on the private socket with one pane
    /// running a long-lived non-shell process (a live agent stand-in).
    fn live_tmux(&self) {
        let mut cmd = Command::new("tmux");
        self.tmux.apply(&mut cmd).args([
            "new-session",
            "-d",
            "-s",
            &self.session,
            "-x",
            "200",
            "-y",
            "50",
            "-c",
            self.root.to_str().expect("utf8"),
            "sleep 3600",
        ]);
        assert!(cmd.status().expect("tmux").success(), "tmux new-session");
    }

    fn tmux_alive(&self) -> bool {
        let mut cmd = Command::new("tmux");
        self.tmux
            .apply(&mut cmd)
            .args(["has-session", "-t", &self.session]);
        cmd.output().is_ok_and(|o| o.status.success())
    }

    fn receipt_status(&self) -> Option<String> {
        let text =
            fs::read_to_string(self.sessions_dir().join(format!("{}.json", self.session))).ok()?;
        let json: serde_json::Value = serde_json::from_str(&text).ok()?;
        json["status"].as_str().map(str::to_string)
    }

    fn run(&self, args: &[&str]) -> Output {
        let mut cmd = Command::new(assert_cmd::cargo::cargo_bin("git-paw"));
        self.tmux
            .apply(&mut cmd)
            .current_dir(&self.root)
            .env("HOME", self.home.path())
            .env_remove("XDG_DATA_HOME")
            .args(args);
        cmd.output().expect("run git-paw")
    }

    /// Replaces run-specific values in `text` with placeholders.
    fn normalize(&self, text: &str, tips: &[(&str, &str)]) -> String {
        let mut out = text.replace(&self.session, "{SESSION}");
        let sandbox = self.root.parent().expect("parent").to_string_lossy();
        out = out.replace(&self.root.to_string_lossy().to_string(), "{REPO}");
        out = out.replace(sandbox.as_ref(), "{SANDBOX}");
        for (tip, name) in tips {
            out = out.replace(tip, name);
        }
        let elapsed = regex::Regex::new(r"\d+\.\ds\)").expect("regex");
        elapsed.replace_all(&out, "N.Ns)").into_owned()
    }

    fn check(&self, out: &Output, code: i32, stdout: &str, stderr: &str, tips: &[(&str, &str)]) {
        let got_out = self.normalize(&String::from_utf8_lossy(&out.stdout), tips);
        let got_err = self.normalize(&String::from_utf8_lossy(&out.stderr), tips);
        assert_eq!(
            (out.status.code(), got_out.as_str(), got_err.as_str()),
            (Some(code), stdout, stderr),
            "exit code / stdout / stderr drifted from the golden output"
        );
    }
}

fn git_out(dir: &Path, args: &[&str]) -> String {
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

fn short_tip(dir: &Path, branch: &str) -> String {
    git_out(dir, &["rev-parse", "--short", branch])
}

/// A port held for the caller's lifetime by a listener that never answers,
/// so no other process can take it and make the broker look alive.
fn silent_port() -> (std::net::TcpListener, u16) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    (listener, port)
}

// ---------------------------------------------------------------------------
// status
// ---------------------------------------------------------------------------

#[test]
fn golden_status_without_a_session() {
    let f = Fixture::new();
    f.check(
        &f.run(&["status"]),
        0,
        "No session for this repo.\n",
        "",
        &[],
    );
    f.check(
        &f.run(&["status", "--json"]),
        0,
        "{\"session\":null}\n",
        "",
        &[],
    );
}

#[test]
fn golden_status_of_a_stopped_session_with_a_silent_broker() {
    let f = Fixture::new();
    let wt = f.worktree("feat/alpha", false);
    let (_held, port) = silent_port();
    f.receipt("stopped", &[("feat/alpha", &wt)], Some(port));
    let out = f.run(&["status"]);
    let port = {
        let text = fs::read_to_string(f.sessions_dir().join(format!("{}.json", f.session)))
            .expect("receipt");
        let json: serde_json::Value = serde_json::from_str(&text).expect("json");
        json["broker_port"].as_u64().expect("port").to_string()
    };
    let expected = format!(
        "Session: {{SESSION}}\n\
         Status:  \u{1f7e1} stopped\n\
         Tmux:    not running\n\
         \n\
         Broker:  http://127.0.0.1:{port} (not responding)\n\
         \n  feat/alpha \u{2192} sleep ({{SANDBOX}}/wt-feat-alpha)\n"
    );
    f.check(&out, 0, &expected, "", &[]);
}

#[test]
fn golden_status_of_a_live_session() {
    let f = Fixture::new();
    let wt = f.worktree("feat/alpha", false);
    f.receipt("active", &[("feat/alpha", &wt)], None);
    f.live_tmux();
    f.check(
        &f.run(&["status"]),
        0,
        "Session: {SESSION}\n\
         Status:  \u{1f7e2} active\n\
         Tmux:    running\n\
         \n  feat/alpha \u{2192} sleep ({SANDBOX}/wt-feat-alpha)\n",
        "",
        &[],
    );
    f.check(
        &f.run(&["status", "--json"]),
        0,
        "{\"session\":\"{SESSION}\",\"status\":\"active\",\"tmux_running\":true,\
         \"worktrees\":[{\"branch\":\"feat/alpha\",\"cli\":\"sleep\",\
         \"worktree_path\":\"{SANDBOX}/wt-feat-alpha\",\"cli_session_id\":null}]}\n",
        "",
        &[],
    );
}

// ---------------------------------------------------------------------------
// pause
// ---------------------------------------------------------------------------

#[test]
fn golden_pause_without_a_session() {
    let f = Fixture::new();
    f.check(
        &f.run(&["pause"]),
        0,
        "No active session for this repo.\n",
        "",
        &[],
    );
}

#[test]
fn golden_pause_of_a_live_session() {
    let f = Fixture::new();
    let wt = f.worktree("feat/alpha", false);
    f.receipt("active", &[("feat/alpha", &wt)], None);
    f.live_tmux();
    f.check(
        &f.run(&["pause"]),
        0,
        "Session '{SESSION}' paused. 1 CLI pane(s) still running. Run 'git paw start' to resume.\n",
        "",
        &[],
    );
    assert!(f.tmux_alive(), "pause leaves the panes running");
    assert_eq!(f.receipt_status().as_deref(), Some("paused"));
    f.check(
        &f.run(&["pause"]),
        0,
        "Session '{SESSION}' is already paused.\n",
        "",
        &[],
    );
}

#[test]
fn golden_pause_of_a_dead_session() {
    let f = Fixture::new();
    let wt = f.worktree("feat/alpha", false);
    f.receipt("active", &[("feat/alpha", &wt)], None);
    f.check(
        &f.run(&["pause"]),
        0,
        "Session '{SESSION}' is already stopped; pause has no effect.\n",
        "",
        &[],
    );
}

// ---------------------------------------------------------------------------
// stop
// ---------------------------------------------------------------------------

#[test]
fn golden_stop_without_a_session() {
    let f = Fixture::new();
    f.check(
        &f.run(&["stop"]),
        0,
        "No active session for this repo.\n",
        "",
        &[],
    );
}

#[test]
fn golden_stop_of_a_live_session() {
    let f = Fixture::new();
    let wt = f.worktree("feat/alpha", false);
    f.receipt("active", &[("feat/alpha", &wt)], None);
    f.live_tmux();
    f.check(
        &f.run(&["stop"]),
        0,
        "Session stopped. Worktrees and state preserved.\nRun `git paw start` to recover.\n",
        "",
        &[],
    );
    assert!(!f.tmux_alive(), "stop kills the session");
    assert_eq!(f.receipt_status().as_deref(), Some("stopped"));
    assert!(wt.exists(), "stop preserves worktrees");
}

// ---------------------------------------------------------------------------
// purge --force
// ---------------------------------------------------------------------------

#[test]
fn golden_purge_force_without_a_session() {
    let f = Fixture::new();
    f.check(
        &f.run(&["purge", "--force"]),
        0,
        "No session to purge for this repo.\n",
        "",
        &[],
    );
}

#[test]
fn golden_purge_force_deletes_merged_and_keeps_unmerged_branches() {
    let f = Fixture::new();
    let merged = f.worktree("feat/merged", false);
    let unmerged = f.worktree("feat/unmerged", true);
    f.receipt(
        "active",
        &[("feat/merged", &merged), ("feat/unmerged", &unmerged)],
        None,
    );
    f.live_tmux();
    let merged_tip = short_tip(&f.root, "feat/merged");
    let unmerged_tip = short_tip(&f.root, "feat/unmerged");
    let tips = [
        (unmerged_tip.as_str(), "{UNMERGED_TIP}"),
        (merged_tip.as_str(), "{MERGED_TIP}"),
    ];
    f.check(
        &f.run(&["purge", "--force"]),
        0,
        "Purged session '{SESSION}'.\n",
        "Warning: 1 branch(es) have unmerged commits and will be kept:\n  \
         feat/unmerged ({UNMERGED_TIP}): 1 commit(s) not in main\n\
         Their worktrees (including any uncommitted changes) and the session state will be \
         removed; the branches themselves are kept.\n\
         Removing worktree {SANDBOX}/wt-feat-merged...\n  \
         ...done (N.Ns)\n\
         Deleted branch feat/merged (was {MERGED_TIP})\n\
         Removing worktree {SANDBOX}/wt-feat-unmerged...\n  \
         ...done (N.Ns)\n\
         Kept branch feat/unmerged at {UNMERGED_TIP} (1 commit(s) not in main)\n  \
         resume it with: git worktree add '{SANDBOX}/wt-feat-unmerged' feat/unmerged\n  \
         discard it with: git branch -D feat/unmerged\n",
        &tips,
    );
    assert!(!f.tmux_alive(), "purge kills the session");
    assert_eq!(f.receipt_status(), None, "purge deletes the receipt");
    assert!(!merged.exists() && !unmerged.exists(), "worktrees removed");
}
