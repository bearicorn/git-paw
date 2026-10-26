//! git-paw — Parallel AI Worktrees.
//!
//! Orchestrates multiple AI coding CLI sessions across git worktrees
//! from a single terminal using tmux.

// `core-project-conventions`: no `unwrap()`/`expect()` in non-test code.
// `clippy.toml` exempts `#[cfg(test)]` code; `just lint`'s `-D warnings`
// turns a violation into a hard failure.
#![warn(clippy::unwrap_used, clippy::expect_used)]

use std::io::IsTerminal;
use std::path::Path;

use clap::Parser;
use dialoguer::Confirm;
use git_paw::cli_setup::facts::SetupItem;
use git_paw::cli_setup::render::{render_plan, render_undo_plan, render_unsupported};

use git_paw::broker;
#[cfg(test)]
use git_paw::broker::publish::build_status_message;
use git_paw::cli::{Cli, Command};
use git_paw::commands::launch::LaunchOutcome;
use git_paw::commands::spec_mode::SpecMode;
use git_paw::commands::supervisor::{SupervisorPrompt, resolve_supervisor_mode};
use git_paw::commands::{self, CommandContext, Output};
use git_paw::config::{self, PawConfig};
use git_paw::error::PawError;
use git_paw::git;
use git_paw::interactive;
use git_paw::session::{self, Session, SessionStatus};
use git_paw::supervisor::loop_binding::BindMode;
use git_paw::tmux;

fn main() {
    let args = Cli::parse();

    let command = args.command.unwrap_or(Command::Start {
        cli: None,
        branches: None,
        from_all_specs: false,
        specs: None,
        specs_format: None,
        dry_run: false,
        preset: None,
        supervisor: false,
        no_supervisor: false,
        force: false,
        no_rebase: false,
        unattended: false,
    });

    if let Err(err) = run(command) {
        err.exit();
    }
}

/// Dispatch and execute the given CLI command.
// Length is inherent to a total dispatch match — it grows by one arm per
// subcommand, with no logic to extract.
#[allow(clippy::too_many_lines)]
fn run(command: Command) -> Result<(), PawError> {
    match command {
        Command::Start {
            cli: cli_flag,
            branches: branches_flag,
            from_all_specs,
            specs,
            specs_format,
            dry_run,
            preset,
            supervisor,
            no_supervisor,
            force,
            no_rebase,
            unattended,
        } => {
            // Resolve supervisor-mode-enabled state BEFORE dispatching so the
            // user's --supervisor flag (or [supervisor] config) is honoured
            // when combined with --from-all-specs (or its hidden alias).
            // `--no-supervisor` (added in no-supervisor-flag) wins over both
            // the flag and any [supervisor] config setting.
            //
            // `--unattended` engages supervisor mode (per the cli-parsing
            // delta): it is treated as an implicit `--supervisor` so the
            // dispatch routes to the supervisor launch path, where step 15
            // branches on the flag to drive the in-process loop. clap already
            // rejects `--unattended --no-supervisor`.
            let supervisor_enabled =
                resolve_supervisor_mode_from_cwd(no_supervisor, supervisor || unattended, dry_run)?;
            let spec_mode = SpecMode::from_flags(from_all_specs, specs.as_deref());
            let specs_format = specs_format.map(|f| f.as_str().to_string());
            let prompter = &interactive::TerminalPrompter;
            let out = &mut CliOutput;
            let outcome = match resolve_dispatch_target(&spec_mode, supervisor_enabled) {
                DispatchTarget::Supervisor { spec_mode } => {
                    let cwd = std::env::current_dir().map_err(|e| {
                        PawError::SessionError(format!("cannot read current directory: {e}"))
                    })?;
                    let repo_root = git::validate_repo(&cwd)?;
                    let config = config::load_config(&repo_root, None)?;
                    commands::supervisor::supervisor(
                        &CommandContext::with_config(repo_root, config),
                        commands::supervisor::SupervisorOptions {
                            cli: cli_flag,
                            branches: branches_flag,
                            spec_mode,
                            specs_format,
                            dry_run,
                            no_rebase,
                            unattended,
                            prompter,
                            interactive: is_interactive_stdin(),
                        },
                        out,
                    )?
                }
                DispatchTarget::StartWithSpecs(spec_mode) => commands::start::start_with_specs(
                    &repo_context()?,
                    commands::start::StartWithSpecsOptions {
                        cli: cli_flag,
                        spec_mode,
                        specs_format,
                        dry_run,
                        force,
                        no_rebase,
                        prompter,
                        interactive: is_interactive_stdin(),
                    },
                    out,
                )?,
                DispatchTarget::Start => commands::start::start(
                    &repo_context()?,
                    commands::start::StartOptions {
                        cli: cli_flag,
                        branches: branches_flag,
                        dry_run,
                        preset,
                        no_supervisor,
                        no_rebase,
                        prompter,
                    },
                    out,
                )?,
            };
            finish_launch(outcome)
        }
        Command::Add {
            branch,
            cli,
            from_spec,
        } => {
            let opts = commands::add::AddOptions {
                branch,
                cli,
                from_spec,
            };
            commands::add::add(&repo_context()?, opts, &mut CliOutput)?;
            Ok(())
        }
        Command::Remove {
            branch,
            keep_worktree,
            force,
        } => {
            let opts = commands::remove::RemoveOptions {
                branch,
                keep_worktree,
                force,
            };
            commands::remove::remove(&repo_context()?, opts, &mut CliOutput)?;
            Ok(())
        }
        Command::Pause => {
            commands::pause::pause(
                &repo_context()?,
                commands::pause::PauseOptions::default(),
                &mut CliOutput,
            )?;
            Ok(())
        }
        Command::Stop => {
            commands::stop::stop(
                &repo_context()?,
                commands::stop::StopOptions::default(),
                &mut CliOutput,
            )?;
            Ok(())
        }
        Command::Purge {
            force,
            stale,
            all_repos,
        } => purge(force, stale, all_repos),
        Command::Status { json } => {
            let opts = commands::status::StatusOptions { json };
            commands::status::status(&repo_context()?, opts, &mut CliOutput)?;
            Ok(())
        }
        Command::Attach => {
            let outcome = commands::attach::attach(
                &repo_context()?,
                commands::attach::AttachOptions::default(),
                &mut CliOutput,
            )?;
            tmux::attach(&outcome.session_name)
        }
        Command::ListClis => {
            commands::clis::list_clis(&repo_context()?, &mut CliOutput)?;
            Ok(())
        }
        Command::AddCli {
            name,
            command,
            display_name,
        } => {
            let opts = commands::clis::AddCliOptions {
                name,
                command,
                display_name,
            };
            commands::clis::add_cli(&opts, &mut CliOutput)
        }
        Command::RemoveCli { name } => {
            // Outside a repository there is no config to consult: the removal
            // is attempted directly.
            let cwd = std::env::current_dir().map_err(|e| {
                PawError::SessionError(format!("cannot read current directory: {e}"))
            })?;
            let repo = git::validate_repo(&cwd).ok().map(CommandContext::new);
            commands::clis::remove_cli(repo.as_ref(), &name, &mut CliOutput)
        }
        Command::SetupCli {
            cli,
            allowlist,
            trust,
            hooks,
            dry_run,
            yes,
            undo,
        } => cmd_setup_cli(&SetupCliArgs {
            cli: &cli,
            allowlist,
            trust,
            hooks,
            dry_run,
            yes,
            undo,
        }),
        Command::Dashboard => cmd_dashboard(),
        Command::Broker { session, log_path } => {
            commands::broker_host::cmd_broker_host(&session, &log_path)
        }
        Command::PromptProfile { cli, field } => cmd_prompt_profile(cli.as_deref(), &field),
        Command::Init => git_paw::init::run_init(),
        Command::Replay {
            branch,
            list,
            color,
            session,
        } => {
            let opts = commands::replay::ReplayOptions {
                branch,
                list,
                color,
                session,
            };
            match commands::replay::replay(&repo_context()?, opts, &mut CliOutput)? {
                commands::replay::ReplayOutcome::Listed => Ok(()),
                commands::replay::ReplayOutcome::Log { path, color: true } => {
                    git_paw::replay::replay_colored(&path)
                }
                commands::replay::ReplayOutcome::Log { path, color: false } => {
                    git_paw::replay::replay_stripped(&path)
                }
            }
        }
        Command::Approvals {
            session,
            limit,
            json,
        } => {
            let opts = commands::approvals::ApprovalsOptions {
                session,
                limit,
                json,
            };
            commands::approvals::approvals(&repo_context()?, opts, &mut CliOutput)?;
            Ok(())
        }
        Command::Mcp { repo, log_file } => {
            git_paw::mcp::cmd_mcp(repo.as_deref(), log_file.as_deref())
        }
        Command::Doctor { json, live } => git_paw::doctor::run(json, live),
        Command::Completions { shell } => commands::completions::cmd_completions(shell),
        Command::Selftest => git_paw::selftest::run(),
    }
}

