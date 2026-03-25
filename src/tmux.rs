//! Tmux session and pane orchestration.
//!
//! Checks tmux availability, creates sessions, splits panes, sends commands,
//! applies layouts, and manages attach/reattach. Uses a builder pattern for
//! testability and dry-run support.

use std::process::Command;

use crate::error::PawError;

/// Maximum number of session name collision retries.
const MAX_COLLISION_RETRIES: u32 = 10;

/// A single tmux CLI invocation, stored as its argument list.
///
/// Can be inspected as a string (for dry-run / testing) or executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxCommand {
    args: Vec<String>,
}

impl TmuxCommand {
    /// Create a new tmux command from the given arguments.
    fn new(args: &[&str]) -> Self {
        Self {
            args: args.iter().map(|&s| s.to_owned()).collect(),
        }
    }

    /// Return a human-readable command string (e.g. `tmux new-session -d -s paw-proj`).
    #[allow(dead_code)]
    pub fn as_command_string(&self) -> String {
        format!("tmux {}", self.args.join(" "))
    }

    /// Execute the command against the live tmux server.
    fn execute(&self) -> Result<String, PawError> {
        let output = Command::new("tmux")
            .args(&self.args)
            .output()
            .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;

        if output.status.success() {
            String::from_utf8(output.stdout)
                .map_err(|e| PawError::TmuxError(format!("invalid utf-8 in tmux output: {e}")))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(PawError::TmuxError(stderr.trim().to_owned()))
        }
    }
}

/// Specification for a single pane: which branch/worktree to `cd` into and which CLI to run.
#[derive(Debug, Clone)]
pub struct PaneSpec {
    /// Branch name (e.g. `feat/auth`). Used for the pane title.
    pub branch: String,
    /// Absolute path to the git worktree directory.
    pub worktree: String,
    /// The CLI command to execute inside the pane.
    pub cli_command: String,
}

/// A fully-resolved tmux session ready to execute or inspect.
#[derive(Debug)]
pub struct TmuxSession {
    /// The resolved session name (e.g. `paw-myproject` or `paw-myproject-2`).
    pub name: String,
    commands: Vec<TmuxCommand>,
}

impl TmuxSession {
    /// Execute all accumulated tmux commands against the live tmux server.
    pub fn execute(&self) -> Result<(), PawError> {
        for cmd in &self.commands {
            cmd.execute()?;
        }
        Ok(())
    }

    /// Return all commands as human-readable strings (for dry-run / testing).
    #[allow(dead_code)]
    pub fn command_strings(&self) -> Vec<String> {
        self.commands
            .iter()
            .map(TmuxCommand::as_command_string)
            .collect()
    }
}

/// Builder that accumulates tmux operations for creating and configuring a session.
///
/// Can either execute operations against a live tmux server or return them
/// as command strings for testing and dry-run.
///
/// # Examples
///
/// ```no_run
/// use git_paw::tmux::{TmuxSessionBuilder, PaneSpec};
///
/// let session = TmuxSessionBuilder::new("my-project")
///     .add_pane(PaneSpec {
///         branch: "feat/auth".into(),
///         worktree: "/tmp/my-project-feat-auth".into(),
///         cli_command: "claude".into(),
///     })
///     .mouse_mode(true)
///     .build()?;
///
/// // Dry-run: inspect commands
/// for cmd in session.command_strings() {
///     println!("{cmd}");
/// }
///
/// // Or execute for real
/// session.execute()?;
/// # Ok::<(), git_paw::error::PawError>(())
/// ```
#[derive(Debug)]
pub struct TmuxSessionBuilder {
    project_name: String,
    panes: Vec<PaneSpec>,
    mouse_mode: bool,
    session_name_override: Option<String>,
}

impl TmuxSessionBuilder {
    /// Create a new builder for the given project name.
    ///
    /// The session will be named `paw-<project_name>` unless overridden
    /// with [`session_name`](Self::session_name).
    pub fn new(project_name: &str) -> Self {
        Self {
            project_name: project_name.to_owned(),
            panes: Vec::new(),
            mouse_mode: true,
            session_name_override: None,
        }
    }

    /// Override the session name instead of deriving it from the project name.
    ///
    /// Use this with [`resolve_session_name`] to handle name collisions.
    #[must_use]
    pub fn session_name(mut self, name: String) -> Self {
        self.session_name_override = Some(name);
        self
    }

