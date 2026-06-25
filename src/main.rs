//! git-paw — Parallel AI Worktrees.
//!
//! Orchestrates multiple AI coding CLI sessions across git worktrees
//! from a single terminal using tmux.

use std::io::IsTerminal;
use std::path::Path;
use std::process::Command as StdCommand;
use std::time::SystemTime;

use clap::Parser;
use dialoguer::Confirm;

use git_paw::agents;
use git_paw::broker;
use git_paw::broker::messages::BrokerMessage;
#[cfg(test)]
use git_paw::broker::publish::build_status_message;
use git_paw::broker::publish::publish_to_broker_http;
use git_paw::cli::{Cli, Command, SpecsFormat};
use git_paw::config::{self, PawConfig, SupervisorConfig};
use git_paw::detect;
use git_paw::error::PawError;
use git_paw::git;
use git_paw::interactive;
use git_paw::session::{self, Session, SessionMode, SessionStatus, WorktreeEntry};
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
    });

    if let Err(err) = run(command) {
        err.exit();
    }
}

/// Dispatch and execute the given CLI command.
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
        } => {
            // Resolve supervisor-mode-enabled state BEFORE dispatching so the
            // user's --supervisor flag (or [supervisor] config) is honoured
            // when combined with --from-all-specs (or its hidden alias).
            // `--no-supervisor` (added in no-supervisor-flag) wins over both
            // the flag and any [supervisor] config setting.
            let supervisor_enabled =
                resolve_supervisor_mode_from_cwd(no_supervisor, supervisor, dry_run)?;
            let spec_mode = SpecMode::from_flags(from_all_specs, specs.as_deref());
            let specs_format_str = specs_format.map(SpecsFormat::as_str);
            match resolve_dispatch_target(&spec_mode, supervisor_enabled) {
                DispatchTarget::Supervisor { spec_mode } => {
                    let cwd = std::env::current_dir().map_err(|e| {
                        PawError::SessionError(format!("cannot read current directory: {e}"))
                    })?;
                    let repo_root = git::validate_repo(&cwd)?;
                    let config = config::load_config(&repo_root, None)?;
                    cmd_supervisor(
                        &repo_root,
                        &config,
                        cli_flag.as_deref(),
                        branches_flag.as_deref(),
                        &spec_mode,
                        specs_format_str,
                        dry_run,
                        no_rebase,
                    )
                }
                DispatchTarget::StartWithSpecs(mode) => cmd_start_with_specs(
                    cli_flag.as_deref(),
                    &mode,
                    specs_format_str,
                    dry_run,
                    force,
                    no_rebase,
                ),
                DispatchTarget::Start => cmd_start(
                    cli_flag,
                    branches_flag,
                    dry_run,
                    preset.as_deref(),
                    no_supervisor,
                    no_rebase,
                ),
            }
        }
        Command::Add {
            branch,
            cli,
            from_spec,
        } => cmd_add(branch.as_deref(), cli.as_deref(), from_spec.as_deref()),
        Command::Remove {
            branch,
            keep_worktree,
            force,
        } => cmd_remove(&branch, keep_worktree, force),
        Command::Pause => cmd_pause(),
        Command::Stop { force } => cmd_stop(force),
        Command::Purge { force, stale } => cmd_purge(force, stale),
        Command::Status { json } => cmd_status(json),
        Command::ListClis => cmd_list_clis(),
        Command::AddCli {
            name,
            command,
            display_name,
        } => cmd_add_cli(&name, &command, display_name.as_deref()),
        Command::RemoveCli { name } => cmd_remove_cli(&name),
        Command::Dashboard => cmd_dashboard(),
        Command::Init => git_paw::init::run_init(),
        Command::Replay {
            branch,
            list,
            color,
            session,
        } => cmd_replay(branch, list, color, session.as_deref()),
        Command::Approvals {
            session,
            limit,
            json,
        } => cmd_approvals(session.as_deref(), limit, json),
        Command::Mcp { repo, log_file } => {
            git_paw::mcp::cmd_mcp(repo.as_deref(), log_file.as_deref())
        }
        Command::Selftest => git_paw::selftest::run(),
    }
}

// ---------------------------------------------------------------------------
// Dispatch routing
// ---------------------------------------------------------------------------

/// Spec selection mode resolved from the `--from-all-specs` and `--specs`
/// CLI flags. The mode controls how the discovered `SpecEntry` set is
/// filtered before launching worktrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecMode {
    /// No spec mode requested — fall through to the regular branch flow.
    None,
    /// `--from-all-specs` (or its hidden `--from-specs` alias) — launch
    /// every discovered spec.
    All,
    /// Bare `--specs` — open the multi-select picker (TTY required).
    Picker,
    /// `--specs NAME[,NAME...]` — narrow to the supplied names.
    Narrow(Vec<String>),
}

impl SpecMode {
    /// Converts the parsed `(from_all_specs, specs)` flag pair into a
    /// `SpecMode`. Mutual exclusion is enforced at the clap layer, so this
    /// translation is total.
    pub fn from_flags(from_all_specs: bool, specs: Option<&[String]>) -> Self {
        if from_all_specs {
            return Self::All;
        }
        match specs {
            None => Self::None,
            Some([]) => Self::Picker,
            Some(values) => Self::Narrow(values.to_vec()),
        }
    }
}

/// Concrete dispatch target for the `start` subcommand. Extracted as a pure
/// function (`resolve_dispatch_target`) so the routing decision is unit-testable
/// independently of any IO.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DispatchTarget {
    /// Route to `cmd_supervisor`, carrying the resolved spec selection mode.
    ///
    /// The mode is passed through verbatim (not collapsed to a boolean) so the
    /// supervisor launch path applies the same subset filtering as the
    /// non-supervisor `--specs` path: `Narrow(names)` launches only the named
    /// subset, `Picker` opens the multi-select picker, `All` launches every
    /// discovered spec, and `None` falls through to the `--branches` /
    /// branch-picker flow. Fixes the v0.6.0 dogfood bug where
    /// `--supervisor --specs a,b` launched every discovered spec.
    Supervisor { spec_mode: SpecMode },
    /// Route to `cmd_start_with_specs` with the resolved spec mode (one of
    /// `All`, `Picker`, `Narrow`).
    StartWithSpecs(SpecMode),
    /// Route to bare `cmd_start`.
    Start,
}

/// Pure routing function for `start` subcommand dispatch.
///
/// When supervisor mode is enabled, every spec mode routes to the supervisor
/// path carrying the resolved `SpecMode` so subset filtering (`Narrow`),
/// the interactive `Picker`, `All`, and the bare branch-picker (`None`) cases
/// behave identically to the non-supervisor path — just with supervisor mode
/// engaged. Without supervisor mode, a populated spec mode routes through
/// `cmd_start_with_specs` and `SpecMode::None` falls through to bare `cmd_start`.
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

/// Applies the `SpecMode` filter to the discovered `SpecEntry` set.
///
/// - `SpecMode::None` and `SpecMode::All` return the input unchanged.
/// - `SpecMode::Picker` requires an interactive stdin; with a non-TTY stdin
///   this returns a `PawError::SpecError` directing the user at the
///   explicit forms.
/// - `SpecMode::Narrow(names)` resolves each name against the discovered
///   set via `git_paw::specs::resolve::resolve_specs`.
fn apply_spec_mode(
    mode: &SpecMode,
    discovered: Vec<git_paw::specs::SpecEntry>,
    prompter: &dyn interactive::Prompter,
) -> Result<Vec<git_paw::specs::SpecEntry>, PawError> {
    match mode {
        SpecMode::None | SpecMode::All => Ok(discovered),
        SpecMode::Picker => {
            if !is_interactive_stdin() {
                return Err(PawError::SpecError(
                    "--specs without values requires an interactive terminal\n  \
                     Use `--specs NAME[,NAME...]` to narrow explicitly, or\n  \
                     `--from-all-specs` to launch every discovered spec."
                        .to_string(),
                ));
            }
            prompter.select_specs(&discovered)
        }
        SpecMode::Narrow(names) => git_paw::specs::resolve::resolve_specs(&discovered, names),
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

/// Builds the per-agent task prompt that gets appended to the supervisor-mode
/// boot block before injection via `tmux send-keys`.
///
/// **Per-backend dispatch.** When `spec_entry` is `Some`, the helper switches
/// on `SpecEntry.backend` to pick the prompt shape. Every branch first points
/// the agent at the gitignored sidecar
/// ([`SIDECAR_REL_PATH`](git_paw::agents::SIDECAR_REL_PATH)) — the combined
/// view of the project's `AGENTS.md` plus the agent's assignment and
/// coordination rules — before its apply/begin step:
///
/// - `SpecBackendKind::OpenSpec` → read the sidecar, then run the
///   `/opsx:apply <id>` slash command. (Pre-`agents-md-sidecar-injection`
///   this branch was the bare slash command with deliberately *no* pointer,
///   because the combined view was auto-loaded from the worktree-root
///   `AGENTS.md`. The sidecar is no longer auto-loaded by the CLIs, so the
///   pointer is now required for the agent to receive its assignment and
///   inter-agent rules. This intentionally overrides that earlier
///   no-pointer decision.)
/// - `SpecBackendKind::Markdown` → read the sidecar (which carries the full
///   spec body), then locate sibling artifacts under `openspec/changes/<id>/`.
/// - `SpecBackendKind::SpecKit` → falls through to the same sidecar pointer
///   as `Markdown`. A `/speckit:apply`-style slash command may land later;
///   this change does not pre-empt its shape.
///
/// The match SHALL be exhaustive over `SpecBackendKind` (no `_ =>`
/// catch-all); the Rust compiler forces a decision for any new variant.
///
/// The returned string points the agent at the sidecar rather than embedding
/// any portion of the spec body. The combined view is written to the sidecar
/// by `setup_worktree_agents_md`; the supported CLIs only auto-load the
/// worktree-root `AGENTS.md`, so the prompt is what directs the agent to the
/// sidecar's combined content. Pointing at a file avoids the paste-buffer trap
/// and duplicate content.
///
/// **Prerequisite:** callers SHALL ensure `setup_worktree_agents_md` has run
/// for the worktree before the resulting prompt is injected. The prompt's
/// guidance is non-actionable if the sidecar is missing.
///
/// When a spec is associated with the branch, the prompt includes the spec
/// ID so the agent can locate sibling artifacts (proposal, design, specs,
/// tasks) under `openspec/changes/<id>/`. When no spec is associated, the
/// prompt is the default fallback that points only at the sidecar.
pub(crate) fn build_task_prompt(spec_entry: Option<&git_paw::specs::SpecEntry>) -> String {
    use git_paw::agents::SIDECAR_REL_PATH;
    use git_paw::specs::SpecBackendKind;
    match spec_entry {
        Some(s) => match s.backend {
            SpecBackendKind::OpenSpec => format!(
                "Read {SIDECAR_REL_PATH} first — it carries your assignment and \
                 coordination rules — then run /opsx:apply {id}.",
                id = s.id,
            ),
            SpecBackendKind::Markdown | SpecBackendKind::SpecKit => format!(
                "Begin your assigned task. Read {SIDECAR_REL_PATH} first — it carries the \
                 project rules, your full spec, and your assignment. Additional artifacts \
                 (proposal, design, specs, tasks) live under openspec/changes/{id}/ — read \
                 them all before starting.",
                id = s.id,
            ),
        },
        None => format!(
            "Read {SIDECAR_REL_PATH} first for your assignment, then begin your assigned task."
        ),
    }
}

// ---------------------------------------------------------------------------
// Supervisor mode resolution
// ---------------------------------------------------------------------------

/// Loads the repo config from the current working directory and resolves
/// whether supervisor mode should be entered for this session.
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

/// Abstraction over the "Start in supervisor mode?" prompt so the resolution
/// logic can be unit-tested without touching stdin.
trait SupervisorPrompt {
    fn ask(&mut self) -> Result<bool, PawError>;
}

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

/// Pure resolution of the supervisor mode chain. See `supervisor-cli` spec.
fn resolve_supervisor_mode(
    no_supervisor_flag: bool,
    supervisor_flag: bool,
    dry_run: bool,
    config: &PawConfig,
    prompt: &mut dyn SupervisorPrompt,
) -> Result<bool, PawError> {
    // Step 0: --no-supervisor wins over everything — explicit session-level off.
    // clap enforces mutual exclusion with --supervisor, so both can't be true.
    if no_supervisor_flag {
        return Ok(false);
    }
    // Step 1: --supervisor flag always wins (over config and prompt).
    if supervisor_flag {
        return Ok(true);
    }
    // Steps 2 & 3: explicit config value.
    if let Some(cfg) = &config.supervisor {
        return Ok(cfg.enabled);
    }
    // Step 5 (evaluated before step 4): dry-run skips the prompt entirely.
    if dry_run {
        return Ok(false);
    }
    // Step 4: no section → prompt.
    prompt.ask()
}

// ---------------------------------------------------------------------------
// Type bridging helpers
// ---------------------------------------------------------------------------

/// Converts custom CLIs from config into the format expected by the detect module.
fn config_to_custom_defs(config: &PawConfig) -> Vec<detect::CustomCliDef> {
    config
        .clis
        .iter()
        .map(|(name, cli)| detect::CustomCliDef {
            name: name.clone(),
            command: cli.command.clone(),
            display_name: cli.display_name.clone(),
        })
        .collect()
}

/// Converts a detected CLI info into the format expected by the interactive module.
fn to_interactive_cli(cli: &detect::CliInfo) -> interactive::CliInfo {
    interactive::CliInfo {
        display_name: cli.display_name.clone(),
        binary_name: cli.binary_name.clone(),
    }
}

// ---------------------------------------------------------------------------
// Command: start
// ---------------------------------------------------------------------------

/// Smart start: reattach if active, recover if stale, launch fresh if new.
#[allow(clippy::too_many_lines)]
fn cmd_start(
    cli_flag: Option<String>,
    branches_flag: Option<Vec<String>>,
    dry_run: bool,
    preset: Option<&str>,
    no_supervisor: bool,
    no_rebase: bool,
) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    // Check for existing session (skip reattach/recovery during dry-run).
    // Before deciding reattach-vs-recover, probe the receipt for staleness:
    // a receipt claiming `active` whose tmux session has vanished is
    // invalidated (purged) here and the launch proceeds fresh (design D5).
    let existing_session = session::find_session_for_repo(&repo_root)?;
    if !dry_run
        && let Some(existing) = &existing_session
        && !invalidate_if_stale(&repo_root, existing)?
    {
        let effective =
            existing.effective_status(|name| tmux::is_session_alive(name).unwrap_or(false));
        match effective {
            SessionStatus::Paused => {
                println!(
                    "Restarting paused session '{}' (broker + reattach)...",
                    existing.session_name
                );
                return restart_from_pause(&repo_root, existing);
            }
            SessionStatus::Active => {
                println!("Reattaching to session '{}'...", existing.session_name);
                return attach_or_print_hint(&existing.session_name);
            }
            SessionStatus::Stopped => {
                println!("Recovering session '{}'...", existing.session_name);
                return recover_session(&repo_root, existing);
            }
        }
    }

    // Fresh launch (or dry-run preview)
    tmux::ensure_tmux_installed()?;
    let config = config::load_config(&repo_root, None)?;

    // Supervisor mode: when the supervisor section is enabled in config, hand
    // off to the auto-start flow that launches all coding agents under a
    // supervisor CLI. The supervisor is responsible for verification and merge.
    // --no-supervisor explicitly overrides this so a user can skip the auto-start
    // flow without editing config.
    if !no_supervisor && config.supervisor.as_ref().is_some_and(|s| s.enabled) {
        return cmd_supervisor(
            &repo_root,
            &config,
            cli_flag.as_deref(),
            branches_flag.as_deref(),
            &SpecMode::None,
            None,
            dry_run,
            no_rebase,
        );
    }

    let custom_defs = config_to_custom_defs(&config);

    // Resolve branches and CLI from preset or flags/interactive
    let (resolved_cli, resolved_branches) = if let Some(preset_name) = preset {
        let p = config
            .get_preset(preset_name)
            .ok_or_else(|| PawError::ConfigError(format!("preset '{preset_name}' not found")))?;
        (Some(p.cli.clone()), Some(p.branches.clone()))
    } else {
        (cli_flag, branches_flag)
    };

    // Detect available CLIs
    let detected = detect::detect_clis(&custom_defs);
    if detected.is_empty() {
        return Err(PawError::NoCLIsFound);
    }

    // List branches
    let all_branches = git::list_branches(&repo_root)?;

    // Interactive selection (or skip via flags)
    let interactive_clis: Vec<interactive::CliInfo> =
        detected.iter().map(to_interactive_cli).collect();
    let prompter = interactive::TerminalPrompter;
    let selection = interactive::run_selection(
        &prompter,
        &all_branches,
        &interactive_clis,
        resolved_cli.as_deref(),
        resolved_branches.as_deref(),
    )?;

    // Dry run — print plan and exit without creating worktrees
    let project = git::project_name(&repo_root);
    let mouse = config.mouse.unwrap_or(true);

    // Resolve a unique session name (handles cross-repo collisions)
    let session_name = tmux::resolve_session_name(&project)?;

    if dry_run {
        if let Some(ref existing) = existing_session {
            eprintln!(
                "warning: session '{}' already exists — purge it before starting a new one\n",
                existing.session_name
            );
        }
        println!("Dry run — session plan:\n");
        println!("  Session:  {session_name}");
        println!("  Mouse:    {}", if mouse { "on" } else { "off" });
        println!();
        for (branch, cli) in &selection.mappings {
            let wt_dir = git::worktree_dir_name(&project, branch);
            println!("  {branch} \u{2192} {cli} (../{wt_dir})");
        }
        return Ok(());
    }

    // Create worktrees and build tmux session
    // Prune stale worktree registrations from previous sessions
    git::prune_worktrees(&repo_root)?;

    let broker_config = config.broker.clone();

    let mut builder = tmux::TmuxSessionBuilder::new(&project)
        .session_name(session_name)
        .mouse_mode(mouse)
        .border_affordances(config.border_affordances_enabled());

    // Broker: inject dashboard pane and environment variable
    if broker_config.enabled {
        let repo_str = repo_root.to_string_lossy().to_string();
        builder = builder.add_pane(tmux::PaneSpec {
            branch: "dashboard".to_string(),
            worktree: repo_str,
            cli_command: format!(
                "{} __dashboard",
                std::env::current_exe()
                    .unwrap_or_else(|_| std::path::PathBuf::from("git-paw"))
                    .display()
            ),
        });
        builder = builder.set_environment("GIT_PAW_BROKER_URL", &broker_config.url());
    }

    let mut worktree_entries = Vec::new();

    // Resolve coordination skill once if broker is enabled
    let skill_content = if broker_config.enabled {
        let template = git_paw::skills::resolve("coordination")?;
        Some(template)
    } else {
        None
    };

    for (branch, cli) in &selection.mappings {
        let wt = git::create_worktree(&repo_root, branch, !no_rebase, config.worktree_placement())?;
        let wt_str = wt.path.to_string_lossy().to_string();

        // Inject AGENTS.md with skill content when broker is enabled.
        // Non-supervisor `start` flow has no resolved spec backends —
        // pass `&[]` so SPEC_PATH_DOCTRINE renders its sentinel. The
        // coordination skill does not reference the placeholder today,
        // but keeping the call shape uniform avoids future drift.
        let rendered_skill = skill_content.as_ref().map(|tmpl| {
            git_paw::skills::render(
                tmpl,
                branch,
                &broker_config.url(),
                &project,
                &git_paw::skills::GateCommands::default(),
                &[],
            )
        });
        let assignment = git_paw::agents::WorktreeAssignment {
            branch: branch.clone(),
            cli: cli.clone(),
            spec_content: None,
            owned_files: None,
            skill_content: rendered_skill,
            inter_agent_rules: None,
        };
        git_paw::agents::setup_worktree_agents_md(&repo_root, &wt.path, &assignment)?;

        if broker_config.enabled {
            let agent_id = git_paw::broker::messages::slugify_branch(branch);
            let strict_guard = config
                .supervisor
                .as_ref()
                .is_none_or(SupervisorConfig::strict_branch_guard);
            git_paw::agents::install_git_hooks(
                &wt.path,
                &broker_config.url(),
                &agent_id,
                branch,
                strict_guard,
            )?;
        }

        builder = builder.add_pane(tmux::PaneSpec {
            branch: branch.clone(),
            worktree: wt_str,
            cli_command: cli.clone(),
        });

        worktree_entries.push(WorktreeEntry {
            branch: branch.clone(),
            worktree_path: wt.path,
            cli: cli.clone(),
            branch_created: wt.branch_created,
            pending_boot_prompt: None,
        });
    }

    let tmux_session = builder.build()?;

    // Execute tmux session
    tmux_session.execute()?;

    // Inject boot blocks for manual broker mode (without supervisor).
    // The argv shape is determined by `tmux::build_boot_inject_args` so the
    // call shape (literal mode, no trailing Enter, `-l` before `-t`) has a
    // single source of truth that tests can verify directly.
    if broker_config.enabled {
        for (idx, (branch, _)) in selection.mappings.iter().enumerate() {
            let pane_idx = if broker_config.enabled { idx + 1 } else { idx };
            let boot_block = git_paw::skills::build_boot_block(branch, &broker_config.url());

            let args =
                git_paw::tmux::build_boot_inject_args(&tmux_session.name, pane_idx, &boot_block);
            let _ = std::process::Command::new("tmux").args(&args).status();
        }
    }

    // Save session state
    let mut state = Session {
        session_name: tmux_session.name.clone(),
        repo_path: repo_root,
        project_name: project,
        created_at: SystemTime::now(),
        status: SessionStatus::Active,
        worktrees: worktree_entries,
        broker_port: None,
        broker_bind: None,
        broker_log_path: None,
        mode: SessionMode::Bare,
        dashboard_pane: None,
    };

    if broker_config.enabled {
        state.broker_port = Some(broker_config.port);
        state.broker_bind = Some(broker_config.bind.clone());
        state.broker_log_path = Some(session::session_state_dir()?.join("broker.log"));
        state.dashboard_pane = Some(0);
    }

    session::save_session(&state)?;

    // Write the per-repo discovery file sweep.sh reads. In the bare layout
    // the dashboard occupies pane 0 when the broker is enabled, so coding
    // agents start at pane 1; without the broker they start at pane 0.
    let pane_offset = usize::from(broker_config.enabled);
    write_repo_discovery_file(
        &state.repo_path,
        &tmux_session.name,
        &state.worktrees,
        pane_offset,
    );

    // Attach (or print hint when stdin is non-TTY).
    attach_or_print_hint(&tmux_session.name)
}

// ---------------------------------------------------------------------------
// Command: supervisor (auto-start flow)
// ---------------------------------------------------------------------------

/// Auto-start flow for supervisor mode.
///
/// Reads the supervisor config, resolves branches (from `--branches`, specs,
/// or interactive selection), creates worktrees, generates per-worktree
/// `AGENTS.md` with spec content, coordination skill, and inter-agent rules,
/// builds a tmux session with the dashboard in pane 0 and coding agents in
/// panes 1..=N, injects `GIT_PAW_BROKER_URL` into the session environment,
/// boots all panes, injects the initial prompt for each coding agent via
/// `tmux send-keys`, and finally starts the supervisor CLI in the foreground
/// terminal (blocking until it exits).
/// Publishes a question from the supervisor to the human dashboard.
/// This function allows the supervisor agent to escalate questions it cannot answer
/// by publishing them as `agent.question` messages to the broker.
///
/// Uses [`publish_to_broker_http`] (which serialises via `serde_json`) instead of
/// hand-rolling a JSON body and shelling out to `curl`, so the question text
/// round-trips byte-for-byte regardless of embedded backslashes, quotes, or
/// shell metacharacters.
#[cfg_attr(not(test), expect(dead_code))]
fn publish_supervisor_question(question: &str, broker_url: &str) -> Result<(), PawError> {
    let msg = BrokerMessage::Question {
        agent_id: "supervisor".to_string(),
        payload: git_paw::broker::messages::QuestionPayload {
            question: question.to_string(),
        },
    };
    publish_to_broker_http(broker_url, &msg)
}

/// Spawns a background thread that periodically polls the broker `/status`
/// endpoint and dispatches auto-approval keystrokes for stalled agents.
///
/// Returns `None` when [`config::AutoApproveConfig`] is absent or the
/// resolved config has `enabled = false`. Otherwise returns a stop flag
/// (set to `true` to ask the thread to exit) and the thread handle for
/// joining.
///
/// The poll period is the resolved `stall_threshold_seconds`, capped to
/// the spec's minimum of 5s. Errors fetching the broker `/status` endpoint
/// are logged once per occurrence — they do not abort the thread.
fn spawn_auto_approve_thread(
    session_name: String,
    broker_url: String,
    config: Option<config::AutoApproveConfig>,
    pane_map: std::collections::HashMap<String, usize>,
    worktree_map: std::collections::HashMap<String, std::path::PathBuf>,
    recorder: git_paw::supervisor::manual_approvals::ManualDecisionRecorder,
) -> Option<(
    std::sync::Arc<std::sync::atomic::AtomicBool>,
    std::thread::JoinHandle<()>,
)> {
    let cfg = config?.resolved();
    if !cfg.enabled {
        return None;
    }
    let period = std::time::Duration::from_secs(
        cfg.stall_threshold_seconds
            .max(git_paw::config::AutoApproveConfig::MIN_STALL_THRESHOLD_SECONDS),
    );
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_clone = stop.clone();

    let handle = std::thread::spawn(move || {
        use git_paw::supervisor::approve::TmuxKeyDispatcher;
        use git_paw::supervisor::poll::{
            PollContext, TmuxPaneInspector, fetch_status_over_http, tick_from_status,
        };

        // The forwarder is invoked by `drive_outcomes` only on the
        // forward-to-human branch (`TickOutcome::Forwarded`) — auto-approved
        // prompts take the `Approved` branch and never reach it. That makes it
        // the single, accurate call site for recording manual-decision-required
        // commands (approval-pattern-surfacing §3, design D2 option A).
        struct BrokerForwarder {
            broker_url: String,
            recorder: git_paw::supervisor::manual_approvals::ManualDecisionRecorder,
        }
        impl git_paw::supervisor::poll::QuestionForwarder for BrokerForwarder {
            fn forward_question(
                &mut self,
                agent_id: &str,
                kind: git_paw::supervisor::permission_prompt::PermissionType,
                captured: &str,
            ) {
                let question = format!(
                    "{agent_id} is stalled on a permission prompt classified as {kind:?}; \
                     please review the pane and decide manually."
                );
                let msg = git_paw::broker::messages::BrokerMessage::Question {
                    agent_id: "supervisor".to_string(),
                    payload: git_paw::broker::messages::QuestionPayload { question },
                };
                if let Err(e) =
                    git_paw::broker::publish::publish_to_broker_http(&self.broker_url, &msg)
                {
                    eprintln!("auto-approve: failed to forward question to dashboard: {e}");
                }

                // Record the manual-decision-required command. On the first
                // sighting this returns a `permission_pattern` learning to
                // publish (when learnings are enabled).
                if let Some(learning) = self.recorder.record_forwarded(agent_id, captured)
                    && let Err(e) = git_paw::broker::publish::publish_to_broker_http(
                        &self.broker_url,
                        &learning,
                    )
                {
                    eprintln!("auto-approve: failed to publish permission_pattern learning: {e}");
                }
            }
        }

        let inspector = TmuxPaneInspector;
        let resolver = move |id: &str| pane_map.get(id).copied();
        let worktree_resolver = move |id: &str| worktree_map.get(id).cloned();
        let mut dispatcher = TmuxKeyDispatcher;
        let mut forwarder = BrokerForwarder {
            broker_url: broker_url.clone(),
            recorder,
        };

        while !stop_clone.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(period);
            if stop_clone.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let rows = match fetch_status_over_http(&broker_url) {
                Ok(rows) => rows,
                Err(e) => {
                    eprintln!("auto-approve: broker /status fetch failed: {e}");
                    continue;
                }
            };
            let mut ctx = PollContext {
                state: None,
                session: &session_name,
                config: &cfg,
                resolver: &resolver,
                inspector: &inspector,
                dispatcher: &mut dispatcher,
                forwarder: &mut forwarder,
                worktree_resolver: &worktree_resolver,
                broker_url: Some(&broker_url),
            };
            let _ = tick_from_status(&rows, &mut ctx);
        }
    });
    Some((stop, handle))
}

