//! Shared test helpers for integration tests.
//!
//! Provides utilities used across multiple integration test files, such as
//! temporary git repository creation and PATH manipulation helpers.
//!
//! Each integration test binary compiles its own copy of this module via
//! `mod helpers;`. Not every binary uses every helper, so dead-code lints
//! are silenced at the module level rather than per-item.
#![allow(dead_code)]
//!
//! # tmux test isolation
//!
//! Every integration test that spawns `tmux` (directly via
//! [`std::process::Command::new("tmux")`] or transitively through a `git paw`
//! subcommand) MUST use [`tmux_test_env`] to point that subprocess at a
//! test-owned tmux socket directory. Without the helper the subprocess uses
//! the user's default tmux socket, where a test-induced server crash or
//! `kill-session` collision can destroy the live `paw-git-paw` supervisor
//! session.
//!
//! The collision-guard [`guard_against_live_session`] runs as the first
//! statement of [`setup_test_repo`] and panics fast if a `paw-*` session is
//! live on the default socket. Export `GIT_PAW_ALLOW_LIVE_SESSION=1` to opt
//! out (e.g. when you have verified that every test in the targeted run is
//! already socket-isolated).
//!
//! see openspec/changes/test-tmux-isolation

use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// Low launch-readiness budget for socket-isolated e2e tests. Test launches use
/// fake CLIs (`echo`) whose panes never match a CLI-readiness marker, so the
/// gate always falls back after its per-attempt budget; a short timeout keeps
/// that fall-back fast instead of paying the production default per pane.
const TEST_READINESS_TIMEOUT_MS: &str = "120";

/// A test sandbox containing a git repository.
///
/// The repo lives at `<sandbox>/repo/` so that worktrees created at
/// `../<project>-<branch>/` land inside `<sandbox>/` and are automatically
/// cleaned up when dropped.
pub struct TestRepo {
    _sandbox: TempDir,
    repo: PathBuf,
}

impl TestRepo {
    /// Returns the path to the git repository root.
    pub fn path(&self) -> &Path {
        &self.repo
    }
}

/// Per-test tmux socket isolation.
///
/// Owns a [`TempDir`] under which tmux creates its socket
/// (`<socket_dir>/tmux-<uid>/default`). Every `Command` builder that will
/// spawn tmux (directly, or transitively via a `git paw` subcommand) MUST
/// have [`TmuxTestEnv::apply`] called on it so the child process sees the
/// test-owned socket directory instead of the user's default socket.
///
/// Callers MUST keep the [`TmuxTestEnv`] alive for at least as long as the
/// tmux server it isolates — dropping the struct removes the socket
/// directory, which kills the server.
///
/// see openspec/changes/test-tmux-isolation
pub struct TmuxTestEnv {
    socket_dir: TempDir,
}

impl TmuxTestEnv {
    /// Creates a fresh tmux socket directory under [`std::env::temp_dir`].
    pub fn new() -> Self {
        let socket_dir = TempDir::new().expect("create tmux socket tempdir");
        Self { socket_dir }
    }

    /// Returns the test-owned socket directory.
    ///
    /// The directory's lifetime is tied to this `TmuxTestEnv`; keep the
    /// owning value bound to a local variable.
    pub fn socket_dir(&self) -> &Path {
        self.socket_dir.path()
    }