    /// Add a pane that will `cd` into the worktree and run the CLI command.
    #[must_use]
    pub fn add_pane(mut self, spec: PaneSpec) -> Self {
        self.panes.push(spec);
        self
    }

    /// Enable or disable mouse mode for the session (default: `true`).
    ///
    /// When enabled, users can click to switch panes, drag borders to resize,
    /// and scroll. This is set per-session and does not affect other tmux sessions.
    #[must_use]
    pub fn mouse_mode(mut self, enabled: bool) -> Self {
        self.mouse_mode = enabled;
        self
    }

    /// Build the full sequence of tmux commands without executing anything.
    ///
    /// Returns a [`TmuxSession`] that can be executed or inspected.
    /// Returns an error if no panes have been added.
    pub fn build(self) -> Result<TmuxSession, PawError> {
        if self.panes.is_empty() {
            return Err(PawError::TmuxError(
                "cannot create a session with no panes".to_owned(),
            ));
        }

        let session_name = self
            .session_name_override
            .unwrap_or_else(|| format!("paw-{}", self.project_name));
        let mut commands = Vec::new();

        // 1. Create detached session (pane 0 is implicit)
        commands.push(TmuxCommand::new(&[
            "new-session",
            "-d",
            "-s",
            &session_name,
        ]));

        // 2. Mouse mode
        if self.mouse_mode {
            commands.push(TmuxCommand::new(&[
                "set-option",
                "-t",
                &session_name,
                "mouse",
                "on",
            ]));
        }

        // 3. Pane border titles — show branch/CLI in each pane's border
        commands.push(TmuxCommand::new(&[
            "set-option",
            "-t",
            &session_name,
            "pane-border-status",
            "top",
        ]));
        commands.push(TmuxCommand::new(&[
            "set-option",
            "-t",
            &session_name,
            "pane-border-format",
            " #{pane_title} ",
        ]));

        // 4. First pane — already exists as pane 0
        let first = &self.panes[0];
        let pane_target = format!("{session_name}:0.0");
        let pane_title = format!("{} \u{2192} {}", first.branch, first.cli_command);
        let pane_cmd = format!("cd {} && {}", first.worktree, first.cli_command);
        commands.push(TmuxCommand::new(&[
            "select-pane",
            "-t",
            &pane_target,
            "-T",
            &pane_title,
        ]));
        commands.push(TmuxCommand::new(&[
            "send-keys",
            "-t",
            &pane_target,
            &pane_cmd,
            "Enter",
        ]));

        // 5. Subsequent panes — tiled layout before each split
        for (i, pane) in self.panes.iter().enumerate().skip(1) {
            // Apply tiled layout before split to ensure space
            commands.push(TmuxCommand::new(&[
                "select-layout",
                "-t",
                &session_name,
                "tiled",
            ]));

            // Split window to create new pane
            commands.push(TmuxCommand::new(&["split-window", "-t", &session_name]));

            // Title and command for the new pane
            let pane_target = format!("{session_name}:0.{i}");
            let pane_title = format!("{} \u{2192} {}", pane.branch, pane.cli_command);
            let pane_cmd = format!("cd {} && {}", pane.worktree, pane.cli_command);
            commands.push(TmuxCommand::new(&[
                "select-pane",
                "-t",
                &pane_target,
                "-T",
                &pane_title,
            ]));
            commands.push(TmuxCommand::new(&[
                "send-keys",
                "-t",
                &pane_target,
                &pane_cmd,
                "Enter",
            ]));
        }

        // 6. Final tiled layout for clean alignment
        commands.push(TmuxCommand::new(&[
            "select-layout",
            "-t",
            &session_name,
            "tiled",
        ]));

        Ok(TmuxSession {
            name: session_name,
            commands,
        })
    }
}

/// Check that tmux is installed on PATH.
///
/// Returns `Ok(())` if found, or `Err(PawError::TmuxNotInstalled)` with
/// install instructions if missing.
pub fn ensure_tmux_installed() -> Result<(), PawError> {
    which::which("tmux").map_err(|_| PawError::TmuxNotInstalled)?;
    Ok(())
}