/// Per-session context for [`attach_agent`] — the values shared across every
/// agent attached to a single session. `cmd_supervisor` builds it once before
/// its per-branch loop; `cmd_add` builds it once for the single new agent.
struct AttachContext<'a> {
    /// Repository root the worktrees hang off.
    repo_root: &'a Path,
    /// Human-readable project name (worktree-path convention).
    project: &'a str,
    /// Broker config (drives skill rendering, hook install, boot block).
    broker_config: &'a git_paw::config::BrokerConfig,
    /// Coding-agent CLI id (no flags).
    agent_cli: &'a str,
    /// Resolved approval flags appended to `agent_cli` for the pane command.
    agent_flags: &'a str,
    /// Pre-resolved coordination skill template (`None` when broker disabled).
    coordination_template: Option<&'a git_paw::skills::SkillTemplate>,
    /// Gate-command substitutions for skill rendering.
    gate_commands: &'a git_paw::skills::GateCommands<'a>,
    /// Distinct spec backends in the session (for `{{SPEC_PATH_DOCTRINE}}`).
    session_backends: &'a [git_paw::specs::SpecBackendKind],
    /// Inter-agent ownership rules block (`None` for the bare/no-rules case).
    inter_agent_rules: Option<&'a str>,
    /// Whether the broker git hooks enforce the strict branch guard.
    strict_guard: bool,
    /// Skip rebasing the branch onto the default branch on worktree create.
    no_rebase: bool,
    /// Worktree placement (child vs sibling) for `create_worktree`.
    placement: git_paw::config::WorktreePlacement,
}

/// Artifacts produced by attaching one agent's worktree: the tmux pane spec to
/// splice into the session, the initial boot+task prompt to submit, and the
/// session-JSON worktree entry to register.
struct AttachedAgent {
    /// Pane spec (branch, worktree path, CLI command) for the tmux layout.
    pane: tmux::PaneSpec,
    /// Combined boot block + initial task prompt to inject into the pane.
    prompt: String,
    /// Session-JSON worktree record for `status` / `stop` / `purge`.
    entry: WorktreeEntry,
}