    /// Configures `cmd` so the spawned tmux subprocess uses this test's
    /// socket directory and behaves as a standalone outer client.
    ///
    /// Sets `TMUX_TMPDIR` to [`Self::socket_dir`] and removes `TMUX` and
    /// `TMUX_PANE` from the child environment so the subprocess does not
    /// mistake itself for an inside-tmux client of an outer server.
    ///
    /// Callers MUST keep the [`TmuxTestEnv`] alive for at least as long as
    /// the tmux server the spawned command starts — when the helper is
    /// dropped the socket directory is removed and any live tmux server
    /// inside it dies with it.
    pub fn apply<'a>(&self, cmd: &'a mut Command) -> &'a mut Command {
        cmd.env("TMUX_TMPDIR", self.socket_dir.path())
            .env("GIT_PAW_READINESS_TIMEOUT_MS", TEST_READINESS_TIMEOUT_MS)
            .env_remove("TMUX")
            .env_remove("TMUX_PANE")
    }

    /// Variant of [`Self::apply`] for `assert_cmd::Command` (the test
    /// harness wrapper around `std::process::Command`).
    pub fn apply_assert<'a>(
        &self,
        cmd: &'a mut assert_cmd::Command,
    ) -> &'a mut assert_cmd::Command {
        cmd.env("TMUX_TMPDIR", self.socket_dir.path())
            .env("GIT_PAW_READINESS_TIMEOUT_MS", TEST_READINESS_TIMEOUT_MS)
            .env_remove("TMUX")
            .env_remove("TMUX_PANE")
    }

    /// Apply the same env mutations to the **current process** for tests
    /// that exercise in-process library code which itself spawns tmux
    /// (e.g. `git_paw::dashboard::send_reply_to_pane`).
    ///
    /// Returns a guard that restores the previous `TMUX_TMPDIR`, `TMUX`,
    /// and `TMUX_PANE` values when dropped.
    ///
    /// Callers MUST gate the test with `#[serial_test::serial]` (or
    /// otherwise serialise it) because this mutates global process state.
    /// The struct's `Drop` impl uses `std::env::set_var` /
    /// `std::env::remove_var` under the same serialisation contract.
    pub fn apply_to_process(&self) -> ProcessTmuxEnvGuard {
        ProcessTmuxEnvGuard::set(self.socket_dir.path())
    }
}

/// RAII guard returned by [`TmuxTestEnv::apply_to_process`]. Restores the
/// previous `TMUX_TMPDIR`, `TMUX`, and `TMUX_PANE` values on drop.
///
/// see openspec/changes/test-tmux-isolation
#[allow(clippy::struct_field_names)]
pub struct ProcessTmuxEnvGuard {
    previous_tmpdir: Option<OsString>,
    previous_tmux: Option<OsString>,
    previous_pane: Option<OsString>,
    previous_readiness: Option<OsString>,
}

impl ProcessTmuxEnvGuard {
    fn set(socket_dir: &Path) -> Self {
        let previous_tmpdir = std::env::var_os("TMUX_TMPDIR");
        let previous_tmux = std::env::var_os("TMUX");
        let previous_pane = std::env::var_os("TMUX_PANE");
        let previous_readiness = std::env::var_os("GIT_PAW_READINESS_TIMEOUT_MS");
        // SAFETY: callers MUST gate the test with `#[serial]` so the env
        // mutation cannot race with other threads inspecting the env.
        unsafe {
            std::env::set_var("TMUX_TMPDIR", socket_dir);
            std::env::set_var("GIT_PAW_READINESS_TIMEOUT_MS", TEST_READINESS_TIMEOUT_MS);
            std::env::remove_var("TMUX");
            std::env::remove_var("TMUX_PANE");
        }
        Self {
            previous_tmpdir,
            previous_tmux,
            previous_pane,
            previous_readiness,
        }
    }
}

impl Drop for ProcessTmuxEnvGuard {
    fn drop(&mut self) {
        // SAFETY: same as `set` — caller's `#[serial]` gate serialises env
        // mutation.
        unsafe {
            match &self.previous_tmpdir {
                Some(v) => std::env::set_var("TMUX_TMPDIR", v),
                None => std::env::remove_var("TMUX_TMPDIR"),
            }
            match &self.previous_readiness {
                Some(v) => std::env::set_var("GIT_PAW_READINESS_TIMEOUT_MS", v),
                None => std::env::remove_var("GIT_PAW_READINESS_TIMEOUT_MS"),
            }
            if let Some(v) = &self.previous_tmux {
                std::env::set_var("TMUX", v);
            }
            if let Some(v) = &self.previous_pane {
                std::env::set_var("TMUX_PANE", v);
            }
        }
    }
}