// ---------------------------------------------------------------------------
// Dispatch routing
// ---------------------------------------------------------------------------

/// The terminal adapter for a command's output: [`Output::line`] goes to
/// stdout and [`Output::warn`] to stderr, each followed by a newline — exactly
/// what the commands printed before they wrote through a sink.
struct CliOutput;

impl Output for CliOutput {
    fn line(&mut self, text: &str) {
        println!("{text}");
    }

    fn warn(&mut self, text: &str) {
        eprintln!("{text}");
    }
}

/// Resolves the repository the command runs against from the current working
/// directory, as a [`CommandContext`] whose config loads on first use.
fn repo_context() -> Result<CommandContext, PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    Ok(CommandContext::new(git::validate_repo(&cwd)?))
}

/// Loads the repo config from the current working directory and resolves
/// whether supervisor mode should be entered for this session, asking the
/// "Start in supervisor mode?" question on the terminal when the chain falls
/// through to it.
fn resolve_supervisor_mode_from_cwd(
    no_supervisor_flag: bool,
    supervisor_flag: bool,
    dry_run: bool,
) -> Result<bool, PawError> {
    if no_supervisor_flag {
        return Ok(false);
    }
    if supervisor_flag {
        return Ok(true);
    }
    // Config lookup may fail outside a git repo; if we can't find a repo or
    // config, fall back to the default (no supervisor) and let downstream
    // commands produce the real error.
    let Ok(cwd) = std::env::current_dir() else {
        return Ok(false);
    };
    let Ok(repo_root) = git::validate_repo(&cwd) else {
        return Ok(false);
    };
    let config = config::load_config(&repo_root, None).unwrap_or_default();
    resolve_supervisor_mode(
        no_supervisor_flag,
        supervisor_flag,
        dry_run,
        &config,
        &mut TerminalSupervisorPrompt,
    )
}

/// The terminal adapter for the "Start in supervisor mode?" question: a
/// non-interactive stdin answers no without asking.
struct TerminalSupervisorPrompt;

impl SupervisorPrompt for TerminalSupervisorPrompt {
    fn ask(&mut self) -> Result<bool, PawError> {
        if !std::io::stdin().is_terminal() {
            return Ok(false);
        }
        Confirm::new()
            .with_prompt("Start in supervisor mode?")
            .default(false)
            .interact()
            .map_err(|e| PawError::SessionError(format!("supervisor prompt failed: {e}")))
    }
}