/// Performs the per-worktree setup `git paw start` (supervisor mode) does for a
/// single agent: create or reuse the worktree, render the coordination skill +
/// spec body into the worktree `AGENTS.md`, install the broker git hooks, and
/// build the pane spec, boot+task prompt, and session entry.
///
/// Factored out of `cmd_supervisor`'s per-branch loop (design D2, task 1.2) so
/// `cmd_supervisor` (looping over every branch) and `cmd_add` (once, for the
/// new branch) share one implementation — guaranteeing an added agent is
/// byte-identical to a start-time agent and that future boot-prompt fixes only
/// have to land here.
fn attach_agent(
    ctx: &AttachContext,
    branch: &str,
    spec_entry: Option<&git_paw::specs::SpecEntry>,
) -> Result<AttachedAgent, PawError> {
    let wt = git::create_worktree(ctx.repo_root, branch, !ctx.no_rebase, ctx.placement)?;
    let wt_str = wt.path.to_string_lossy().to_string();

    let rendered_skill = ctx.coordination_template.map(|tmpl| {
        git_paw::skills::render(
            tmpl,
            branch,
            &ctx.broker_config.url(),
            ctx.project,
            ctx.gate_commands,
            ctx.session_backends,
        )
    });

    let spec_content = spec_entry.map(|s| s.prompt.clone());
    let owned_files = spec_entry.and_then(|s| s.owned_files.clone());

    let assignment = git_paw::agents::WorktreeAssignment {
        branch: branch.to_string(),
        cli: ctx.agent_cli.to_string(),
        spec_content,
        owned_files,
        skill_content: rendered_skill,
        inter_agent_rules: ctx.inter_agent_rules.map(str::to_string),
    };
    git_paw::agents::setup_worktree_agents_md(ctx.repo_root, &wt.path, &assignment)?;

    if ctx.broker_config.enabled {
        let agent_id = git_paw::broker::messages::slugify_branch(branch);
        git_paw::agents::install_git_hooks(
            &wt.path,
            &ctx.broker_config.url(),
            &agent_id,
            branch,
            ctx.strict_guard,
        )?;
    }

    let cli_command = if ctx.agent_flags.is_empty() {
        ctx.agent_cli.to_string()
    } else {
        format!("{} {}", ctx.agent_cli, ctx.agent_flags)
    };

    let boot_block = git_paw::skills::build_boot_block(branch, &ctx.broker_config.url());
    let task_prompt = build_task_prompt(spec_entry);

    Ok(AttachedAgent {
        pane: tmux::PaneSpec {
            branch: branch.to_string(),
            worktree: wt_str,
            cli_command,
        },
        prompt: format!("{boot_block}\n\n{task_prompt}"),
        entry: WorktreeEntry {
            branch: branch.to_string(),
            worktree_path: wt.path,
            cli: ctx.agent_cli.to_string(),
            branch_created: wt.branch_created,
            pending_boot_prompt: None,
        },
    })
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn cmd_supervisor(
    repo_root: &Path,
    config: &PawConfig,
    cli_flag: Option<&str>,
    branches_flag: Option<&[String]>,
    spec_mode: &SpecMode,
    specs_format_override: Option<&str>,
    dry_run: bool,
    no_rebase: bool,
) -> Result<(), PawError> {
    // Fall back to a synthesized default when [supervisor] is absent.
    // `resolve_supervisor_mode` already prompts the user to opt in to
    // supervisor mode without forcing them to hand-author a [supervisor]
    // block; the hard-error path that used to live here defeated that.
    let default_supervisor_cfg = SupervisorConfig::default();
    let supervisor_cfg = config
        .supervisor
        .as_ref()
        .unwrap_or(&default_supervisor_cfg);

    // Resolve the supervisor CLI: explicit override > default_cli > error.
    let supervisor_cli = supervisor_cfg
        .cli
        .clone()
        .or_else(|| config.default_cli.clone())
        .ok_or_else(|| {
            PawError::ConfigError(
                "supervisor mode requires either [supervisor].cli or default_cli to be set"
                    .to_string(),
            )
        })?;

    // Resolve coding agent CLI: explicit flag > default_cli > supervisor CLI.
    let agent_cli = cli_flag
        .map(ToString::to_string)
        .or_else(|| config.default_cli.clone())
        .unwrap_or_else(|| supervisor_cli.clone());

    // Resolve branches. Precedence:
    //   1. `--branches` — explicit branch list wins.
    //   2. `SpecMode::None` (bare `--supervisor`, no spec flag) — behave like
    //      `git paw start`: interactive branch picker, no spec discovery.
    //   3. `SpecMode::{All,Picker,Narrow}` — scan specs, then apply the same
    //      subset filter the non-supervisor `--specs` path uses so the named
    //      subset / picker selection is honoured (the v0.6.0 dogfood fix).
    let mut spec_by_branch: std::collections::HashMap<String, git_paw::specs::SpecEntry> =
        std::collections::HashMap::new();
    let branches: Vec<String> = if let Some(bs) = branches_flag {
        bs.to_vec()
    } else if matches!(spec_mode, SpecMode::None) {
        let custom_defs = config_to_custom_defs(config);
        let detected = detect::detect_clis(&custom_defs);
        if detected.is_empty() {
            return Err(PawError::NoCLIsFound);
        }
        let all_branches = git::list_branches(repo_root)?;
        let interactive_clis: Vec<interactive::CliInfo> =
            detected.iter().map(to_interactive_cli).collect();
        let prompter = interactive::TerminalPrompter;
        let selection = interactive::run_selection(
            &prompter,
            &all_branches,
            &interactive_clis,
            cli_flag,
            None,
        )?;
        selection.mappings.into_iter().map(|(b, _)| b).collect()
    } else {
        let discovered =
            git_paw::specs::scan_specs_with_override(config, repo_root, specs_format_override)?;
        if discovered.is_empty() {
            return Err(PawError::ConfigError(
                "supervisor mode found no branches: pass --branches or define specs".to_string(),
            ));
        }
        let specs = apply_spec_mode(spec_mode, discovered, &interactive::TerminalPrompter)?;
        if specs.is_empty() {
            return Err(PawError::ConfigError(
                "supervisor mode found no branches: pass --branches or define specs".to_string(),
            ));
        }
        let mut out = Vec::with_capacity(specs.len());
        for spec in specs {
            out.push(spec.branch.clone());
            spec_by_branch.insert(spec.branch.clone(), spec);
        }
        out
    };

    let project = git::project_name(repo_root);
    let session_name = tmux::resolve_session_name(&project)?;
    let mouse = config.mouse.unwrap_or(true);
    let broker_config = config.broker.clone();
    let approval = &supervisor_cfg.agent_approval;
    let agent_flags = config::approval_flags(&agent_cli, approval);
    let supervisor_flags = config::approval_flags(&supervisor_cli, approval);

    // Dry-run: print the plan and exit without touching the filesystem.
    if dry_run {
        println!("Dry run — supervisor session plan:\n");
        println!("  Session:    {session_name}");
        println!("  Supervisor: {supervisor_cli}");
        println!("  Agent CLI:  {agent_cli}");
        println!("  Approval:   {approval:?}");
        println!("  Mouse:      {}", if mouse { "on" } else { "off" });
        if broker_config.enabled {
            println!("  Broker URL: {}", broker_config.url());
        }
        println!();
        for branch in &branches {
            let wt_dir = git::worktree_dir_name(&project, branch);
            let cmd = if agent_flags.is_empty() {
                agent_cli.clone()
            } else {
                format!("{agent_cli} {agent_flags}")
            };
            println!("  {branch} \u{2192} {cmd} (../{wt_dir})");
        }
        return Ok(());
    }

    // Hard cap (D4): 25 agents per session for v0.5.0. Configurable layout
    // arrives in v1.0.0 (issue #17).
    let layout = git_paw::supervisor::layout::supervisor_layout(branches.len())?;

    // Real launch.
    git::prune_worktrees(repo_root)?;

    // Pre-populate `.claude/settings.json` with the least-privilege
    // agent-broker helper-path grant so the coding agents do not hit an
    // approval prompt when they invoke `.git-paw/scripts/broker.sh` on every
    // broker round-trip. Failures are logged but non-fatal.
    if broker_config.enabled {
        let claude_settings = repo_root.join(".claude").join("settings.json");
        if let Err(e) = git_paw::supervisor::curl_allowlist::setup_curl_allowlist(&claude_settings)
        {
            eprintln!("warning: failed to setup broker-helper allowlist: {e}");
        }
        // W15-6 (2026-05-31 dogfood): a custom CLI that reads a non-default
        // claude-format settings file (e.g. one reading
        // `~/.config/<variant>/settings.json`) needs the helper-path grant
        // seeded there too, or its boot-time `broker.sh status booting` hits a
        // permission prompt the auto-approve thread cannot clear before the
        // agent registers (W15-7). The path is CONFIG-DRIVEN
        // (`[clis.<name>].settings_path`), never a hardcoded CLI name — so
        // this stays CLI-agnostic. Seed each distinct session CLI's
        // configured settings file once.
        for cli in session_cli_settings_paths(config, &supervisor_cli, &agent_cli) {
            if let Err(e) = git_paw::supervisor::curl_allowlist::setup_curl_allowlist(&cli) {
                eprintln!(
                    "warning: failed to setup broker-helper allowlist at {}: {e}",
                    cli.display()
                );
            }
        }
    }

    // Seed the common dev-command allowlist preset. Independent of broker
    // status (per design D4) — non-broker supervisor sessions also benefit
    // from suppressed dev-loop prompts.
    if supervisor_cfg.common_dev_allowlist.enabled {
        for (path, err) in git_paw::supervisor::dev_allowlist::seed_supervisor_session(
            &supervisor_cfg.common_dev_allowlist.stacks,
            &supervisor_cfg.common_dev_allowlist.extra,
            repo_root,
            &configured_settings_paths(config),
        ) {
            eprintln!(
                "warning: failed to seed dev allowlist into {}: {err}",
                path.display(),
            );
        }
    }

    // Collect the distinct spec backends for this session so the
    // supervisor skill can render `{{SPEC_PATH_DOCTRINE}}` per backend.
    // Empty when branches came from `--branches` (no spec scan) — the
    // doctrine placeholder then renders the no-backend sentinel.
    let session_backends: Vec<git_paw::specs::SpecBackendKind> = {
        let mut seen: Vec<git_paw::specs::SpecBackendKind> = Vec::new();
        for entry in spec_by_branch.values() {
            if !seen.contains(&entry.backend) {
                seen.push(entry.backend);
            }
        }
        seen
    };

    // Resolve and materialise the supervisor skill into the repo-root
    // AGENTS.md BEFORE pane 0 starts the supervisor CLI. The supervisor pane
    // launches from `repo_root` so Claude reads this file as its skill.
    let supervisor_skill_template = git_paw::skills::resolve("supervisor")?;
    let supervisor_md = git_paw::skills::render(
        &supervisor_skill_template,
        "supervisor",
        &broker_config.url(),
        &project,
        &supervisor_cfg.gate_commands(),
        &session_backends,
    );
    let supervisor_assignment = git_paw::agents::WorktreeAssignment {
        branch: "supervisor".to_string(),
        cli: supervisor_cli.clone(),
        spec_content: None,
        owned_files: None,
        skill_content: Some(supervisor_md),
        inter_agent_rules: None,
    };
    git_paw::agents::setup_worktree_agents_md(repo_root, repo_root, &supervisor_assignment)?;

    // Resolve the coordination skill once for all agent panes.
    let coordination_template = if broker_config.enabled {
        Some(git_paw::skills::resolve("coordination")?)
    } else {
        None
    };

    // Build the inter-agent rules block for this session.
    let branch_refs: Vec<&str> = branches.iter().map(String::as_str).collect();
    let inter_agent_rules = git_paw::agents::build_inter_agent_rules(&branch_refs);

    let repo_str = repo_root.to_string_lossy().to_string();
    let dashboard_command = format!(
        "{} __dashboard",
        std::env::current_exe()
            .unwrap_or_else(|_| std::path::PathBuf::from("git-paw"))
            .display()
    );

    let supervisor_cli_command = if supervisor_flags.is_empty() {
        supervisor_cli.clone()
    } else {
        format!("{supervisor_cli} {supervisor_flags}")
    };

    let supervisor_pane = tmux::PaneSpec {
        branch: "supervisor".to_string(),
        worktree: repo_str.clone(),
        cli_command: supervisor_cli_command,
    };
    let dashboard_pane = tmux::PaneSpec {
        branch: "dashboard".to_string(),
        worktree: repo_str,
        cli_command: dashboard_command,
    };

    // Pre-compute per-agent panes, prompts, and worktree records.
    let mut agent_panes: Vec<tmux::PaneSpec> = Vec::with_capacity(branches.len());
    let mut agent_prompts: Vec<String> = Vec::with_capacity(branches.len());
    let mut worktree_entries: Vec<WorktreeEntry> = Vec::with_capacity(branches.len());

    // Per-agent setup is delegated to the shared `attach_agent` pipeline
    // (design D2, task 1.2) so a `git paw add`-attached agent is byte-identical
    // to a start-time one. The context is built once and reused for every
    // branch; the combined boot+task prompt `attach_agent` returns becomes the
    // agent's first message after attach (it points the agent at the gitignored
    // sidecar `.git-paw/AGENTS.local.md`, which `setup_worktree_agents_md` has
    // already populated with the combined spec + assignment view — see
    // `build_task_prompt`).
    let strict_guard = config
        .supervisor
        .as_ref()
        .is_none_or(SupervisorConfig::strict_branch_guard);
    let gate_commands = supervisor_cfg.gate_commands();
    let attach_ctx = AttachContext {
        repo_root,
        project: &project,
        broker_config: &broker_config,
        agent_cli: &agent_cli,
        agent_flags,
        coordination_template: coordination_template.as_ref(),
        gate_commands: &gate_commands,
        session_backends: &session_backends,
        inter_agent_rules: Some(inter_agent_rules.as_str()),
        strict_guard,
        no_rebase,
        placement: config.worktree_placement(),
    };

    for branch in &branches {
        let attached = attach_agent(&attach_ctx, branch, spec_by_branch.get(branch))?;
        agent_panes.push(attached.pane);
        agent_prompts.push(attached.prompt);
        worktree_entries.push(attached.entry);
    }

    let env_vars: Vec<(String, String)> = if broker_config.enabled {
        vec![("GIT_PAW_BROKER_URL".to_string(), broker_config.url())]
    } else {
        Vec::new()
    };

    let tmux_session = tmux::build_supervisor_session(
        &project,
        Some(session_name.clone()),
        &supervisor_pane,
        &dashboard_pane,
        &agent_panes,
        layout,
        mouse,
        config.border_affordances_enabled(),
        &env_vars,
    )?;
    tmux_session.execute()?;

    // Rebalance each agent row to equal width on the live window (design D4,
    // G3): the raw `split-window -h` chain renders a row as 50/25/25, so a
    // column-precise resize evens it out now that the panes exist.
    if let Err(e) = tmux::rebalance_agent_rows(&tmux_session.name, agent_panes.len()) {
        eprintln!("warning: could not rebalance agent-row widths: {e}");
    }

    // Save session state so `git paw status`/`stop`/`purge` see the session.
    let mut state = Session {
        session_name: tmux_session.name.clone(),
        repo_path: repo_root.to_path_buf(),
        project_name: project.clone(),
        created_at: SystemTime::now(),
        status: SessionStatus::Active,
        worktrees: worktree_entries,
        broker_port: None,
        broker_bind: None,
        broker_log_path: None,
        mode: SessionMode::Supervisor,
        dashboard_pane: None,
    };
    if broker_config.enabled {
        state.broker_port = Some(broker_config.port);
        state.broker_bind = Some(broker_config.bind.clone());
        state.broker_log_path = Some(session::session_state_dir()?.join("broker.log"));
        state.dashboard_pane = Some(1);
    }
    session::save_session(&state)?;

    // Write the per-repo discovery file sweep.sh reads. Coding agents start
    // at SUPERVISOR_PANE_OFFSET (supervisor pane 0, dashboard pane 1).
    write_repo_discovery_file(
        repo_root,
        &tmux_session.name,
        &state.worktrees,
        git_paw::supervisor::layout::SUPERVISOR_PANE_OFFSET,
    );

    // Inject the initial prompt into the supervisor pane (index 0) and each
    // coding agent pane (indices 2..N+1). The dashboard pane (index 1) is a
    // TUI process and does NOT receive a send-keys prompt.
    //
    // Instead of a blind fixed sleep, each pane's boot block is gated on
    // observed CLI readiness (design D1, G1): `gate_pane_for_injection` polls
    // the pane for its CLI's interactive marker — relaunching a still-bare
    // shell, and falling back to injection after the budget for an
    // unrecognised CLI — so the multi-line boot block is never typed into a
    // bare shell (the v0.8.0 G1 failure).
    //
    // A single Enter is sent per pane — on paste-aware CLIs (Claude Code
    // v2.1.x) this leaves the prompt in a paste-buffer state which is then
    // recovered by the supervisor agent via the paste-buffer-recovery skill
    // (see assets/agent-skills/supervisor.md). Sending more than one Enter
    // at launch risks accidentally accepting a follow-on permission prompt
    // on fast CLIs and is intentionally avoided.
    let supervisor_boot_block =
        git_paw::skills::build_boot_block("supervisor", &broker_config.url());
    let supervisor_framing = format!(
        "Begin observing the spec implementation session. Your skill \
         ({skill}) describes your role — read it, then start the autonomous loop. The user \
         can type questions or directives directly into your pane; handle them per the 'When \
         the user types in your pane' section of your skill.",
        skill = git_paw::agents::SIDECAR_REL_PATH,
    );
    let supervisor_prompt = format!("{supervisor_boot_block}\n\n{supervisor_framing}");
    let supervisor_delay = resolve_submit_delay_ms(&supervisor_cli, config);
    let _ = tmux::gate_pane_for_injection(&tmux_session.name, 0, &supervisor_pane.cli_command);
    submit_prompt_to_pane(&tmux_session.name, 0, &supervisor_prompt, supervisor_delay);

    let agent_delay = resolve_submit_delay_ms(&agent_cli, config);
    for (idx, prompt) in agent_prompts.iter().enumerate() {
        let pane_idx = git_paw::supervisor::layout::SUPERVISOR_PANE_OFFSET + idx;
        let _ = tmux::gate_pane_for_injection(
            &tmux_session.name,
            pane_idx,
            &agent_panes[idx].cli_command,
        );
        submit_prompt_to_pane(&tmux_session.name, pane_idx, prompt, agent_delay);
    }

    // Supervisor self-registration is published from inside the supervisor
    // pane itself (via the embedded supervisor skill's bootstrap curl).
    // The launcher does not publish on the supervisor's behalf so aborted
    // launches do not leave a phantom supervisor row on the dashboard
    // (D1 of supervisor-as-pane-followups).

    // Learnings-mode privacy disclosure: when the user has opted into
    // learnings (`[supervisor] learnings = true`), surface where the local
    // file is written, that nothing leaves the machine, and how to optionally
    // share it. Prints exactly when opted in and is silent otherwise so a
    // non-learnings session's output is unchanged.
    if let Some(notice) = learnings_disclosure_notice(config.supervisor.as_ref()) {
        println!("{notice}");
    }

    println!(
        "Supervisor session '{}' launched with {} coding agent(s).",
        tmux_session.name,
        branches.len()
    );
    println!("Attach with:  tmux attach -t {}", tmux_session.name);
    Ok(())
}

/// Canonical GitHub issues URL for optional learnings sharing. Tracks the
/// `repository` field in `Cargo.toml` and the README links so the disclosure
/// notice never drifts to a stale repo location.
const GIT_PAW_ISSUES_URL: &str = "https://github.com/bearicorn/git-paw/issues";

/// Build the session-start learnings privacy disclosure notice.
///
/// Returns `Some(notice)` only when the resolved supervisor config has both
/// `enabled` and `learnings` set — mirroring the aggregator's attach predicate
/// so the notice appears exactly when learnings output is actually produced.
/// Returns `None` when learnings is disabled or the `[supervisor]` section is
/// absent, so a session that has not opted in prints no extra output.
///
/// The notice states (a) the local `.git-paw/session-learnings.md` path,
/// (b) that no telemetry is performed / nothing is sent anywhere, and (c) the
/// optional-share-via-GitHub-issue invitation with the review-and-anonymise
/// caveat. git-paw never scrubs the file itself — only the user knows what is
/// repo-sensitive — so the guidance is advisory.
#[must_use]
fn learnings_disclosure_notice(supervisor: Option<&SupervisorConfig>) -> Option<String> {
    supervisor.filter(|s| s.enabled && s.learnings)?;
    Some(format!(
        "Learnings mode is on. Friction signals are written locally to \
         .git-paw/session-learnings.md — no telemetry, nothing is sent anywhere.\n\
         If a recurring rough edge is worth fixing in git-paw, you can optionally \
         share that file by opening an issue at {GIT_PAW_ISSUES_URL} — review it \
         first and strip or anonymise any repo-specific details (branch names, \
         file paths, spec IDs); your own LLM can help with that."
    ))
}

/// Resolve the boot-prompt settle delay (ms) for `cli` from config,
/// falling back to [`git_paw::DEFAULT_SUBMIT_DELAY_MS`].
///
/// `cli` may carry flags (e.g. `"mycli --foo"`); the lookup keys on the
/// leading binary token. The delay is config-driven, never a hardcoded
/// CLI-name table, so the launcher stays CLI-agnostic — a CLI whose
/// large-paste handling needs more time sets `[clis.<name>].submit_delay_ms`
/// rather than requiring a code change (W15-1, 2026-05-31 dogfood).
#[must_use]
fn resolve_submit_delay_ms(cli: &str, config: &git_paw::config::PawConfig) -> u64 {
    let base = cli.split_whitespace().next().unwrap_or(cli);
    config
        .clis
        .get(base)
        .and_then(|c| c.submit_delay_ms)
        .unwrap_or(git_paw::DEFAULT_SUBMIT_DELAY_MS)
}

/// Distinct config-declared `settings_path` files for the session's CLIs
/// (supervisor + agents), expanded and filtered to those whose parent
/// directory already exists.
///
/// CLI-agnostic: only a CLI with `[clis.<name>].settings_path` set
/// contributes a path; built-in CLIs (no custom entry) contribute nothing
/// here — the repo-local `.claude/settings.json` is seeded separately. The
/// parent-exists gate means git-paw never creates a CLI's config dir
/// (matching the dev-allowlist seeder's caution).
fn session_cli_settings_paths(
    config: &git_paw::config::PawConfig,
    supervisor_cli: &str,
    agent_cli: &str,
) -> Vec<std::path::PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for cli in [supervisor_cli, agent_cli] {
        let base = cli.split_whitespace().next().unwrap_or(cli);
        if let Some(raw) = config
            .clis
            .get(base)
            .and_then(|c| c.settings_path.as_deref())
        {
            let expanded = expand_tilde(raw);
            let parent_exists = expanded.parent().is_some_and(std::path::Path::is_dir);
            if parent_exists && seen.insert(expanded.clone()) {
                out.push(expanded);
            }
        }
    }
    out
}

/// Every configured `[clis.<name>].settings_path` (tilde-expanded) whose
/// parent directory already exists, deduplicated.
///
/// Used to seed the dev-command allowlist into each registered CLI's
/// alternate settings file in a CLI-agnostic way — there is no hardcoded
/// path. The parent-exists filter preserves the "never create the directory"
/// guarantee: a configured target whose parent is absent is skipped rather
/// than created.
fn configured_settings_paths(config: &git_paw::config::PawConfig) -> Vec<std::path::PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for custom in config.clis.values() {
        if let Some(raw) = custom.settings_path.as_deref() {
            let expanded = expand_tilde(raw);
            let parent_exists = expanded.parent().is_some_and(std::path::Path::is_dir);
            if parent_exists && seen.insert(expanded.clone()) {
                out.push(expanded);
            }
        }
    }
    out
}

/// Expand a leading `~` / `~/` in `path` to the home directory.
fn expand_tilde(path: &str) -> std::path::PathBuf {
    match git_paw::dirs::home_dir() {
        Some(home) if path == "~" => home,
        Some(home) => match path.strip_prefix("~/") {
            Some(rest) => home.join(rest),
            None => std::path::PathBuf::from(path),
        },
        None => std::path::PathBuf::from(path),
    }
}

/// Inject and submit an initial prompt into a tmux pane.
///
/// The boot block is injected literally, then — after `delay_ms` for a
/// paste-aware CLI to settle the (often large) paste — a separate `Enter`
/// submits it. Splitting the inject from the submit (rather than a
/// same-call trailing `Enter`) is what reliably submits a large paste
/// across CLIs (W15-1). `delay_ms` is resolved per-CLI from config so this
/// path carries no CLI-specific assumptions. Failures are swallowed —
/// best-effort by design.
fn submit_prompt_to_pane(session_name: &str, pane_idx: usize, prompt: &str, delay_ms: u64) {
    let target = format!("{session_name}:0.{pane_idx}");
    // 1. Inject the boot block literally (no Enter yet).
    let _ = std::process::Command::new("tmux")
        .args(["send-keys", "-t", &target, "-l", prompt])
        .status();
    // 2. Let a paste-aware CLI settle the paste before we submit.
    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    // 3. Submit with a separate Enter.
    let _ = std::process::Command::new("tmux")
        .args(["send-keys", "-t", &target, "Enter"])
        .status();
}

/// Writes the per-repo discovery file (`<repo>/.git-paw/sessions/<name>.json`)
/// the bundled `sweep.sh` helper reads (capability `session-json-location`).
///
/// Builds the sweep.sh-compatible agent roster from the launched worktrees:
/// each entry carries the broker agent id (`branch_id`), worktree path, CLI,
/// and the tmux `pane_index`. `pane_offset` is the index of the first coding
/// agent pane — `SUPERVISOR_PANE_OFFSET` for the supervisor layout, or
/// `1`/`0` for the bare layout depending on whether the dashboard pane is
/// present. Best-effort: a write failure is surfaced as a warning and does
/// not abort the launch, since the global receipt remains the source of truth.
fn write_repo_discovery_file(
    repo_root: &Path,
    session_name: &str,
    worktrees: &[WorktreeEntry],
    pane_offset: usize,
) {
    let agents = worktrees
        .iter()
        .enumerate()
        .map(|(idx, wt)| session::RepoAgentEntry {
            branch_id: git_paw::broker::messages::slugify_branch(&wt.branch),
            worktree_path: wt.worktree_path.clone(),
            cli: wt.cli.clone(),
            pane_index: pane_offset + idx,
        })
        .collect();
    let file = session::RepoSessionFile {
        session_name: session_name.to_string(),
        agents,
    };
    if let Err(e) = session::write_repo_session_file(repo_root, &file) {
        eprintln!("warning: failed to write per-repo session discovery file: {e}");
    }
}

// ---------------------------------------------------------------------------
// Command: start --from-specs
// ---------------------------------------------------------------------------