/// Check whether a tmux session with the given name is currently alive.
pub fn is_session_alive(name: &str) -> Result<bool, PawError> {
    let status = Command::new("tmux")
        .args(["has-session", "-t", name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;

    Ok(status.success())
}

/// Resolve a unique session name, handling collisions with existing sessions.
///
/// Starts with `paw-<project_name>` and appends `-2`, `-3`, etc. if the name
/// is already taken by another session.
pub fn resolve_session_name(project_name: &str) -> Result<String, PawError> {
    let base = format!("paw-{project_name}");

    if !is_session_alive(&base)? {
        return Ok(base);
    }

    for suffix in 2..=MAX_COLLISION_RETRIES + 1 {
        let candidate = format!("{base}-{suffix}");
        if !is_session_alive(&candidate)? {
            return Ok(candidate);
        }
    }

    Err(PawError::TmuxError(format!(
        "too many session name collisions for '{base}'"
    )))
}

/// Attach the current terminal to the named tmux session.
///
/// This replaces the current process's stdio. Returns an error if the
/// session does not exist or tmux fails.
pub fn attach(name: &str) -> Result<(), PawError> {
    let status = Command::new("tmux")
        .args(["attach-session", "-t", name])
        .status()
        .map_err(|e| PawError::TmuxError(format!("failed to attach to tmux session: {e}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(PawError::TmuxError(format!(
            "failed to attach to session '{name}'"
        )))
    }
}

/// Kill the named tmux session.
pub fn kill_session(name: &str) -> Result<(), PawError> {
    let output = Command::new("tmux")
        .args(["kill-session", "-t", name])
        .output()
        .map_err(|e| PawError::TmuxError(format!("failed to kill tmux session: {e}")))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(PawError::TmuxError(stderr.trim().to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pane(branch: &str, worktree: &str, cli: &str) -> PaneSpec {
        PaneSpec {
            branch: branch.to_owned(),
            worktree: worktree.to_owned(),
            cli_command: cli.to_owned(),
        }
    }

    /// Helper: extract command strings matching a keyword from a session's commands.
    fn commands_containing(cmds: &[String], keyword: &str) -> Vec<String> {
        cmds.iter()
            .filter(|c| c.contains(keyword))
            .cloned()
            .collect()
    }

    // -----------------------------------------------------------------------
    // AC: Checks tmux presence with actionable error
    // -----------------------------------------------------------------------

    #[test]
    #[serial_test::serial]
    fn ensure_tmux_installed_succeeds_when_present() {
        // Requires #[serial] because detect tests modify PATH.
        assert!(ensure_tmux_installed().is_ok());
    }

    // -----------------------------------------------------------------------
    // AC: Creates named sessions, handles collision
    // -----------------------------------------------------------------------

    #[test]
    fn session_is_named_after_project() {
        let session = TmuxSessionBuilder::new("my-project")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .build()
            .unwrap();

        assert_eq!(session.name, "paw-my-project");
    }

    #[test]
    fn session_creation_command_uses_session_name() {
        let session = TmuxSessionBuilder::new("app")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        assert!(
            cmds.iter()
                .any(|c| c.contains("new-session") && c.contains("paw-app")),
            "should create a tmux session named paw-app"
        );
    }

    // -----------------------------------------------------------------------
    // AC: Dynamic pane count based on input
    // -----------------------------------------------------------------------

    #[test]
    fn pane_count_matches_input_for_two_panes() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("feat/auth", "/tmp/wt1", "claude"))
            .add_pane(make_pane("feat/api", "/tmp/wt2", "codex"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let send_keys = commands_containing(&cmds, "send-keys");
        assert_eq!(
            send_keys.len(),
            2,
            "should send commands to exactly 2 panes"
        );
    }

    #[test]
    fn pane_count_matches_input_for_five_panes() {
        let mut builder = TmuxSessionBuilder::new("proj");
        for i in 0..5 {
            builder = builder.add_pane(make_pane(
                &format!("feat/b{i}"),
                &format!("/tmp/wt{i}"),
                "claude",
            ));
        }
        let session = builder.build().unwrap();

        let cmds = session.command_strings();
        let send_keys = commands_containing(&cmds, "send-keys");
        assert_eq!(
            send_keys.len(),
            5,
            "should send commands to exactly 5 panes"
        );
    }

    #[test]
    fn building_with_no_panes_is_an_error() {
        let result = TmuxSessionBuilder::new("proj").build();
        assert!(result.is_err(), "session with no panes should fail");
    }

    // -----------------------------------------------------------------------
    // AC: Correct commands sent to panes
    // -----------------------------------------------------------------------

    #[test]
    fn each_pane_receives_cd_and_cli_command() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("feat/auth", "/home/user/wt-auth", "claude"))
            .add_pane(make_pane("feat/api", "/home/user/wt-api", "gemini"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let send_keys = commands_containing(&cmds, "send-keys");

        assert!(
            send_keys[0].contains("cd /home/user/wt-auth && claude"),
            "first pane should cd into wt-auth and run claude"
        );
        assert!(
            send_keys[1].contains("cd /home/user/wt-api && gemini"),
            "second pane should cd into wt-api and run gemini"
        );
    }

    #[test]
    fn pane_commands_are_submitted_with_enter() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("main", "/tmp/wt", "aider"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let send_keys = commands_containing(&cmds, "send-keys");
        assert!(
            send_keys[0].contains("Enter"),
            "send-keys should press Enter to submit"
        );
    }

    #[test]
    fn each_pane_targets_a_distinct_pane_index() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("feat/a", "/tmp/a", "claude"))
            .add_pane(make_pane("feat/b", "/tmp/b", "codex"))
            .add_pane(make_pane("feat/c", "/tmp/c", "gemini"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let send_keys = commands_containing(&cmds, "send-keys");

        assert!(
            send_keys[0].contains(":0.0"),
            "first pane should target :0.0"
        );
        assert!(
            send_keys[1].contains(":0.1"),
            "second pane should target :0.1"
        );
        assert!(
            send_keys[2].contains(":0.2"),
            "third pane should target :0.2"
        );
    }

    // -----------------------------------------------------------------------
    // AC: Pane titles show branch and CLI
    // -----------------------------------------------------------------------

    #[test]
    fn each_pane_is_titled_with_branch_and_cli() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("feat/auth", "/tmp/wt1", "claude"))
            .add_pane(make_pane("fix/api", "/tmp/wt2", "gemini"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let select_panes = commands_containing(&cmds, "select-pane");

        assert_eq!(select_panes.len(), 2, "each pane should get a title");
        assert!(
            select_panes[0].contains("feat/auth \u{2192} claude"),
            "first pane title should be 'feat/auth \u{2192} claude', got: {}",
            select_panes[0]
        );
        assert!(
            select_panes[1].contains("fix/api \u{2192} gemini"),
            "second pane title should be 'fix/api \u{2192} gemini', got: {}",
            select_panes[1]
        );
    }

    #[test]
    fn pane_border_status_is_configured() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        assert!(
            cmds.iter()
                .any(|c| c.contains("pane-border-status") && c.contains("top")),
            "should configure pane-border-status to top"
        );
        assert!(
            cmds.iter()
                .any(|c| c.contains("pane-border-format") && c.contains("#{pane_title}")),
            "should configure pane-border-format to show pane title"
        );
    }

    #[test]
    fn pane_titles_target_the_session() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let border_cmds: Vec<&String> = cmds
            .iter()
            .filter(|c| c.contains("pane-border-status") || c.contains("pane-border-format"))
            .collect();

        for cmd in &border_cmds {
            assert!(
                cmd.contains("-t paw-proj"),
                "pane border config should target the session, got: {cmd}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // AC: Tiled layout applied (before each split + final)
    // -----------------------------------------------------------------------

    #[test]
    fn tiled_layout_applied_before_every_split() {
        let mut builder = TmuxSessionBuilder::new("proj");
        for i in 0..6 {
            builder = builder.add_pane(make_pane(
                &format!("feat/b{i}"),
                &format!("/tmp/wt{i}"),
                "claude",
            ));
        }
        let session = builder.build().unwrap();
        let cmds = session.command_strings();

        let split_indices: Vec<usize> = cmds
            .iter()
            .enumerate()
            .filter(|(_, c)| c.contains("split-window"))
            .map(|(i, _)| i)
            .collect();

        assert_eq!(split_indices.len(), 5, "6 panes need 5 splits");

        for &idx in &split_indices {
            assert!(
                idx > 0
                    && cmds[idx - 1].contains("select-layout")
                    && cmds[idx - 1].contains("tiled"),
                "split at position {idx} must be preceded by tiled layout"
            );
        }
    }

    #[test]
    fn final_tiled_layout_applied_after_last_pane() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("feat/a", "/tmp/a", "claude"))
            .add_pane(make_pane("feat/b", "/tmp/b", "codex"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let last = cmds.last().expect("should have commands");
        assert!(
            last.contains("select-layout") && last.contains("tiled"),
            "last command should be final tiled layout, got: {last}"
        );
    }

    #[test]
    fn single_pane_still_gets_final_tiled_layout() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let last = cmds.last().expect("should have commands");
        assert!(
            last.contains("select-layout") && last.contains("tiled"),
            "even a single pane should get final tiled layout"
        );
    }

    // -----------------------------------------------------------------------
    // AC: Builder returns command strings without executing (testable)
    // -----------------------------------------------------------------------

    #[test]
    fn command_strings_are_valid_tmux_invocations() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .build()
            .unwrap();

        for cmd in &session.command_strings() {
            assert!(
                cmd.starts_with("tmux "),
                "all command strings should be tmux invocations, got: {cmd}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // AC: Mouse mode (per-session, configurable, default on)
    // -----------------------------------------------------------------------

    #[test]
    fn mouse_mode_enabled_by_default() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        assert!(
            cmds.iter().any(|c| c.contains("mouse on")),
            "mouse should be enabled by default"
        );
    }

    #[test]
    fn mouse_mode_targets_session_not_global() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let mouse_cmd = cmds
            .iter()
            .find(|c| c.contains("mouse on"))
            .expect("should have mouse command");
        assert!(
            mouse_cmd.contains("-t paw-proj"),
            "mouse setting should target the session, got: {mouse_cmd}"
        );
    }

    #[test]
    fn mouse_mode_can_be_disabled() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .mouse_mode(false)
            .build()
            .unwrap();

        let cmds = session.command_strings();
        assert!(
            !cmds.iter().any(|c| c.contains("mouse on")),
            "no mouse-on command should be emitted when disabled"
        );
    }

    // -----------------------------------------------------------------------
    // AC: Session liveness and collision handling
    // -----------------------------------------------------------------------

    /// Helper to create a detached tmux session for testing.
    fn create_test_session(name: &str) {
        let output = std::process::Command::new("tmux")
            .args(["new-session", "-d", "-s", name])
            .output()
            .expect("create tmux session");
        assert!(
            output.status.success(),
            "failed to create test session '{name}'"
        );
    }

    /// Helper to kill a tmux session, ignoring errors.
    fn cleanup_session(name: &str) {
        let _ = kill_session(name);
    }

    #[test]
    #[serial_test::serial]
    fn is_session_alive_returns_false_for_nonexistent() {
        let alive = is_session_alive("paw-definitely-does-not-exist-12345").unwrap();
        assert!(!alive);
    }

    #[test]
    #[serial_test::serial]
    fn session_lifecycle_create_check_kill() {
        let name = "paw-unit-test-lifecycle";
        cleanup_session(name);

        create_test_session(name);
        assert!(is_session_alive(name).unwrap());

        kill_session(name).unwrap();
        assert!(!is_session_alive(name).unwrap());
    }

    #[test]
    #[serial_test::serial]
    fn resolve_session_name_returns_base_when_no_collision() {
        let name = resolve_session_name("unit-test-no-collision-xyz").unwrap();
        assert_eq!(name, "paw-unit-test-no-collision-xyz");
    }

    #[test]
    #[serial_test::serial]
    fn resolve_session_name_appends_suffix_on_collision() {
        let base_name = "paw-unit-test-collision";
        cleanup_session(base_name);
        cleanup_session(&format!("{base_name}-2"));

        create_test_session(base_name);

        let resolved = resolve_session_name("unit-test-collision").unwrap();
        assert_eq!(resolved, format!("{base_name}-2"));

        cleanup_session(base_name);
    }

    #[test]
    #[serial_test::serial]
    fn built_session_can_be_executed_and_killed() {
        let project = "unit-test-execute";
        let session_name = format!("paw-{project}");
        cleanup_session(&session_name);

        let session = TmuxSessionBuilder::new(project)
            .add_pane(make_pane("main", "/tmp", "echo hello"))
            .build()
            .unwrap();

        session.execute().unwrap();
        assert!(is_session_alive(&session_name).unwrap());

        kill_session(&session_name).unwrap();
        assert!(!is_session_alive(&session_name).unwrap());
    }
}
