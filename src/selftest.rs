//! `git paw selftest` — an isolated, end-to-end session-lifecycle smoke check.
//!
//! The harness exercises the riskiest orchestration plumbing (`start` → add →
//! remove → `stop`) against a throwaway repository and a dummy CLI, with **no
//! real AI CLI backend and no interactive terminal**, then reports a single
//! pass/fail verdict. It packages the dogfood isolation recipe that previously
//! lived only as ad-hoc shell incantations:
//!
//! - a **private tmux socket** dedicated to the run, via a per-run
//!   `TMUX_TMPDIR`, with `TMUX` and `TMUX_PANE` stripped from every child so
//!   the spawned tmux server never attaches to the caller's session;
//! - an **OS-assigned ephemeral broker port** (`bind 127.0.0.1:0`, read back),
//!   so concurrent runs never collide on a fixed or PID-derived port;
//! - a **throwaway git repository** under `.git-paw/tmp/` (a stale-dir sweep
//!   removes any prior aborted run before a fresh one is created);
//! - an **isolated `HOME`/XDG** so the global session receipt never touches the
//!   user's real sessions directory;
//! - a **dummy CLI** (`cat`) in place of a real agent CLI, so the session boots
//!   deterministically in detached mode without spawning an LLM.
//!
//! The harness drives the lifecycle by re-invoking the running `git-paw` binary
//! ([`std::env::current_exe`]) as child processes — the same code path a user
//! exercises — and observes the agent **roster** through the per-repo discovery
//! file (`<repo>/.git-paw/sessions/<name>.json`), which is independent of the
//! isolated `HOME`. After starting it adds an agent worktree and asserts the
//! roster grows, then removes it and asserts the roster shrinks.
//!
//! Cleanup runs on **both** the success and failure paths: the private-socket
//! tmux server is killed and the `.git-paw/tmp/` throwaway tree is removed. When
//! tmux is unavailable the harness skips with a message and exits zero; it exits
//! non-zero only on an actual lifecycle failure, naming the failing step.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::error::PawError;
use crate::session::{RepoSessionFile, repo_session_path};

/// Settle budget (ms) for the per-pane CLI-readiness gate during child
/// `git paw start`/`add` launches. The dummy `cat` pane never matches a real
/// CLI's interactive marker, so the gate always falls back after this budget;
/// a short value keeps each launch fast rather than paying the production
/// per-pane default.
const READINESS_TIMEOUT_MS: &str = "120";

/// The dummy CLI used in place of a real AI CLI. `cat` holds its pane open
/// without producing output or requiring input, so the session boots
/// deterministically with no LLM process spawned.
const DUMMY_CLI: &str = "cat";

/// Initial agent branch started with the session.
const INITIAL_BRANCH: &str = "selftest-a";

/// Agent branch added and then removed to observe the roster transitions.
const TRANSIENT_BRANCH: &str = "selftest-b";

/// Environment variable that injects a forced failure at a named lifecycle
/// step, used by the integration suite to verify the non-zero/named-step
/// failure path. When its value equals one of the [`step`] names, that step
/// fails deterministically.
const FORCE_FAIL_ENV: &str = "GIT_PAW_SELFTEST_FORCE_FAIL";

/// Stable lifecycle-step names. Used both in the failure message (so the
/// verdict names the failing step) and as the values accepted by
/// [`FORCE_FAIL_ENV`].
mod step {
    pub const PICK_PORT: &str = "pick-port";
    pub const CREATE_REPO: &str = "create-repo";
    pub const START: &str = "start";
    pub const ROSTER_INITIAL: &str = "roster-initial";
    pub const ADD: &str = "add";
    pub const ROSTER_AFTER_ADD: &str = "roster-after-add";
    pub const REMOVE: &str = "remove";
    pub const ROSTER_AFTER_REMOVE: &str = "roster-after-remove";
    pub const STOP: &str = "stop";
}