/// Launches sessions from spec files instead of interactive branch selection.
fn cmd_start_with_specs(
    cli_flag: Option<&str>,
    spec_mode: &SpecMode,
    specs_format_override: Option<&str>,
    dry_run: bool,
    force: bool,
    no_rebase: bool,
) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    // Check for existing session (skip reattach/recovery during dry-run).
    // Before deciding reattach-vs-recover, probe the receipt for staleness:
    // a receipt claiming `active` whose tmux session has vanished is
    // invalidated (purged) here and the launch proceeds fresh (design D5).
    let existing_session = session::find_session_for_repo(&repo_root)?;
    if !dry_run
        && let Some(existing) = &existing_session
        && !invalidate_if_stale(&repo_root, existing)?
    {
        let effective =
            existing.effective_status(|name| tmux::is_session_alive(name).unwrap_or(false));
        match effective {
            SessionStatus::Paused => {
                println!(
                    "Restarting paused session '{}' (broker + reattach)...",
                    existing.session_name
                );
                return restart_from_pause(&repo_root, existing);
            }
            SessionStatus::Active => {
                println!("Reattaching to session '{}'...", existing.session_name);
                return attach_or_print_hint(&existing.session_name);
            }
            SessionStatus::Stopped => {
                println!("Recovering session '{}'...", existing.session_name);
                return recover_session(&repo_root, existing);
            }
        }
    }

    // Fresh launch from specs (or dry-run preview)
    tmux::ensure_tmux_installed()?;
    let config = config::load_config(&repo_root, None)?;

    // Scan for pending specs (honouring `--specs-format` override), then
    // apply the spec-mode filter (Picker / Narrow). For SpecMode::All, the
    // filter is the identity transform.
    let discovered =
        git_paw::specs::scan_specs_with_override(&config, &repo_root, specs_format_override)?;
    if discovered.is_empty() {
        println!("No pending specs found.");
        return Ok(());
    }
    let specs = apply_spec_mode(spec_mode, discovered, &interactive::TerminalPrompter)?;
    if specs.is_empty() {
        println!("No pending specs found.");
        return Ok(());
    }

    // Check for uncommitted spec changes unless force flag is used
    let uncommitted_specs = git::check_uncommitted_specs(&repo_root, &specs)?;
    if !uncommitted_specs.is_empty() && !force {
        eprintln!(
            "warning: Uncommitted spec changes detected in: {}\n       Commit your changes or use --force to proceed",
            uncommitted_specs.join(", ")
        );
    } else if !uncommitted_specs.is_empty() && force {
        eprintln!("Proceeding with --force flag (uncommitted spec changes ignored)");
    }

    // Detect available CLIs
    let custom_defs = config_to_custom_defs(&config);
    let detected = detect::detect_clis(&custom_defs);
    if detected.is_empty() {
        return Err(PawError::NoCLIsFound);
    }

    // Resolve CLI assignments for specs
    let interactive_clis: Vec<interactive::CliInfo> =
        detected.iter().map(to_interactive_cli).collect();
    let prompter = interactive::TerminalPrompter;
    let mappings = interactive::resolve_cli_for_specs(
        &specs,
        cli_flag,
        &config,
        &interactive_clis,
        &prompter,
    )?;

    // Build a lookup from branch to spec for prompt/owned_files
    let spec_by_branch: std::collections::HashMap<&str, &git_paw::specs::SpecEntry> =
        specs.iter().map(|s| (s.branch.as_str(), s)).collect();

    let project = git::project_name(&repo_root);
    let mouse = config.mouse.unwrap_or(true);
    let session_name = tmux::resolve_session_name(&project)?;

    // Dry run — print plan and exit
    if dry_run {
        if let Some(ref existing) = existing_session {
            eprintln!(
                "warning: session '{}' already exists — purge it before starting a new one\n",
                existing.session_name
            );
        }
        println!("Dry run — session plan (from specs):\n");
        println!("  Session:  {session_name}");
        println!("  Mouse:    {}", if mouse { "on" } else { "off" });
        println!();
        for (branch, cli) in &mappings {
            let wt_dir = git::worktree_dir_name(&project, branch);
            println!("  {branch} \u{2192} {cli} (../{wt_dir})");
        }
        return Ok(());
    }

    launch_spec_session(
        &repo_root,
        &config,
        &mappings,
        &spec_by_branch,
        &project,
        mouse,
        no_rebase,
    )
}

/// Creates worktrees, sets up AGENTS.md, builds the tmux session, and attaches.
#[allow(clippy::too_many_lines)]
fn launch_spec_session(
    repo_root: &std::path::Path,
    config: &PawConfig,
    mappings: &[(String, String)],
    spec_by_branch: &std::collections::HashMap<&str, &git_paw::specs::SpecEntry>,
    project: &str,
    mouse: bool,
    no_rebase: bool,
) -> Result<(), PawError> {
    let session_name = tmux::resolve_session_name(project)?;

    // Prune stale worktree registrations from previous sessions
    git::prune_worktrees(repo_root)?;

    let broker_config = config.broker.clone();

    let mut builder = tmux::TmuxSessionBuilder::new(project)
        .session_name(session_name)
        .mouse_mode(mouse)
        .border_affordances(config.border_affordances_enabled());

    // Broker: inject dashboard pane and environment variable
    if broker_config.enabled {
        let repo_str = repo_root.to_string_lossy().to_string();
        builder = builder.add_pane(tmux::PaneSpec {
            branch: "dashboard".to_string(),
            worktree: repo_str,
            cli_command: format!(
                "{} __dashboard",
                std::env::current_exe()
                    .unwrap_or_else(|_| std::path::PathBuf::from("git-paw"))
                    .display()
            ),
        });
        builder = builder.set_environment("GIT_PAW_BROKER_URL", &broker_config.url());
    }

    // Resolve coordination skill once if broker is enabled
    let skill_template = if broker_config.enabled {
        Some(git_paw::skills::resolve("coordination")?)
    } else {
        None
    };

    let mut worktree_entries = Vec::new();

    // Collect the distinct spec backends present in this session so
    // coordination renders embed the right `{{SPEC_PATH_DOCTRINE}}` if
    // they ever start referencing it. Today coordination.md does not,
    // but plumbing the value avoids a future drift.
    let session_backends: Vec<git_paw::specs::SpecBackendKind> = {
        let mut seen: Vec<git_paw::specs::SpecBackendKind> = Vec::new();
        for entry in spec_by_branch.values() {
            if !seen.contains(&entry.backend) {
                seen.push(entry.backend);
            }
        }
        seen
    };

    for (branch, cli) in mappings {
        let wt = git::create_worktree(repo_root, branch, !no_rebase, config.worktree_placement())?;
        let wt_str = wt.path.to_string_lossy().to_string();

        // Set up AGENTS.md with spec + skill content
        let rendered_skill = skill_template.as_ref().map(|tmpl| {
            git_paw::skills::render(
                tmpl,
                branch,
                &broker_config.url(),
                project,
                &config
                    .supervisor
                    .as_ref()
                    .map(|s| s.gate_commands())
                    .unwrap_or_default(),
                &session_backends,
            )
        });

        let spec_content = spec_by_branch
            .get(branch.as_str())
            .map(|s| s.prompt.clone());
        let owned_files = spec_by_branch
            .get(branch.as_str())
            .and_then(|s| s.owned_files.clone());

        let assignment = git_paw::agents::WorktreeAssignment {
            branch: branch.clone(),
            cli: cli.clone(),
            spec_content,
            owned_files,
            skill_content: rendered_skill,
            inter_agent_rules: None,
        };
        git_paw::agents::setup_worktree_agents_md(repo_root, &wt.path, &assignment)?;

        if broker_config.enabled {
            let agent_id = git_paw::broker::messages::slugify_branch(branch);
            let strict_guard = config
                .supervisor
                .as_ref()
                .is_none_or(SupervisorConfig::strict_branch_guard);
            git_paw::agents::install_git_hooks(
                &wt.path,
                &broker_config.url(),
                &agent_id,
                branch,
                strict_guard,
            )?;
        }

        builder = builder.add_pane(tmux::PaneSpec {
            branch: branch.clone(),
            worktree: wt_str,
            cli_command: cli.clone(),
        });

        worktree_entries.push(WorktreeEntry {
            branch: branch.clone(),
            worktree_path: wt.path,
            cli: cli.clone(),
            branch_created: wt.branch_created,
            pending_boot_prompt: None,
        });
    }

    let mut tmux_session = builder.build()?;

    // Set up logging if enabled — pane indices shift by 1 when broker is enabled
    if config.logging.as_ref().is_some_and(|l| l.enabled) {
        let pane_offset = usize::from(broker_config.enabled);
        git_paw::logging::ensure_log_dir(repo_root, &tmux_session.name)?;
        for (i, (branch, _)) in mappings.iter().enumerate() {
            let log_path = git_paw::logging::log_file_path(repo_root, &tmux_session.name, branch);
            let pane_target = format!("{}:{}.{}", tmux_session.name, 0, i + pane_offset);
            tmux_session.pipe_pane(&pane_target, &log_path);
        }
    }

    // Execute tmux session
    tmux_session.execute()?;

    // Inject broker boot blocks per pane (mirrors cmd_start; cmd_start_from_specs
    // was missing this in v0.4 — fixes dogfood D4 in `from-specs-launch-fixes`).
    if broker_config.enabled {
        let pane_offset = usize::from(broker_config.enabled);
        for (idx, (branch, _)) in mappings.iter().enumerate() {
            let pane_idx = idx + pane_offset;
            let boot_block = git_paw::skills::build_boot_block(branch, &broker_config.url());
            let args =
                git_paw::tmux::build_boot_inject_args(&tmux_session.name, pane_idx, &boot_block);
            let _ = std::process::Command::new("tmux").args(&args).status();
        }
    }

    // Save session state
    let mut state = Session {
        session_name: tmux_session.name.clone(),
        repo_path: repo_root.to_path_buf(),
        project_name: project.to_string(),
        created_at: SystemTime::now(),
        status: SessionStatus::Active,
        worktrees: worktree_entries,
        broker_port: None,
        broker_bind: None,
        broker_log_path: None,
        mode: SessionMode::Bare,
        dashboard_pane: None,
    };

    if broker_config.enabled {
        state.broker_port = Some(broker_config.port);
        state.broker_bind = Some(broker_config.bind.clone());
        state.broker_log_path = Some(session::session_state_dir()?.join("broker.log"));
        state.dashboard_pane = Some(0);
    }

    session::save_session(&state)?;

    attach_or_print_hint(&tmux_session.name)
}

/// Restarts a paused session: recreates the dashboard pane (re-spawning
/// the broker), updates status to `Active`, and re-attaches the user's
/// client. Skips worktree creation, CLI spawning, and boot-prompt
/// injection — coding-agent panes are already running and retain their
/// in-memory conversation state.
fn restart_from_pause(repo_root: &Path, existing: &Session) -> Result<(), PawError> {
    tmux::ensure_tmux_installed()?;

    let dashboard_index = existing.dashboard_pane.unwrap_or(0);

    // Recreate the dashboard pane only when the original session had a
    // broker enabled. Without a broker there is no dashboard pane to
    // recreate; pause+resume on a no-broker session is purely the tmux
    // detach/attach cycle.
    if existing.broker_port.is_some() {
        let dashboard_command = format!(
            "{} __dashboard",
            std::env::current_exe()
                .unwrap_or_else(|_| std::path::PathBuf::from("git-paw"))
                .display()
        );
        let repo_str = repo_root.to_string_lossy().to_string();
        // Anchor the new pane on the first agent pane (which still
        // exists in the live session); -b places the dashboard before
        // it, mirroring the original layout. The pane index in tmux is
        // not directly addressable for spawn — tmux assigns the next
        // available index — so this leaves the dashboard at whatever
        // index tmux picks. The session-state field remains a hint for
        // future restarts; we re-write it below to reflect reality.
        let split_status = StdCommand::new("tmux")
            .args([
                "split-window",
                "-h",
                "-b",
                "-t",
                &format!("{}:0.{dashboard_index}", existing.session_name),
                "-c",
                &repo_str,
            ])
            .status()
            .map_err(|e| PawError::TmuxError(format!("failed to spawn dashboard pane: {e}")))?;
        if !split_status.success() {
            return Err(PawError::TmuxError(
                "failed to recreate dashboard pane".to_string(),
            ));
        }
        // The new pane is the focused pane; target it via :0.{dashboard_index}.
        let target = format!("{}:0.{dashboard_index}", existing.session_name);
        // Title matches the per-pane labelling scheme: the pane's role only,
        // rendered in the `pane-border-format` strip the session already has.
        let _ = StdCommand::new("tmux")
            .args(["select-pane", "-t", &target, "-T", "dashboard"])
            .status();
        let send_status = StdCommand::new("tmux")
            .args(["send-keys", "-t", &target, &dashboard_command, "Enter"])
            .status()
            .map_err(|e| PawError::TmuxError(format!("failed to send dashboard command: {e}")))?;
        if !send_status.success() {
            return Err(PawError::TmuxError(
                "failed to send dashboard command".to_string(),
            ));
        }
    }

    // Update session state: status flips back to Active.
    let mut updated = existing.clone();
    updated.status = SessionStatus::Active;

    // Submit boot prompts held for agents added while the session was paused
    // (design D4 of git-paw-add): they were registered with the pane created
    // but their prompt left unsubmitted; resume starts them alongside the
    // rest. Existing (pre-pause) agents carry no pending prompt and are left
    // untouched — they continue their in-flight conversations on reattach.
    let has_pending = updated
        .worktrees
        .iter()
        .any(|w| w.pending_boot_prompt.is_some());
    if has_pending {
        let session_name = updated.session_name.clone();
        let offset = agent_pane_offset(&updated);
        let config = config::load_config(repo_root, None)?;
        for (idx, wt) in updated.worktrees.iter_mut().enumerate() {
            if let Some(pending) = wt.pending_boot_prompt.take() {
                let delay = resolve_submit_delay_ms(&wt.cli, &config);
                submit_prompt_to_pane(&session_name, offset + idx, &pending, delay);
            }
        }
    }

    session::save_session(&updated)?;

    attach_or_print_hint(&existing.session_name)
}

/// Recovers a stopped/stale session by recreating the tmux session from saved state.
fn recover_session(repo_root: &Path, existing: &Session) -> Result<(), PawError> {
    tmux::ensure_tmux_installed()?;
    let config = config::load_config(repo_root, None)?;
    let mouse = config.mouse.unwrap_or(true);

    // Detect supervisor mode: explicit marker on the saved session wins; if
    // missing AND config currently has supervisor enabled, this is a v0.4
    // session being recovered with v0.5 layout — warn and proceed.
    let supervisor_enabled_in_config = config.supervisor.as_ref().is_some_and(|s| s.enabled);
    let mode = if existing.mode == SessionMode::Supervisor {
        SessionMode::Supervisor
    } else if supervisor_enabled_in_config {
        eprintln!(
            "warning: session '{}' was created with a v0.4 layout but [supervisor] is enabled \
             in current config — rebuilding with v0.5 supervisor-as-pane layout.",
            existing.session_name
        );
        SessionMode::Supervisor
    } else {
        SessionMode::Bare
    };

    let broker_url = existing
        .broker_port
        .zip(existing.broker_bind.as_ref())
        .map(|(port, bind)| format!("http://{bind}:{port}"));

    if broker_url.is_some() {
        // Re-populate the broker-helper allowlist when recovering — a
        // re-attached session must carry the helper-path grant so the agents'
        // first `broker.sh` invocation does not re-trigger a permission prompt.
        let claude_settings = repo_root.join(".claude").join("settings.json");
        if let Err(e) = git_paw::supervisor::curl_allowlist::setup_curl_allowlist(&claude_settings)
        {
            eprintln!("warning: failed to setup broker-helper allowlist: {e}");
        }
    }

    // Re-seed the dev allowlist on recovery so re-attached sessions pick up
    // preset updates. Only runs for supervisor mode with the feature enabled;
    // broker status does not gate this (design D4).
    if mode == SessionMode::Supervisor
        && let Some(supervisor_cfg) = config.supervisor.as_ref()
        && supervisor_cfg.common_dev_allowlist.enabled
    {
        for (path, err) in git_paw::supervisor::dev_allowlist::seed_supervisor_session(
            &supervisor_cfg.common_dev_allowlist.stacks,
            &supervisor_cfg.common_dev_allowlist.extra,
            repo_root,
            &configured_settings_paths(&config),
        ) {
            eprintln!(
                "warning: failed to seed dev allowlist into {}: {err}",
                path.display(),
            );
        }
    }

    // Tear down any stale tmux session of this name before rebuilding so the
    // recovery starts from a clean `new-session`. A half-built session left
    // by a prior crashed/aborted launch would otherwise let the rebuild's
    // `split-window` commands accumulate panes on top of it, overflowing the
    // window (W2-3: a 4-worktree recovery produced 10-11 panes and
    // `no space for new pane`). Killing a non-existent session is a no-op here.
    if tmux::is_session_alive(&existing.session_name).unwrap_or(false)
        && let Err(e) = tmux::kill_session(&existing.session_name)
    {
        eprintln!(
            "warning: could not tear down stale tmux session '{}' before recovery: {e}",
            existing.session_name
        );
    }

    let tmux_session = match mode {
        SessionMode::Supervisor => {
            recover_supervisor_session(repo_root, existing, &config, broker_url.as_deref(), mouse)?
        }
        SessionMode::Bare => recover_bare_session(
            repo_root,
            existing,
            broker_url.as_deref(),
            mouse,
            config.border_affordances_enabled(),
        )?,
    };
    tmux_session.execute()?;

    // Update session status + record the resolved mode.
    let mut updated = existing.clone();
    updated.status = SessionStatus::Active;
    updated.mode = mode;
    session::save_session(&updated)?;

    attach_or_print_hint(&tmux_session.name)
}

/// Rebuild a bare-mode (non-supervisor) session: dashboard at pane 0 (when
/// broker enabled), coding agents at pane 1+.
fn recover_bare_session(
    repo_root: &Path,
    existing: &Session,
    broker_url: Option<&str>,
    mouse: bool,
    border_affordances: bool,
) -> Result<tmux::TmuxSession, PawError> {
    let mut builder = tmux::TmuxSessionBuilder::new(&existing.project_name)
        .session_name(existing.session_name.clone())
        .mouse_mode(mouse)
        .border_affordances(border_affordances);

    if let Some(url) = broker_url {
        let repo_str = repo_root.to_string_lossy().to_string();
        builder = builder.add_pane(tmux::PaneSpec {
            branch: "dashboard".to_string(),
            worktree: repo_str,
            cli_command: format!(
                "{} __dashboard",
                std::env::current_exe()
                    .unwrap_or_else(|_| std::path::PathBuf::from("git-paw"))
                    .display()
            ),
        });
        builder = builder.set_environment("GIT_PAW_BROKER_URL", url);
    }

    for entry in &existing.worktrees {
        builder = builder.add_pane(tmux::PaneSpec {
            branch: entry.branch.clone(),
            worktree: entry.worktree_path.to_string_lossy().to_string(),
            cli_command: entry.cli.clone(),
        });
    }

    builder.build()
}

/// Rebuild a supervisor-mode session with the v0.5 layout: supervisor at pane
/// 0, dashboard at pane 1, coding agents at pane 2+.
fn recover_supervisor_session(
    repo_root: &Path,
    existing: &Session,
    config: &PawConfig,
    broker_url: Option<&str>,
    mouse: bool,
) -> Result<tmux::TmuxSession, PawError> {
    // Fall back to a default config when [supervisor] is absent so a
    // re-launched session does not error on a config the prior boot
    // already accepted.
    let default_supervisor_cfg = SupervisorConfig::default();
    let supervisor_cfg = config
        .supervisor
        .as_ref()
        .unwrap_or(&default_supervisor_cfg);
    let supervisor_cli = supervisor_cfg
        .cli
        .clone()
        .or_else(|| config.default_cli.clone())
        .ok_or_else(|| {
            PawError::ConfigError(
                "supervisor recovery requires either [supervisor].cli or default_cli to be set"
                    .to_string(),
            )
        })?;
    let supervisor_flags = config::approval_flags(&supervisor_cli, &supervisor_cfg.agent_approval);
    let supervisor_cli_command = if supervisor_flags.is_empty() {
        supervisor_cli
    } else {
        format!("{supervisor_cli} {supervisor_flags}")
    };

    let layout = git_paw::supervisor::layout::supervisor_layout(existing.worktrees.len())?;

    let repo_str = repo_root.to_string_lossy().to_string();
    let supervisor_pane = tmux::PaneSpec {
        branch: "supervisor".to_string(),
        worktree: repo_str.clone(),
        cli_command: supervisor_cli_command,
    };
    let dashboard_pane = tmux::PaneSpec {
        branch: "dashboard".to_string(),
        worktree: repo_str,
        cli_command: format!(
            "{} __dashboard",
            std::env::current_exe()
                .unwrap_or_else(|_| std::path::PathBuf::from("git-paw"))
                .display()
        ),
    };
    let agent_panes: Vec<tmux::PaneSpec> = existing
        .worktrees
        .iter()
        .map(|entry| tmux::PaneSpec {
            branch: entry.branch.clone(),
            worktree: entry.worktree_path.to_string_lossy().to_string(),
            cli_command: entry.cli.clone(),
        })
        .collect();

    let env_vars: Vec<(String, String)> = broker_url
        .map(|url| vec![("GIT_PAW_BROKER_URL".to_string(), url.to_string())])
        .unwrap_or_default();

    tmux::build_supervisor_session(
        &existing.project_name,
        Some(existing.session_name.clone()),
        &supervisor_pane,
        &dashboard_pane,
        &agent_panes,
        layout,
        mouse,
        config.border_affordances_enabled(),
        &env_vars,
    )
}

