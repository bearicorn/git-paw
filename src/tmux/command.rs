//! Tmux command and session assembly.
//!
//! The [`TmuxCommand`]/[`TmuxSession`] model, the [`TmuxSessionBuilder`], and
//! the supervisor/agent command builders. This is the builder + dry-run
//! surface: pure argv assembly plus the `execute`/`command_strings` methods on
//! a built session.

use crate::command_runner::{CommandRunner, RealCommandRunner};
use crate::error::PawError;

/// A single tmux CLI invocation, stored as its argument list.
///
/// Can be inspected as a string (for dry-run / testing) or executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxCommand {
    args: Vec<String>,
    /// When `true`, a non-zero exit is treated as a non-fatal warning rather
    /// than aborting the build. Used for the border-affordance `set-option`
    /// invocations, which older tmux versions may not recognise (design D4).
    soft: bool,
}

impl TmuxCommand {
    /// Create a new tmux command from the given arguments.
    fn new(args: &[&str]) -> Self {
        Self {
            args: args.iter().map(|&s| s.to_owned()).collect(),
            soft: false,
        }
    }

    /// Create a "soft" tmux command whose failure is non-fatal.
    ///
    /// On a non-zero exit (e.g. an option unsupported by an older tmux), the
    /// session executor emits a stderr warning naming the failed invocation
    /// and continues with the remaining commands. See [`TmuxSession::execute`].
    fn new_soft(args: &[&str]) -> Self {
        Self {
            args: args.iter().map(|&s| s.to_owned()).collect(),
            soft: true,
        }
    }

    /// Return a human-readable command string (e.g. `tmux new-session -d -s paw-proj`).
    // Not called by production code — used by `TmuxSession::command_strings()` for
    // dry-run contract tests that verify the commands shown to users via `--dry-run`.
    pub fn as_command_string(&self) -> String {
        format!("tmux {}", self.args.join(" "))
    }