impl Default for TmuxTestEnv {
    fn default() -> Self {
        Self::new()
    }
}

/// Thin constructor wrapper so callers can write
/// `let tmux_env = helpers::tmux_test_env();`.
///
/// see openspec/changes/test-tmux-isolation
pub fn tmux_test_env() -> TmuxTestEnv {
    TmuxTestEnv::new()
}

/// Fails fast if a `paw-*` tmux session is live on the user's **default**
/// tmux socket.
///
/// The check runs `tmux ls` without applying [`TmuxTestEnv`], so it inspects
/// the real default socket (the same socket the user's supervisor session
/// lives on). If any session whose name starts with `paw-` is present, the
/// function panics with a message naming the offending session(s) and
/// recommending one of:
///
/// - `tmux kill-session -t <name>` — kill the live session before re-running
///   the test suite.
/// - `cargo test --test <name>` — run a targeted test that does not depend
///   on `setup_test_repo()`.
/// - `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test ...` — explicitly opt out of
///   the guard (only safe when you have verified every test in the run is
///   socket-isolated).
///
/// Returns silently if:
///
/// - `GIT_PAW_ALLOW_LIVE_SESSION=1` is set (escape hatch).
/// - `tmux ls` exits non-zero with the "no server running" message
///   (the expected state on a fresh dev machine or in CI).
/// - `tmux ls` succeeds but no session name starts with `paw-`.
///
/// see openspec/changes/test-tmux-isolation
pub fn guard_against_live_session() {
    if std::env::var_os("GIT_PAW_ALLOW_LIVE_SESSION").is_some() {
        return;
    }

    // Strip `TMUX_TMPDIR`, `TMUX`, and `TMUX_PANE` before running `tmux ls`.
    // The guard's contract is to inspect the user's REAL default socket —
    // but a parallel `#[serial]` test elsewhere in the binary may have set
    // `TMUX_TMPDIR` on the process via `apply_to_process()`, and Rust's
    // test runner shares `std::env` across parallel test threads. Without
    // stripping these, the guard inspects whatever isolated socket the
    // parallel test happens to be using mid-execution and reports its
    // in-flight sessions as "live default-socket sessions" — the exact
    // false-positive that caused the v0.5.0 CI failure with
    // `paw-test-repo` showing up across non-serial `from_specs_*` tests.
    let Ok(output) = Command::new("tmux")
        .arg("ls")
        .env_remove("TMUX_TMPDIR")
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .output()
    else {
        // tmux not installed → nothing to guard against
        return;
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("no server running") || output.stdout.is_empty() {
            return;
        }
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if let Some(msg) = build_offending_session_message(&stdout) {
        panic!("{msg}");
    }
}

/// Pure parser: given the stdout of `tmux ls`, find any session whose name
/// starts with `paw-` and build the guard's panic message. Returns `None`
/// when no `paw-*` session is present.
///
/// Split out from [`guard_against_live_session`] so the matcher logic is
/// testable without spawning a real tmux session on the default socket —
/// previously, the integration test for the guard created `paw-guard-test`
/// on the default socket, which then leaked across the test-binary
/// boundary (cargo test compiles each `tests/*.rs` into its own binary
/// and runs them in parallel, all sharing the same default socket).
pub(crate) fn build_offending_session_message(tmux_ls_stdout: &str) -> Option<String> {
    let offending: Vec<&str> = tmux_ls_stdout
        .lines()
        .filter_map(|line| {
            // `tmux ls` format: "<name>: <windows> windows (created ...) ..."
            let name = line.split(':').next()?.trim();
            if name.starts_with("paw-") {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    if offending.is_empty() {
        return None;
    }

    let mut msg = String::from(
        "Refusing to run integration tests while a paw-* tmux session is live on the default socket.\n\n\
         The test suite spawns tmux against the default socket by default; running it concurrently with a live\n\
         supervisor session has crashed the tmux server in the past, killing every pane.\n\n\
         Offending session(s):\n",
    );
    for name in &offending {
        let _ = writeln!(msg, "  - {name}");
    }
    msg.push_str(
        "\nRemediation options:\n\
         \n\
         1. Kill the live session(s) and re-run:\n",
    );
    for name in &offending {
        let _ = writeln!(msg, "       tmux kill-session -t {name}");
    }
    msg.push_str(
        "\n\
         2. Run a targeted test file that does not depend on setup_test_repo():\n\
                cargo test --test <name>\n\
         \n\
         3. Opt out (only when every test in the run is socket-isolated):\n\
                GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test ...\n",
    );
    Some(msg)
}

/// Creates a temporary git repository with an initial commit.
///
/// The repo is nested inside a sandbox directory so worktrees land as siblings
/// and are cleaned up automatically.
pub fn setup_test_repo() -> TestRepo {
    guard_against_live_session();

    let sandbox = TempDir::new().expect("create sandbox dir");
    let repo = sandbox.path().join("repo");
    std::fs::create_dir(&repo).expect("create repo dir");

    // Force `main` as the initial branch — Ubuntu CI defaults to `master`
    // unless `init.defaultBranch` is set, and the production
    // `resolve_default_branch` falls back to `"main"` when there is no
    // remote, so the unmerged-commits warning silently no-ops on a
    // master-default repo.
    Command::new("git")
        .current_dir(&repo)
        .args(["init", "-b", "main"])
        .output()
        .expect("git init");

    Command::new("git")
        .current_dir(&repo)
        .args(["config", "user.email", "test@test.com"])
        .output()
        .expect("git config email");

    Command::new("git")
        .current_dir(&repo)
        .args(["config", "user.name", "Test"])
        .output()
        .expect("git config name");

    std::fs::write(repo.join("README.md"), "# test").expect("write file");

    Command::new("git")
        .current_dir(&repo)
        .args(["add", "."])
        .output()
        .expect("git add");

    Command::new("git")
        .current_dir(&repo)
        .args(["commit", "-m", "initial"])
        .output()
        .expect("git commit");

    TestRepo {
        _sandbox: sandbox,
        repo,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::panic;

    struct AllowLiveSessionGuard {
        previous: Option<OsString>,
    }

    impl AllowLiveSessionGuard {
        fn set(value: &str) -> Self {
            let previous = std::env::var_os("GIT_PAW_ALLOW_LIVE_SESSION");
            unsafe {
                std::env::set_var("GIT_PAW_ALLOW_LIVE_SESSION", value);
            }
            Self { previous }
        }

        fn unset() -> Self {
            let previous = std::env::var_os("GIT_PAW_ALLOW_LIVE_SESSION");
            unsafe {
                std::env::remove_var("GIT_PAW_ALLOW_LIVE_SESSION");
            }
            Self { previous }
        }
    }

    impl Drop for AllowLiveSessionGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.previous {
                    Some(v) => std::env::set_var("GIT_PAW_ALLOW_LIVE_SESSION", v),
                    None => std::env::remove_var("GIT_PAW_ALLOW_LIVE_SESSION"),
                }
            }
        }
    }

    fn tmux_available() -> bool {
        Command::new("tmux")
            .arg("-V")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    #[test]
    fn tmux_test_env_sets_expected_env_vars() {
        let env = tmux_test_env();
        let mut cmd = Command::new("/usr/bin/env");
        env.apply(&mut cmd);

        let output = cmd.output().expect("run /usr/bin/env");
        assert!(output.status.success(), "env should succeed");
        let stdout = String::from_utf8(output.stdout).expect("utf8");

        let expected_tmpdir = env
            .socket_dir()
            .to_str()
            .expect("socket_dir is utf8")
            .to_string();
        let tmux_tmpdir_line = stdout
            .lines()
            .find(|line| line.starts_with("TMUX_TMPDIR="))
            .unwrap_or_else(|| panic!("TMUX_TMPDIR missing from child env: {stdout}"));
        assert_eq!(
            tmux_tmpdir_line,
            format!("TMUX_TMPDIR={expected_tmpdir}"),
            "TMUX_TMPDIR should point at the helper's socket_dir"
        );

        assert!(
            !stdout.lines().any(|line| line.starts_with("TMUX=")),
            "TMUX should be removed from child env, got:\n{stdout}"
        );
        assert!(
            !stdout.lines().any(|line| line.starts_with("TMUX_PANE=")),
            "TMUX_PANE should be removed from child env, got:\n{stdout}"
        );
    }

    #[test]
    #[serial]
    fn guard_returns_when_no_tmux_server() {
        if !tmux_available() {
            eprintln!("skipping: tmux not available");
            return;
        }
        let socket_env = tmux_test_env();
        let _tmpdir_guard = socket_env.apply_to_process();

        // Sanity: no server on this freshly-created socket dir.
        let ls = Command::new("tmux")
            .arg("ls")
            .output()
            .expect("run tmux ls");
        assert!(
            !ls.status.success(),
            "tmux ls should fail on an empty socket dir"
        );

        guard_against_live_session();
    }

    /// Test the matcher logic via the pure-function helper instead of
    /// creating a real `paw-*` session on the default tmux socket.
    ///
    /// Previously this test created `paw-guard-test` on the default socket,
    /// which leaked across the test-binary boundary: cargo test compiles
    /// each `tests/*.rs` into its own binary process, and those processes
    /// run in parallel against the same default tmux socket. While the
    /// guard test was mid-flight, other test binaries' `setup_test_repo()`
    /// guards would see `paw-guard-test` and panic. Switching to a pure-
    /// function test removes any default-socket footprint.
    #[test]
    fn build_offending_session_message_detects_paw_sessions() {
        // No paw-* session → returns None
        let stdout = "0: 1 windows (created Wed May 21 14:00:00 2026) [80x24]\n\
                      other-thing: 2 windows (created Wed May 21 14:00:00 2026)\n";
        assert!(build_offending_session_message(stdout).is_none());

        // Single paw-* session → message names it
        let stdout = "paw-test-fixture: 1 windows (created Wed May 21 14:00:00 2026) [80x24]\n";
        let msg = build_offending_session_message(stdout)
            .expect("paw-* session should produce a panic message");
        assert!(
            msg.contains("paw-test-fixture"),
            "message should name the offending session, got:\n{msg}"
        );
        assert!(
            msg.contains("kill-session -t paw-test-fixture"),
            "message should include the kill-session command, got:\n{msg}"
        );
        assert!(
            msg.contains("GIT_PAW_ALLOW_LIVE_SESSION=1"),
            "message should mention the escape hatch, got:\n{msg}"
        );

        // Multiple paw-* sessions → both named
        let stdout = "paw-fixture-a: 1 windows\n\
                      ignored-session: 1 windows\n\
                      paw-fixture-b: 2 windows\n";
        let msg = build_offending_session_message(stdout)
            .expect("paw-* sessions should produce a panic message");
        assert!(msg.contains("paw-fixture-a") && msg.contains("paw-fixture-b"));
        assert!(!msg.contains("ignored-session"));
    }

    #[test]
    #[serial]
    fn guard_honours_allow_live_session_env() {
        if !tmux_available() {
            eprintln!("skipping: tmux not available");
            return;
        }
        let socket_env = tmux_test_env();
        let _tmpdir_guard = socket_env.apply_to_process();
        let _allow_guard = AllowLiveSessionGuard::set("1");

        let status = Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                "paw-guard-escape",
                "-x",
                "200",
                "-y",
                "50",
                "sh",
            ])
            .status()
            .expect("tmux new-session");
        assert!(status.success(), "create paw-guard-escape session");

        // No panic expected.
        guard_against_live_session();

        let _ = Command::new("tmux")
            .args(["kill-session", "-t", "paw-guard-escape"])
            .status();
    }
}
