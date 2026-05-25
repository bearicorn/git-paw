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
    // Not called by production code — used by `TmuxSession::command_strings()` for
    // dry-run contract tests that verify the commands shown to users via `--dry-run`.
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
    // Not called by production code — used by unit tests as the dry-run contract
    // surface to verify the tmux commands shown to users via `--dry-run`.
    #[allow(dead_code)]
    pub fn command_strings(&self) -> Vec<String> {
        self.commands
            .iter()
            .map(TmuxCommand::as_command_string)
            .collect()
    }

    /// Queue a `pipe-pane` command to capture pane output to a log file.
    ///
    /// Appends `tmux pipe-pane -o -t <pane_target> "cat >> <log_path>"` to the
    /// command queue. Should be called after the pane has been created.
    pub fn pipe_pane(&mut self, pane_target: &str, log_path: &std::path::Path) -> &mut Self {
        self.commands.push(TmuxCommand::new(&[
            "pipe-pane",
            "-o",
            "-t",
            pane_target,
            &format!("cat >> {}", log_path.display()),
        ]));
        self
    }

    /// Queue a command to reapply the tiled layout after any resize operation.
    ///
    /// This ensures that the layout remains consistent even when tmux windows
    /// are resized from unattached clients. Should be called after any operation
    /// that might affect window dimensions.
    pub fn reapply_tiled_layout(&mut self, session_name: &str) -> &mut Self {
        self.commands.push(TmuxCommand::new(&[
            "select-layout",
            "-t",
            session_name,
            "tiled",
        ]));
        self
    }

    /// Queue a command to apply the main-horizontal layout for dashboard sessions.
    ///
    /// This layout puts the dashboard pane in a full-width row at the top,
    /// with worktree panes tiled below. Should be used when a dashboard pane
    /// is present (pane 0) and worktree panes follow.
    pub fn apply_dashboard_layout(&mut self, session_name: &str) -> &mut Self {
        self.commands.push(TmuxCommand::new(&[
            "select-layout",
            "-t",
            session_name,
            "main-horizontal",
        ]));
        self
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
    env_vars: Vec<(String, String)>,
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
            env_vars: Vec::new(),
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

    /// Set a session-level environment variable.
    ///
    /// The resulting `tmux set-environment -t <session> <key> <value>` command
    /// is emitted before any `send-keys` commands so all panes inherit it.
    #[must_use]
    pub fn set_environment(mut self, key: &str, value: &str) -> Self {
        self.env_vars.push((key.to_owned(), value.to_owned()));
        self
    }

    /// Build the full sequence of tmux commands without executing anything.
    ///
    /// Returns a [`TmuxSession`] that can be executed or inspected.
    /// Returns an error if no panes have been added.
    #[allow(clippy::too_many_lines)]
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

        // 1. Create detached session (pane 0 is implicit).
        // Use -c to set pane 0's working directory directly, avoiding a race
        // condition where send-keys fires before the shell is ready.
        // -x/-y give tmux explicit dimensions so it can start without an
        // attached client — required in non-TTY environments (CI, integration
        // tests). The user's real terminal resizes the session on attach.
        let first_worktree = &self.panes[0].worktree;
        commands.push(TmuxCommand::new(&[
            "new-session",
            "-d",
            "-s",
            &session_name,
            "-x",
            "200",
            "-y",
            "50",
            "-c",
            first_worktree,
        ]));

        // 2. Pin default-size globally so subsequent split-window operations
        // have a fallback size context. On Linux tmux 3.4+, `-x/-y` on
        // new-session alone is insufficient — subsequent splits still fail
        // with `size missing` because the per-session dimensions aren't
        // propagated to the layout engine when no client is attached.
        // set-option requires a running server (new-session above starts it).
        commands.push(TmuxCommand::new(&[
            "set-option",
            "-g",
            "default-size",
            "200x50",
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

        // 4. Session-level environment variables (before any send-keys)
        for (key, value) in &self.env_vars {
            commands.push(TmuxCommand::new(&[
                "set-environment",
                "-t",
                &session_name,
                key,
                value,
            ]));
        }

        // 5. First pane — already exists as pane 0 (directory set by -c above)
        let first = &self.panes[0];
        let pane_target = format!("{session_name}:0.0");
        let pane_title = format!("{} \u{2192} {}", first.branch, first.cli_command);
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
            &first.cli_command,
            "Enter",
        ]));

        // 6. Subsequent panes — tiled layout before each split
        for (i, pane) in self.panes.iter().enumerate().skip(1) {
            // Apply tiled layout before split to ensure space
            commands.push(TmuxCommand::new(&[
                "select-layout",
                "-t",
                &session_name,
                "tiled",
            ]));

            // Split window to create new pane. Pass `-c <worktree>` so the
            // new pane's shell starts in the agent worktree directly — this
            // avoids the `cd <worktree> && <cli>` send-keys race where the
            // `cd` prefix is lost when send-keys fires before the shell is
            // ready to accept input.
            commands.push(TmuxCommand::new(&[
                "split-window",
                "-t",
                &session_name,
                "-c",
                &pane.worktree,
            ]));

            // Title and command for the new pane
            let pane_target = format!("{session_name}:0.{i}");
            let pane_title = format!("{} \u{2192} {}", pane.branch, pane.cli_command);
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
                &pane.cli_command,
                "Enter",
            ]));
        }

        // 7. Final layout - use main-horizontal if we have a dashboard, otherwise tiled
        if self.panes.len() > 1 && self.panes[0].branch == "dashboard" {
            // Dashboard layout: dashboard pane takes full width at top, worktree panes tiled below
            commands.push(TmuxCommand::new(&[
                "select-layout",
                "-t",
                &session_name,
                "main-horizontal",
            ]));
        } else {
            // Standard tiled layout for sessions without dashboard
            commands.push(TmuxCommand::new(&[
                "select-layout",
                "-t",
                &session_name,
                "tiled",
            ]));
        }

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

/// Detach all clients attached to the named tmux session.
///
/// Wraps `tmux detach-client -s <session>`. Idempotent: returns `Ok(())`
/// if the command succeeds OR if tmux reports the session has no
/// clients attached (the typical no-op error path on already-detached
/// sessions). Leaves the tmux server, the session, and every pane
/// process untouched.
pub fn detach_client(session_name: &str) -> Result<(), PawError> {
    let output = Command::new("tmux")
        .args(["detach-client", "-s", session_name])
        .output()
        .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;

    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    // "no clients attached" is the idempotent no-op case.
    if stderr.contains("no clients") || stderr.contains("no current client") {
        return Ok(());
    }
    Err(PawError::TmuxError(
        String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    ))
}