/// Concrete dispatch target for the `start` subcommand. Extracted as a pure
/// function (`resolve_dispatch_target`) so the routing decision is unit-testable
/// independently of any IO.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DispatchTarget {
    /// Route to `supervisor`, carrying the resolved spec selection mode.
    ///
    /// The mode is passed through verbatim (not collapsed to a boolean) so the
    /// supervisor launch path applies the same subset filtering as the
    /// non-supervisor `--specs` path: `Narrow(names)` launches only the named
    /// subset, `Picker` opens the multi-select picker, `All` launches every
    /// discovered spec, and `None` falls through to the `--branches` /
    /// branch-picker flow. Fixes the v0.6.0 dogfood bug where
    /// `--supervisor --specs a,b` launched every discovered spec.
    Supervisor { spec_mode: SpecMode },
    /// Route to `start_with_specs` with the resolved spec mode (one of
    /// `All`, `Picker`, `Narrow`).
    StartWithSpecs(SpecMode),
    /// Route to bare `start`.
    Start,
}

/// Pure routing function for `start` subcommand dispatch.
///
/// When supervisor mode is enabled, every spec mode routes to the supervisor
/// path carrying the resolved `SpecMode` so subset filtering (`Narrow`),
/// the interactive `Picker`, `All`, and the bare branch-picker (`None`) cases
/// behave identically to the non-supervisor path — just with supervisor mode
/// engaged. Without supervisor mode, a populated spec mode routes through
/// `start_with_specs` and `SpecMode::None` falls through to bare `start`.
fn resolve_dispatch_target(spec_mode: &SpecMode, supervisor_enabled: bool) -> DispatchTarget {
    match (supervisor_enabled, spec_mode) {
        (true, mode) => DispatchTarget::Supervisor {
            spec_mode: mode.clone(),
        },
        (false, SpecMode::All | SpecMode::Picker | SpecMode::Narrow(_)) => {
            DispatchTarget::StartWithSpecs(spec_mode.clone())
        }
        (false, SpecMode::None) => DispatchTarget::Start,
    }
}

// ---------------------------------------------------------------------------
// Non-TTY launch handling
// ---------------------------------------------------------------------------

/// Returns `true` when stdin is connected to an interactive terminal.
///
/// Used to gate `tmux::attach` and the foreground supervisor-CLI launch so
/// non-TTY invocations (CI, scripts, harness tools) exit cleanly with an
/// attach hint instead of "failed to attach" errors.
/// Fixes dogfood D2 in `from-specs-launch-fixes`.
fn is_interactive_stdin() -> bool {
    std::io::stdin().is_terminal()
}

/// Calls `tmux::attach` when stdin is interactive; otherwise prints an
/// attach-hint to stdout and returns `Ok(())` so the launch flow exits
/// cleanly without making the session look broken.
fn attach_or_print_hint(session_name: &str) -> Result<(), PawError> {
    if is_interactive_stdin() {
        tmux::attach(session_name)
    } else {
        println!("Session '{session_name}' started in detached mode.");
        println!("Attach with:  tmux attach -t {session_name}");
        Ok(())
    }
}

/// `git paw purge`: with `--stale`, sweeps stale receipts (this repository's,
/// or every repository's with `--all-repos`); otherwise previews the purge,
/// asks for confirmation unless `--force`, and purges on the answer.
/// `--force` is redundant with `--stale` (a stale entry is never prompted for).
fn purge(force: bool, stale: bool, all_repos: bool) -> Result<(), PawError> {
    let out = &mut CliOutput;
    if stale {
        let repo_root = if all_repos {
            None
        } else {
            Some(repo_context()?.repo_root().to_path_buf())
        };
        commands::purge::purge_stale(commands::purge::PurgeStaleOptions { repo_root }, out)?;
        return Ok(());
    }
    let ctx = repo_context()?;
    let Some(preview) = commands::purge::preview(&ctx, out)? else {
        return Ok(());
    };
    let confirmed = force
        || Confirm::new()
            .with_prompt(&preview.prompt)
            .default(false)
            .interact()
            .map_err(|_| PawError::UserCancelled)?;
    commands::purge::purge(
        &ctx,
        commands::purge::PurgeOptions { force, confirmed },
        out,
    )?;
    Ok(())
}

/// Performs what a launch left for the terminal (`LaunchOutcome`): attach or
/// print the hint, reattach or refuse, or take the drive-loop binding and run
/// the unattended loop.
fn finish_launch(outcome: LaunchOutcome) -> Result<(), PawError> {
    match outcome {
        LaunchOutcome::Done => Ok(()),
        LaunchOutcome::Attach { session_name } => attach_or_print_hint(&session_name),
        LaunchOutcome::AlreadyRunning { session_name } => {
            reattach_active_session_or_refuse(&session_name)
        }
        LaunchOutcome::DriveFresh {
            repo_root,
            config,
            session,
        } => drive_fresh(&repo_root, &config, &session),
        LaunchOutcome::DriveExisting {
            repo_root,
            config,
            existing,
            status,
        } => drive_existing(&repo_root, &config, &existing, &status),
    }
}

/// The repository's `[supervisor]` table, or its defaults when absent.
fn supervisor_config(config: &PawConfig) -> config::SupervisorConfig {
    config.supervisor.clone().unwrap_or_default()
}

/// Runs the unattended drive loop on a session `--unattended` just launched.
///
/// The binding is taken after the session is saved (so the instance token is
/// the one on disk) and before the loop runs. A fresh launch just created
/// this instance, so nothing may already be driving it: `Fresh` refuses
/// rather than displacing a holder.
fn drive_fresh(repo_root: &Path, config: &PawConfig, state: &Session) -> Result<(), PawError> {
    let created_at = commands::supervisor::recorded_created_at(repo_root, state.created_at);
    let binding = commands::supervisor::bind_drive_loop(
        repo_root,
        &state.session_name,
        created_at,
        BindMode::Fresh,
    )?;
    run_drive_loop(commands::supervisor::unattended_drive_run(
        repo_root,
        &supervisor_config(config),
        &config.broker,
        state,
        created_at,
        binding,
    ))
}