    /// Execute the command against the live tmux server through `runner`.
    ///
    /// Behaviour is unchanged from the previous inline
    /// `Command::new("tmux")…output()`: on success the captured stdout is
    /// returned as UTF-8; on a non-zero exit the trimmed stderr becomes a
    /// [`PawError::TmuxError`]. Routing through the [`CommandRunner`] seam lets
    /// tests assert the exact argv and script success/failure without a live
    /// tmux server.
    fn execute(&self, runner: &dyn CommandRunner) -> Result<String, PawError> {
        let args: Vec<&str> = self.args.iter().map(String::as_str).collect();
        let output = runner
            .run("tmux", &args)
            .map_err(|e| PawError::TmuxError(format!("failed to run tmux: {e}")))?;

        if output.success {
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

/// Push the five border-affordance `set-option` invocations onto `commands`,
/// scoped to `session` (`-t <session>`, never the server or other windows).
///
/// The options give git-paw-managed sessions heavier, labelled, and
/// active-highlighted pane borders so the supervisor↔agent boundary is
/// visually distinct (see `supervisor-pane-affordances` spec):
///
/// - `pane-border-lines double` — `═║` double-line borders (tmux 3.2+) that
///   read as a stronger row separator than single/heavy lines. tmux has no
///   inter-pane margin/padding (panes tile flush), so the divider weight plus
///   the label bar below are the only levers for perceived separation.
/// - `pane-border-style fg=colour238` — dim inactive borders
/// - `pane-active-border-style fg=colour45,bold` — focused pane pops
/// - `pane-border-status top` — label strip above each pane
/// - `pane-border-format '#[fg=colour39,bold,reverse] #{pane_index}: #{?#{@paw_role},#{@paw_role},#{pane_title}} #[default]'`
///   — a reverse-video colored label *bar* per pane (reads as a header chip,
///   not plain text on the line), preferring the git-paw-set `@paw_role` pane
///   option over `#{pane_title}`. The format prefers `@paw_role` because the
///   agent CLI emits OSC title escape sequences that overwrite `#{pane_title}`
///   with its current activity; the pane-scoped `@paw_role` option (set by
///   [`push_pane_title`]) is not clobbered, so the role label survives. A pane
///   without `@paw_role` (e.g. a user-created pane) falls back to `#{pane_title}`.
///
/// Each is queued as a *soft* command: a non-zero exit on an older tmux that
/// lacks the option produces a stderr warning and the build continues (D4).
fn push_border_affordances(commands: &mut Vec<TmuxCommand>, session: &str) {
    for (option, value) in [
        ("pane-border-lines", "double"),
        ("pane-border-style", "fg=colour238"),
        ("pane-active-border-style", "fg=colour45,bold"),
        ("pane-border-status", "top"),
        (
            "pane-border-format",
            "#[fg=colour39,bold,reverse] #{pane_index}: #{?#{@paw_role},#{@paw_role},#{pane_title}} #[default]",
        ),
    ] {
        commands.push(TmuxCommand::new_soft(&[
            "set-option",
            "-t",
            session,
            option,
            value,
        ]));
    }
}

/// Queue the pane-title invocations that label a pane, but only when
/// `border_affordances` is enabled. The title is the pane's role or branch id
/// (`supervisor`, `dashboard`, or e.g. `feat/foo`) and renders in the
/// `pane-border-format` strip configured by [`push_border_affordances`].
///
/// Two commands are queued:
/// - `select-pane -T <title>` sets `#{pane_title}` (the OSC-style title).
/// - `set-option -p @paw_role <title>` sets a pane-scoped user option.
///
/// Both carry the same label, but the agent CLI overwrites `#{pane_title}` via
/// its own OSC title escape sequences as it works, so the `select-pane -T`
/// value does not survive. The pane-scoped `@paw_role` option is git-paw's and
/// is never clobbered, so the border-format prefers it (see
/// [`push_border_affordances`]) and the role label stays stable for the life
/// of the pane.
fn push_pane_title(
    commands: &mut Vec<TmuxCommand>,
    border_affordances: bool,
    target: &str,
    title: &str,
) {
    if border_affordances {
        commands.push(TmuxCommand::new(&[
            "select-pane",
            "-t",
            target,
            "-T",
            title,
        ]));
        // Pane-scoped user option: stable, not clobbered by the CLI's OSC
        // title sequences. The border-format prefers this over `#{pane_title}`.
        commands.push(TmuxCommand::new_soft(&[
            "set-option",
            "-p",
            "-t",
            target,
            "@paw_role",
            title,
        ]));
    }
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
    ///
    /// Soft commands (the border affordances) that fail produce a stderr
    /// warning naming the failed invocation and do not abort the build; any
    /// other command failure propagates as an error.
    pub fn execute(&self) -> Result<(), PawError> {
        let runner = RealCommandRunner;
        self.execute_with(|cmd| cmd.execute(&runner).map(|_| ()), |w| eprintln!("{w}"))
    }

    /// Run every queued command via `run`, routing non-fatal warnings to
    /// `warn`. Pulled out of [`execute`](Self::execute) so the soft-failure
    /// contract (warn + continue for soft commands, abort for the rest) can be
    /// exercised without a live tmux server.
    pub(crate) fn execute_with<R, W>(&self, mut run: R, mut warn: W) -> Result<(), PawError>
    where
        R: FnMut(&TmuxCommand) -> Result<(), PawError>,
        W: FnMut(String),
    {
        for cmd in &self.commands {
            if let Err(e) = run(cmd) {
                if cmd.soft {
                    warn(format!(
                        "warning: tmux option not supported: {} ({e})",
                        cmd.args.join(" ")
                    ));
                } else {
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Return all commands as human-readable strings (for dry-run / testing).
    // Not called by production code — used by unit tests as the dry-run contract
    // surface to verify the tmux commands shown to users via `--dry-run`.
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
    ///
    /// tmux runs the argument through `/bin/sh -c`, so `log_path` is
    /// shell-quoted ([`crate::domain::shell_quote`]) — a repository path
    /// containing a space would otherwise split into two shell words and the
    /// capture would append to the wrong file.
    pub fn pipe_pane(&mut self, pane_target: &str, log_path: &std::path::Path) -> &mut Self {
        self.commands.push(TmuxCommand::new(&[
            "pipe-pane",
            "-o",
            "-t",
            pane_target,
            &format!(
                "cat >> {}",
                crate::domain::shell_quote(&log_path.display().to_string())
            ),
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
    border_affordances: bool,
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
            border_affordances: true,
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

    /// Enable or disable the border affordances for the session (default:
    /// `true`).
    ///
    /// When enabled, the session receives heavy borders, dim/active border
    /// styling, and a per-pane label strip, and each pane's title is set to
    /// its role/branch id. When disabled, none of these `set-option` or
    /// `select-pane -T` invocations are emitted and the session inherits the
    /// user's default tmux styling. Driven by `[layout].border_affordances`.
    #[must_use]
    pub fn border_affordances(mut self, enabled: bool) -> Self {
        self.border_affordances = enabled;
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

        // An override carries a name already resolved through `SessionName`
        // (see `resolve_session_name`); otherwise derive it here so a project
        // directory named `my.app` or `My Project` still yields a valid tmux
        // target.
        let session_name = self.session_name_override.unwrap_or_else(|| {
            crate::domain::SessionName::from_project(&self.project_name).into_string()
        });
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
            "480",
            "-y",
            "140",
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
            "480x140",
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

        // 3. Border affordances — heavy borders, dim/active styling, and the
        //    per-pane label strip. Gated by `border_affordances`; when off the
        //    session inherits the user's default tmux styling.
        if self.border_affordances {
            push_border_affordances(&mut commands, &session_name);
        }

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

        // 5. First pane — already exists as pane 0 (directory set by -c above).
        //    The title is the pane's role/branch id (not the CLI command) so it
        //    reads cleanly in the label strip configured above.
        let first = &self.panes[0];
        let pane_target = format!("{session_name}:0.0");
        push_pane_title(
            &mut commands,
            self.border_affordances,
            &pane_target,
            &first.branch,
        );
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
            push_pane_title(
                &mut commands,
                self.border_affordances,
                &pane_target,
                &pane.branch,
            );
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
    border_affordances: bool,
    env_vars: &[(String, String)],
) -> Result<TmuxSession, PawError> {
    use crate::supervisor::layout::{SUPERVISOR_AGENTS_PER_ROW, SUPERVISOR_PANE_OFFSET};

    let session_name = session_name_override
        .unwrap_or_else(|| crate::domain::SessionName::from_project(project_name).into_string());
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
            "480",
            "-y",
            "140",
            // Suppress interactive shell startup prompts that would otherwise
            // fire as pane 0's shell reads its rc and could swallow the first
            // keystroke of the CLI-launch command (W2-2: oh-my-zsh's
            // `Would you like to update? [Y/n]` ate the leading `c` of the CLI
            // name). `-e` sets the variables BEFORE the shell starts, so the
            // framework never prompts. Inert for shells that don't read them.
            "-e",
            "DISABLE_AUTO_UPDATE=true",
            "-e",
            "DISABLE_UPDATE_PROMPT=true",
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
        &["set-option", "-g", "default-size", "480x140"],
    );

    // Carry the shell-startup-prompt suppression (W2-2) into the session
    // environment too, so the agent panes created by later `split-window`
    // calls inherit it (the `-e` flags above only cover pane 0's shell).
    push(
        &mut commands,
        &[
            "set-environment",
            "-t",
            &session_name,
            "DISABLE_AUTO_UPDATE",
            "true",
        ],
    );
    push(
        &mut commands,
        &[
            "set-environment",
            "-t",
            &session_name,
            "DISABLE_UPDATE_PROMPT",
            "true",
        ],
    );

    // 2. Mouse + pane border config.
    if mouse_mode {
        push(
            &mut commands,
            &["set-option", "-t", &session_name, "mouse", "on"],
        );
    }
    if border_affordances {
        push_border_affordances(&mut commands, &session_name);
    }

    // 3. Session-level environment variables (before any send-keys).
    for (key, value) in env_vars {
        push(
            &mut commands,
            &["set-environment", "-t", &session_name, key, value],
        );
    }

    let supervisor_target = format!("{session_name}:0.0");
    push_pane_title(
        &mut commands,
        border_affordances,
        &supervisor_target,
        &supervisor.branch,
    );
    // Clear the input line before launching (W2-2): a stray shell-startup
    // prompt or buffered keystroke could otherwise corrupt the leading
    // character of the CLI command. `C-u` on a clean prompt is a no-op.
    push(
        &mut commands,
        &["send-keys", "-t", &supervisor_target, "C-u"],
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
    //
    // Use `-l <N>%` (the modern tmux 3.1+ form) instead of the deprecated
    // `-p <N>`. On Linux tmux 3.4 (Ubuntu 24.04 apt-package), `-p`
    // resolves the percentage against the parent pane's laid-out size,
    // which is empty on a detached server with no attached client — tmux
    // bails with `cmd-split-window.c: "size missing"`. `-l <N>%` resolves
    // against the window's `-y` dimension instead, which is the value we
    // set on `new-session -x 200 -y 50`, so the split math succeeds in
    // headless mode. macOS tmux 3.6a tolerates either form.
    let bottom_pct = format!("{}%", 100u16 - u16::from(layout.top_row_pct));
    // W3-1: step 6 swaps panes 1 and 2. `swap-pane` carries each pane's cwd
    // to the OTHER index, but the CLI commands + titles are sent post-swap by
    // index — so the `-c` cwds must be assigned to COMPENSATE for the swap.
    // The agent-area split (which lands at the dashboard's index-1 after the
    // swap) therefore gets the dashboard's cwd, and the dashboard split (which
    // lands at the agent's index-2) gets the first agent's worktree. Without
    // this compensation the first agent's pane inherits the supervisor's
    // repo-root cwd and its commits land on the wrong branch (contamination).
    if agents.is_empty() {
        push(
            &mut commands,
            &[
                "split-window",
                "-v",
                "-t",
                &supervisor_target,
                "-l",
                &bottom_pct,
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
                "-l",
                &bottom_pct,
                "-c",
                &dashboard.worktree,
            ],
        );
    }

    // 5. Split pane 0 horizontally -> creates the top-right pane (currently
    //    index 2, swapped to index 1 below) at 50% width.
    // Same `-l <N>%` reasoning as step 4. Per the W3-1 swap-compensation note
    // above, this split (which lands at the agent's index-2 after the swap)
    // is born in the FIRST agent's worktree, so the agent's CLI — sent to
    // index 2 post-swap — runs in its own worktree, not the repo root.
    let dashboard_split_cwd = agents
        .first()
        .map_or(dashboard.worktree.as_str(), |a| a.worktree.as_str());
    push(
        &mut commands,
        &[
            "split-window",
            "-h",
            "-t",
            &supervisor_target,
            "-l",
            "50%",
            "-c",
            dashboard_split_cwd,
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
    push_pane_title(
        &mut commands,
        border_affordances,
        &dashboard_target,
        &dashboard.branch,
    );
    push(
        &mut commands,
        &["send-keys", "-t", &dashboard_target, "C-u"],
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
        push_pane_title(
            &mut commands,
            border_affordances,
            &first_target,
            &first.branch,
        );
        push(&mut commands, &["send-keys", "-t", &first_target, "C-u"]);
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

            push_pane_title(
                &mut commands,
                border_affordances,
                &pane_target,
                &agent.branch,
            );
            push(&mut commands, &["send-keys", "-t", &pane_target, "C-u"]);
            push(
                &mut commands,
                &["send-keys", "-t", &pane_target, &agent.cli_command, "Enter"],
            );
        }
    }

    // 9. Final pass: resize-pane to enforce the layout-table heights. One
    //    resize-pane per row (top + each agent row). Shared with the add /
    //    remove re-tile path via `push_supervisor_resize_pass` so an
    //    incrementally re-tiled grid matches a start-time grid of the same
    //    agent count. Percentages use `<pct>%` syntax which tmux 3.x accepts.
    push_supervisor_resize_pass(&mut commands, &session_name, layout, agents.len());

    Ok(TmuxSession {
        name: session_name,
        commands,
    })
}

/// Build the tmux commands that splice ONE new agent pane into a running
/// supervisor-mode session and re-tile the grid to `layout` (design D1, the
/// add path).
///
/// `prev_agent_count` is the number of coding agents already in the session
/// (N); the new agent becomes agent index N (0-based), landing at pane
/// `SUPERVISOR_PANE_OFFSET + N`. The split mirrors `build_supervisor_session`'s
/// grid logic:
///
/// - When the new agent starts a fresh row (`N % AGENTS_PER_ROW == 0`, N > 0),
///   `split-window -v` from the previous row's first pane.
/// - Otherwise `split-window -h` from the immediately preceding pane.
///
/// `select-layout` is intentionally avoided (as in `build_supervisor_session`)
/// so existing panes keep their indices for in-flight `send-keys` targeting;
/// the new pane gets the next index. A final `resize-pane` pass per row
/// enforces `layout`'s height proportions for the new total (N+1).
///
/// Returns a [`TmuxSession`] so the caller runs it with
/// [`TmuxSession::execute`] and tests inspect it with
/// [`TmuxSession::command_strings`]. The boot-prompt submit is the caller's
/// responsibility (it differs for active vs. paused sessions).
#[must_use]
pub fn build_add_agent_commands(
    session_name: &str,
    new_agent: &PaneSpec,
    prev_agent_count: usize,
    layout: crate::supervisor::layout::SupervisorLayout,
    border_affordances: bool,
) -> TmuxSession {
    use crate::supervisor::layout::{SUPERVISOR_AGENTS_PER_ROW, SUPERVISOR_PANE_OFFSET};

    let mut commands: Vec<TmuxCommand> = Vec::new();
    let i = prev_agent_count; // 0-based agent index of the new agent
    let pane_idx = SUPERVISOR_PANE_OFFSET + i;
    let pane_target = format!("{session_name}:0.{pane_idx}");

    if i > 0 && i.is_multiple_of(SUPERVISOR_AGENTS_PER_ROW) {
        // New row: vertical split from the previous row's first pane.
        let prev_row_first = SUPERVISOR_PANE_OFFSET + (i - SUPERVISOR_AGENTS_PER_ROW);
        let src = format!("{session_name}:0.{prev_row_first}");
        commands.push(TmuxCommand::new(&[
            "split-window",
            "-v",
            "-t",
            &src,
            "-c",
            &new_agent.worktree,
        ]));
    } else {
        // Same row: horizontal split from the immediately preceding pane.
        let prev = format!("{session_name}:0.{}", pane_idx - 1);
        commands.push(TmuxCommand::new(&[
            "split-window",
            "-h",
            "-t",
            &prev,
            "-c",
            &new_agent.worktree,
        ]));
    }

    push_pane_title(
        &mut commands,
        border_affordances,
        &pane_target,
        &new_agent.branch,
    );
    commands.push(TmuxCommand::new(&["send-keys", "-t", &pane_target, "C-u"]));
    commands.push(TmuxCommand::new(&[
        "send-keys",
        "-t",
        &pane_target,
        &new_agent.cli_command,
        "Enter",
    ]));

    push_supervisor_resize_pass(&mut commands, session_name, layout, prev_agent_count + 1);

    TmuxSession {
        name: session_name.to_string(),
        commands,
    }
}

/// Build the tmux commands that re-tile a supervisor-mode grid AFTER one
/// agent's pane has been killed (design D6, the remove path).
///
/// The caller kills the target pane first (via [`kill_pane`]); tmux then
/// renumbers the remaining panes to be contiguous, so each surviving row's
/// first pane is still addressable at `SUPERVISOR_PANE_OFFSET + row * AGENTS_PER_ROW`.
/// This emits the per-row `resize-pane` pass for `layout` (computed for the new,
/// smaller `remaining_agent_count`) so the grid re-flows to the proportions a
/// start of that many agents would produce, without leaving a hole.
///
/// Returns an empty command set when no agents remain (the supervisor +
/// dashboard top row is left as-is). Branch→pane mapping for the survivors is
/// re-derived by the supervisor via `pane_current_path` each sweep, so the
/// transient index shift is invisible to targeting.
#[must_use]
pub fn build_remove_retile_commands(
    session_name: &str,
    remaining_agent_count: usize,
    layout: crate::supervisor::layout::SupervisorLayout,
) -> TmuxSession {
    let mut commands: Vec<TmuxCommand> = Vec::new();
    if remaining_agent_count > 0 {
        push_supervisor_resize_pass(&mut commands, session_name, layout, remaining_agent_count);
    }
    TmuxSession {
        name: session_name.to_string(),
        commands,
    }
}

/// Push the per-row `resize-pane -y <pct>%` pass that enforces a supervisor
/// layout's height proportions: one resize for the top row (supervisor +
/// dashboard) and one per agent row (targeting each row's first pane). Shared
/// by the start-time builder's final pass and the add/remove re-tile builders.
///
/// The per-row equal-width rebalance is applied separately at runtime by
/// [`rebalance_agent_rows`] (design D4, G3): it needs the live window width to
/// resize each row to exact, equal columns, which a pure command builder
/// cannot know.
fn push_supervisor_resize_pass(
    commands: &mut Vec<TmuxCommand>,
    session_name: &str,
    layout: crate::supervisor::layout::SupervisorLayout,
    agent_count: usize,
) {
    use crate::supervisor::layout::{SUPERVISOR_AGENTS_PER_ROW, SUPERVISOR_PANE_OFFSET};

    let top_target = format!("{session_name}:0.0");
    let top_pct_str = format!("{}%", layout.top_row_pct);
    commands.push(TmuxCommand::new(&[
        "resize-pane",
        "-t",
        &top_target,
        "-y",
        &top_pct_str,
    ]));

    let agent_row_pct_str = format_supervisor_pct(layout.agent_row_pct);
    for row in 0..layout.agent_rows {
        let pane_idx = SUPERVISOR_PANE_OFFSET + row * SUPERVISOR_AGENTS_PER_ROW;
        if pane_idx < SUPERVISOR_PANE_OFFSET + agent_count {
            let target = format!("{session_name}:0.{pane_idx}");
            commands.push(TmuxCommand::new(&[
                "resize-pane",
                "-t",
                &target,
                "-y",
                &agent_row_pct_str,
            ]));
        }
    }
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
    use crate::command_runner::test_support::FakeCommandRunner;

    #[test]
    fn tmux_command_execute_sends_tmux_argv_and_returns_stdout() {
        let cmd = TmuxCommand::new(&["list-panes", "-t", "paw-x"]);
        let fake = FakeCommandRunner::succeeding("0\n1\n");
        let out = cmd.execute(&fake).expect("success returns stdout");
        assert_eq!(out, "0\n1\n");
        assert_eq!(
            fake.calls(),
            vec![(
                "tmux".to_string(),
                vec![
                    "list-panes".to_string(),
                    "-t".to_string(),
                    "paw-x".to_string()
                ]
            )],
            "execute must invoke `tmux` with the command's exact argv"
        );
    }

    #[test]
    fn tmux_command_execute_maps_failure_to_trimmed_stderr_error() {
        let cmd = TmuxCommand::new(&["has-session", "-t", "nope"]);
        let fake = FakeCommandRunner::failing("  no server running  ");
        match cmd.execute(&fake) {
            Err(PawError::TmuxError(msg)) => assert_eq!(msg, "no server running"),
            other => panic!("expected a trimmed TmuxError, got {other:?}"),
        }
    }

    #[test]
    fn tmux_session_execute_drives_each_command_through_the_runner_verbatim() {
        // The execute path must send exactly the argv that `command_strings`
        // (the dry-run surface) reports — the seam proves render == send.
        let session = TmuxSessionBuilder::new("proj")
            .border_affordances(false)
            .mouse_mode(false)
            .add_pane(PaneSpec {
                branch: "feat/x".into(),
                worktree: "/tmp/proj-feat-x".into(),
                cli_command: "claude".into(),
            })
            .build()
            .expect("build session");
        let fake = FakeCommandRunner::succeeding("");
        session
            .execute_with(|cmd| cmd.execute(&fake).map(|_| ()), |_| {})
            .expect("execute session");
        let sent: Vec<String> = fake
            .calls()
            .into_iter()
            .map(|(prog, args)| format!("{prog} {}", args.join(" ")))
            .collect();
        assert_eq!(sent, session.command_strings());
    }
}