/// Kill a single pane within a session by `(session, pane_index)`.
///
/// Wraps `tmux kill-pane -t <session>:0.<index>`. Returns `Ok(())` if
/// the pane was killed OR if tmux reports the pane does not exist
/// (idempotent no-op on missing panes). Used by the pause flow to take
/// down the dashboard pane (which owns the broker subprocess) without
/// killing the rest of the session.
pub fn kill_pane(session_name: &str, pane_index: u32) -> Result<(), PawError> {
    let target = format!("{session_name}:0.{pane_index}");
    let output = Command::new("tmux")
        .args(["kill-pane", "-t", &target])
        .output()
        .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;

    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    // Pane-doesn't-exist is the idempotent no-op case.
    if stderr.contains("can't find pane")
        || stderr.contains("no such pane")
        || stderr.contains("pane not found")
    {
        return Ok(());
    }
    Err(PawError::TmuxError(
        String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    ))
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

/// Builds the argv for `tmux send-keys` that injects `text` into
/// `<session_name>:0.<pane_index>` literally (`-l`) and *without* a trailing
/// `Enter` key.
///
/// Pulled out as a free function so the manual-mode boot-block injection in
/// `cmd_start` and tests share a single source of truth: the call must be
/// `send-keys -l -t <target> <text>` (the `-l` flag must come *before* `-t`,
/// otherwise tmux parses it as a key spec rather than the literal flag).
pub fn build_boot_inject_args(session_name: &str, pane_index: usize, text: &str) -> Vec<String> {
    vec![
        "send-keys".to_string(),
        "-l".to_string(),
        "-t".to_string(),
        format!("{session_name}:0.{pane_index}"),
        text.to_string(),
    ]
}

/// Build the tmux commands that materialise the supervisor-mode pane layout
/// described in `openspec/changes/supervisor-as-pane/specs/tmux-orchestration/`.
///
/// Pane ordering:
///
/// - Pane 0: supervisor agent (top-left, 50% of the top row)
/// - Pane 1: dashboard (top-right, 50% of the top row)
/// - Panes 2..N+1: coding agents, row-major (left-to-right, top-to-bottom),
///   up to [`crate::supervisor::layout::SUPERVISOR_AGENTS_PER_ROW`] columns
///   per row
///
/// Sequence (see design D2):
///
/// 1. `new-session -d` creates pane 0 (supervisor).
/// 2. `split-window -v -p <bottom_pct>` on pane 0 creates the full-width agent
///    area as pane 1 (temporary index).
/// 3. `split-window -h -p 50` on pane 0 creates the top-right pane (pane 2),
///    the dashboard candidate.
/// 4. `swap-pane -s :0.1 -t :0.2` reorders the indices so pane 1 = dashboard
///    and pane 2 = agent area.
/// 5. For each subsequent agent: `split-window -h` within the current row to
///    add a sibling, or `split-window -v` to start a new row.
/// 6. Final pass: `resize-pane -t <pane> -y <pct>%` enforces the height
///    proportions from the layout table.
///
/// `select-layout` is intentionally avoided here — it does not preserve the
/// predictable pane-index ordering the rest of the system relies on.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn build_supervisor_session(
    project_name: &str,
    session_name_override: Option<String>,
    supervisor: &PaneSpec,
    dashboard: &PaneSpec,
    agents: &[PaneSpec],
    layout: crate::supervisor::layout::SupervisorLayout,
    mouse_mode: bool,
    env_vars: &[(String, String)],
) -> Result<TmuxSession, PawError> {
    use crate::supervisor::layout::{SUPERVISOR_AGENTS_PER_ROW, SUPERVISOR_PANE_OFFSET};

    let session_name = session_name_override.unwrap_or_else(|| format!("paw-{project_name}"));
    let mut commands: Vec<TmuxCommand> = Vec::new();

    let push = |cmds: &mut Vec<TmuxCommand>, parts: &[&str]| {
        cmds.push(TmuxCommand::new(parts));
    };

    // 1. Create the detached session with pane 0 = supervisor.
    // -x/-y give tmux explicit dimensions so it can start without an attached
    // client (required in non-TTY environments like CI). The real terminal
    // resizes the session on attach.
    push(
        &mut commands,
        &[
            "new-session",
            "-d",
            "-s",
            &session_name,
            "-x",
            "200",
            "-y",
            "50",
            "-c",
            &supervisor.worktree,
        ],
    );

    // 2. Pin default-size globally so subsequent split-window operations
    // have a fallback size context. On Linux tmux 3.4+, `-x/-y` on
    // new-session alone is insufficient — subsequent splits fail with
    // `size missing` because the per-session dimensions aren't propagated
    // to the layout engine when no client is attached.
    push(
        &mut commands,
        &["set-option", "-g", "default-size", "200x50"],
    );

    // 2. Mouse + pane border config.
    if mouse_mode {
        push(
            &mut commands,
            &["set-option", "-t", &session_name, "mouse", "on"],
        );
    }
    push(
        &mut commands,
        &[
            "set-option",
            "-t",
            &session_name,
            "pane-border-status",
            "top",
        ],
    );
    push(
        &mut commands,
        &[
            "set-option",
            "-t",
            &session_name,
            "pane-border-format",
            " #{pane_title} ",
        ],
    );

    // 3. Session-level environment variables (before any send-keys).
    for (key, value) in env_vars {
        push(
            &mut commands,
            &["set-environment", "-t", &session_name, key, value],
        );
    }

    let supervisor_target = format!("{session_name}:0.0");
    let supervisor_title = format!("{} \u{2192} {}", supervisor.branch, supervisor.cli_command);
    push(
        &mut commands,
        &[
            "select-pane",
            "-t",
            &supervisor_target,
            "-T",
            &supervisor_title,
        ],
    );
    push(
        &mut commands,
        &[
            "send-keys",
            "-t",
            &supervisor_target,
            &supervisor.cli_command,
            "Enter",
        ],
    );

    // 4. Split pane 0 vertically -> creates the full-width agent area (now
    //    index 1, swapped to index 2 below). When there is at least one
    //    coding agent we pass `-c <first_agent.worktree>` so the agent area
    //    pane is born in the first agent's worktree directly — this avoids
    //    the `cd <worktree> && <cli>` send-keys race that previously left
    //    resumed agent panes anchored in the supervisor's cwd.
    let bottom_pct = (100u16 - u16::from(layout.top_row_pct)).to_string();
    if let Some(first_agent) = agents.first() {
        push(
            &mut commands,
            &[
                "split-window",
                "-v",
                "-t",
                &supervisor_target,
                "-p",
                &bottom_pct,
                "-c",
                &first_agent.worktree,
            ],
        );
    } else {
        push(
            &mut commands,
            &[
                "split-window",
                "-v",
                "-t",
                &supervisor_target,
                "-p",
                &bottom_pct,
            ],
        );
    }

    // 5. Split pane 0 horizontally -> creates the top-right pane (currently
    //    index 2, swapped to index 1 below) at 50% width.
    push(
        &mut commands,
        &[
            "split-window",
            "-h",
            "-t",
            &supervisor_target,
            "-p",
            "50",
            "-c",
            &dashboard.worktree,
        ],
    );

    // 6. Swap indices so pane 1 = dashboard, pane 2 = agent area.
    let pane_one = format!("{session_name}:0.1");
    let pane_two = format!("{session_name}:0.2");
    push(
        &mut commands,
        &["swap-pane", "-s", &pane_one, "-t", &pane_two],
    );

    // 7. Set dashboard title + run its command in pane 1 (after swap).
    let dashboard_target = format!("{session_name}:0.1");
    let dashboard_title = format!("{} \u{2192} {}", dashboard.branch, dashboard.cli_command);
    push(
        &mut commands,
        &[
            "select-pane",
            "-t",
            &dashboard_target,
            "-T",
            &dashboard_title,
        ],
    );
    push(
        &mut commands,
        &[
            "send-keys",
            "-t",
            &dashboard_target,
            &dashboard.cli_command,
            "Enter",
        ],
    );

    // 8. Populate the agent grid.
    if !agents.is_empty() {
        // First agent: the agent area is already pane 2 (post-swap) and was
        // created with `-c <first.worktree>` above, so its shell is already
        // running in the first agent's worktree. Send only the bare CLI
        // command — no `cd <worktree> && <cli>` chain, which would race with
        // shell startup.
        let first_target = format!("{session_name}:0.{SUPERVISOR_PANE_OFFSET}");
        let first = &agents[0];
        let first_title = format!("{} \u{2192} {}", first.branch, first.cli_command);
        push(
            &mut commands,
            &["select-pane", "-t", &first_target, "-T", &first_title],
        );
        push(
            &mut commands,
            &[
                "send-keys",
                "-t",
                &first_target,
                &first.cli_command,
                "Enter",
            ],
        );

        let mut row_first_pane = SUPERVISOR_PANE_OFFSET;

        for (i, agent) in agents.iter().enumerate().skip(1) {
            let pane_idx = SUPERVISOR_PANE_OFFSET + i;
            let pane_target = format!("{session_name}:0.{pane_idx}");
            let position_in_row = i % SUPERVISOR_AGENTS_PER_ROW;
            let starts_new_row = position_in_row == 0;

            if starts_new_row {
                // Vertical split from this row's first pane to add a new row
                // below.
                let src_target = format!("{session_name}:0.{row_first_pane}");
                push(
                    &mut commands,
                    &[
                        "split-window",
                        "-v",
                        "-t",
                        &src_target,
                        "-c",
                        &agent.worktree,
                    ],
                );
                row_first_pane = pane_idx;
            } else {
                // Horizontal split from the previous pane to add a sibling in
                // the same row.
                let prev_idx = pane_idx - 1;
                let prev_target = format!("{session_name}:0.{prev_idx}");
                push(
                    &mut commands,
                    &[
                        "split-window",
                        "-h",
                        "-t",
                        &prev_target,
                        "-c",
                        &agent.worktree,
                    ],
                );
            }

            let title = format!("{} \u{2192} {}", agent.branch, agent.cli_command);
            push(
                &mut commands,
                &["select-pane", "-t", &pane_target, "-T", &title],
            );
            push(
                &mut commands,
                &["send-keys", "-t", &pane_target, &agent.cli_command, "Enter"],
            );
        }
    }

    // 9. Final pass: resize-pane to enforce the layout-table heights. One
    //    resize-pane per row (top + each agent row). Percentages here are
    //    `<pct>%` syntax which tmux 3.x accepts on `-y`.
    let top_pct_str = format!("{}%", layout.top_row_pct);
    push(
        &mut commands,
        &["resize-pane", "-t", &supervisor_target, "-y", &top_pct_str],
    );
    let agent_row_pct_str = format_supervisor_pct(layout.agent_row_pct);
    for row in 0..layout.agent_rows {
        let pane_idx = SUPERVISOR_PANE_OFFSET + row * SUPERVISOR_AGENTS_PER_ROW;
        if pane_idx < SUPERVISOR_PANE_OFFSET + agents.len() {
            let target = format!("{session_name}:0.{pane_idx}");
            push(
                &mut commands,
                &["resize-pane", "-t", &target, "-y", &agent_row_pct_str],
            );
        }
    }

    Ok(TmuxSession {
        name: session_name,
        commands,
    })
}