// ---------------------------------------------------------------------------
// Command: add / remove (live branch-set editing)
// ---------------------------------------------------------------------------

/// Index of the first coding-agent pane in a session's tmux window.
///
/// Supervisor mode reserves pane 0 (supervisor) and pane 1 (dashboard), so
/// agents start at [`SUPERVISOR_PANE_OFFSET`](git_paw::supervisor::layout::SUPERVISOR_PANE_OFFSET).
/// Bare mode places the dashboard at pane 0 when the broker is enabled (agents
/// at pane 1), or has no dashboard pane at all (agents at pane 0).
fn agent_pane_offset(session: &Session) -> usize {
    match session.mode {
        SessionMode::Supervisor => git_paw::supervisor::layout::SUPERVISOR_PANE_OFFSET,
        SessionMode::Bare => usize::from(session.broker_port.is_some()),
    }
}

/// Error returned when add/remove is invoked on a bare-mode session.
fn bare_mode_unsupported(session_name: &str, verb: &str) -> PawError {
    PawError::SessionError(format!(
        "`git paw {verb}` supports supervisor-mode sessions (the default). Session \
         '{session_name}' was started in bare (no-supervisor) mode, whose tiled grid is \
         not re-tiled incrementally in v0.6.0. Stop and re-start with the full branch set, \
         or run the session in supervisor mode to use add/remove."
    ))
}

/// `git paw add <branch>` — hot-attach a worktree + agent pane to a running
/// session (capability `add-branch`).
#[allow(clippy::too_many_lines)]
fn cmd_add(
    branch_arg: Option<&str>,
    cli_flag: Option<&str>,
    from_spec: Option<&str>,
) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    // 4.1 Resolve the active session; error cleanly when none.
    let Some(existing) = session::find_session_for_repo(&repo_root)? else {
        return Err(PawError::SessionError(
            "no active session for this repository. Start one with `git paw start`.".to_string(),
        ));
    };

    let effective = existing.effective_status(|n| tmux::is_session_alive(n).unwrap_or(false));
    let paused = match effective {
        SessionStatus::Active => false,
        SessionStatus::Paused => true,
        SessionStatus::Stopped => {
            return Err(PawError::SessionError(format!(
                "session '{}' is stopped — recover it with `git paw start` before adding agents.",
                existing.session_name
            )));
        }
    };

    if existing.mode == SessionMode::Bare {
        return Err(bare_mode_unsupported(&existing.session_name, "add"));
    }

    tmux::ensure_tmux_installed()?;
    let config = config::load_config(&repo_root, None)?;
    let broker_config = config.broker.clone();
    let project = git::project_name(&repo_root);

    // 4.2 Resolve branch + CLI from the positional arg or --from-spec.
    let (branch, resolved_cli, spec_entry): (
        String,
        Option<String>,
        Option<git_paw::specs::SpecEntry>,
    ) = if let Some(spec_name) = from_spec {
        let discovered = git_paw::specs::scan_specs(&config, &repo_root)?;
        // resolve_specs errors with the discovered candidate list on an
        // unknown name — exactly the UX `--specs NAME` gives.
        let mut resolved =
            git_paw::specs::resolve::resolve_specs(&discovered, &[spec_name.to_string()])?;
        let spec = resolved.drain(..).next().ok_or_else(|| {
            PawError::SpecError(format!("spec '{spec_name}' resolved to no entries"))
        })?;
        let cli = cli_flag.map(str::to_string).or_else(|| spec.cli.clone());
        (spec.branch.clone(), cli, Some(spec))
    } else {
        let branch = branch_arg
            .expect("clap requires a branch when --from-spec is absent")
            .to_string();
        (branch, cli_flag.map(str::to_string), None)
    };

    if existing.worktrees.iter().any(|w| w.branch == branch) {
        return Err(PawError::SessionError(format!(
            "branch '{branch}' is already an agent in session '{}'.",
            existing.session_name
        )));
    }

    // Effective CLI: --cli > spec paw_cli > session's CLI > config default_cli.
    let session_default_cli = existing.worktrees.first().map(|w| w.cli.clone());
    let agent_cli = resolved_cli
        .or(session_default_cli)
        .or_else(|| config.default_cli.clone())
        .ok_or_else(|| {
            PawError::ConfigError(
                "no CLI specified and the session has no default to fall back to; pass --cli <id>."
                    .to_string(),
            )
        })?;

    // 4.3 Validate the CLI against detected CLIs — before mutating anything.
    // A CLI already in use by a session agent is trusted (it was accepted at
    // start, and may be a custom CLI absent from this machine's detect set),
    // so only a CLI that is neither detected nor already running is rejected.
    // This is what catches an unknown `--cli nonesuch` without breaking the
    // common "fall back to the session's CLI" path.
    let custom_defs = config_to_custom_defs(&config);
    let detected = detect::detect_clis(&custom_defs);
    let agent_cli_base = agent_cli.split_whitespace().next().unwrap_or(&agent_cli);
    let cli_in_session = existing
        .worktrees
        .iter()
        .any(|w| w.cli.split_whitespace().next() == Some(agent_cli_base));
    if !cli_in_session && !detected.iter().any(|c| c.binary_name == agent_cli_base) {
        let ids: Vec<&str> = detected.iter().map(|c| c.binary_name.as_str()).collect();
        return Err(PawError::ConfigError(format!(
            "unknown CLI '{agent_cli_base}'. Detected CLIs: {}.",
            if ids.is_empty() {
                "(none)".to_string()
            } else {
                ids.join(", ")
            }
        )));
    }

    // 4.4 Enforce the 25-agent cap BEFORE mutating. layout_for(N+1) errors with
    // the same "split into multiple sessions" message `start` uses.
    let prev_agent_count = existing.worktrees.len();
    let layout = git_paw::supervisor::layout::layout_for(prev_agent_count + 1)?;

    // 4.5 Take the advisory lock for the rest of the critical section.
    let _lock = git_paw::lock::SessionLock::acquire(&repo_root)?;

    // Build the shared attach context (mirrors cmd_supervisor's loop setup).
    let default_sup = SupervisorConfig::default();
    let supervisor_cfg = config.supervisor.as_ref().unwrap_or(&default_sup);
    let approval = &supervisor_cfg.agent_approval;
    let agent_flags = config::approval_flags(&agent_cli, approval);
    let strict_guard = config
        .supervisor
        .as_ref()
        .is_none_or(SupervisorConfig::strict_branch_guard);
    let gate_commands = supervisor_cfg.gate_commands();
    let coordination_template = if broker_config.enabled {
        Some(git_paw::skills::resolve("coordination")?)
    } else {
        None
    };
    let session_backends: Vec<git_paw::specs::SpecBackendKind> = spec_entry
        .as_ref()
        .map(|s| vec![s.backend])
        .unwrap_or_default();

    // The new agent's AGENTS.md should list every peer (existing + new) so its
    // inter-agent ownership rules reflect the full session.
    let mut all_branches: Vec<&str> = existing
        .worktrees
        .iter()
        .map(|w| w.branch.as_str())
        .collect();
    all_branches.push(branch.as_str());
    let inter_agent_rules = git_paw::agents::build_inter_agent_rules(&all_branches);

    let attach_ctx = AttachContext {
        repo_root: &repo_root,
        project: &project,
        broker_config: &broker_config,
        agent_cli: &agent_cli,
        agent_flags,
        coordination_template: coordination_template.as_ref(),
        gate_commands: &gate_commands,
        session_backends: &session_backends,
        inter_agent_rules: Some(inter_agent_rules.as_str()),
        strict_guard,
        no_rebase: false,
        placement: config.worktree_placement(),
    };

    // 4.6 Reuse create_worktree + attach_agent to build the new pane's setup.
    let AttachedAgent {
        pane,
        prompt,
        mut entry,
    } = attach_agent(&attach_ctx, &branch, spec_entry.as_ref())?;
    // Capture the new worktree's path before `entry` is moved into the
    // session, so we can register it as a live broker watch target below.
    let new_worktree_path = entry.worktree_path.clone();

    // 4.7 Recompute layout_for(N+1) and re-apply (splice the pane + re-tile).
    let offset = agent_pane_offset(&existing);
    let new_pane_idx = offset + prev_agent_count;
    tmux::build_add_agent_commands(
        &existing.session_name,
        &pane,
        prev_agent_count,
        layout,
        config.border_affordances_enabled(),
    )
    .execute()?;

    // Rebalance the (possibly newly-full) agent row to equal width on the live
    // window so the added grid matches a start-time grid width-for-width
    // (design D4, G3).
    if let Err(e) = tmux::rebalance_agent_rows(&existing.session_name, prev_agent_count + 1) {
        eprintln!("warning: could not rebalance agent-row widths: {e}");
    }

    // 4.8 Append the branch/pane entry to the session JSON.
    // 4.9 When paused, hold the boot prompt for `resume` instead of submitting.
    if paused {
        entry.pending_boot_prompt = Some(prompt.clone());
    }
    let mut updated = existing.clone();
    updated.worktrees.push(entry);
    session::save_session(&updated)?;
    write_repo_discovery_file(
        &repo_root,
        &updated.session_name,
        &updated.worktrees,
        offset,
    );

    // Register the new worktree as a live broker watch target so the watcher
    // surfaces the agent in `/status` from worktree activity, identical to a
    // start-time agent — even before its CLI self-publishes (capability
    // `broker-live-watch-registration`). Best-effort: a broker that is down
    // or predates `/watch` leaves the agent to self-register via its boot
    // block, exactly as in v0.6.0, so a failure here is logged, not fatal.
    if broker_config.enabled {
        let agent_id = broker::messages::slugify_branch(&branch);
        if let Err(e) = broker::publish::register_watch_target_http(
            &broker_config.url(),
            &agent_id,
            &new_worktree_path,
            &agent_cli,
        ) {
            eprintln!("warning: could not register '{branch}' with the broker watcher: {e}");
        }
    }

    if paused {
        println!(
            "Added '{branch}' to paused session '{}' (pane {new_pane_idx}); it will start on \
             `git paw resume`.",
            updated.session_name
        );
    } else {
        // Gate boot-block injection on observed CLI readiness, matching the
        // start path (design D1, G1): poll the new pane for its CLI's
        // interactive marker — relaunching a still-bare shell, falling back to
        // injection after the budget — instead of a blind fixed sleep.
        let _ =
            tmux::gate_pane_for_injection(&updated.session_name, new_pane_idx, &pane.cli_command);
        let delay = resolve_submit_delay_ms(&agent_cli, &config);
        submit_prompt_to_pane(&updated.session_name, new_pane_idx, &prompt, delay);
        println!(
            "Added '{branch}' to session '{}' (pane {new_pane_idx}).",
            updated.session_name
        );
    }

    // Reconcile the session JSON against the live panes after the re-tile and
    // surface any agent with no live pane (design D3, G2b) so a dropped/
    // orphaned pane is visible and recoverable rather than silent.
    let reconcile_agents: Vec<(String, std::path::PathBuf)> = updated
        .worktrees
        .iter()
        .map(|w| (w.branch.clone(), w.worktree_path.clone()))
        .collect();
    if let Ok(missing) = tmux::reconcile_agents_to_panes(&updated.session_name, &reconcile_agents)
        && !missing.is_empty()
    {
        eprintln!(
            "warning: {} agent(s) in the session JSON have no live tmux pane \
             (JSON↔tmux desync): {}. Recover with `git paw remove <branch>` then \
             `git paw add <branch>`.",
            missing.len(),
            missing.join(", ")
        );
    }

    Ok(())
}