/// Binds a new drive loop to the repository's existing supervised session and
/// runs it (`git paw start --unattended` against a live session).
///
/// The order is the whole safety argument: the bare-session refusal and the
/// binding both come BEFORE any revive or recovery, so a refused caller leaves
/// every session, pane and worktree exactly as it found them. A failed revive
/// drops the binding on the way out, releasing it.
fn drive_existing(
    repo_root: &Path,
    config: &PawConfig,
    existing: &Session,
    effective: &SessionStatus,
) -> Result<(), PawError> {
    commands::supervisor::ensure_drivable(existing)?;
    let created_at = commands::supervisor::recorded_created_at(repo_root, existing.created_at);
    let binding = commands::supervisor::bind_drive_loop(
        repo_root,
        &existing.session_name,
        created_at,
        commands::supervisor::reattach_bind_mode(effective),
    )?;
    let (state, broker_config) = commands::supervisor::revive_for_drive(
        repo_root,
        config,
        existing,
        effective,
        &mut CliOutput,
    )?;
    run_drive_loop(commands::supervisor::unattended_drive_run(
        repo_root,
        &supervisor_config(config),
        &broker_config,
        &state,
        created_at,
        binding,
    ))
}

/// Drives `run` to an exit condition in the foreground, printing its summary.
fn run_drive_loop(run: commands::supervisor::UnattendedRun) -> Result<(), PawError> {
    git_paw::supervisor::drive::run_drive_loop(
        &run.session_name,
        &run.repo_root,
        &run.agents,
        run.options,
    )?;
    Ok(())
}

/// GP-13 reattach guard: called when a live session already exists for the
/// repository, right before a fresh launch would otherwise reach
/// `resolve_session_name` and fork a `-N`-suffixed parallel one.
///
/// Interactively, attaches to the existing session (identical to
/// [`attach_or_print_hint`]'s interactive branch). Non-interactively there is
/// no terminal to attach to, and silently printing a hint and exiting `Ok`
/// would let an automated caller (a script, the drive loop, a scanner
/// advancing a phase) believe a fresh launch happened when it did not — so
/// this refuses with an actionable error instead.
fn reattach_active_session_or_refuse(session_name: &str) -> Result<(), PawError> {
    if !is_interactive_stdin() {
        return Err(PawError::SessionError(format!(
            "session '{session_name}' is already running for this repository. Re-running \
             `git paw start` non-interactively would otherwise fork a second, parallel session. \
             Attach from an interactive terminal, or run: tmux attach -t {session_name}"
        )));
    }
    println!("Reattaching to session '{session_name}'...");
    tmux::attach(session_name)
}

// ---------------------------------------------------------------------------
// Command: __prompt-profile
// ---------------------------------------------------------------------------