/// Format a row-height percentage. Whole numbers render as "28%"; the 14.4%
/// bucket renders as "14.4%".
fn format_supervisor_pct(pct: f32) -> String {
    if (pct - pct.round()).abs() < 0.05 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let rounded = pct.round().clamp(0.0, 100.0) as u32;
        format!("{rounded}%")
    } else {
        format!("{pct:.1}%")
    }
}

/// Build the argv pair for submitting a supervisor-mode initial prompt to a
/// coding agent pane. The first argv pastes the prompt and sends `Enter`
/// (which paste-aware CLIs consume to confirm the paste buffer). The second
/// argv sends a second `Enter` to actually submit the buffered content. On
/// non-paste-aware CLIs the first `Enter` submits and the second `Enter` is
/// a benign no-op or blank prompt.
///
/// Returns a tuple `(first_argv, second_argv)`. Callers are expected to
/// invoke `tmux send-keys <first_argv>`, sleep `SUBMIT_DELAY_MS`, then invoke
/// `tmux send-keys <second_argv>` as a separate process invocation so the
/// CLI has wall-clock time to render the paste-buffer placeholder.
#[must_use]
pub fn build_supervisor_submit_argv_pair(
    session_name: &str,
    pane_index: usize,
    prompt: &str,
) -> (Vec<String>, Vec<String>) {
    let target = format!("{session_name}:0.{pane_index}");
    let first = vec![
        "send-keys".to_string(),
        "-t".to_string(),
        target.clone(),
        prompt.to_string(),
        "Enter".to_string(),
    ];
    let second = vec![
        "send-keys".to_string(),
        "-t".to_string(),
        target,
        "Enter".to_string(),
    ];
    (first, second)
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
    // Behavioral: verifies the public contract — does the system detect tmux?
    // -----------------------------------------------------------------------

    #[test]
    #[serial_test::serial]
    fn ensure_tmux_installed_succeeds_when_present() {
        // Requires #[serial] because detect tests modify PATH.
        assert!(ensure_tmux_installed().is_ok());
    }

    // -----------------------------------------------------------------------
    // AC: Creates named sessions, handles collision
    // Behavioral: session name is a public field used by attach, status, and
    // dry-run output. The exact naming convention is the public contract.
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

    /// AC: Session creation passes explicit dimensions for headless environments
    /// — basic builder.
    #[test]
    fn new_session_passes_explicit_x_and_y() {
        let session = TmuxSessionBuilder::new("app")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let new_session_cmd = cmds
            .iter()
            .find(|c| c.contains("new-session"))
            .expect("new-session command present");
        assert!(
            new_session_cmd.contains("-x 200"),
            "new-session must pass -x 200; got: {new_session_cmd}"
        );
        assert!(
            new_session_cmd.contains("-y 50"),
            "new-session must pass -y 50; got: {new_session_cmd}"
        );
    }

    /// AC: Session creation sets global default-size after new-session
    /// — basic builder.
    #[test]
    fn basic_builder_sets_default_size_after_new_session() {
        let session = TmuxSessionBuilder::new("app")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let new_session_idx = cmds
            .iter()
            .position(|c| c.contains("new-session"))
            .expect("new-session in command list");
        let default_size_idx = cmds
            .iter()
            .position(|c| {
                c.contains("set-option") && c.contains("default-size") && c.contains("200x50")
            })
            .expect("set-option default-size 200x50 in command list");
        assert!(
            default_size_idx > new_session_idx,
            "set-option default-size must come AFTER new-session (set-option needs a running server); got order new={new_session_idx}, default-size={default_size_idx}"
        );
    }

    #[test]
    fn session_name_override_replaces_default() {
        let session = TmuxSessionBuilder::new("my-project")
            .session_name("custom-session-name".to_string())
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .build()
            .unwrap();

        assert_eq!(session.name, "custom-session-name");
        let cmds = session.command_strings();
        assert!(
            cmds.iter()
                .any(|c| c.contains("new-session") && c.contains("custom-session-name")),
            "should use overridden session name"
        );
    }

    // -----------------------------------------------------------------------
    // AC: Dynamic pane count based on input
    // Dry-run contract: verifies the number of commands matches the number of
    // panes the user requested. Actual pane creation verified by e2e test
    // tmux_session_with_five_panes_and_different_clis.
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
    // Dry-run contract: users see these exact commands in --dry-run output,
    // so the format (CLI command in send-keys, worktree on split-window -c)
    // is user-facing.
    // -----------------------------------------------------------------------

    #[test]
    fn each_pane_receives_bare_cli_command_and_split_carries_worktree() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("feat/auth", "/home/user/wt-auth", "claude"))
            .add_pane(make_pane("feat/api", "/home/user/wt-api", "gemini"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let send_keys = commands_containing(&cmds, "send-keys");

        // Pane 0 uses `-c` on `new-session` for its directory and runs only
        // the bare CLI command.
        assert!(
            send_keys[0].contains("claude"),
            "first pane should run claude; got: {}",
            send_keys[0]
        );

        // Subsequent panes must not prefix `cd <worktree> &&` — the cwd is
        // baked into the split via `-c <worktree>` instead, avoiding the
        // send-keys race documented at the call site.
        assert!(
            send_keys[1].contains("gemini"),
            "second pane should run gemini; got: {}",
            send_keys[1]
        );
        assert!(
            !send_keys[1].contains("cd /home/user/wt-api"),
            "second pane send-keys MUST NOT prefix `cd <worktree>`; got: {}",
            send_keys[1]
        );

        // The split-window that creates pane 1 should carry the worktree as
        // `-c <worktree>`.
        let splits = commands_containing(&cmds, "split-window");
        assert!(
            splits.iter().any(|c| c.contains("-c /home/user/wt-api")),
            "split-window for pane 1 should pass -c /home/user/wt-api; got: {splits:?}"
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
    // Dry-run contract: title format is user-visible in both --dry-run output
    // and tmux pane borders. Actual tmux titles verified by e2e test
    // tmux_session_with_five_panes_and_different_clis.
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

    // -----------------------------------------------------------------------
    // AC: Mouse mode (per-session, configurable, default on)
    // Dry-run contract: users see mouse config in --dry-run output.
    // Actual tmux behavior verified by e2e test tmux_mouse_mode_enabled_by_default.
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
    // Behavioral: tests against a real tmux server — verifies observable
    // outcomes (session exists, session is killed, names are unique).
    // -----------------------------------------------------------------------

    /// Helper to create a detached tmux session for testing.
    fn create_test_session(name: &str) {
        let output = std::process::Command::new("tmux")
            .args(["new-session", "-d", "-s", name, "-x", "200", "-y", "50"])
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

    // -----------------------------------------------------------------------
    // AC: pipe-pane logging integration
    // Dry-run contract: verifies the pipe-pane command is queued correctly.
    // -----------------------------------------------------------------------

    #[test]
    fn pipe_pane_queues_correct_command() {
        let mut session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("feat/auth", "/tmp/wt1", "claude"))
            .build()
            .unwrap();

        let log_path = std::path::PathBuf::from("/repo/.git-paw/logs/paw-proj/feat--auth.log");
        session.pipe_pane("paw-proj:0.0", &log_path);

        let cmds = session.command_strings();
        let pipe_cmds: Vec<&String> = cmds.iter().filter(|c| c.contains("pipe-pane")).collect();
        assert_eq!(pipe_cmds.len(), 1);
        assert!(pipe_cmds[0].contains("pipe-pane -o -t paw-proj:0.0"));
        assert!(pipe_cmds[0].contains("cat >> /repo/.git-paw/logs/paw-proj/feat--auth.log"));
    }

    // --- Gap #10: pipe-pane conditional on logging ---

    #[test]
    fn session_without_pipe_pane_has_no_pipe_pane_commands() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        assert!(
            !cmds.iter().any(|c| c.contains("pipe-pane")),
            "session built without pipe_pane calls should have no pipe-pane commands"
        );
    }

    #[test]
    fn session_with_pipe_pane_differs_from_without() {
        let session_without = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .build()
            .unwrap();
        let cmds_without = session_without.command_strings();

        let mut session_with = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .build()
            .unwrap();
        let log_path = std::path::PathBuf::from("/repo/.git-paw/logs/paw-proj/main.log");
        session_with.pipe_pane("paw-proj:0.0", &log_path);
        let cmds_with = session_with.command_strings();

        assert_ne!(
            cmds_without, cmds_with,
            "command lists should differ when pipe-pane is added"
        );
        assert!(
            cmds_with.iter().any(|c| c.contains("pipe-pane")),
            "session with pipe_pane should contain pipe-pane command"
        );
    }

    // --- Gap #11: pipe-pane ordering ---

    #[test]
    fn pipe_pane_appears_after_send_keys_for_pane() {
        let mut session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("feat/auth", "/tmp/wt1", "claude"))
            .add_pane(make_pane("feat/api", "/tmp/wt2", "codex"))
            .build()
            .unwrap();

        let log0 = std::path::PathBuf::from("/repo/logs/feat--auth.log");
        let log1 = std::path::PathBuf::from("/repo/logs/feat--api.log");
        session.pipe_pane("paw-proj:0.0", &log0);
        session.pipe_pane("paw-proj:0.1", &log1);

        let cmds = session.command_strings();

        // Find the last send-keys index and first pipe-pane index
        let last_send_keys = cmds
            .iter()
            .rposition(|c| c.contains("send-keys"))
            .expect("should have send-keys");
        let first_pipe_pane = cmds
            .iter()
            .position(|c| c.contains("pipe-pane"))
            .expect("should have pipe-pane");

        assert!(
            first_pipe_pane > last_send_keys,
            "pipe-pane commands (index {first_pipe_pane}) should appear after \
             all send-keys commands (last at index {last_send_keys})"
        );
    }

    #[test]
    fn pipe_pane_appears_in_dry_run_output() {
        let mut session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .build()
            .unwrap();

        let log_path = std::path::PathBuf::from("/repo/.git-paw/logs/paw-proj/main.log");
        session.pipe_pane("paw-proj:0.0", &log_path);

        let cmds = session.command_strings();
        assert!(
            cmds.iter().any(|c| c.starts_with("tmux pipe-pane")),
            "dry-run output should include pipe-pane command"
        );
    }

    // -----------------------------------------------------------------------
    // AC: set_environment emits correct commands
    // -----------------------------------------------------------------------

    #[test]
    fn set_environment_emits_correct_tmux_command() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .set_environment("GIT_PAW_BROKER_URL", "http://127.0.0.1:9119")
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let env_cmds = commands_containing(&cmds, "set-environment");
        assert_eq!(env_cmds.len(), 1, "should have exactly one set-environment");
        assert!(
            env_cmds[0]
                .contains("set-environment -t paw-proj GIT_PAW_BROKER_URL http://127.0.0.1:9119"),
            "set-environment command should contain key and value, got: {}",
            env_cmds[0]
        );
    }

    #[test]
    fn set_environment_appears_before_send_keys() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("feat/a", "/tmp/a", "claude"))
            .add_pane(make_pane("feat/b", "/tmp/b", "codex"))
            .set_environment("GIT_PAW_BROKER_URL", "http://127.0.0.1:9119")
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let first_env = cmds
            .iter()
            .position(|c| c.contains("set-environment"))
            .expect("should have set-environment");
        let first_send = cmds
            .iter()
            .position(|c| c.contains("send-keys"))
            .expect("should have send-keys");

        assert!(
            first_env < first_send,
            "set-environment (index {first_env}) should appear before first send-keys (index {first_send})"
        );
    }

    #[test]
    fn multiple_env_vars_both_appear() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .set_environment("A", "1")
            .set_environment("B", "2")
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let env_cmds = commands_containing(&cmds, "set-environment");
        assert_eq!(
            env_cmds.len(),
            2,
            "should have two set-environment commands"
        );
        assert!(env_cmds[0].contains("A 1"));
        assert!(env_cmds[1].contains("B 2"));
    }

    #[test]
    fn set_environment_in_dry_run_output() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .set_environment("MY_VAR", "my_val")
            .build()
            .unwrap();

        let cmds = session.command_strings();
        assert!(
            cmds.iter().any(|c| c.starts_with("tmux set-environment")),
            "dry-run output should include set-environment command"
        );
    }

    // -----------------------------------------------------------------------
    // AC: Dashboard layout selection
    // Behavioral: verifies the correct layout is chosen based on pane structure
    // -----------------------------------------------------------------------

    #[test]
    fn session_without_dashboard_uses_tiled_layout() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("feat/a", "/tmp/a", "claude"))
            .add_pane(make_pane("feat/b", "/tmp/b", "codex"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let layout_cmds: Vec<&String> = cmds
            .iter()
            .filter(|c| c.contains("select-layout"))
            .collect();
        let final_layout = layout_cmds
            .last()
            .expect("should have at least one select-layout");
        assert!(
            final_layout.contains("tiled"),
            "sessions without dashboard should use tiled layout, got: {final_layout}"
        );
    }

    #[test]
    fn session_with_dashboard_uses_main_horizontal_layout() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("dashboard", "/tmp/repo", "git-paw __dashboard"))
            .add_pane(make_pane("feat/a", "/tmp/a", "claude"))
            .add_pane(make_pane("feat/b", "/tmp/b", "codex"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let layout_cmds: Vec<&String> = cmds
            .iter()
            .filter(|c| c.contains("select-layout"))
            .collect();
        let final_layout = layout_cmds
            .last()
            .expect("should have at least one select-layout");
        assert!(
            final_layout.contains("main-horizontal"),
            "sessions with dashboard should use main-horizontal layout, got: {final_layout}"
        );
    }

    #[test]
    fn single_pane_session_uses_tiled_layout() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("main", "/tmp/wt", "claude"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        let layout_cmds: Vec<&String> = cmds
            .iter()
            .filter(|c| c.contains("select-layout"))
            .collect();
        let final_layout = layout_cmds
            .last()
            .expect("should have at least one select-layout");
        assert!(
            final_layout.contains("tiled"),
            "single pane sessions should use tiled layout, got: {final_layout}"
        );
    }

    #[test]
    fn dashboard_layout_appears_in_dry_run_output() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("dashboard", "/tmp/repo", "git-paw __dashboard"))
            .add_pane(make_pane("feat/a", "/tmp/a", "claude"))
            .build()
            .unwrap();

        let cmds = session.command_strings();
        assert!(
            cmds.iter().any(|c| c.contains("main-horizontal")),
            "dry-run output should include main-horizontal layout command"
        );
    }

    // -----------------------------------------------------------------------
    // AC: detach_client + kill_pane behave idempotently
    // -----------------------------------------------------------------------

    /// Helper that yields a unique detached test session name and cleans it
    /// up on drop. Used to keep pause-related tmux tests hermetic.
    struct PausePaneSession {
        name: String,
    }

    impl PausePaneSession {
        fn new(label: &str) -> Self {
            let pid = std::process::id();
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            let name = format!("paw-pause-test-{label}-{pid}-{nanos}");
            let output = std::process::Command::new("tmux")
                .args(["new-session", "-d", "-s", &name, "-x", "200", "-y", "50"])
                .output()
                .expect("create tmux test session");
            assert!(
                output.status.success(),
                "failed to create test session '{name}'"
            );
            Self { name }
        }
    }

    impl Drop for PausePaneSession {
        fn drop(&mut self) {
            let _ = kill_session(&self.name);
        }
    }

    #[test]
    #[serial_test::serial]
    fn detach_client_succeeds_on_attached_session() {
        // No client is actually attached in headless test, but a detached
        // session under tmux server is the closest the unit layer can get
        // without a pty; the public contract is "exit Ok" either way.
        let session = PausePaneSession::new("detach-attached");
        detach_client(&session.name).expect("detach should succeed");
        assert!(is_session_alive(&session.name).unwrap());
    }

    #[test]
    #[serial_test::serial]
    fn detach_client_is_noop_with_no_clients() {
        let session = PausePaneSession::new("detach-noop");
        // First call: no clients attached.
        detach_client(&session.name).expect("first detach should succeed");
        // Second call: also no clients (still alive).
        detach_client(&session.name).expect("second detach should succeed");
        assert!(is_session_alive(&session.name).unwrap());
    }

    /// Spec-aligned alias of `detach_client_is_noop_with_no_clients`
    /// (task 9.11). A detached test session has no client attached;
    /// `detach_client` must still return Ok(()).
    #[test]
    #[serial_test::serial]
    fn detach_client_noop_when_no_clients_attached() {
        let session = PausePaneSession::new("detach-9-11");
        detach_client(&session.name).expect("detach with no clients should be Ok");
        assert!(is_session_alive(&session.name).unwrap());
    }

    #[test]
    #[serial_test::serial]
    fn kill_pane_removes_pane() {
        let session = PausePaneSession::new("killpane");
        // Add a second pane so the kill doesn't take down the whole session.
        let _ = std::process::Command::new("tmux")
            .args(["split-window", "-t", &session.name])
            .output();
        let pane_count_before = std::process::Command::new("tmux")
            .args(["list-panes", "-t", &session.name, "-F", "#{pane_index}"])
            .output()
            .map_or(0, |o| String::from_utf8_lossy(&o.stdout).lines().count());
        assert_eq!(pane_count_before, 2, "should have 2 panes before kill");

        kill_pane(&session.name, 1).expect("kill_pane should succeed");

        let pane_count_after = std::process::Command::new("tmux")
            .args(["list-panes", "-t", &session.name, "-F", "#{pane_index}"])
            .output()
            .map_or(0, |o| String::from_utf8_lossy(&o.stdout).lines().count());
        assert_eq!(pane_count_after, 1, "should have 1 pane after kill");
    }

    #[test]
    #[serial_test::serial]
    fn kill_pane_is_noop_for_missing_pane() {
        let session = PausePaneSession::new("killpane-missing");
        // Pane index 99 does not exist — should not error.
        kill_pane(&session.name, 99).expect("kill missing pane should be ok");
        assert!(is_session_alive(&session.name).unwrap());
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

    // -----------------------------------------------------------------------
    // AC: Supervisor-mode initial prompt is injected as a paste + two Enters
    // Behavioral: callers iterate the argv pair and run each as a separate
    // `tmux send-keys` invocation. The pair shape is the public contract.
    // -----------------------------------------------------------------------

    #[test]
    fn supervisor_submit_argv_pair_has_two_invocations() {
        let (first, second) = build_supervisor_submit_argv_pair("paw-proj", 3, "do the thing");
        // Both invocations are non-empty argv vectors.
        assert!(!first.is_empty(), "first send-keys argv must be non-empty");
        assert!(
            !second.is_empty(),
            "second send-keys argv must be non-empty"
        );
    }

    #[test]
    fn supervisor_submit_first_invocation_sends_prompt_and_enter() {
        let (first, _second) = build_supervisor_submit_argv_pair("paw-proj", 3, "do the thing");
        assert_eq!(first[0], "send-keys");
        assert_eq!(first[1], "-t");
        assert_eq!(first[2], "paw-proj:0.3");
        assert_eq!(first[3], "do the thing");
        assert_eq!(first[4], "Enter");
    }

    #[test]
    fn supervisor_submit_second_invocation_is_enter_only() {
        let (_first, second) = build_supervisor_submit_argv_pair("paw-proj", 3, "do the thing");
        assert_eq!(second[0], "send-keys");
        assert_eq!(second[1], "-t");
        assert_eq!(second[2], "paw-proj:0.3");
        assert_eq!(second[3], "Enter");
        assert_eq!(
            second.len(),
            4,
            "second invocation should be send-keys -t <target> Enter (no prompt)"
        );
    }

    #[test]
    fn supervisor_submit_targets_same_pane_in_both_invocations() {
        let (first, second) = build_supervisor_submit_argv_pair("paw-proj", 7, "prompt");
        // The target (third positional arg after `send-keys -t`) must match
        // so the second Enter lands in the same pane the prompt was sent to.
        assert_eq!(first[2], second[2]);
        assert_eq!(first[2], "paw-proj:0.7");
    }

    #[test]
    fn supervisor_submit_argv_pair_preserves_prompt_with_newlines_and_quotes() {
        let prompt = "line1\nline2 with \"quoted\" text";
        let (first, _second) = build_supervisor_submit_argv_pair("paw-proj", 1, prompt);
        // The prompt is passed verbatim as its own argv element; tmux's
        // send-keys treats it as literal text. No shell escaping needed.
        assert_eq!(first[3], prompt);
    }

    // Maps to scenario `Launch flow sends exactly one Enter per pane`
    // (cmd_supervisor invariant) from prompt-submit-fix. The
    // `submit_prompt_to_pane` helper in main.rs sends prompt + one Enter
    // per pane and is shaped identically to the FIRST argv returned by
    // `build_supervisor_submit_argv_pair`. We count Enter tokens across
    // the first-argv portion of N=3 invocations to lock in the
    // single-Enter-per-pane invariant. (test-coverage-v0-5-0 task 3.1)
    #[test]
    fn cmd_supervisor_inject_argv_has_single_enter_per_pane() {
        let panes: Vec<(usize, &str)> = vec![(2, "p2"), (3, "p3"), (4, "p4")];

        let mut total_enters = 0;
        for (pane_idx, prompt) in &panes {
            let (first, _second) = build_supervisor_submit_argv_pair("paw-proj", *pane_idx, prompt);
            let enter_positions: Vec<usize> = first
                .iter()
                .enumerate()
                .filter(|(_, tok)| tok.as_str() == "Enter")
                .map(|(i, _)| i)
                .collect();
            assert_eq!(
                enter_positions.len(),
                1,
                "each per-pane invocation must send exactly one Enter; got argv: {first:?}"
            );
            let enter_pos = enter_positions[0];
            assert!(
                enter_pos > 0,
                "Enter token must follow a prompt-string argument; got argv: {first:?}"
            );
            assert_eq!(
                first[enter_pos - 1].as_str(),
                *prompt,
                "Enter token must directly follow the prompt argument; got argv: {first:?}"
            );
            total_enters += enter_positions.len();
        }
        assert_eq!(
            total_enters, 3,
            "for N=3 panes the launch flow must send exactly N=3 Enters"
        );
    }

    // -----------------------------------------------------------------------
    // build_supervisor_session — layout-shape contract (tasks 9.1–9.7)
    //
    // Behavioral: we inspect the emitted command strings to verify the layout
    // shape. The exact tmux side effects are integration-tested elsewhere;
    // here we lock in the deterministic command sequence the supervisor-mode
    // pane assumptions depend on (supervisor=0, dashboard=1, agents=2+).
    // -----------------------------------------------------------------------

    fn make_layout_panes(n: usize) -> (PaneSpec, PaneSpec, Vec<PaneSpec>) {
        let supervisor = make_pane("supervisor", "/repo", "claude");
        let dashboard = make_pane("dashboard", "/repo", "git-paw __dashboard");
        let agents = (0..n)
            .map(|i| make_pane(&format!("feat/b{i}"), &format!("/tmp/wt{i}"), "claude"))
            .collect();
        (supervisor, dashboard, agents)
    }

    fn build_for(agent_count: usize) -> TmuxSession {
        let layout =
            crate::supervisor::layout::supervisor_layout(agent_count).expect("layout computes");
        let (supervisor, dashboard, agents) = make_layout_panes(agent_count);
        build_supervisor_session(
            "proj",
            None,
            &supervisor,
            &dashboard,
            &agents,
            layout,
            true,
            &[("GIT_PAW_BROKER_URL".to_string(), "http://x".to_string())],
        )
        .expect("session builds")
    }

    /// 9.1 — 5-agent layout: 1 agent row, top 60% / agent row 40%.
    #[test]
    fn supervisor_layout_5_agents_single_row() {
        let session = build_for(5);
        let cmds = session.command_strings();
        let send_keys = commands_containing(&cmds, "send-keys");
        assert_eq!(
            send_keys.len(),
            7,
            "5 agents → 1 supervisor + 1 dashboard + 5 agents = 7 send-keys, got {send_keys:#?}"
        );
        let supervisor_pane = send_keys
            .iter()
            .find(|c| c.contains("0.0 "))
            .unwrap_or(&send_keys[0]);
        assert!(supervisor_pane.contains("claude"));
        let dashboard_pane = send_keys
            .iter()
            .find(|c| c.contains(":0.1 ") && c.contains("__dashboard"))
            .expect("dashboard send-keys at pane :0.1");
        let _ = dashboard_pane;
        // Top row resize-pane uses 60%.
        let resizes = commands_containing(&cmds, "resize-pane");
        assert!(
            resizes
                .iter()
                .any(|c| c.contains(":0.0") && c.contains("60%")),
            "top row resize to 60%, got resizes {resizes:#?}"
        );
        // Single agent row resize at pane :0.2 with 40%.
        assert!(
            resizes
                .iter()
                .any(|c| c.contains(":0.2") && c.contains("40%")),
            "agent-row resize to 40% at :0.2, got resizes {resizes:#?}"
        );
    }

    /// 9.2 — 10-agent layout: 2 rows of 5, top 40% / each agent row 30%.
    #[test]
    fn supervisor_layout_10_agents_two_rows() {
        let session = build_for(10);
        let cmds = session.command_strings();
        let send_keys = commands_containing(&cmds, "send-keys");
        assert_eq!(
            send_keys.len(),
            12,
            "10 agents → 1 supervisor + 1 dashboard + 10 agents = 12 send-keys"
        );
        let resizes = commands_containing(&cmds, "resize-pane");
        assert!(
            resizes
                .iter()
                .any(|c| c.contains(":0.0") && c.contains("40%"))
        );
        assert!(
            resizes.iter().filter(|c| c.contains("30%")).count() >= 2,
            "two agent rows at 30% each, got {resizes:#?}"
        );
    }

    /// 9.3 — 11-agent layout: 3 agent rows (5+5+1), top 28% / each agent row 24%.
    #[test]
    fn supervisor_layout_11_agents_three_rows() {
        let session = build_for(11);
        let cmds = session.command_strings();
        let resizes = commands_containing(&cmds, "resize-pane");
        assert!(
            resizes
                .iter()
                .any(|c| c.contains(":0.0") && c.contains("28%"))
        );
        assert!(
            resizes.iter().filter(|c| c.contains("24%")).count() >= 3,
            "three agent rows at 24% each, got {resizes:#?}"
        );
        // 11 agents start at pane 2 and run through pane 12.
        let send_keys = commands_containing(&cmds, "send-keys");
        assert_eq!(send_keys.len(), 13);
        assert!(send_keys.iter().any(|c| c.contains(":0.12 ")));
    }

    /// 9.4 — 20-agent layout: 4 rows of 5, top 28% / each agent row 18%.
    #[test]
    fn supervisor_layout_20_agents_four_rows() {
        let session = build_for(20);
        let cmds = session.command_strings();
        let resizes = commands_containing(&cmds, "resize-pane");
        assert!(
            resizes
                .iter()
                .any(|c| c.contains(":0.0") && c.contains("28%"))
        );
        assert!(
            resizes.iter().filter(|c| c.contains("18%")).count() >= 4,
            "four agent rows at 18% each, got {resizes:#?}"
        );
    }

    /// 9.5 — 25-agent layout: 5 rows of 5, top 28% / each agent row 14.4%.
    #[test]
    fn supervisor_layout_25_agents_five_rows() {
        let session = build_for(25);
        let cmds = session.command_strings();
        let resizes = commands_containing(&cmds, "resize-pane");
        assert!(
            resizes
                .iter()
                .any(|c| c.contains(":0.0") && c.contains("28%"))
        );
        assert!(
            resizes.iter().filter(|c| c.contains("14.4%")).count() >= 5,
            "five agent rows at 14.4% each, got {resizes:#?}"
        );
    }

    /// 9.6 — 26-agent attempt errors before any tmux command runs.
    #[test]
    fn supervisor_layout_26_agents_rejected_by_layout_helper() {
        // The layout helper is the single gate for the hard cap; the tmux
        // builder is unreachable when supervisor_layout errors.
        let err = crate::supervisor::layout::supervisor_layout(26).expect_err("26 agents rejected");
        let msg = err.to_string();
        assert!(msg.contains("26 agents requested"));
        assert!(msg.contains("maximum is 25"));
    }

    /// 9.7 — pane indices follow row-major order. With 7 agents, pane 2 is
    /// the first agent (top-left), pane 6 is the fifth (top-right of row 1),
    /// pane 7 is the sixth (start of row 2).
    #[test]
    fn supervisor_layout_7_agents_row_major_indices() {
        let session = build_for(7);
        let cmds = session.command_strings();
        let send_keys = commands_containing(&cmds, "send-keys");
        // pane :0.2 is the first agent — its send-keys must contain its CLI
        // command. Likewise :0.6 (fifth agent) and :0.7 (sixth agent).
        assert!(
            send_keys
                .iter()
                .any(|c| c.contains(":0.2 ") && c.contains("claude")),
            "pane :0.2 is the first agent (top-left); send-keys {send_keys:#?}"
        );
        assert!(
            send_keys
                .iter()
                .any(|c| c.contains(":0.6 ") && c.contains("claude")),
            "pane :0.6 is the fifth agent (top-right of row 1)"
        );
        assert!(
            send_keys
                .iter()
                .any(|c| c.contains(":0.7 ") && c.contains("claude")),
            "pane :0.7 is the sixth agent (start of row 2)"
        );
    }

    // Maps to scenario `Top row is split 50/50 between supervisor and
    // dashboard` from supervisor-as-pane. (test-coverage-v0-5-0 task 12.7)
    #[test]
    fn supervisor_top_row_split_50_50() {
        let session = build_for(3);
        let cmds = session.command_strings();
        let h_split = cmds
            .iter()
            .find(|c| c.contains("split-window") && c.contains("-h") && c.contains("-p 50"))
            .unwrap_or_else(|| panic!("expected horizontal 50% split; got cmds: {cmds:#?}"));
        assert!(
            h_split.contains(":0.0") || h_split.contains("split-window -h -t paw-proj"),
            "horizontal split should target the supervisor pane; got: {h_split}"
        );
    }

    /// AC: Supervisor session passes -x/-y to new-session for headless
    /// environments.
    #[test]
    fn supervisor_new_session_passes_explicit_x_and_y() {
        let session = build_for(2);
        let cmds = session.command_strings();
        let new_session_cmd = cmds
            .iter()
            .find(|c| c.contains("new-session"))
            .expect("supervisor build emits a new-session command");
        assert!(
            new_session_cmd.contains("-x 200"),
            "supervisor new-session must pass -x 200; got: {new_session_cmd}"
        );
        assert!(
            new_session_cmd.contains("-y 50"),
            "supervisor new-session must pass -y 50; got: {new_session_cmd}"
        );
    }

    /// AC: Supervisor session sets global default-size after new-session.
    #[test]
    fn supervisor_sets_default_size_after_new_session() {
        let session = build_for(2);
        let cmds = session.command_strings();
        let new_session_idx = cmds
            .iter()
            .position(|c| c.contains("new-session"))
            .expect("new-session in command list");
        let default_size_idx = cmds
            .iter()
            .position(|c| {
                c.contains("set-option") && c.contains("default-size") && c.contains("200x50")
            })
            .expect("set-option default-size 200x50 in command list");
        assert!(
            default_size_idx > new_session_idx,
            "set-option default-size must come AFTER new-session; got order new={new_session_idx}, default-size={default_size_idx}"
        );
    }

    // Maps to scenario `Broker enabled in bare-start mode adds dashboard as
    // pane 0` from supervisor-as-pane. The bare-start tmux build uses
    // `TmuxSessionBuilder::add_pane(...)` in source order — production code
    // adds the dashboard pane first when broker is enabled. We mirror that
    // order in the test fixture so the pane-index contract is asserted.
    // (test-coverage-v0-5-0 task 12.1)
    #[test]
    fn bare_start_with_broker_places_dashboard_at_pane_0() {
        // Mirror cmd_start with broker enabled: dashboard first, then agents.
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("dashboard", "/repo", "git-paw __dashboard"))
            .add_pane(make_pane("feat/a", "/tmp/wt-a", "claude"))
            .add_pane(make_pane("feat/b", "/tmp/wt-b", "claude"))
            .add_pane(make_pane("feat/c", "/tmp/wt-c", "claude"))
            .build()
            .expect("session builds");

        let cmds = session.command_strings();
        let dashboard_send = cmds
            .iter()
            .find(|c| c.contains("send-keys") && c.contains("__dashboard"))
            .expect("dashboard send-keys present");
        assert!(
            dashboard_send.contains(":0.0 "),
            "dashboard pane must be index 0; got: {dashboard_send}"
        );
        // Each agent pane carries its worktree on the `split-window -c`
        // (the pane is created in the worktree directly to avoid the
        // `cd && cli` send-keys race) AND has a `select-pane -T` at the
        // expected pane index.
        for (pane_idx, branch_marker, worktree) in [
            (1, "feat/a", "/tmp/wt-a"),
            (2, "feat/b", "/tmp/wt-b"),
            (3, "feat/c", "/tmp/wt-c"),
        ] {
            let select_target = format!(":0.{pane_idx} ");
            assert!(
                cmds.iter()
                    .any(|c| c.contains(&select_target) && c.contains(branch_marker)),
                "agent {branch_marker} should land at pane {pane_idx}; cmds:\n{cmds:#?}"
            );
            let split_marker = format!("-c {worktree}");
            assert!(
                cmds.iter()
                    .any(|c| c.contains("split-window") && c.contains(&split_marker)),
                "agent {branch_marker} split should carry {split_marker}; cmds:\n{cmds:#?}"
            );
        }
    }

    // Maps to scenario `Broker disabled produces no dashboard pane` from
    // supervisor-as-pane. (test-coverage-v0-5-0 task 12.2)
    #[test]
    fn broker_disabled_produces_no_dashboard_pane() {
        let session = TmuxSessionBuilder::new("proj")
            .add_pane(make_pane("feat/a", "/tmp/wt-a", "claude"))
            .add_pane(make_pane("feat/b", "/tmp/wt-b", "claude"))
            .add_pane(make_pane("feat/c", "/tmp/wt-c", "claude"))
            .build()
            .expect("session builds");

        let cmds = session.command_strings();
        assert!(
            !cmds.iter().any(|c| c.contains("__dashboard")),
            "broker disabled must not add a dashboard pane; got cmds:\n{cmds:#?}"
        );
        // Three send-keys (one per agent pane), no dashboard send-keys.
        let send_keys: Vec<&String> = cmds.iter().filter(|c| c.contains("send-keys")).collect();
        assert_eq!(
            send_keys.len(),
            3,
            "broker-disabled launch with 3 agents must emit 3 send-keys; got: {send_keys:#?}"
        );
    }

    // Maps to scenario `Dashboard pane title` from supervisor-as-pane.
    // (test-coverage-v0-5-0 task 12.3)
    #[test]
    fn dashboard_pane_has_title_dashboard() {
        // Use the supervisor layout (the dashboard-bearing argv builder).
        let session = build_for(2);
        let cmds = session.command_strings();
        let dashboard_select = cmds
            .iter()
            .find(|c| {
                c.contains("select-pane")
                    && c.contains(":0.1")
                    && c.contains("-T")
                    && c.contains("dashboard")
            })
            .unwrap_or_else(|| {
                panic!("expected select-pane -T dashboard at :0.1; cmds:\n{cmds:#?}")
            });
        // The shipped title shape is `<branch> → <cli_command>` with branch =
        // "dashboard". Confirm the title argument contains the bare word.
        assert!(
            dashboard_select.contains("dashboard"),
            "dashboard pane title must include `dashboard`; got: {dashboard_select}"
        );
    }

    /// Sanity: `env_vars` surface as set-environment commands BEFORE any
    /// agent-pane send-keys, so coding agents inherit `GIT_PAW_BROKER_URL`.
    #[test]
    fn supervisor_layout_emits_env_before_agent_send_keys() {
        let session = build_for(3);
        let cmds = session.command_strings();
        let first_env = cmds
            .iter()
            .position(|c| c.contains("set-environment") && c.contains("GIT_PAW_BROKER_URL"))
            .expect("set-environment GIT_PAW_BROKER_URL present");
        let first_agent_send = cmds
            .iter()
            .position(|c| c.contains("send-keys") && c.contains(":0.2 "))
            .expect("first agent send-keys at :0.2");
        assert!(
            first_env < first_agent_send,
            "set-environment must come before agent-pane send-keys"
        );
    }

    /// Bug B regression coverage: every agent pane SHALL be created with
    /// `-c <agent.worktree>` on its split, and the follow-up `send-keys`
    /// SHALL NOT use the `cd <worktree> && <cli>` race chain.
    #[test]
    fn supervisor_layout_agent_splits_carry_worktree_no_cd_chain() {
        let layout = crate::supervisor::layout::supervisor_layout(2).expect("layout");
        let supervisor = make_pane("supervisor", "/repo", "claude");
        let dashboard = make_pane("dashboard", "/repo", "git-paw __dashboard");
        let agent_a = make_pane("feat/a", "/tmp/wt-a", "claude");
        let agent_b = make_pane("feat/b", "/tmp/wt-b", "claude");
        let session = build_supervisor_session(
            "proj",
            None,
            &supervisor,
            &dashboard,
            &[agent_a, agent_b],
            layout,
            true,
            &[],
        )
        .expect("session builds");

        let cmds = session.command_strings();
        let splits = commands_containing(&cmds, "split-window");
        assert!(
            splits.iter().any(|c| c.contains("-c /tmp/wt-a")),
            "split for agent a should pass -c /tmp/wt-a; splits: {splits:#?}"
        );
        assert!(
            splits.iter().any(|c| c.contains("-c /tmp/wt-b")),
            "split for agent b should pass -c /tmp/wt-b; splits: {splits:#?}"
        );

        let send_keys = commands_containing(&cmds, "send-keys");
        for entry in &send_keys {
            assert!(
                !entry.contains("cd /tmp/wt-a &&"),
                "no send-keys should chain `cd /tmp/wt-a &&`; got: {entry}"
            );
            assert!(
                !entry.contains("cd /tmp/wt-b &&"),
                "no send-keys should chain `cd /tmp/wt-b &&`; got: {entry}"
            );
        }
    }
}