/// `git paw remove <branch>` — detach a single agent from a running session
/// (capability `remove-branch`).
#[allow(clippy::too_many_lines)]
fn cmd_remove(branch: &str, keep_worktree: bool, force: bool) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    // 5.2 Refuse `git paw remove supervisor` with a pointer to `git paw stop`.
    if branch == "supervisor" {
        return Err(PawError::SessionError(
            "refusing to remove the supervisor. To end the whole session, run `git paw stop` \
             (or `git paw purge` to also remove worktrees)."
                .to_string(),
        ));
    }

    // 5.1 Resolve the active session and locate the target branch.
    let Some(existing) = session::find_session_for_repo(&repo_root)? else {
        return Err(PawError::SessionError(
            "no active session for this repository.".to_string(),
        ));
    };

    if existing.mode == SessionMode::Bare {
        return Err(bare_mode_unsupported(&existing.session_name, "remove"));
    }

    let Some(pos) = existing.worktrees.iter().position(|w| w.branch == branch) else {
        let live: Vec<&str> = existing
            .worktrees
            .iter()
            .map(|w| w.branch.as_str())
            .collect();
        return Err(PawError::SessionError(format!(
            "branch '{branch}' is not an agent in session '{}'. Live agents: {}.",
            existing.session_name,
            if live.is_empty() {
                "(none)".to_string()
            } else {
                live.join(", ")
            }
        )));
    };
    let target = existing.worktrees[pos].clone();

    // 5.3 Uncommitted-work safety check (D7) — unless --force or --keep-worktree.
    if !force && !keep_worktree {
        let dirty = git::uncommitted_files(&target.worktree_path).unwrap_or_default();
        // Filter out git-paw's own managed/injected files (the gitignored
        // sidecar and any residual managed `AGENTS.md` block). Only genuine
        // user work should block removal — a just-started worktree whose only
        // dirt is git-paw's injection is treated as clean (see
        // `agents::is_managed_path`). The refusal message lists only the
        // residual user files, so the user is never told to commit git-paw's
        // own bookkeeping.
        let residual: Vec<String> = dirty
            .into_iter()
            .filter(|f| !agents::is_managed_path(&target.worktree_path, f))
            .collect();
        if !residual.is_empty() {
            let list = residual
                .iter()
                .map(|f| format!("  {f}"))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(PawError::SessionError(format!(
                "worktree for '{branch}' has uncommitted changes:\n{list}\n\n\
                 Commit them first, or pass --force to remove anyway (the changes will be lost), \
                 or --keep-worktree to detach the pane and leave the worktree on disk."
            )));
        }
    }

    tmux::ensure_tmux_installed()?;

    // 5.4 Take the advisory lock for the mutate-the-session section.
    let _lock = git_paw::lock::SessionLock::acquire(&repo_root)?;

    let offset = agent_pane_offset(&existing);
    let session_alive = tmux::is_session_alive(&existing.session_name).unwrap_or(false);

    // 5.5 Kill the target tmux pane by RESOLVED pane id, not a JSON-position
    // index (design D2, G2a): map the removed branch's worktree to its live
    // pane via `pane_current_path` and kill that pane id, regardless of the
    // process running in it (a bare shell from a failed launch, a CLI, or
    // anything else). This never targets a different agent's pane even when a
    // stale orphan pane has shifted the grid (the v0.8.0 G2 failure), and is
    // an idempotent no-op when no pane maps (the pane is already gone).
    if session_alive
        && let Some(pane_id) =
            tmux::resolve_pane_id_for_worktree(&existing.session_name, &target.worktree_path)?
    {
        tmux::kill_pane_by_id(&pane_id)?;
    }

    // 5.6 Recompute layout_for(N-1) and re-apply so the grid re-flows.
    let remaining = existing.worktrees.len() - 1;
    if session_alive && remaining > 0 {
        let layout = git_paw::supervisor::layout::layout_for(remaining)?;
        tmux::build_remove_retile_commands(&existing.session_name, remaining, layout).execute()?;
        // Rebalance the re-flowed agent rows to equal width on the live window
        // (design D4, G3): tmux renumbered the survivors after the kill, so the
        // contiguous rows are resized to even columns for the new count.
        if let Err(e) = tmux::rebalance_agent_rows(&existing.session_name, remaining) {
            eprintln!("warning: could not rebalance agent-row widths: {e}");
        }
    }

    // 5.7 Delegate to detach_worktree for removal, unless --keep-worktree.
    if keep_worktree {
        println!(
            "Keeping worktree on disk: {}",
            target.worktree_path.display()
        );
    } else {
        detach_worktree(&repo_root, &target, &mut std::io::stderr());
    }

    // 5.8 Drop the branch/pane entry from the session JSON.
    let mut updated = existing.clone();
    updated.worktrees.remove(pos);
    session::save_session(&updated)?;
    write_repo_discovery_file(
        &repo_root,
        &updated.session_name,
        &updated.worktrees,
        offset,
    );

    println!(
        "Removed '{branch}' from session '{}'.",
        updated.session_name
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Command: __dashboard
// ---------------------------------------------------------------------------

/// Runs the broker and dashboard in pane 0 (internal command).
#[allow(clippy::too_many_lines)]
fn cmd_dashboard() -> Result<(), PawError> {
    // This is an internal command that should only run inside tmux
    if std::env::var("TMUX").is_err() {
        return Err(PawError::DashboardError(
            "this is an internal command that should only be run by git-paw inside tmux"
                .to_string(),
        ));
    }

    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;
    let config = config::load_config(&repo_root, None)?;
    let broker_config = config.broker.clone();
    // Broker log panel: ring-buffer cap + initial visibility from
    // `[dashboard.broker_log]`. An absent `[dashboard]` section uses the
    // documented defaults (cap 500, visible).
    let broker_log_cfg = config
        .dashboard
        .as_ref()
        .map(|d| d.broker_log.clone())
        .unwrap_or_default();

    // The conflict detector subsystem runs only when supervisor mode is
    // enabled — its outputs flow through the supervisor inbox and are
    // tagged for the supervisor agent's attention.
    let conflict_cfg = config
        .supervisor
        .as_ref()
        .filter(|s| s.enabled)
        .map(|s| s.conflict.clone());

    // Learnings flush cadence (seconds). The aggregator itself is attached
    // separately to `broker_state` when `[supervisor] learnings = true`;
    // here we just pass the interval into start_broker_with so the flush
    // thread spawns with the user-configured cadence.
    let learnings_interval_seconds = config
        .supervisor
        .as_ref()
        .map_or(60, |s| s.learnings_config.flush_interval_seconds);

    // Per-commit verification nudge: when an agent commits, the broker pings
    // the supervisor to verify that commit immediately (default on; opt out
    // with `[supervisor] verify_on_commit_nudge = false`). Resolves to `true`
    // when no `[supervisor]` section is present.
    let verify_on_commit_nudge = config
        .supervisor
        .as_ref()
        .is_none_or(SupervisorConfig::verify_on_commit_nudge_enabled);

    // Resolve the supervisor's CLI the same way the launcher did
    // (`[supervisor].cli` > `default_cli`) and seed it authoritatively. The
    // supervisor is not a filesystem watch target, so this is the only
    // deterministic source for its dashboard CLI column — relying on the
    // supervisor to self-report via `agent.status` is unreliable (W15-15).
    let supervisor_cli = config
        .supervisor
        .as_ref()
        .and_then(|s| s.cli.clone())
        .or_else(|| config.default_cli.clone())
        .unwrap_or_default();

    let log_path = session::session_state_dir()?.join("broker.log");
    let mut broker_state = broker::BrokerState::new(Some(log_path))
        .with_verify_on_commit_nudge(verify_on_commit_nudge)
        .with_seeded_cli("supervisor", &supervisor_cli);

    // Look up the saved session once: needed for the broker watcher target
    // list AND to discover the supervisor pane_map for the auto-approve thread.
    let saved_session = session::find_session_for_repo(&repo_root)?;
    let watch_targets = saved_session
        .as_ref()
        .map(|sess| {
            sess.worktrees
                .iter()
                .map(|wt| broker::WatchTarget {
                    agent_id: broker::messages::slugify_branch(&wt.branch),
                    cli: wt.cli.clone(),
                    worktree_path: wt.worktree_path.clone(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Attach the opsx role-gating context. The guard is scoped to the OpenSpec
    // spec engine: under speckit/markdown (or no spec source) `engine_is_openspec`
    // is false and the guard stays inert regardless of the configured mode. The
    // roster maps each coding agent's worktree plus the supervisor's repo root,
    // so the guard can attribute a committing worktree and clear the supervisor's
    // own archives.
    {
        let engine_is_openspec =
            git_paw::specs::resolved_spec_type(&config, &repo_root).as_deref() == Some("openspec");
        let mut roster: Vec<(String, std::path::PathBuf)> = watch_targets
            .iter()
            .map(|t| (t.agent_id.clone(), t.worktree_path.clone()))
            .collect();
        roster.push((
            git_paw::opsx::SUPERVISOR_AGENT_ID.to_string(),
            repo_root.clone(),
        ));
        broker_state = broker_state.with_role_gating(git_paw::opsx::RoleGatingContext {
            mode: config.role_gating_mode(),
            engine_is_openspec,
            roster,
        });
    }

    // Attach the learnings aggregator when supervisor mode + learnings are
    // both enabled (mirrors the `should_attach` predicate pinned by the
    // learnings unit test). The aggregator appends to
    // `.git-paw/session-learnings.md` and — when broker publish resolves to
    // active — additionally emits `agent.learning` records through the
    // broker (the `agent-learning-variant` dual-output path).
    if let Some(sup) = config
        .supervisor
        .as_ref()
        .filter(|s| s.enabled && s.learnings)
    {
        let learnings_path = repo_root.join(".git-paw").join("session-learnings.md");
        let mut aggregator = broker::learnings::LearningsAggregator::new(learnings_path);
        aggregator.set_broker_publish(
            sup.learnings_config
                .broker_publish
                .resolve(broker_config.enabled),
        );
        for target in &watch_targets {
            aggregator.register_agent(&target.agent_id);
        }
        broker_state.attach_learnings(std::sync::Arc::new(std::sync::Mutex::new(aggregator)));
    }

    let handle = broker::start_broker_with(
        &broker_config,
        broker_state,
        watch_targets,
        conflict_cfg,
        learnings_interval_seconds,
    )?;
    let state = std::sync::Arc::clone(&handle.state);

    // Set up a flag that SIGHUP sets to signal the dashboard to exit gracefully.
    // tmux sends SIGHUP to pane processes when killing sessions. Without this,
    // the process would be terminated before BrokerHandle::drop runs, losing
    // the final log flush.
    let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    #[cfg(unix)]
    {
        use std::sync::atomic::AtomicPtr;

        static SHUTDOWN_PTR: AtomicPtr<std::sync::atomic::AtomicBool> =
            AtomicPtr::new(std::ptr::null_mut());

        const SIGHUP: std::ffi::c_int = 1;

        unsafe extern "C" {
            fn signal(signum: std::ffi::c_int, handler: extern "C" fn(std::ffi::c_int)) -> usize;
        }

        extern "C" fn sighup_handler(_: std::ffi::c_int) {
            let ptr = SHUTDOWN_PTR.load(std::sync::atomic::Ordering::Acquire);
            if !ptr.is_null() {
                unsafe {
                    (*ptr).store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }

        // Store the flag pointer so the signal handler can access it.
        SHUTDOWN_PTR.store(
            std::sync::Arc::as_ptr(&shutdown).cast_mut(),
            std::sync::atomic::Ordering::Release,
        );

        // SAFETY: sighup_handler only sets an AtomicBool, which is
        // async-signal-safe. The pointer is valid for the process lifetime.
        unsafe {
            signal(SIGHUP, sighup_handler);
        }
    }

    // Auto-approve thread (supervisor mode only). v0.5.0 moves the thread
    // out of cmd_supervisor (which now returns immediately) into this long-
    // lived dashboard subprocess. Killing the dashboard pane terminates the
    // thread, matching user expectation that supervision stops when the
    // dashboard is gone.
    let auto_approve_handle = saved_session
        .as_ref()
        .filter(|sess| sess.mode == SessionMode::Supervisor && broker_config.enabled)
        .and_then(|sess| {
            let auto_approve_cfg = config
                .supervisor
                .as_ref()
                .and_then(|s| s.auto_approve.clone())?;
            let pane_map: std::collections::HashMap<String, usize> = sess
                .worktrees
                .iter()
                .enumerate()
                .map(|(idx, wt)| {
                    (
                        broker::messages::slugify_branch(&wt.branch),
                        idx + git_paw::supervisor::layout::SUPERVISOR_PANE_OFFSET,
                    )
                })
                .collect();
            // Map each agent to its worktree root so the file-op classifier
            // (bug 3) can resolve write/edit prompts against the boundary.
            let worktree_map: std::collections::HashMap<String, std::path::PathBuf> = sess
                .worktrees
                .iter()
                .map(|wt| {
                    (
                        broker::messages::slugify_branch(&wt.branch),
                        wt.worktree_path.clone(),
                    )
                })
                .collect();
            // Build the manual-decision recorder from supervisor config. It is
            // inert when `[supervisor] manual_approvals_log = false`; learnings
            // emission additionally requires `[supervisor] learnings = true`.
            let supervisor = config.supervisor.as_ref();
            let manual_enabled =
                supervisor.is_none_or(SupervisorConfig::manual_approvals_log_enabled);
            let learnings_enabled = supervisor.is_some_and(|s| s.learnings);
            let cli = supervisor.and_then(|s| s.cli.clone());
            let recorder = git_paw::supervisor::manual_approvals::ManualDecisionRecorder::new(
                git_paw::supervisor::manual_approvals::log_path(
                    &sess.repo_path,
                    &sess.session_name,
                ),
                manual_enabled,
                learnings_enabled,
                sess.project_name.clone(),
                cli,
            );
            spawn_auto_approve_thread(
                sess.session_name.clone(),
                broker_config.url(),
                Some(auto_approve_cfg),
                pane_map,
                worktree_map,
                recorder,
            )
        });

    let dashboard_result = git_paw::dashboard::run_dashboard_with_panes(
        &state,
        handle,
        &shutdown,
        &std::collections::HashMap::new(),
        None,
        broker_log_cfg.max_messages,
        broker_log_cfg.default_visible,
        broker_log_cfg.height_lines,
    );

    if let Some((stop, join)) = auto_approve_handle {
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = join.join();
    }

    dashboard_result
}

// ---------------------------------------------------------------------------
// Command: pause
// ---------------------------------------------------------------------------

/// Pauses the session: detaches the user's tmux client, stops the broker
/// (by killing the dashboard pane only), and updates session status to
/// `Paused`. CLI panes keep running and retain their in-memory state.
///
/// Idempotent: pausing an already-paused or already-stopped session is
/// a no-op with a friendly message.
fn cmd_pause() -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    let Some(existing) = session::find_session_for_repo(&repo_root)? else {
        println!("No active session for this repo.");
        return Ok(());
    };

    // Idempotency: already paused.
    if existing.status == SessionStatus::Paused {
        println!("Session '{}' is already paused.", existing.session_name);
        return Ok(());
    }

    // Effective status check: stopped sessions can't be paused.
    let effective = existing.effective_status(|name| tmux::is_session_alive(name).unwrap_or(false));
    if effective == SessionStatus::Stopped {
        println!(
            "Session '{}' is already stopped; pause has no effect.",
            existing.session_name
        );
        return Ok(());
    }

    // Detach the user's tmux client. Idempotent in detach_client.
    tmux::detach_client(&existing.session_name)?;

    // Kill the dashboard pane only (which hosts the broker subprocess);
    // the BrokerHandle drop runs and broker shuts down gracefully. Only
    // applies when broker was enabled — without a broker there's no
    // dashboard pane to kill.
    if existing.broker_port.is_some() {
        let pane_idx = existing.dashboard_pane.unwrap_or(0);
        tmux::kill_pane(&existing.session_name, pane_idx)?;
    }

    let cli_pane_count = existing.worktrees.len();
    let session_name = existing.session_name.clone();

    let mut updated = existing;
    updated.status = SessionStatus::Paused;
    session::save_session(&updated)?;

    println!(
        "Session '{session_name}' paused. {cli_pane_count} CLI pane(s) still running. \
         Run 'git paw start' to resume."
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Command: stop
// ---------------------------------------------------------------------------

/// Stops the session: kills tmux but preserves worktrees and state.
fn cmd_stop(_force: bool) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    let Some(existing) = session::find_session_for_repo(&repo_root)? else {
        println!("No active session for this repo.");
        return Ok(());
    };

    if tmux::is_session_alive(&existing.session_name)? {
        tmux::kill_session(&existing.session_name)?;
    }

    // Bug E (v0-5-0-audit-cleanup §9c) — strip the supervisor-pane boot
    // block from AGENTS.md so it does not accumulate across sessions.
    // Idempotent: missing block / missing AGENTS.md is a no-op.
    if let Err(e) = agents::remove_session_boot_block(&repo_root) {
        eprintln!("warning: failed to clean session boot block from AGENTS.md: {e}");
    }

    let mut updated = existing;
    updated.status = SessionStatus::Stopped;
    session::save_session(&updated)?;

    println!("Session stopped. Worktrees and state preserved.");
    println!("Run `git paw start` to recover.");
    Ok(())
}

// ---------------------------------------------------------------------------
// Command: purge
// ---------------------------------------------------------------------------

/// Outcome of the purge UX flow.
#[derive(Debug, PartialEq, Eq)]
enum PurgeOutcome {
    Purged,
    Cancelled,
}

/// Removes everything: tmux session, worktrees, and state.
///
/// With `stale = true`, purges only sessions whose tmux session is gone (a
/// stale receipt) across the whole machine, leaving live sessions untouched.
/// `--force` is redundant in that combination (a stale entry is never
/// prompted for) — passing both behaves identically to `--stale` alone.
fn cmd_purge(force: bool, stale: bool) -> Result<(), PawError> {
    if stale {
        return cmd_purge_stale();
    }

    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    let Some(existing) = session::find_session_for_repo(&repo_root)? else {
        println!("No session to purge for this repo.");
        return Ok(());
    };

    let sessions_dir = session::session_state_dir()?;
    let mut confirm = |prompt: &str| -> Result<bool, PawError> {
        Confirm::new()
            .with_prompt(prompt)
            .default(false)
            .interact()
            .map_err(|_| PawError::UserCancelled)
    };
    let mut kill_tmux = |name: &str| -> Result<(), PawError> {
        if tmux::is_session_alive(name)? {
            tmux::kill_session(name)?;
        }
        Ok(())
    };

    let outcome = purge_with_prompt(
        &repo_root,
        &sessions_dir,
        &existing,
        force,
        &mut confirm,
        &mut kill_tmux,
        &mut std::io::stderr(),
    )?;

    match outcome {
        PurgeOutcome::Purged => println!("Purged session '{}'.", existing.session_name),
        PurgeOutcome::Cancelled => println!("Purge cancelled."),
    }
    Ok(())
}

/// Probes the existing receipt for staleness and, when stale (the receipt
/// claims active but the tmux session is gone), invalidates it — purging the
/// recorded worktrees, branches, and receipt equivalent to `purge --force` —
/// and emits a stderr notice naming the entry (design D5).
///
/// Returns `true` when invalidation fired; the caller SHALL then proceed with
/// a fresh launch as if no prior session existed. Returns `false` for a live
/// (`active` + alive), paused, stopped, or indeterminate receipt, leaving the
/// caller's normal reattach/recover decision intact.
fn invalidate_if_stale(repo_root: &Path, existing: &Session) -> Result<bool, PawError> {
    let liveness = tmux::session_liveness(&existing.session_name);
    if session::DisplayStatus::from_receipt(&existing.status, liveness)
        != session::DisplayStatus::Stale
    {
        return Ok(false);
    }

    let when = existing
        .created_at_iso8601()
        .map(|t| format!(", last seen {t}"))
        .unwrap_or_default();
    eprintln!(
        "notice: removed stale session receipt\n  ({}{}, tmux session no longer exists)",
        existing.session_name, when
    );

    let sessions_dir = session::session_state_dir()?;
    let mut confirm = |_: &str| -> Result<bool, PawError> { Ok(true) };
    let mut kill_tmux = |name: &str| -> Result<(), PawError> {
        if tmux::is_session_alive(name)? {
            tmux::kill_session(name)?;
        }
        Ok(())
    };
    purge_with_prompt(
        repo_root,
        &sessions_dir,
        existing,
        true,
        &mut confirm,
        &mut kill_tmux,
        &mut std::io::stderr(),
    )?;
    Ok(true)
}

/// Purges only stale sessions (receipt claims active but the tmux session is
/// gone) across the whole machine. Live sessions are left untouched.
///
/// Stale is defined exactly as [`session::DisplayStatus::Stale`]: an `active`
/// receipt whose `tmux has-session` probe returns
/// [`tmux::SessionLiveness::Stale`]. Stopped receipts (intentionally stopped)
/// and sessions on a host with no tmux binary (Indeterminate probe) are NOT
/// touched. Exits 0 with a "nothing to purge" message when no stale receipt
/// exists.
fn cmd_purge_stale() -> Result<(), PawError> {
    let sessions_dir = session::session_state_dir()?;
    let all = session::load_all_sessions_in(&sessions_dir)?;

    let stale: Vec<session::Session> = all
        .into_iter()
        .filter(|s| {
            let liveness = tmux::session_liveness(&s.session_name);
            session::DisplayStatus::from_receipt(&s.status, liveness)
                == session::DisplayStatus::Stale
        })
        .collect();

    if stale.is_empty() {
        println!("No stale sessions to purge.");
        return Ok(());
    }

    let mut confirm = |_: &str| -> Result<bool, PawError> { Ok(true) };
    let mut kill_tmux = |name: &str| -> Result<(), PawError> {
        if tmux::is_session_alive(name)? {
            tmux::kill_session(name)?;
        }
        Ok(())
    };

    for session_entry in &stale {
        // force = true: stale entries are orphaned, nothing to confirm.
        let outcome = purge_with_prompt(
            &session_entry.repo_path,
            &sessions_dir,
            session_entry,
            true,
            &mut confirm,
            &mut kill_tmux,
            &mut std::io::stderr(),
        )?;
        if outcome == PurgeOutcome::Purged {
            println!("Purged stale session '{}'.", session_entry.session_name);
        }
    }
    Ok(())
}

/// Tears down a single agent's worktree: removes the worktree directory (with
/// the per-worktree `Removing worktree ...` / `...done (Xs)` progress markers)
/// and, when git-paw created the branch, deletes it afterwards.
///
/// Extracted from `cmd_purge`'s per-worktree loop (design D6, task 1.3) so
/// `cmd_remove` performs byte-identical removal for a single agent that
/// `git paw purge` performs for every agent. Best-effort: a failed
/// worktree-remove or branch-delete is surfaced as a `warning:` on `stderr`
/// and does not abort — matching purge's resilience on large or busy
/// worktrees.
fn detach_worktree(repo_root: &Path, entry: &WorktreeEntry, stderr: &mut dyn std::io::Write) {
    let _ = writeln!(
        stderr,
        "Removing worktree {}...",
        entry.worktree_path.display()
    );
    let _ = stderr.flush();
    let started = std::time::Instant::now();
    let result = git::remove_worktree(repo_root, &entry.worktree_path);
    let elapsed_secs = started.elapsed().as_secs_f64();
    match result {
        Ok(()) => {
            let _ = writeln!(stderr, "  ...done ({elapsed_secs:.1}s)");
        }
        Err(e) => {
            let _ = writeln!(
                stderr,
                "warning: failed to remove worktree '{}' after {:.1}s: {e}",
                entry.worktree_path.display(),
                elapsed_secs
            );
        }
    }
    let _ = stderr.flush();

    if entry.branch_created
        && let Err(e) = git::delete_branch(repo_root, &entry.branch)
    {
        let _ = writeln!(
            stderr,
            "warning: failed to delete branch '{}': {e}",
            entry.branch
        );
    }
}

/// Testable core of the purge UX. Emits the unmerged-commits warning to
/// `stderr`, prompts via `confirm` unless `force` is set, then tears down
/// tmux, worktrees, branches git-paw created, and the session file.
fn purge_with_prompt(
    repo_root: &Path,
    sessions_dir: &Path,
    session: &Session,
    force: bool,
    confirm: &mut dyn FnMut(&str) -> Result<bool, PawError>,
    kill_tmux: &mut dyn FnMut(&str) -> Result<(), PawError>,
    stderr: &mut dyn std::io::Write,
) -> Result<PurgeOutcome, PawError> {
    let default_branch = resolve_default_branch(repo_root);
    let unmerged = collect_unmerged_branches(repo_root, session, &default_branch);

    if !unmerged.is_empty() {
        let _ = writeln!(
            stderr,
            "Warning: {} branch(es) have unmerged commits:",
            unmerged.len()
        );
        for (branch, count) in &unmerged {
            let _ = writeln!(
                stderr,
                "  {branch}: {count} commit(s) not in {default_branch}"
            );
        }
        let _ = writeln!(
            stderr,
            "Purging is irreversible — those commits will be lost."
        );
        // Flush the warning block so it lands on the terminal before the
        // dialoguer prompt reads from stdin. Without this flush the
        // warning can race against the prompt — on some TTYs the prompt's
        // stdin read fires while the stderr buffer is still draining,
        // dialoguer sees an unexpected I/O state, and the `y` + Enter
        // never reaches `Confirm::interact`. Bug C in v0-5-0-audit-cleanup.
        let _ = stderr.flush();
    }

    if !force {
        let prompt_text = if unmerged.is_empty() {
            "This will remove the tmux session, all worktrees, and session state. Continue?"
        } else {
            "Purge is irreversible. Continue?"
        };
        if !confirm(prompt_text)? {
            return Ok(PurgeOutcome::Cancelled);
        }
    }

    kill_tmux(&session.session_name)?;

    // Per-worktree teardown (worktree-remove + branch cleanup), delegated to
    // the shared `detach_worktree` helper so `cmd_remove` performs the exact
    // same per-worktree removal `purge` does (design D6, task 1.3). The helper
    // emits the per-worktree begin / `...done (Xs)` progress markers Bug D in
    // v0-5-0-audit-cleanup added.
    for entry in &session.worktrees {
        detach_worktree(repo_root, entry, stderr);
    }

    if let Some(ref log_path) = session.broker_log_path {
        let _ = std::fs::remove_file(log_path);
    }

    // Bug E (v0-5-0-audit-cleanup §9c) — strip the supervisor-pane boot
    // block from AGENTS.md so it does not accumulate across sessions.
    // Idempotent: missing block / missing AGENTS.md is a no-op.
    if let Err(e) = agents::remove_session_boot_block(repo_root) {
        let _ = writeln!(
            stderr,
            "warning: failed to clean session boot block from AGENTS.md: {e}"
        );
    }

    session::delete_session_in(&session.session_name, sessions_dir)?;

    // Remove the per-repo discovery file sweep.sh reads (capability
    // `session-json-location`). Idempotent — a missing file is not an error.
    if let Err(e) = session::remove_repo_session_file(repo_root, &session.session_name) {
        let _ = writeln!(
            stderr,
            "warning: failed to remove per-repo session discovery file: {e}"
        );
    }

    Ok(PurgeOutcome::Purged)
}

/// Resolves the default branch name for the repo, falling back to `"main"`.
fn resolve_default_branch(repo_root: &Path) -> String {
    let output = StdCommand::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
        .current_dir(repo_root)
        .output();
    if let Ok(out) = output
        && out.status.success()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        if let Some(name) = s.trim().strip_prefix("refs/remotes/origin/") {
            return name.to_string();
        }
    }
    "main".to_string()
}

/// For each worktree branch in the session, returns the list of branches with
/// at least one commit not yet merged to the default branch, along with the
/// count of such commits.
fn collect_unmerged_branches(
    repo_root: &Path,
    session: &Session,
    default_branch: &str,
) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for entry in &session.worktrees {
        if entry.branch == default_branch {
            continue;
        }
        let result = StdCommand::new("git")
            .args(["log", &entry.branch, "--not", default_branch, "--oneline"])
            .current_dir(repo_root)
            .output();
        let Ok(output) = result else { continue };
        if !output.status.success() {
            continue;
        }
        let count = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();
        if count > 0 {
            out.push((entry.branch.clone(), count));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Command: status
// ---------------------------------------------------------------------------

/// Shows session state for the current repo.
fn cmd_status(json: bool) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    let Some(existing) = session::find_session_for_repo(&repo_root)? else {
        if json {
            println!("{}", serde_json::json!({ "session": null }));
        } else {
            println!("No session for this repo.");
        }
        return Ok(());
    };

    // Single cheap liveness probe (spec: "Liveness probe is cheap"). The
    // probe distinguishes a genuinely-absent tmux session (Stale) from a
    // probe that could not run at all (Indeterminate → never reports stale).
    let liveness = tmux::session_liveness(&existing.session_name);
    let display = session::DisplayStatus::from_receipt(&existing.status, liveness);
    let alive = matches!(liveness, tmux::SessionLiveness::Alive);

    if json {
        let worktrees: Vec<_> = existing
            .worktrees
            .iter()
            .map(|e| {
                serde_json::json!({
                    "branch": e.branch,
                    "cli": e.cli,
                    "worktree_path": e.worktree_path,
                })
            })
            .collect();
        let obj = serde_json::json!({
            "session": existing.session_name,
            "status": display.as_str(),
            "tmux_running": alive,
            "worktrees": worktrees,
        });
        println!("{obj}");
        return Ok(());
    }

    println!("Session: {}", existing.session_name);
    println!("Status:  {} {display}", display.icon());
    match display {
        session::DisplayStatus::Paused => {
            println!("  \u{21b3} run 'git paw start' to resume");
        }
        session::DisplayStatus::Stale => {
            println!(
                "  \u{21b3} tmux session no longer exists — run 'git paw start' to \
                 self-heal, or 'git paw purge --stale' to clear it"
            );
        }
        _ => {}
    }
    println!("Tmux:    {}", if alive { "running" } else { "not running" });
    println!();

    // Broker info
    if let (Some(bind), Some(port)) = (&existing.broker_bind, existing.broker_port) {
        let url = format!("http://{bind}:{port}");
        match broker::probe_broker(&url) {
            broker::ProbeResult::LiveBroker => println!("Broker:  {url} (running)"),
            _ if display == session::DisplayStatus::Paused => {
                println!("Broker:  {url} (paused \u{2014} run 'git paw start' to resume)");
            }
            _ => println!("Broker:  {url} (not responding)"),
        }
        println!();
    }

    if existing.worktrees.is_empty() {
        println!("  (no worktrees)");
    } else {
        for entry in &existing.worktrees {
            println!(
                "  {} \u{2192} {} ({})",
                entry.branch,
                entry.cli,
                entry.worktree_path.display()
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Command: list-clis
// ---------------------------------------------------------------------------

/// Lists all detected and custom AI CLIs.
fn cmd_list_clis() -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;
    let config = config::load_config(&repo_root, None)?;
    let custom_defs = config_to_custom_defs(&config);
    let clis = detect::detect_clis(&custom_defs);

    if clis.is_empty() {
        println!("No AI CLIs found.");
        println!("Install one or use `git paw add-cli` to register a custom CLI.");
        return Ok(());
    }

    println!("{:<15} {:<10} PATH", "NAME", "SOURCE");
    for cli in &clis {
        println!(
            "{:<15} {:<10} {}",
            cli.binary_name,
            cli.source,
            cli.path.display()
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Command: add-cli
// ---------------------------------------------------------------------------

/// Registers a custom AI CLI in the global config.
fn cmd_add_cli(name: &str, command: &str, display_name: Option<&str>) -> Result<(), PawError> {
    config::add_custom_cli(name, command, display_name)?;
    println!("Added custom CLI '{name}'.");
    Ok(())
}

// ---------------------------------------------------------------------------
// Command: remove-cli
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Command: replay
// ---------------------------------------------------------------------------

/// Replays captured session logs.
fn cmd_replay(
    branch: Option<String>,
    list: bool,
    color: bool,
    session: Option<&str>,
) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    if list {
        return git_paw::replay::display_list(&repo_root);
    }

    // clap ensures branch is present when --list is absent
    let branch = branch.expect("branch is required unless --list is passed");
    let session_name = git_paw::replay::resolve_session(&repo_root, session)?;
    let log_path = git_paw::replay::find_log(&repo_root, &session_name, &branch)?;

    if color {
        git_paw::replay::replay_colored(&log_path)
    } else {
        git_paw::replay::replay_stripped(&log_path)
    }
}

// ---------------------------------------------------------------------------
// Command: approvals
// ---------------------------------------------------------------------------

/// Resolves which session's manual-approval log to read.
///
/// `--session` wins when present. Otherwise the active session for the current
/// repo is used. Unlike `replay`, a missing log file is not an error (the
/// session simply recorded no manual approvals), so this only needs to name a
/// session — it does not validate that a log exists.
fn resolve_approvals_session(
    repo_root: &Path,
    session_flag: Option<&str>,
) -> Result<String, PawError> {
    if let Some(name) = session_flag {
        return Ok(name.to_string());
    }
    match session::find_session_for_repo(repo_root)? {
        Some(s) => Ok(s.session_name),
        None => Err(PawError::SessionError(
            "no active session for this repo; pass --session <NAME> to target one".to_string(),
        )),
    }
}

/// Reports the manually-approved command patterns for a session.
///
/// Reads the per-session manual-approval JSONL log, aggregates by pattern,
/// applies the promotion-target heuristic, sorts by descending count, and
/// renders either a text table (default) or JSON (`--json`). An empty/missing
/// log produces an empty result without error.
fn cmd_approvals(
    session_flag: Option<&str>,
    limit: Option<usize>,
    json: bool,
) -> Result<(), PawError> {
    use git_paw::supervisor::manual_approvals::{self, AggregatedApproval, Suggestion};

    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;
    let session_name = resolve_approvals_session(&repo_root, session_flag)?;
    let project_name = git::project_name(&repo_root);

    let log_path = manual_approvals::log_path(&repo_root, &session_name);
    let mut rows = manual_approvals::aggregate(&log_path)
        .map_err(|e| PawError::SessionError(format!("failed to read manual-approvals log: {e}")))?;
    if let Some(n) = limit {
        rows.truncate(n);
    }

    // Pair each pattern with its promotion-target suggestion. Branch/worktree
    // context is per-agent and not retained by aggregation, so the report
    // leans on the project name plus the `./`-token rule.
    let classified: Vec<(AggregatedApproval, Suggestion)> = rows
        .into_iter()
        .map(|r| {
            let s = manual_approvals::suggest_target(&r.pattern, &project_name, "", None);
            (r, s)
        })
        .collect();

    if json {
        let approvals: Vec<serde_json::Value> = classified
            .iter()
            .map(|(r, s)| {
                serde_json::json!({
                    "pattern": r.pattern,
                    "count": r.count,
                    "suggested_target": s.json_value(),
                    "first_seen": r.first_seen,
                    "last_seen": r.last_seen,
                })
            })
            .collect();
        let out = serde_json::json!({
            "session": session_name,
            "approvals": approvals,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&out)
                .map_err(|e| PawError::SessionError(format!("failed to serialize JSON: {e}")))?
        );
        return Ok(());
    }

    if classified.is_empty() {
        println!("no manual approvals recorded for session '{session_name}'");
        return Ok(());
    }

    // Text table: PATTERN / COUNT / SUGGEST, columns sized to content.
    let pattern_w = classified
        .iter()
        .map(|(r, _)| r.pattern.len())
        .max()
        .unwrap_or(0)
        .max("PATTERN".len());
    let count_w = classified
        .iter()
        .map(|(r, _)| r.count.to_string().len())
        .max()
        .unwrap_or(0)
        .max("COUNT".len());

    println!("{:<pattern_w$}  {:>count_w$}  SUGGEST", "PATTERN", "COUNT");
    for (r, s) in &classified {
        println!(
            "{:<pattern_w$}  {:>count_w$}  {}",
            r.pattern,
            r.count,
            s.label()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Command: remove-cli
// ---------------------------------------------------------------------------

/// Removes a custom AI CLI from the global config.
fn cmd_remove_cli(name: &str) -> Result<(), PawError> {
    // Check if it's an auto-detected CLI (not in config)
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;

    // Try to load config to check if it's a custom CLI
    // If we're not in a repo, just attempt removal directly
    if let Ok(repo_root) = git::validate_repo(&cwd) {
        let config = config::load_config(&repo_root, None)?;
        if !config.clis.contains_key(name) {
            // Check if it's a known auto-detected CLI
            let detected = detect::detect_known_clis();
            if detected.iter().any(|c| c.binary_name == name) {
                return Err(PawError::CliNotFound(format!(
                    "CLI '{name}' is auto-detected, not a custom CLI. Cannot remove."
                )));
            }
        }
    }

    config::remove_custom_cli(name)?;
    println!("Removed custom CLI '{name}'.");
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use git_paw::config::SupervisorConfig;
    use serial_test::serial;
    use std::path::PathBuf;
    use std::time::UNIX_EPOCH;
    use tempfile::TempDir;

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
        // cmd_supervisor resolves to the `--branches` / branch-picker flow —
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

    #[test]
    fn spec_mode_from_flags_translates_each_combination() {
        assert_eq!(SpecMode::from_flags(true, None), SpecMode::All);
        assert_eq!(SpecMode::from_flags(false, None), SpecMode::None);
        let empty: Vec<String> = Vec::new();
        assert_eq!(SpecMode::from_flags(false, Some(&empty)), SpecMode::Picker);
        let names = vec!["add-auth".to_string(), "fix-session".to_string()];
        assert_eq!(
            SpecMode::from_flags(false, Some(&names)),
            SpecMode::Narrow(names)
        );
    }

    // -----------------------------------------------------------------------
    // build_task_prompt — pure helper that constructs the per-agent task
    // prompt appended to the supervisor-mode boot block. Replaces the old
    // first-line-of-spec-body truncation (MILESTONE drift item 29).
    // -----------------------------------------------------------------------

    fn make_spec_entry(id: &str, prompt_body: &str) -> git_paw::specs::SpecEntry {
        // Default to the `Markdown` backend so the regression tests
        // (`task_prompt_with_spec_points_at_sidecar_and_includes_id`,
        // `task_prompt_does_not_include_spec_body_first_line`) keep exercising
        // the sidecar-pointer branch of `build_task_prompt`.
        // Tests that want the OpenSpec branch construct a `SpecEntry`
        // literal directly with `backend = SpecBackendKind::OpenSpec`.
        git_paw::specs::SpecEntry {
            id: id.to_string(),
            backend: git_paw::specs::SpecBackendKind::Markdown,
            branch: format!("feat/{id}"),
            cli: None,
            prompt: prompt_body.to_string(),
            owned_files: None,
        }
    }

    #[test]
    fn task_prompt_with_spec_points_at_sidecar_and_includes_id() {
        let entry = make_spec_entry("my-change", "## 1. First section\n\nbody body body");
        let prompt = build_task_prompt(Some(&entry));
        assert!(
            prompt.contains(git_paw::agents::SIDECAR_REL_PATH),
            "spec-derived task prompt should point at the sidecar, got: {prompt}"
        );
        assert!(
            prompt.contains("openspec/changes/my-change"),
            "spec-derived task prompt should include the spec id directory, got: {prompt}"
        );
    }

    #[test]
    fn task_prompt_without_spec_points_at_sidecar() {
        let prompt = build_task_prompt(None);
        assert_eq!(
            prompt,
            "Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task.",
            "no-spec task prompt should point at the sidecar verbatim"
        );
    }

    #[test]
    fn task_prompt_openspec_backend_points_at_sidecar_then_invokes_opsx_apply() {
        // OVERRIDE (agents-md-sidecar-injection): pre-sidecar this branch was
        // the bare `/opsx:apply <id>` with deliberately NO pointer, because the
        // combined view was auto-loaded from the worktree-root AGENTS.md. The
        // managed block now lives in a gitignored sidecar the CLIs do not
        // auto-load, so the prompt MUST point the agent at the sidecar first,
        // then run the slash command. This intentionally reverses the earlier
        // "no AGENTS.md pointer" decision.
        let entry = git_paw::specs::SpecEntry {
            id: "my-change".to_string(),
            backend: git_paw::specs::SpecBackendKind::OpenSpec,
            branch: "feat/my-change".to_string(),
            cli: None,
            prompt: String::new(),
            owned_files: None,
        };
        let prompt = build_task_prompt(Some(&entry));
        assert!(
            prompt.contains(git_paw::agents::SIDECAR_REL_PATH),
            "OpenSpec branch must point the agent at the sidecar, got: {prompt}"
        );
        assert!(
            prompt.contains("/opsx:apply my-change"),
            "OpenSpec branch must still invoke the opsx:apply slash command, got: {prompt}"
        );
        assert!(
            !prompt.contains("openspec/changes/"),
            "OpenSpec branch must suppress the openspec/changes/ path prose, got: {prompt}"
        );
    }

    #[test]
    fn task_prompt_markdown_backend_uses_sidecar_pointer() {
        let entry = git_paw::specs::SpecEntry {
            id: "my-feature".to_string(),
            backend: git_paw::specs::SpecBackendKind::Markdown,
            branch: "feat/my-feature".to_string(),
            cli: None,
            prompt: String::new(),
            owned_files: None,
        };
        let prompt = build_task_prompt(Some(&entry));
        assert!(
            prompt.contains(git_paw::agents::SIDECAR_REL_PATH),
            "Markdown-backed task prompt should point at the sidecar, got: {prompt}"
        );
        assert!(
            prompt.contains("openspec/changes/my-feature"),
            "Markdown-backed task prompt should include the spec id directory, got: {prompt}"
        );
        assert!(
            !prompt.contains("/opsx:apply"),
            "Markdown branch must NOT invoke the OpenSpec slash command, got: {prompt}"
        );
    }

    #[test]
    fn task_prompt_without_spec_points_at_sidecar_verbatim() {
        // Regression: the `None` branch must produce the exact sidecar-pointer
        // fallback string byte-for-byte, even after the per-backend dispatch.
        let prompt = build_task_prompt(None);
        assert_eq!(
            prompt,
            "Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task.",
            "no-spec task prompt should be the sidecar-pointer fallback verbatim"
        );
    }

    #[test]
    fn task_prompt_does_not_include_spec_body_first_line() {
        // Regression test for the original bug: the task prompt used to be
        // `spec_entry.prompt.lines().next()`, which produced exactly this
        // truncated heading. The fix replaces it with an AGENTS.md pointer
        // that does NOT contain the heading.
        let entry = make_spec_entry(
            "prompt-submit-fix",
            "## 1. Code fix in cmd_supervisor\n\n- [ ] 1.1 Add a constant.\n",
        );
        let prompt = build_task_prompt(Some(&entry));
        assert!(
            !prompt.contains("## 1. Code fix in cmd_supervisor"),
            "task prompt MUST NOT include the spec body's first heading in raw form, got: {prompt}"
        );
        // The body should not leak in either.
        assert!(
            !prompt.contains("- [ ] 1.1"),
            "task prompt MUST NOT include spec body content, got: {prompt}"
        );
    }

    // Maps to scenario "Spec-derived task prompt points at the sidecar and
    // includes spec id" — fixes the named-test coverage gap from
    // boot-prompt-full-body. (test-coverage-v0-5-0 task 2.1)
    #[test]
    fn build_task_prompt_spec_entry_contains_sidecar_and_spec_id() {
        let entry = make_spec_entry("governance-config", "## 1. Struct definitions\n\nBody.");
        let prompt = build_task_prompt(Some(&entry));
        assert!(
            prompt.contains(git_paw::agents::SIDECAR_REL_PATH),
            "spec-derived prompt should point at the sidecar, got: {prompt}"
        );
        assert!(
            prompt.contains("openspec/changes/governance-config"),
            "spec-derived prompt should embed the spec id directory, got: {prompt}"
        );
        assert!(
            !prompt.contains("## 1. Struct definitions"),
            "spec body's first heading MUST NOT leak into the prompt, got: {prompt}"
        );
    }

    // Maps to scenario `Supervisor pane receives a boot block` from
    // supervisor-as-pane. The pane-0 prompt is constructed inline in
    // `cmd_supervisor` (it is not extracted into a helper); we reconstruct
    // it using the same public helpers production uses and lock in the
    // resulting prefix + framing. (test-coverage-v0-5-0 task 12.10)
    #[test]
    fn supervisor_pane_prompt_starts_with_boot_block() {
        let broker_url = "http://127.0.0.1:9119";
        let boot_block = git_paw::skills::build_boot_block("supervisor", broker_url);
        let supervisor_framing = "Begin observing the spec implementation session. Your skill (.git-paw/AGENTS.local.md) describes \
             your role — read it, then start the autonomous loop. The user can type questions or \
             directives directly into your pane; handle them per the 'When the user types in your \
             pane' section of your skill.";
        let supervisor_prompt = format!("{boot_block}\n\n{supervisor_framing}");

        assert!(
            supervisor_prompt.starts_with(&boot_block),
            "supervisor pane prompt should begin with the boot block; got:\n{supervisor_prompt}"
        );
        // The boot-block template uses `{{BRANCH_ID}}` as a placeholder
        // that `build_boot_block` substitutes to the slugified agent id.
        // For the supervisor pane the substituted value is the literal
        // string `"supervisor"`, which appears in every `agent.*` curl
        // payload as `"agent_id":"supervisor"`. That is the production
        // shape of "BRANCH_ID=supervisor" from the spec scenario.
        assert!(
            boot_block.contains("\"agent_id\":\"supervisor\""),
            "boot block must carry the supervisor agent_id substitution; got:\n{boot_block}"
        );
        let framing_idx = supervisor_prompt
            .find("Begin observing")
            .expect("`Begin observing` framing must follow the boot block");
        assert!(
            framing_idx > boot_block.len(),
            "framing must follow the boot block, not precede it; framing_idx={framing_idx}"
        );
    }

    // Maps to scenario `Boot block withheld until the pane is CLI-ready` from
    // session-orchestration-robustness (G1). The blind pre-`send-keys`
    // fixed sleep was replaced by the launch-readiness gate: `cmd_supervisor`
    // MUST gate every prompt-bearing pane via `tmux::gate_pane_for_injection`
    // before injecting its boot block, rather than relying on a wall-clock
    // sleep that the v0.8.0 dogfood proved racy.
    #[test]
    fn supervisor_launch_gates_injection_on_readiness() {
        let src = include_str!("main.rs");
        let cmd_start = src
            .find("fn cmd_supervisor(")
            .expect("cmd_supervisor signature present");
        let body_start = src[cmd_start..]
            .find('{')
            .map(|o| cmd_start + o)
            .expect("opening brace");
        let mut depth: i32 = 0;
        let mut end = body_start;
        for (i, ch) in src[body_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = body_start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let body = &src[body_start..end];

        let gate_idx = body.find("gate_pane_for_injection").expect(
            "cmd_supervisor must gate boot-block injection on CLI readiness \
             (tmux::gate_pane_for_injection) instead of a blind fixed sleep",
        );
        let submit_idx = body
            .find("submit_prompt_to_pane")
            .expect("cmd_supervisor must still inject the boot block via submit_prompt_to_pane");
        assert!(
            gate_idx < submit_idx,
            "the readiness gate must run BEFORE the first boot-block injection"
        );
        assert!(
            !body.contains("from_secs(2)"),
            "the blind 2s pre-injection sleep must be gone (replaced by the readiness gate)"
        );
    }

    // Maps to scenario "build_task_prompt is a pure function" — pairs a
    // determinism assertion with a static-source check that the function body
    // contains no filesystem / process IO calls. (test-coverage-v0-5-0 task 2.2)
    #[test]
    fn build_task_prompt_is_deterministic_and_io_free() {
        let entry = make_spec_entry("governance-config", "## body\n\nmore body");
        let a = build_task_prompt(Some(&entry));
        let b = build_task_prompt(Some(&entry));
        assert_eq!(a.as_bytes(), b.as_bytes(), "must be deterministic");

        let src = include_str!("main.rs");
        let needle = "pub(crate) fn build_task_prompt";
        let start = src
            .find(needle)
            .unwrap_or_else(|| panic!("build_task_prompt signature not found in main.rs"));
        let body_start = src[start..].find('{').map_or_else(
            || panic!("opening brace not found after signature"),
            |o| start + o,
        );
        // Walk braces to find the matching closing brace.
        let mut depth: i32 = 0;
        let mut end = body_start;
        for (i, ch) in src[body_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = body_start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        assert!(end > body_start, "did not find closing brace");
        let body = &src[body_start..end];

        for needle in [
            "std::fs::",
            "File::open",
            "File::create",
            "Command::new",
            "tokio::fs::",
        ] {
            assert!(
                !body.contains(needle),
                "build_task_prompt body must not contain `{needle}`; body:\n{body}"
            );
        }
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

    // --- purge UX ---

    /// Repo sandbox with a committed `main`, one feature worktree, and a
    /// session file persisted into a sibling sessions dir. Everything is under
    /// a single `TempDir` so drop cleans up.
    struct PurgeSandbox {
        _sandbox: TempDir,
        repo: PathBuf,
        sessions_dir: PathBuf,
        session: Session,
    }

    fn git(dir: &Path, args: &[&str]) {
        let out = StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git spawn");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn setup_purge_sandbox(with_unmerged_commit: bool) -> PurgeSandbox {
        let sandbox = TempDir::new().expect("tempdir");
        let repo = sandbox.path().join("repo");
        std::fs::create_dir(&repo).expect("mkdir repo");
        let sessions_dir = sandbox.path().join("sessions");

        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["config", "user.email", "test@test.com"]);
        git(&repo, &["config", "user.name", "Test"]);
        std::fs::write(repo.join("README.md"), "# test").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "initial"]);

        let wt = git::create_worktree(
            &repo,
            "feature/test",
            false,
            git_paw::config::WorktreePlacement::Sibling,
        )
        .expect("create worktree");
        if with_unmerged_commit {
            std::fs::write(wt.path.join("new.txt"), "hello").unwrap();
            git(&wt.path, &["add", "."]);
            git(&wt.path, &["commit", "-q", "-m", "feature work"]);
        }

        let session = Session {
            session_name: "paw-repo".to_string(),
            repo_path: repo.clone(),
            project_name: "repo".to_string(),
            created_at: UNIX_EPOCH,
            status: SessionStatus::Stopped,
            worktrees: vec![WorktreeEntry {
                branch: "feature/test".to_string(),
                worktree_path: wt.path.clone(),
                cli: "claude".to_string(),
                branch_created: wt.branch_created,
                pending_boot_prompt: None,
            }],
            broker_port: None,
            broker_bind: None,
            broker_log_path: None,
            mode: SessionMode::Bare,
            dashboard_pane: None,
        };
        session::save_session_in(&session, &sessions_dir).expect("save session");

        PurgeSandbox {
            _sandbox: sandbox,
            repo,
            sessions_dir,
            session,
        }
    }

    #[test]
    #[serial]
    fn purge_no_unmerged_commits_runs_without_warning() {
        let sb = setup_purge_sandbox(false);
        let worktree_path = sb.session.worktrees[0].worktree_path.clone();

        let mut confirm_calls = 0;
        let mut confirm = |_: &str| -> Result<bool, PawError> {
            confirm_calls += 1;
            Ok(true)
        };
        let mut stderr = Vec::<u8>::new();

        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            false,
            &mut confirm,
            &mut |_: &str| Ok(()),
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PurgeOutcome::Purged);
        assert_eq!(confirm_calls, 1);
        let err_text = String::from_utf8(stderr).unwrap();
        assert!(
            !err_text.contains("unmerged"),
            "stderr should not mention unmerged: {err_text:?}"
        );
        assert!(!worktree_path.exists(), "worktree should be removed");
        assert!(
            !sb.sessions_dir.join("paw-repo.json").exists(),
            "session file should be removed"
        );
    }

    #[test]
    #[serial]
    fn purge_with_unmerged_commits_emits_warning_to_stderr() {
        let sb = setup_purge_sandbox(true);

        let mut confirm = |_: &str| -> Result<bool, PawError> { Ok(true) };
        let mut stderr = Vec::<u8>::new();

        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            false,
            &mut confirm,
            &mut |_: &str| Ok(()),
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PurgeOutcome::Purged);
        let err_text = String::from_utf8(stderr).unwrap();
        assert!(
            err_text.contains("Warning:") && err_text.contains("unmerged commits"),
            "stderr should contain unmerged warning: {err_text:?}"
        );
        assert!(
            err_text.contains("feature/test"),
            "stderr should name the branch: {err_text:?}"
        );
        assert!(
            err_text.contains("irreversible"),
            "stderr should warn about data loss: {err_text:?}"
        );
    }

    #[test]
    #[serial]
    fn purge_force_skips_confirm_but_still_warns() {
        let sb = setup_purge_sandbox(true);

        let mut confirm_calls = 0;
        let mut confirm = |_: &str| -> Result<bool, PawError> {
            confirm_calls += 1;
            Ok(false)
        };
        let mut stderr = Vec::<u8>::new();

        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            true, // force
            &mut confirm,
            &mut |_: &str| Ok(()),
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PurgeOutcome::Purged);
        assert_eq!(confirm_calls, 0, "force should not invoke confirm");
        let err_text = String::from_utf8(stderr).unwrap();
        assert!(
            err_text.contains("Warning:") && err_text.contains("unmerged commits"),
            "force mode should still warn: {err_text:?}"
        );
    }

    #[test]
    #[serial]
    fn purge_cancelled_leaves_worktree_in_place() {
        let sb = setup_purge_sandbox(true);
        let worktree_path = sb.session.worktrees[0].worktree_path.clone();

        let mut confirm = |_: &str| -> Result<bool, PawError> { Ok(false) };
        let mut tmux_calls = 0;
        let mut kill_tmux = |_: &str| -> Result<(), PawError> {
            tmux_calls += 1;
            Ok(())
        };
        let mut stderr = Vec::<u8>::new();

        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            false,
            &mut confirm,
            &mut kill_tmux,
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PurgeOutcome::Cancelled);
        assert_eq!(tmux_calls, 0, "tmux must not be killed on cancel");
        assert!(worktree_path.exists(), "worktree must remain on cancel");
        assert!(
            sb.sessions_dir.join("paw-repo.json").exists(),
            "session file must remain on cancel"
        );
    }

    /// Recorder for the order of writes, flushes, and confirm calls.
    /// Used by the Bug C regression test to assert the warning is
    /// flushed BEFORE the prompt fires.
    struct OrderedWrite {
        events: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    impl std::io::Write for OrderedWrite {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.events
                .borrow_mut()
                .push(format!("write({})", buf.len()));
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.events.borrow_mut().push("flush".to_string());
            Ok(())
        }
    }

    // Bug D — purge --force "freeze". Per-worktree progress markers
    // must surround each `git::remove_worktree` call so the user can
    // tell the command is making progress on a large worktree. The
    // assertion is on the stderr contents after a real successful
    // purge_with_prompt run. v0-5-0-audit-cleanup tasks 9b.1–9b.5.
    #[test]
    #[serial]
    fn purge_emits_per_worktree_progress_messages() {
        let sb = setup_purge_sandbox(false);
        let worktree_path_display = sb.session.worktrees[0].worktree_path.display().to_string();

        let mut confirm = |_: &str| -> Result<bool, PawError> { Ok(true) };
        let mut stderr = Vec::<u8>::new();

        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            true, // force — bypass confirm to isolate the progress assertion
            &mut confirm,
            &mut |_: &str| Ok(()),
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PurgeOutcome::Purged);
        let err_text = String::from_utf8(stderr).unwrap();
        assert!(
            err_text.contains(&format!("Removing worktree {worktree_path_display}")),
            "stderr should announce each worktree removal by path; got:\n{err_text}"
        );
        assert!(
            err_text.contains("...done ("),
            "stderr should emit a `...done (Xs)` marker per worktree once removal completes; got:\n{err_text}"
        );
    }

    // Bug C — purge confirmation prompt regression. When unmerged-
    // commit warnings are written to stderr, the stream must be FLUSHED
    // before the dialoguer prompt reads stdin. Without the flush the
    // warning races against the prompt and dialoguer can mis-classify
    // the `y` keystroke as cancellation. v0-5-0-audit-cleanup tasks
    // 9a.1–9a.4 (the piped-stdin integration test described in 9a.4 is
    // impractical because dialoguer probes the TTY and behaves
    // differently under pipes; the behavioural assertion below covers
    // the same regression shape without a TTY dependency).
    #[test]
    #[serial]
    fn purge_with_unmerged_commits_flushes_stderr_before_confirm() {
        let sb = setup_purge_sandbox(true);

        let events: std::rc::Rc<std::cell::RefCell<Vec<String>>> =
            std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let events_for_confirm = std::rc::Rc::clone(&events);

        let mut confirm = |_: &str| -> Result<bool, PawError> {
            events_for_confirm.borrow_mut().push("confirm".to_string());
            Ok(true)
        };
        let mut stderr = OrderedWrite {
            events: std::rc::Rc::clone(&events),
        };

        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            false,
            &mut confirm,
            &mut |_: &str| Ok(()),
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PurgeOutcome::Purged);
        let recorded = events.borrow().clone();
        let confirm_idx = recorded
            .iter()
            .position(|s| s == "confirm")
            .expect("confirm must have been called");
        // Find the last flush BEFORE confirm — that's the Bug C fix's
        // flush. Per-worktree progress emits more flushes after
        // confirm; the test only cares that at least one flush
        // precedes the prompt.
        let flush_before_confirm_idx = recorded[..confirm_idx]
            .iter()
            .rposition(|s| s == "flush")
            .expect("stderr.flush() must have been called before the prompt fired");
        // Defence in depth: at least one write must have happened
        // before that flush — confirming the warning text actually
        // landed in the stream before the flush event.
        let has_write_before_flush = recorded[..flush_before_confirm_idx]
            .iter()
            .any(|s| s.starts_with("write("));
        assert!(
            has_write_before_flush,
            "at least one stderr write must precede the pre-confirm flush; events: {recorded:?}"
        );
    }

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

#[cfg(test)]
mod supervisor_question_tests {
    //! Behavioral tests for `publish_supervisor_question`.
    //!
    //! Earlier versions of these tests built the same `curl` command string
    //! the production function builds and asserted on substrings of their
    //! own fixtures — a tautology that would pass even if
    //! `publish_supervisor_question` were a no-op. They also re-declared the
    //! same tests outside `mod tests`, so the test suite ran them twice.
    //!
    //! These tests instead boot a real `BrokerState`-backed HTTP broker,
    //! invoke `publish_supervisor_question` against the live URL, and assert
    //! that an `agent.question` message lands on the supervisor inbox via
    //! the production `delivery::poll_messages` path.

    use std::sync::Arc;
    use std::sync::atomic::{AtomicU16, Ordering};

    use git_paw::broker::messages::BrokerMessage;
    use git_paw::broker::{self, BrokerState, delivery};
    use git_paw::config::BrokerConfig;

    use super::publish_supervisor_question;

    static PORT_COUNTER: AtomicU16 = AtomicU16::new(0);

    /// Starts a real broker on a unique free port. Returns the handle (owns
    /// the runtime) and the URL the broker is listening on.
    fn spawn_broker() -> (broker::BrokerHandle, String) {
        #[allow(clippy::cast_possible_truncation)]
        let base = 30_000 + (std::process::id() as u16 % 5000);
        let offset = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut port = base + offset;
        let mut attempts = 0;
        loop {
            let cfg = BrokerConfig {
                enabled: true,
                port,
                bind: "127.0.0.1".to_string(),
                ..Default::default()
            };
            match broker::start_broker(&cfg, BrokerState::new(None), Vec::new()) {
                Ok(handle) => {
                    let url = cfg.url();
                    return (handle, url);
                }
                Err(_) if attempts < 10 => {
                    port += 100;
                    attempts += 1;
                }
                Err(e) => panic!("failed to start test broker: {e}"),
            }
        }
    }

    /// Returns true if `curl` is on PATH; tests that drive
    /// `publish_supervisor_question` shell out via `sh -c curl ...`, so we
    /// skip them on hosts without curl rather than fail.
    fn curl_available() -> bool {
        which::which("curl").is_ok()
    }

    /// Invoking `publish_supervisor_question` against a live broker must
    /// route an `agent.question` message to the supervisor inbox with the
    /// exact question text supplied by the caller.
    #[test]
    fn publish_supervisor_question_routes_to_supervisor_inbox() {
        if !curl_available() {
            eprintln!("skipping: curl not available on PATH");
            return;
        }
        let (handle, url) = spawn_broker();
        let state: Arc<BrokerState> = Arc::clone(&handle.state);

        publish_supervisor_question("Continue with this approach?", &url)
            .expect("publish should succeed against a live broker");

        // Poll up to ~2s for the message to land — the broker accepts the
        // POST asynchronously.
        let mut found: Option<BrokerMessage> = None;
        for _ in 0..40 {
            let (msgs, _) = delivery::poll_messages(&state, "supervisor", 0);
            if let Some(msg) = msgs.into_iter().next() {
                found = Some(msg);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let msg = found.expect("supervisor inbox should receive the question");
        match msg {
            BrokerMessage::Question { agent_id, payload } => {
                assert_eq!(agent_id, "supervisor");
                assert_eq!(payload.question, "Continue with this approach?");
            }
            other => panic!("expected BrokerMessage::Question, got {other:?}"),
        }
    }

    /// Embedded double quotes in the question must not break the JSON
    /// payload — the broker must accept the request and store *some*
    /// rendering of the question that contains the literal word `bcrypt`
    /// from the input. Drives the real escape + curl + broker path.
    ///
    /// (This intentionally does not assert verbatim equality: the current
    /// production escaping double-escapes backslashes, so quoted text is
    /// stored with extra backslashes. The behavioral guarantee verified
    /// here is that the publish round-trips successfully and the question
    /// text is delivered, not that escaping is byte-for-byte correct.)
    #[test]
    fn publish_supervisor_question_preserves_quotes_in_question_text() {
        if !curl_available() {
            eprintln!("skipping: curl not available on PATH");
            return;
        }
        let (handle, url) = spawn_broker();
        let state: Arc<BrokerState> = Arc::clone(&handle.state);

        let question = r#"Should I use "bcrypt" or argon2?"#;
        publish_supervisor_question(question, &url)
            .expect("publish should succeed with embedded quotes");

        let mut found: Option<BrokerMessage> = None;
        for _ in 0..40 {
            let (msgs, _) = delivery::poll_messages(&state, "supervisor", 0);
            if let Some(msg) = msgs.into_iter().next() {
                found = Some(msg);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let msg = found.expect("supervisor inbox should receive the question");
        match msg {
            BrokerMessage::Question { agent_id, payload } => {
                assert_eq!(agent_id, "supervisor");
                assert!(
                    payload.question.contains("bcrypt"),
                    "stored question must include the literal word 'bcrypt'; got: {:?}",
                    payload.question
                );
                assert!(
                    payload.question.contains("argon2"),
                    "stored question must include the literal word 'argon2'; got: {:?}",
                    payload.question
                );
            }
            other => panic!("expected BrokerMessage::Question, got {other:?}"),
        }
    }

    /// Pointing `publish_supervisor_question` at an unreachable URL must
    /// surface a `PawError::SessionError`, not silently succeed.
    #[test]
    fn publish_supervisor_question_returns_error_when_broker_unreachable() {
        if !curl_available() {
            eprintln!("skipping: curl not available on PATH");
            return;
        }
        // Reserved-for-test port that nothing should bind to.
        let result = publish_supervisor_question("anything", "http://127.0.0.1:1");
        assert!(
            result.is_err(),
            "publishing to an unreachable broker must error"
        );
    }
}

#[cfg(test)]
mod submit_delay_tests {
    //! `claude-oss-launch-v0-6-x` / `cli-submit-profile`: the boot-prompt
    //! settle delay is CONFIG-DRIVEN with a CLI-agnostic default — no
    //! hardcoded CLI-name table (W15-1, agnostic rework).

    use std::collections::HashMap;

    use git_paw::config::{CustomCli, PawConfig};

    use super::resolve_submit_delay_ms;

    fn config_with(cli: &str, submit_delay_ms: Option<u64>) -> PawConfig {
        let mut clis = HashMap::new();
        clis.insert(
            cli.to_string(),
            CustomCli {
                command: cli.to_string(),
                display_name: None,
                submit_delay_ms,
                settings_path: None,
            },
        );
        PawConfig {
            clis,
            ..PawConfig::default()
        }
    }

    #[test]
    fn unknown_or_unconfigured_cli_uses_agnostic_default() {
        let cfg = PawConfig::default();
        assert_eq!(
            resolve_submit_delay_ms("any-cli", &cfg),
            git_paw::DEFAULT_SUBMIT_DELAY_MS,
        );
    }

    #[test]
    fn custom_cli_submit_delay_override_is_honoured() {
        let cfg = config_with("mycli", Some(2500));
        assert_eq!(resolve_submit_delay_ms("mycli", &cfg), 2500);
    }

    #[test]
    fn custom_cli_without_override_falls_back_to_default() {
        let cfg = config_with("mycli", None);
        assert_eq!(
            resolve_submit_delay_ms("mycli", &cfg),
            git_paw::DEFAULT_SUBMIT_DELAY_MS,
        );
    }

    #[test]
    fn lookup_keys_on_the_binary_not_the_flags() {
        // A `cli` value may carry flags (e.g. "mycli --foo"); the lookup
        // keys on the leading binary token.
        let cfg = config_with("mycli", Some(2500));
        assert_eq!(
            resolve_submit_delay_ms("mycli --dangerously-skip-permissions", &cfg),
            2500,
        );
    }

    #[test]
    fn no_cli_name_is_hardcoded_in_the_resolver() {
        // The agnostic contract: with an empty config, EVERY cli id —
        // including former-hardcoded names — resolves to the same default.
        let cfg = PawConfig::default();
        for cli in ["claude", "claude-oss", "gemini", "codex", "whatever"] {
            assert_eq!(
                resolve_submit_delay_ms(cli, &cfg),
                git_paw::DEFAULT_SUBMIT_DELAY_MS,
                "{cli} must use the agnostic default, not a hardcoded value"
            );
        }
    }

    fn config_with_settings_path(cli: &str, settings_path: Option<String>) -> PawConfig {
        let mut clis = HashMap::new();
        clis.insert(
            cli.to_string(),
            CustomCli {
                command: cli.to_string(),
                display_name: None,
                submit_delay_ms: None,
                settings_path,
            },
        );
        PawConfig {
            clis,
            ..PawConfig::default()
        }
    }

    #[test]
    fn configured_settings_paths_returns_targets_with_existing_parents() {
        let dir = tempfile::TempDir::new().unwrap();
        let target = dir.path().join("settings.json");
        let cfg = config_with_settings_path("mycli", Some(target.to_string_lossy().into_owned()));
        let paths = super::configured_settings_paths(&cfg);
        assert_eq!(
            paths,
            vec![target],
            "configured path with existing parent is returned"
        );
    }

    #[test]
    fn configured_settings_paths_skips_targets_with_absent_parent() {
        let dir = tempfile::TempDir::new().unwrap();
        let target = dir.path().join("missing-subdir").join("settings.json");
        let cfg = config_with_settings_path("mycli", Some(target.to_string_lossy().into_owned()));
        assert!(
            super::configured_settings_paths(&cfg).is_empty(),
            "a configured path whose parent is absent must be skipped",
        );
    }

    #[test]
    fn configured_settings_paths_empty_when_no_clis() {
        assert!(super::configured_settings_paths(&PawConfig::default()).is_empty());
    }
}