/// Regex-escapes `marker` so it can be joined into an extended-regex
/// alternation without its literal characters (parentheses, brackets, …)
/// being read as regex syntax.
fn escape_for_regex_alternation(marker: &str) -> String {
    let mut escaped = String::with_capacity(marker.len());
    for c in marker.chars() {
        if ".^$*+?()[]{}|\\".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// Renders `broad_grant_marker` as a `grep -E` pattern that matches the
/// marker whether the terminal drew an ASCII apostrophe or the Unicode right
/// single quote.
///
/// Claude's marker is `don't ask again`, and a terminal may render that
/// apostrophe either way, so a literal match misses half the time. The
/// tolerance belongs here rather than in `sweep.sh` for two reasons: the
/// shell helper must stay free of apostrophes (they desync quoting inside its
/// `$(cat <<'PY' …)` heredocs), and relaxing the ESCAPED form in shell is
/// subtly wrong — a marker containing a regex metacharacter escapes to `\(`,
/// and relaxing both characters would then demand two characters where the
/// pane renders one, making the guard MISS. Escaping after the substitution
/// keeps every other character literal.
fn broad_grant_marker_regex(marker: &str) -> String {
    let mut out = String::with_capacity(marker.len() + 8);
    for c in marker.chars() {
        if c == '\'' || c == '\u{2019}' {
            // A character class of both quote forms; neither needs escaping
            // inside brackets.
            out.push_str("['\u{2019}]");
        } else if ".^$*+?()[]{}|\\".contains(c) {
            out.push('\\');
            out.push(c);
        } else {
            out.push(c);
        }
    }
    out
}

/// Prints one resolved [`config::CliPromptProfile`] field for the bundled
/// `sweep.sh` helper (internal command): a marker-list field prints as a
/// regex-escaped `|`-joined alternation (ready for `grep -E`); a pattern
/// field prints its raw regex source.
///
/// Resolves the profile for `cli` (falling back to `[supervisor].cli`, then
/// `default_cli`, matching how the boot-prompt flags resolve the session
/// CLI) against the config loaded from the CURRENT WORKING DIRECTORY — MUST
/// be invoked from inside the target repository, as `sweep.sh` does.
///
/// `cli-prompt-profiles` capability: this is the single source `sweep.sh`
/// reads instead of carrying its own copies of the markers/regexes.
fn cmd_prompt_profile(cli: Option<&str>, field: &str) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;
    let config = config::load_config(&repo_root, None).unwrap_or_default();
    let resolved_cli = cli
        .map(str::to_string)
        .or_else(|| config.supervisor.as_ref().and_then(|s| s.cli.clone()))
        .or_else(|| config.default_cli.clone())
        .unwrap_or_default();
    let profile = config::resolve_prompt_profile(&resolved_cli, &config.clis);

    let marker_alternation = |markers: &[String]| {
        markers
            .iter()
            .map(|m| escape_for_regex_alternation(m))
            .collect::<Vec<_>>()
            .join("|")
    };

    let value = match field {
        "readiness_markers" => marker_alternation(&profile.readiness_markers),
        "approval_markers" => marker_alternation(&profile.approval_markers),
        "live_prompt_markers" => marker_alternation(&profile.live_prompt_markers),
        "mid_response_markers" => marker_alternation(&profile.mid_response_markers),
        "prompt_boilerplate_markers" => marker_alternation(&profile.prompt_boilerplate_markers),
        "command_header_markers" => marker_alternation(&profile.command_header_markers),
        "mode_accept_edits_markers" => marker_alternation(&profile.mode_accept_edits_markers),
        "mode_interactive_markers" => marker_alternation(&profile.mode_interactive_markers),
        "stream_error_markers" => marker_alternation(&profile.stream_error_markers),
        "input_sigils" => marker_alternation(&profile.input_sigils),
        "file_prompt_pattern" => profile.file_prompt_pattern.clone(),
        "option_line_pattern" => profile.option_line_pattern.clone(),
        "broad_grant_marker" => broad_grant_marker_regex(&profile.broad_grant_marker),
        "context_bloat_pattern" => profile.context_bloat_pattern.clone(),
        "paste_buffer_pattern" => profile.paste_buffer_pattern.clone(),
        "cli_unavailable_markers" => marker_alternation(&profile.cli_unavailable_markers),
        other => {
            return Err(PawError::SessionError(format!(
                "unknown prompt-profile field '{other}'"
            )));
        }
    };
    println!("{value}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Command: __dashboard
// ---------------------------------------------------------------------------

/// Renders the dashboard in pane 0 as a thin client of the session's broker
/// (internal command).
///
/// Holds no broker state and needs no `$TMUX`. The broker URL comes from the
/// saved session's `broker_port` / `broker_bind` whatever its recorded status
/// — restart from pause recreates this pane while the session is still
/// `Paused` — and is re-resolved on every wake-up until a receipt exists,
/// since a fresh launch saves it only after the panes are up.
fn cmd_dashboard() -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;
    // Broker log panel: ring-buffer cap + initial visibility from
    // `[dashboard.broker_log]`. A config that will not load uses the
    // documented defaults (cap 500, visible) rather than leaving the pane
    // blank.
    let broker_log_cfg = config::load_config(&repo_root, None)
        .ok()
        .and_then(|c| c.dashboard.map(|d| d.broker_log))
        .unwrap_or_default();

    let mut resolve = || match session::find_session_for_repo(&repo_root).ok().flatten() {
        Some(sess) => dashboard_broker_url(&sess)
            .map_or(git_paw::dashboard::BrokerSource::Disabled, |url| {
                git_paw::dashboard::BrokerSource::Ready(broker::client::BrokerClient::new(url))
            }),
        None => git_paw::dashboard::BrokerSource::Pending,
    };
    let shutdown = std::sync::atomic::AtomicBool::new(false);
    git_paw::dashboard::run_dashboard_with_panes(
        &mut resolve,
        &shutdown,
        broker_log_cfg.max_messages,
        broker_log_cfg.default_visible,
        broker_log_cfg.height_lines,
    )
}

/// The broker URL the dashboard of `session` polls: from its recorded
/// `broker_port` / `broker_bind`, whatever the session's status. `None` for a
/// session saved without a broker.
fn dashboard_broker_url(session: &Session) -> Option<String> {
    session
        .broker_port
        .zip(session.broker_bind.as_ref())
        .map(|(port, bind)| format!("http://{bind}:{port}"))
}

// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use git_paw::commands::supervisor::{
        GIT_PAW_ISSUES_URL, SupervisorPrompt, learnings_disclosure_notice, resolve_supervisor_mode,
    };
    use git_paw::config::{PawConfig, SupervisorConfig};

    // -----------------------------------------------------------------------
    // resolve_dispatch_target — pure routing for the `start` subcommand
    // -----------------------------------------------------------------------

    #[test]
    fn dispatch_from_all_specs_with_supervisor_routes_to_supervisor_with_all() {
        let target = resolve_dispatch_target(&SpecMode::All, true);
        assert_eq!(
            target,
            DispatchTarget::Supervisor {
                spec_mode: SpecMode::All
            }
        );
    }

    #[test]
    fn dispatch_from_all_specs_without_supervisor_routes_to_start_with_specs() {
        let target = resolve_dispatch_target(&SpecMode::All, false);
        assert_eq!(target, DispatchTarget::StartWithSpecs(SpecMode::All));
    }

    #[test]
    fn dispatch_supervisor_without_specs_routes_to_supervisor_with_none() {
        // Bare `--supervisor` (no spec flag) carries `SpecMode::None`, which
        // supervisor resolves to the `--branches` / branch-picker flow —
        // NOT spec auto-discovery.
        let target = resolve_dispatch_target(&SpecMode::None, true);
        assert_eq!(
            target,
            DispatchTarget::Supervisor {
                spec_mode: SpecMode::None
            }
        );
    }

    #[test]
    fn dispatch_neither_flag_routes_to_start() {
        let target = resolve_dispatch_target(&SpecMode::None, false);
        assert_eq!(target, DispatchTarget::Start);
    }

    #[test]
    fn dispatch_picker_routes_to_start_with_specs_picker() {
        let target = resolve_dispatch_target(&SpecMode::Picker, false);
        assert_eq!(target, DispatchTarget::StartWithSpecs(SpecMode::Picker));
    }

    #[test]
    fn dispatch_narrow_routes_to_start_with_specs_narrow() {
        let names = vec!["add-auth".to_string()];
        let target = resolve_dispatch_target(&SpecMode::Narrow(names.clone()), false);
        assert_eq!(
            target,
            DispatchTarget::StartWithSpecs(SpecMode::Narrow(names))
        );
    }

    // Task 1.4 — `--supervisor --specs a,b` routes to the supervisor path
    // carrying the named subset, NOT `use_specs`-collapses-to-all.
    #[test]
    fn dispatch_narrow_with_supervisor_routes_to_supervisor_with_named_subset() {
        let names = vec!["a".to_string(), "b".to_string()];
        let target = resolve_dispatch_target(&SpecMode::Narrow(names.clone()), true);
        assert_eq!(
            target,
            DispatchTarget::Supervisor {
                spec_mode: SpecMode::Narrow(names)
            }
        );
    }

    // Task 1.5 — `--supervisor --from-all-specs` routes to the supervisor path
    // carrying `All` (every discovered spec).
    #[test]
    fn dispatch_picker_with_supervisor_routes_to_supervisor_with_picker() {
        let target = resolve_dispatch_target(&SpecMode::Picker, true);
        assert_eq!(
            target,
            DispatchTarget::Supervisor {
                spec_mode: SpecMode::Picker
            }
        );
    }

    // Task 2.3 — `--unattended --branches feat/a,feat/b` routes to the
    // supervisor launch path. `--unattended` engages supervisor mode via the
    // `supervisor || unattended` coupling `run()` applies before resolving the
    // dispatch target, so the wave routes to supervisor even with
    // `--supervisor` absent and `[supervisor]` config disabled.
    #[test]
    fn dispatch_unattended_routes_to_supervisor_launch_path() {
        let cfg = cfg_with_supervisor(false);
        let mut prompt = StubPrompt {
            answer: false,
            called: false,
        };
        let (supervisor_flag, unattended) = (false, true);
        // `run()` passes `supervisor || unattended` as the supervisor flag.
        let supervisor_enabled = resolve_supervisor_mode(
            false,
            supervisor_flag || unattended,
            false,
            &cfg,
            &mut prompt,
        )
        .unwrap();
        assert!(
            supervisor_enabled,
            "--unattended resolves supervisor mode active"
        );
        assert!(!prompt.called, "the flag short-circuits before any prompt");
        // With `--branches` the spec mode is None; the target is the supervisor
        // launch path carrying that mode.
        let target = resolve_dispatch_target(&SpecMode::None, supervisor_enabled);
        assert_eq!(
            target,
            DispatchTarget::Supervisor {
                spec_mode: SpecMode::None
            }
        );
    }

    struct StubPrompt {
        answer: bool,
        called: bool,
    }

    impl SupervisorPrompt for StubPrompt {
        fn ask(&mut self) -> Result<bool, PawError> {
            self.called = true;
            Ok(self.answer)
        }
    }

    fn cfg_with_supervisor(enabled: bool) -> PawConfig {
        PawConfig {
            supervisor: Some(SupervisorConfig {
                enabled,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    // --- resolve_supervisor_mode ---

    #[test]
    fn resolve_flag_wins_over_disabled_config() {
        let cfg = cfg_with_supervisor(false);
        let mut prompt = StubPrompt {
            answer: false,
            called: false,
        };
        let result = resolve_supervisor_mode(false, true, false, &cfg, &mut prompt).unwrap();
        assert!(result);
        assert!(!prompt.called);
    }

    #[test]
    fn resolve_config_enabled_skips_prompt() {
        let cfg = cfg_with_supervisor(true);
        let mut prompt = StubPrompt {
            answer: false,
            called: false,
        };
        let result = resolve_supervisor_mode(false, false, false, &cfg, &mut prompt).unwrap();
        assert!(result);
        assert!(!prompt.called);
    }

    #[test]
    fn resolve_config_disabled_skips_prompt() {
        let cfg = cfg_with_supervisor(false);
        let mut prompt = StubPrompt {
            answer: true,
            called: false,
        };
        let result = resolve_supervisor_mode(false, false, false, &cfg, &mut prompt).unwrap();
        assert!(!result);
        assert!(!prompt.called);
    }

    #[test]
    fn resolve_dry_run_no_section_skips_prompt() {
        let cfg = PawConfig::default();
        let mut prompt = StubPrompt {
            answer: true,
            called: false,
        };
        let result = resolve_supervisor_mode(false, false, true, &cfg, &mut prompt).unwrap();
        assert!(!result);
        assert!(!prompt.called);
    }

    #[test]
    fn resolve_no_section_prompts_and_returns_answer() {
        let cfg = PawConfig::default();
        let mut prompt = StubPrompt {
            answer: true,
            called: false,
        };
        let result = resolve_supervisor_mode(false, false, false, &cfg, &mut prompt).unwrap();
        assert!(result);
        assert!(prompt.called);
    }

    #[test]
    fn resolve_dry_run_plus_flag_still_enables() {
        let cfg = PawConfig::default();
        let mut prompt = StubPrompt {
            answer: false,
            called: false,
        };
        let result = resolve_supervisor_mode(false, true, true, &cfg, &mut prompt).unwrap();
        assert!(result);
    }

    // --- resolve_supervisor_mode: --no-supervisor short-circuit (Step 0) ---

    #[test]
    fn resolve_no_supervisor_overrides_config_enabled() {
        let cfg = cfg_with_supervisor(true);
        let mut prompt = StubPrompt {
            answer: true,
            called: false,
        };
        // no_supervisor_flag = true, supervisor_flag = false (mutex enforced by clap)
        let result = resolve_supervisor_mode(true, false, false, &cfg, &mut prompt).unwrap();
        assert!(
            !result,
            "--no-supervisor must override config enabled = true"
        );
        assert!(!prompt.called);
    }

    #[test]
    fn resolve_no_supervisor_with_config_disabled_is_idempotent() {
        let cfg = cfg_with_supervisor(false);
        let mut prompt = StubPrompt {
            answer: true,
            called: false,
        };
        let result = resolve_supervisor_mode(true, false, false, &cfg, &mut prompt).unwrap();
        assert!(!result);
        assert!(!prompt.called);
    }

    #[test]
    fn resolve_no_supervisor_with_no_section_skips_prompt() {
        // No [supervisor] section + --no-supervisor → off, no prompt regardless
        // of TTY (StubPrompt would have returned true if asked).
        let cfg = PawConfig::default();
        let mut prompt = StubPrompt {
            answer: true,
            called: false,
        };
        let result = resolve_supervisor_mode(true, false, false, &cfg, &mut prompt).unwrap();
        assert!(!result);
        assert!(
            !prompt.called,
            "--no-supervisor must short-circuit before the prompt"
        );
    }

    #[test]
    fn resolve_no_supervisor_with_dry_run_disables() {
        let cfg = cfg_with_supervisor(true);
        let mut prompt = StubPrompt {
            answer: true,
            called: false,
        };
        let result = resolve_supervisor_mode(true, false, true, &cfg, &mut prompt).unwrap();
        assert!(!result);
        assert!(!prompt.called);
    }

    // (merge-ordering tests previously here have been removed alongside
    // src/merge_loop.rs — supervisor-as-pane moves merge orchestration into
    // the supervisor skill as a v0.5.0 design change.)

    // -----------------------------------------------------------------------
    // learnings_disclosure_notice — session-start privacy disclosure gating
    // (spec: learnings-mode "Session-start learnings disclosure notice")
    // -----------------------------------------------------------------------

    #[test]
    fn disclosure_notice_prints_when_learnings_enabled() {
        // GIVEN [supervisor] enabled = true AND learnings = true.
        let cfg = SupervisorConfig {
            enabled: true,
            learnings: true,
            ..SupervisorConfig::default()
        };
        let notice =
            learnings_disclosure_notice(Some(&cfg)).expect("notice must print when opted in");
        // THEN it names the local path, the no-telemetry stance, and the
        // optional share-via-issue invitation with the review/anonymise caveat.
        assert!(
            notice.contains(".git-paw/session-learnings.md"),
            "notice must name the local path; got: {notice}"
        );
        assert!(
            notice.contains("no telemetry") && notice.contains("nothing is sent anywhere"),
            "notice must state the no-telemetry stance; got: {notice}"
        );
        assert!(
            notice.contains(GIT_PAW_ISSUES_URL),
            "notice must invite sharing via the canonical issues URL; got: {notice}"
        );
        assert!(
            notice.contains("anonymise"),
            "notice must carry the review/anonymise caveat; got: {notice}"
        );
    }

    #[test]
    fn no_disclosure_notice_when_learnings_disabled() {
        // GIVEN [supervisor] enabled = true but learnings = false.
        let cfg = SupervisorConfig {
            enabled: true,
            learnings: false,
            ..SupervisorConfig::default()
        };
        assert!(
            learnings_disclosure_notice(Some(&cfg)).is_none(),
            "no notice when learnings is disabled"
        );
    }

    #[test]
    fn no_disclosure_notice_when_supervisor_section_absent() {
        // GIVEN no [supervisor] section at all — output identical to pre-change.
        assert!(
            learnings_disclosure_notice(None).is_none(),
            "no notice when the [supervisor] section is absent"
        );
    }

    #[test]
    fn no_disclosure_notice_when_supervisor_disabled() {
        // Defence in depth: learnings = true but supervisor disabled mirrors
        // the aggregator attach predicate (enabled && learnings) — no file is
        // produced, so no notice.
        let cfg = SupervisorConfig {
            enabled: false,
            learnings: true,
            ..SupervisorConfig::default()
        };
        assert!(
            learnings_disclosure_notice(Some(&cfg)).is_none(),
            "no notice when supervisor mode itself is disabled"
        );
    }

    // Spec scenario: "Learnings doc carries the privacy and sharing section".
    // Guards the user-guide chapter against silently losing the no-telemetry
    // stance or the optional-share invitation with its review/anonymise caveat.
    #[test]
    fn learnings_doc_carries_privacy_and_sharing_section() {
        let doc = include_str!("../docs/src/user-guide/learnings.md");
        assert!(
            doc.contains("## Privacy & Sharing"),
            "learnings doc must carry a Privacy & Sharing section"
        );
        assert!(
            doc.contains("no telemetry"),
            "section must state the no-telemetry / local / opt-in stance"
        );
        assert!(
            doc.contains(GIT_PAW_ISSUES_URL),
            "section must link to the GitHub issue tracker for optional sharing"
        );
        assert!(
            doc.contains("anonymise"),
            "section must carry the review-and-anonymise caveat"
        );
    }

    #[test]
    fn learnings_doc_documents_broker_publication_and_file_sourced_get_learnings() {
        let doc = include_str!("../docs/src/user-guide/learnings.md");
        assert!(
            doc.contains("agent.learning"),
            "learnings doc must mention agent.learning broker messages"
        );
        assert!(
            doc.contains("get_learnings") || doc.contains("get_learnings()"),
            "learnings doc must mention that get_learnings() reads the file"
        );
        assert!(
            !doc.contains("deferred"),
            "learnings doc must not claim the feature is deferred"
        );
    }
}

#[cfg(test)]
mod supervisor_self_register_tests {
    //! Tests that the supervisor self-register helper publishes an
    //! `agent.status` message with the spec-mandated fields and that the
    //! resulting broker state surfaces the supervisor row in
    //! `broker::delivery::agent_status_snapshot`.

    use std::sync::Arc;

    use git_paw::broker;
    use git_paw::broker::delivery;
    use git_paw::broker::messages::BrokerMessage;

    use super::build_status_message;

    #[test]
    fn supervisor_boot_status_message_has_spec_fields() {
        let msg = build_status_message(
            "supervisor",
            "working",
            Some("Supervisor booting".to_string()),
            None,
        );
        match msg {
            BrokerMessage::Status { agent_id, payload } => {
                assert_eq!(agent_id, "supervisor");
                assert_eq!(payload.status, "working");
                assert_eq!(payload.message.as_deref(), Some("Supervisor booting"));
                assert!(payload.modified_files.is_empty());
            }
            _ => panic!("expected BrokerMessage::Status"),
        }
    }

    #[test]
    fn publish_message_with_supervisor_boot_registers_supervisor() {
        let state = Arc::new(broker::BrokerState::new(None));
        let msg = build_status_message(
            "supervisor",
            "working",
            Some("Supervisor booting".to_string()),
            None,
        );
        delivery::publish_message(&state, &msg);

        let inner = state.read();
        let record = inner
            .agents
            .get("supervisor")
            .expect("supervisor agent record exists");
        assert_eq!(record.status, "working");
    }

    #[test]
    fn supervisor_appears_in_agent_status_snapshot_after_boot() {
        let state = Arc::new(broker::BrokerState::new(None));
        let msg = build_status_message(
            "supervisor",
            "working",
            Some("Supervisor booting".to_string()),
            None,
        );
        delivery::publish_message(&state, &msg);

        let snapshot = delivery::agent_status_snapshot(&state);
        let entry = snapshot
            .iter()
            .find(|e| e.agent_id == "supervisor")
            .expect("supervisor row appears in snapshot");
        assert_eq!(entry.status, "working");
    }
}

/// The parsed `setup-cli` invocation, kept as one struct so the entry
/// function takes a single argument rather than seven loose parameters.
///
/// The booleans mirror the command-line flags one for one; collapsing them
/// into enums here would only re-encode what clap already parsed, and would
/// hide which flag a given branch is reacting to.
#[allow(clippy::struct_excessive_bools)]
struct SetupCliArgs<'a> {
    /// CLI identity named on the command line.
    cli: &'a str,
    /// `--allowlist`.
    allowlist: bool,
    /// `--trust`.
    trust: bool,
    /// `--hooks`.
    hooks: bool,
    /// `--dry-run`.
    dry_run: bool,
    /// `--yes`.
    yes: bool,
    /// `--undo`.
    undo: bool,
}

impl SetupCliArgs<'_> {
    /// Returns the requested items, in flag order.
    fn items(&self) -> Vec<SetupItem> {
        let mut items = Vec::new();
        if self.allowlist {
            items.push(SetupItem::Allowlist);
        }
        if self.trust {
            items.push(SetupItem::Trust);
        }
        if self.hooks {
            items.push(SetupItem::Hooks);
        }
        items
    }
}

/// Entry layer for `git paw setup-cli`.
///
/// Owns everything the use case deliberately does not: printing the plan, the
/// terminal check, the confirmation, and the mapping from outcome to exit
/// code. Writing is gated three ways — the item must be supported, its facts
/// must be verified, and the user must have confirmed (or passed `--yes`).
fn cmd_setup_cli(args: &SetupCliArgs<'_>) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git_paw::git::validate_repo(&cwd)?;
    let config = git_paw::config::load_config(&repo_root, None)?;
    let runner = git_paw::command_runner::RealCommandRunner;

    let request = commands::setup_cli::SetupRequest {
        cli: args.cli.to_string(),
        items: args.items(),
        undo: args.undo,
    };

    match commands::setup_cli::prepare(&runner, &cwd, &config, &request)? {
        commands::setup_cli::Prepared::Unsupported { cli, items } => {
            print!("{}", render_unsupported(&cli, &items));
            Err(PawError::NotAttempted(format!(
                "nothing was written: {} not supported for `{cli}`",
                if items.len() == 1 {
                    "that setup item is"
                } else {
                    "those setup items are"
                }
            )))
        }
        commands::setup_cli::Prepared::Setup { plan, snapshots } => {
            print!("{}", render_plan(&plan));
            if args.dry_run {
                return Ok(());
            }
            if plan.is_empty() {
                // Nothing to do: no prompt, no write, success.
                return Ok(());
            }
            if !plan.verified {
                return Err(PawError::NotAttempted(format!(
                    "nothing was written: these facts have not been verified against an \
                     installed copy of `{}`, so writing is disabled — `--dry-run` still shows \
                     the plan",
                    plan.cli
                )));
            }
            confirm_setup(args, "Apply this plan?")?;
            let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
            let outcome = commands::setup_cli::apply_setup(&plan, &snapshots, &now)?;
            println!(
                "\nApplied {} entr{} across {} file{}.",
                outcome.entries_added,
                if outcome.entries_added == 1 {
                    "y"
                } else {
                    "ies"
                },
                outcome.written.len(),
                if outcome.written.len() == 1 { "" } else { "s" }
            );
            Ok(())
        }
        commands::setup_cli::Prepared::Undo {
            plan,
            record,
            snapshots,
        } => {
            print!("{}", render_undo_plan(&plan));
            if args.dry_run {
                return Ok(());
            }
            if plan.is_empty() {
                return Ok(());
            }
            confirm_setup(args, "Remove these entries?")?;
            let outcome = commands::setup_cli::apply_undo(&plan, &record, &snapshots)?;
            println!(
                "\nRemoved {} entr{}.",
                outcome.entries_removed,
                if outcome.entries_removed == 1 {
                    "y"
                } else {
                    "ies"
                }
            );
            Ok(())
        }
    }
}

/// The confirmation gate shared by setup and undo.
///
/// `--yes` proceeds. On a terminal the user is asked, defaulting to No. Off a
/// terminal there is nobody to ask, so nothing is written and the message
/// names the flags that would proceed — a bare "Cancelled." would leave the
/// user with no idea what to do next.
fn confirm_setup(args: &SetupCliArgs<'_>, prompt: &str) -> Result<(), PawError> {
    if args.yes {
        return Ok(());
    }
    if !is_interactive_stdin() {
        return Err(PawError::NotAttempted(
            "nothing was written: the confirmation could not be shown because stdin is not a \
             terminal — re-run with `--yes` to apply the printed plan, or `--dry-run` to preview \
             it"
            .into(),
        ));
    }
    let approved = Confirm::new()
        .with_prompt(prompt)
        .default(false)
        .interact()
        .map_err(|e| PawError::SessionError(format!("confirmation prompt failed: {e}")))?;
    if approved {
        Ok(())
    } else {
        Err(PawError::NotAttempted(
            "nothing was written: the plan was not approved".into(),
        ))
    }
}