/// Runs the isolated session-lifecycle selftest and reports a pass/fail verdict.
///
/// Returns `Ok(())` after printing `selftest passed` when the full lifecycle
/// completes, or after printing a skip message when tmux is unavailable (a skip
/// is not a failure). Returns an error naming the failing step otherwise; the
/// caller maps the error to a non-zero process exit code.
///
/// # Errors
///
/// Returns [`PawError::SessionError`] naming the failing lifecycle step when any
/// step fails, or [`PawError::NotAGitRepo`] when not invoked from inside a git
/// repository.
pub fn run() -> Result<(), PawError> {
    if !tmux_available() {
        println!("selftest skipped: tmux not available");
        return Ok(());
    }

    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = crate::git::validate_repo(&cwd)?;

    let mut harness = Harness::new(&repo_root)?;
    let result = harness.run_lifecycle();
    harness.cleanup();

    match result {
        Ok(()) => {
            println!("selftest passed");
            Ok(())
        }
        Err(err) => Err(err),
    }
}

/// Returns `true` when a `tmux` binary is callable on `PATH`.
fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Allocates an OS-assigned ephemeral TCP port on the loopback interface.
///
/// Binds `127.0.0.1:0`, reads back the kernel-assigned local port, and releases
/// the listener so the broker can claim it. The kernel guarantees each bind
/// returns a port not currently in use, so this is collision-proof at any
/// concurrency — unlike a fixed or PID-derived port. This mirrors the canonical
/// `tests/e2e_supervisor_stop.rs::pick_broker_port` helper.
fn pick_broker_port() -> Result<u16, PawError> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|e| PawError::SessionError(format!("could not bind an ephemeral port: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| PawError::SessionError(format!("could not read local address: {e}")))?
        .port();
    // `listener` drops here, releasing the port for the broker to bind.
    Ok(port)
}

/// Builds a step-named failure so the verdict names the failing step.
fn step_failure(step: &str, detail: impl std::fmt::Display) -> PawError {
    PawError::SessionError(format!("selftest failed at step '{step}': {detail}"))
}

/// Returns an injected failure when [`FORCE_FAIL_ENV`] names `step`, else `Ok`.
fn maybe_force_fail(step: &str) -> Result<(), PawError> {
    match std::env::var(FORCE_FAIL_ENV) {
        Ok(target) if target == step => Err(step_failure(
            step,
            format!("forced failure injected via {FORCE_FAIL_ENV}"),
        )),
        _ => Ok(()),
    }
}

/// Parses the session name from `git paw start` stdout (the `tmux attach -t
/// <name>` hint line).
fn extract_session_name(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
        .map(str::to_string)
}

/// Owns the isolated resources for one selftest run and drives the lifecycle.
struct Harness {
    /// The running `git-paw` binary, re-invoked for each lifecycle subcommand.
    exe: PathBuf,
    /// The `.git-paw/tmp/` namespace root (removed wholesale on cleanup).
    tmp_root: PathBuf,
    /// The throwaway git repository (`<tmp_root>/repo`).
    repo: PathBuf,
    /// Isolated `HOME`, so the global session receipt never touches the user's
    /// real data directory (`<tmp_root>/home`).
    home: PathBuf,
    /// Private tmux socket directory (`TMUX_TMPDIR`). Lives under the system
    /// temp dir to keep the unix socket path short.
    socket_dir: PathBuf,
    /// OS-assigned ephemeral broker port, allocated in the `pick-port` step.
    port: u16,
    /// Tmux session name, learned from `start` stdout.
    session_name: Option<String>,
}

impl Harness {
    /// Sweeps any prior aborted run, then provisions the isolated directories.
    ///
    /// Creating the directories here (before any lifecycle step) guarantees
    /// [`Self::cleanup`] always has something well-defined to remove on both
    /// the success and failure paths.
    fn new(repo_root: &Path) -> Result<Self, PawError> {
        let exe = std::env::current_exe()
            .map_err(|e| PawError::SessionError(format!("cannot resolve git-paw binary: {e}")))?;

        let tmp_root = repo_root.join(".git-paw").join("tmp");
        // Stale-dir sweep: remove any leftover directory from a prior aborted
        // run before creating a fresh one.
        if tmp_root.exists() {
            let _ = std::fs::remove_dir_all(&tmp_root);
        }
        std::fs::create_dir_all(&tmp_root).map_err(|e| {
            PawError::SessionError(format!("could not create selftest tmp dir: {e}"))
        })?;

        let repo = tmp_root.join("repo");
        let home = tmp_root.join("home");
        std::fs::create_dir_all(&home)
            .map_err(|e| PawError::SessionError(format!("could not create isolated HOME: {e}")))?;

        // The tmux socket lives under the system temp dir (a short path) so the
        // unix socket path stays under the platform length limit; a path under
        // the repo's `.git-paw/tmp/` could exceed it on deep checkouts.
        let socket_dir =
            std::env::temp_dir().join(format!("git-paw-selftest-{}", std::process::id()));
        if socket_dir.exists() {
            let _ = std::fs::remove_dir_all(&socket_dir);
        }
        std::fs::create_dir_all(&socket_dir).map_err(|e| {
            PawError::SessionError(format!("could not create private tmux socket dir: {e}"))
        })?;

        Ok(Self {
            exe,
            tmp_root,
            repo,
            home,
            socket_dir,
            port: 0,
            session_name: None,
        })
    }

    /// Builds a `git-paw` child command with the full isolation environment
    /// applied and the working directory set to the throwaway repo.
    fn paw(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(&self.exe);
        cmd.args(args);
        self.isolate(&mut cmd);
        cmd.current_dir(&self.repo);
        cmd
    }

    /// Applies the isolation recipe to any child `Command`: private tmux
    /// socket, isolated `HOME`/XDG, short readiness budget, and `TMUX` /
    /// `TMUX_PANE` removed so the child does not attach to the caller's tmux
    /// server.
    fn isolate(&self, cmd: &mut Command) {
        cmd.env("TMUX_TMPDIR", &self.socket_dir)
            .env("HOME", &self.home)
            .env("XDG_DATA_HOME", self.home.join(".local/share"))
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("GIT_PAW_READINESS_TIMEOUT_MS", READINESS_TIMEOUT_MS)
            .env_remove("TMUX")
            .env_remove("TMUX_PANE");
    }

    /// Drives the full lifecycle, returning a step-named error on the first
    /// failure. Cleanup is the caller's responsibility (run unconditionally).
    fn run_lifecycle(&mut self) -> Result<(), PawError> {
        // 1. Allocate an OS-assigned ephemeral broker port.
        maybe_force_fail(step::PICK_PORT)?;
        self.port = pick_broker_port()?;

        // 2. Create the throwaway repo + config under `.git-paw/tmp/`.
        maybe_force_fail(step::CREATE_REPO)?;
        self.create_throwaway_repo()?;

        // 3. Start an isolated supervisor session with the dummy CLI.
        maybe_force_fail(step::START)?;
        self.start_session()?;

        // 4. The starting roster holds exactly the initial agent.
        maybe_force_fail(step::ROSTER_INITIAL)?;
        let roster = self.read_roster()?;
        println!("selftest: roster after start: {}", render_roster(&roster));
        if roster != [slug(INITIAL_BRANCH)] {
            return Err(step_failure(
                step::ROSTER_INITIAL,
                format!("expected roster [{}], got {roster:?}", slug(INITIAL_BRANCH)),
            ));
        }

        // 5. Add an agent worktree.
        maybe_force_fail(step::ADD)?;
        self.run_paw_step(step::ADD, &["add", TRANSIENT_BRANCH])?;

        // 6. The roster grew to include the added agent.
        maybe_force_fail(step::ROSTER_AFTER_ADD)?;
        let roster = self.read_roster()?;
        println!("selftest: roster after add: {}", render_roster(&roster));
        if !roster.contains(&slug(TRANSIENT_BRANCH)) || !roster.contains(&slug(INITIAL_BRANCH)) {
            return Err(step_failure(
                step::ROSTER_AFTER_ADD,
                format!(
                    "expected roster to include {} and {}, got {roster:?}",
                    slug(INITIAL_BRANCH),
                    slug(TRANSIENT_BRANCH),
                ),
            ));
        }

        // 7. Remove the added agent worktree.
        maybe_force_fail(step::REMOVE)?;
        self.run_paw_step(step::REMOVE, &["remove", TRANSIENT_BRANCH])?;

        // 8. The roster shrank: the removed agent is gone, the rest unchanged.
        maybe_force_fail(step::ROSTER_AFTER_REMOVE)?;
        let roster = self.read_roster()?;
        println!("selftest: roster after remove: {}", render_roster(&roster));
        if roster != [slug(INITIAL_BRANCH)] {
            return Err(step_failure(
                step::ROSTER_AFTER_REMOVE,
                format!(
                    "expected roster [{}] after remove, got {roster:?}",
                    slug(INITIAL_BRANCH),
                ),
            ));
        }

        // 9. Tear the session down.
        maybe_force_fail(step::STOP)?;
        self.run_paw_step(step::STOP, &["stop", "--force"])?;

        Ok(())
    }

    /// `git init`s the throwaway repo with one base commit and writes a
    /// supervisor-mode config wiring the dummy CLI and the ephemeral broker
    /// port. The repo is configured with a local committer identity so it does
    /// not depend on the isolated (empty) global git config.
    fn create_throwaway_repo(&self) -> Result<(), PawError> {
        std::fs::create_dir_all(&self.repo)
            .map_err(|e| step_failure(step::CREATE_REPO, format!("create repo dir: {e}")))?;

        self.git(&["init", "-b", "main"])?;
        self.git(&["config", "user.email", "selftest@git-paw.invalid"])?;
        self.git(&["config", "user.name", "git-paw selftest"])?;

        std::fs::write(self.repo.join("README.md"), "# git-paw selftest\n")
            .map_err(|e| step_failure(step::CREATE_REPO, format!("write README: {e}")))?;
        self.git(&["add", "."])?;
        self.git(&["commit", "-m", "selftest base commit"])?;

        let paw_dir = self.repo.join(".git-paw");
        std::fs::create_dir_all(&paw_dir)
            .map_err(|e| step_failure(step::CREATE_REPO, format!("create .git-paw: {e}")))?;
        let config = format!(
            "default_cli = \"{DUMMY_CLI}\"\n\n\
             [broker]\n\
             enabled = true\n\
             port = {port}\n\n\
             [supervisor]\n\
             enabled = true\n\
             cli = \"{DUMMY_CLI}\"\n",
            port = self.port,
        );
        std::fs::write(paw_dir.join("config.toml"), config)
            .map_err(|e| step_failure(step::CREATE_REPO, format!("write config.toml: {e}")))?;
        Ok(())
    }

    /// Runs a git subcommand in the throwaway repo under the isolated env.
    fn git(&self, args: &[&str]) -> Result<(), PawError> {
        let mut cmd = Command::new("git");
        cmd.args(args);
        self.isolate(&mut cmd);
        cmd.current_dir(&self.repo);
        let out = cmd
            .output()
            .map_err(|e| step_failure(step::CREATE_REPO, format!("git {args:?}: {e}")))?;
        if !out.status.success() {
            return Err(step_failure(
                step::CREATE_REPO,
                format!(
                    "git {args:?} failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            ));
        }
        Ok(())
    }

    /// Launches the isolated supervisor session with the dummy CLI, records the
    /// session name, and confirms the session landed on the private socket.
    fn start_session(&mut self) -> Result<(), PawError> {
        let out = self
            .paw(&["start", "--supervisor", "--branches", INITIAL_BRANCH])
            .output()
            .map_err(|e| step_failure(step::START, format!("spawn start: {e}")))?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        if !out.status.success() {
            return Err(step_failure(
                step::START,
                format!(
                    "start exited non-zero; stdout: {}; stderr: {}",
                    stdout.trim(),
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            ));
        }
        let name = extract_session_name(&stdout).ok_or_else(|| {
            step_failure(
                step::START,
                format!("no session name in start output: {stdout}"),
            )
        })?;

        // The session must be live on the harness's private socket.
        if !self.session_on_private_socket(&name) {
            return Err(step_failure(
                step::START,
                format!("session '{name}' did not appear on the private tmux socket"),
            ));
        }
        println!("selftest: session '{name}' booted on its private tmux socket");
        self.session_name = Some(name);
        Ok(())
    }

    /// Runs a `git-paw` lifecycle subcommand, mapping a non-zero exit to a
    /// step-named failure carrying the child's stderr.
    fn run_paw_step(&self, step: &str, args: &[&str]) -> Result<(), PawError> {
        let out = self
            .paw(args)
            .output()
            .map_err(|e| step_failure(step, format!("spawn {args:?}: {e}")))?;
        if !out.status.success() {
            return Err(step_failure(
                step,
                format!(
                    "{args:?} exited non-zero; stderr: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            ));
        }
        Ok(())
    }

    /// Reads the observable agent roster from the per-repo discovery file
    /// (`<repo>/.git-paw/sessions/<name>.json`), returning the `branch_id`s.
    /// This file lives inside the throwaway repo, so it is observable
    /// independent of the isolated `HOME`.
    fn read_roster(&self) -> Result<Vec<String>, PawError> {
        let name = self
            .session_name
            .as_deref()
            .ok_or_else(|| step_failure(step::ROSTER_INITIAL, "no session name recorded"))?;
        let path = repo_session_path(&self.repo, name);
        let raw = std::fs::read_to_string(&path).map_err(|e| {
            step_failure(
                step::ROSTER_INITIAL,
                format!("read discovery file {}: {e}", path.display()),
            )
        })?;
        let file: RepoSessionFile = serde_json::from_str(&raw).map_err(|e| {
            step_failure(step::ROSTER_INITIAL, format!("parse discovery file: {e}"))
        })?;
        Ok(file.agents.into_iter().map(|a| a.branch_id).collect())
    }

    /// Returns `true` when a tmux session named `name` is live on the harness's
    /// private socket.
    fn session_on_private_socket(&self, name: &str) -> bool {
        let mut cmd = Command::new("tmux");
        cmd.args(["has-session", "-t", name]);
        self.isolate(&mut cmd);
        cmd.output().is_ok_and(|o| o.status.success())
    }

    /// Tears down every resource the run created. Idempotent and best-effort:
    /// kills the private-socket tmux server and removes the `.git-paw/tmp/`
    /// throwaway tree and the private socket directory. Runs on both the
    /// success and failure paths.
    fn cleanup(&self) {
        // Kill the whole private-socket server (covers the case where `stop`
        // never ran because an earlier step failed). tmux sends SIGHUP to each
        // pane process; the broker lives in the dashboard pane and shuts down
        // *gracefully* on SIGHUP — it drops its `BrokerHandle`, which does a
        // final learnings/log flush that writes back under the throwaway tree.
        let mut kill = Command::new("tmux");
        kill.arg("kill-server");
        self.isolate(&mut kill);
        let _ = kill.output();

        // Because that shutdown flush writes into `.git-paw/tmp/`, a naive
        // `remove_dir_all` right after `kill-server` races the flush and leaves
        // a non-empty (thus un-rmdir-able) tree — the failure path never runs
        // `stop`, so nothing else drains the broker first. Wait for the broker
        // to release its port (its server task has stopped), then retry the
        // removal until the tree is actually gone or a bounded budget elapses;
        // once a removal fully succeeds, a late flush write cannot recreate the
        // tree (its parent directory no longer exists).
        if self.port != 0 {
            await_port_free(self.port, Duration::from_secs(10));
        }
        remove_dir_all_with_retry(&self.tmp_root, Duration::from_secs(10));
        let _ = std::fs::remove_dir_all(&self.socket_dir);
    }
}

/// Polls until nothing is listening on `127.0.0.1:port` (the broker released
/// it) or `budget` elapses. Best-effort: it only lets the broker's graceful
/// shutdown drain before we remove its working tree, so a timeout is not an
/// error — the retry in [`remove_dir_all_with_retry`] is the real safety net.
fn await_port_free(port: u16, budget: Duration) {
    let deadline = Instant::now() + budget;
    loop {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return;
        }
        if Instant::now() >= deadline {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Removes `dir` recursively, retrying until it is gone or `budget` elapses.
///
/// The broker's graceful-shutdown flush can write files back into the tree
/// after removal starts, so a single `remove_dir_all` may fail with
/// `ENOTEMPTY`; each retry sweeps any straggler. Best-effort — if the budget
/// elapses with the directory still present the caller (and the selftest
/// integration test) surfaces the lingering tree rather than this masking it.
fn remove_dir_all_with_retry(dir: &Path, budget: Duration) {
    let deadline = Instant::now() + budget;
    loop {
        let _ = std::fs::remove_dir_all(dir);
        if !dir.exists() {
            return;
        }
        if Instant::now() >= deadline {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Slugifies a branch name into the `branch_id` form the discovery file uses.
fn slug(branch: &str) -> String {
    crate::broker::messages::slugify_branch(branch)
}

/// Renders a roster (`branch_id`s) as a comma-separated list for progress
/// output, or `(empty)` when there are no agents.
fn render_roster(roster: &[String]) -> String {
    if roster.is_empty() {
        "(empty)".to_string()
    } else {
        roster.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_broker_port_returns_an_immediately_bindable_port() {
        let port = pick_broker_port().expect("pick a port");
        assert_ne!(port, 0, "port should be a real OS-assigned port");
        // The port must be free to bind right after the helper releases it.
        let rebound = std::net::TcpListener::bind(("127.0.0.1", port));
        assert!(
            rebound.is_ok(),
            "the returned port {port} should be immediately bindable"
        );
    }

    #[test]
    fn two_helper_calls_yield_distinct_ports_under_concurrency() {
        use std::thread;

        // Each call binds 127.0.0.1:0; while both listeners are simultaneously
        // held open (below), the kernel cannot have handed out the same port.
        let handles: Vec<_> = (0..8)
            .map(|_| {
                thread::spawn(|| {
                    let listener =
                        std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
                    let port = listener.local_addr().expect("local addr").port();
                    // Hold the listener so the ports cannot be reused mid-test.
                    (port, listener)
                })
            })
            .collect();

        let mut ports: Vec<u16> = handles
            .into_iter()
            .map(|h| h.join().expect("thread joined").0)
            .collect();
        ports.sort_unstable();
        let distinct = {
            let mut p = ports.clone();
            p.dedup();
            p.len()
        };
        assert_eq!(
            distinct,
            ports.len(),
            "concurrent ephemeral binds must yield distinct ports, got {ports:?}"
        );
    }

    #[test]
    fn maybe_force_fail_triggers_only_for_the_named_step() {
        // SAFETY: this test is the sole mutator of FORCE_FAIL_ENV and runs in a
        // process whose other tests do not read it; set then remove.
        unsafe {
            std::env::set_var(FORCE_FAIL_ENV, step::ADD);
        }
        assert!(maybe_force_fail(step::ADD).is_err(), "named step must fail");
        assert!(
            maybe_force_fail(step::START).is_ok(),
            "a different step must not fail"
        );
        unsafe {
            std::env::remove_var(FORCE_FAIL_ENV);
        }
        assert!(
            maybe_force_fail(step::ADD).is_ok(),
            "unset env must not fail any step"
        );
    }

    #[test]
    fn step_failure_message_names_the_step() {
        let err = step_failure(step::STOP, "boom");
        let msg = err.to_string();
        assert!(
            msg.contains(step::STOP),
            "message should name the step: {msg}"
        );
        assert!(
            msg.contains("boom"),
            "message should carry the detail: {msg}"
        );
    }

    #[test]
    fn extract_session_name_parses_the_attach_hint() {
        let stdout = "Supervisor session 'paw-repo' launched with 1 coding agent(s).\n\
                      Attach with:  tmux attach -t paw-repo\n";
        assert_eq!(extract_session_name(stdout).as_deref(), Some("paw-repo"));
        assert!(
            extract_session_name("no hint here").is_none(),
            "absent hint yields None"
        );
    }

    #[test]
    fn slug_matches_discovery_file_branch_id_form() {
        assert_eq!(slug("selftest-a"), "selftest-a");
        assert_eq!(slug("feat/sel-b"), "feat-sel-b");
    }

    #[test]
    fn remove_dir_all_with_retry_removes_a_populated_tree() {
        let base = std::env::temp_dir().join(format!("git-paw-selftest-rm-{}", std::process::id()));
        let nested = base.join("a").join("b");
        std::fs::create_dir_all(&nested).expect("create nested dirs");
        std::fs::write(nested.join("f.txt"), "x").expect("write file");

        remove_dir_all_with_retry(&base, Duration::from_secs(2));
        assert!(!base.exists(), "the populated tree should be removed");
    }

    #[test]
    fn remove_dir_all_with_retry_is_a_noop_on_a_missing_dir() {
        let missing =
            std::env::temp_dir().join(format!("git-paw-selftest-absent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&missing);
        // Must return promptly (well under the budget) without panicking.
        remove_dir_all_with_retry(&missing, Duration::from_secs(2));
        assert!(!missing.exists());
    }

    #[test]
    fn await_port_free_returns_immediately_when_port_is_free() {
        // Bind then release to get a port that is currently free.
        let port = pick_broker_port().expect("pick a free port");
        let start = Instant::now();
        await_port_free(port, Duration::from_secs(5));
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "a free port should be detected on the first poll"
        );
    }
}
