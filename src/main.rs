//! git-paw — Parallel AI Worktrees.
//!
//! Orchestrates multiple AI coding CLI sessions across git worktrees
//! from a single terminal using tmux.

use std::path::Path;
use std::time::SystemTime;

use clap::Parser;
use dialoguer::Confirm;

use git_paw::broker;
use git_paw::cli::{Cli, Command};
use git_paw::config::{self, PawConfig};
use git_paw::detect;
use git_paw::error::PawError;
use git_paw::git;
use git_paw::interactive;
use git_paw::session::{self, Session, SessionStatus, WorktreeEntry};
use git_paw::tmux;

fn main() {
    let args = Cli::parse();

    let command = args.command.unwrap_or(Command::Start {
        cli: None,
        branches: None,
        from_specs: false,
        dry_run: false,
        preset: None,
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
            from_specs,
            dry_run,
            preset,
        } => {
            if from_specs {
                return cmd_start_from_specs(cli_flag.as_deref(), dry_run);
            }
            cmd_start(cli_flag, branches_flag, dry_run, preset.as_deref())
        }
        Command::Stop => cmd_stop(),
        Command::Purge { force } => cmd_purge(force),
        Command::Status => cmd_status(),
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
    }
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
) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    // Check for existing session (skip reattach/recovery during dry-run)
    let existing_session = session::find_session_for_repo(&repo_root)?;
    if !dry_run && let Some(existing) = &existing_session {
        let alive = tmux::is_session_alive(&existing.session_name)?;

        if alive {
            println!("Reattaching to session '{}'...", existing.session_name);
            return tmux::attach(&existing.session_name);
        }

        println!("Recovering session '{}'...", existing.session_name);
        return recover_session(&repo_root, existing);
    }

    // Fresh launch (or dry-run preview)
    tmux::ensure_tmux_installed()?;
    let config = config::load_config(&repo_root)?;
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
        .mouse_mode(mouse);

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
        let wt = git::create_worktree(&repo_root, branch)?;
        let wt_str = wt.path.to_string_lossy().to_string();

        // Inject AGENTS.md with skill content when broker is enabled
        let rendered_skill = skill_content
            .as_ref()
            .map(|tmpl| git_paw::skills::render(tmpl, branch, &broker_config.url()));
        let assignment = git_paw::agents::WorktreeAssignment {
            branch: branch.clone(),
            cli: cli.clone(),
            spec_content: None,
            owned_files: None,
            skill_content: rendered_skill,
        };
        git_paw::agents::setup_worktree_agents_md(&repo_root, &wt.path, &assignment)?;

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
        });
    }

    let tmux_session = builder.build()?;

    // Execute tmux session
    tmux_session.execute()?;

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
    };

    if broker_config.enabled {
        state.broker_port = Some(broker_config.port);
        state.broker_bind = Some(broker_config.bind.clone());
        state.broker_log_path = Some(session::session_state_dir()?.join("broker.log"));
    }

    session::save_session(&state)?;

    // Attach
    tmux::attach(&tmux_session.name)
}

// ---------------------------------------------------------------------------
// Command: start --from-specs
// ---------------------------------------------------------------------------

/// Launches sessions from spec files instead of interactive branch selection.
fn cmd_start_from_specs(cli_flag: Option<&str>, dry_run: bool) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    // Check for existing session (skip reattach/recovery during dry-run)
    let existing_session = session::find_session_for_repo(&repo_root)?;
    if !dry_run && let Some(existing) = &existing_session {
        let alive = tmux::is_session_alive(&existing.session_name)?;

        if alive {
            println!("Reattaching to session '{}'...", existing.session_name);
            return tmux::attach(&existing.session_name);
        }

        println!("Recovering session '{}'...", existing.session_name);
        return recover_session(&repo_root, existing);
    }

    // Fresh launch from specs (or dry-run preview)
    tmux::ensure_tmux_installed()?;
    let config = config::load_config(&repo_root)?;

    // Scan for pending specs
    let specs = git_paw::specs::scan_specs(&config, &repo_root)?;
    if specs.is_empty() {
        println!("No pending specs found.");
        return Ok(());
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
    )
}

/// Creates worktrees, sets up AGENTS.md, builds the tmux session, and attaches.
fn launch_spec_session(
    repo_root: &std::path::Path,
    config: &PawConfig,
    mappings: &[(String, String)],
    spec_by_branch: &std::collections::HashMap<&str, &git_paw::specs::SpecEntry>,
    project: &str,
    mouse: bool,
) -> Result<(), PawError> {
    let session_name = tmux::resolve_session_name(project)?;

    // Prune stale worktree registrations from previous sessions
    git::prune_worktrees(repo_root)?;

    let broker_config = config.broker.clone();

    let mut builder = tmux::TmuxSessionBuilder::new(project)
        .session_name(session_name)
        .mouse_mode(mouse);

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

    for (branch, cli) in mappings {
        let wt = git::create_worktree(repo_root, branch)?;
        let wt_str = wt.path.to_string_lossy().to_string();

        // Set up AGENTS.md with spec + skill content
        let rendered_skill = skill_template
            .as_ref()
            .map(|tmpl| git_paw::skills::render(tmpl, branch, &broker_config.url()));

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
        };
        git_paw::agents::setup_worktree_agents_md(repo_root, &wt.path, &assignment)?;

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
    };

    if broker_config.enabled {
        state.broker_port = Some(broker_config.port);
        state.broker_bind = Some(broker_config.bind.clone());
        state.broker_log_path = Some(session::session_state_dir()?.join("broker.log"));
    }

    session::save_session(&state)?;

    tmux::attach(&tmux_session.name)
}

/// Recovers a stopped/stale session by recreating the tmux session from saved state.
fn recover_session(repo_root: &Path, existing: &Session) -> Result<(), PawError> {
    tmux::ensure_tmux_installed()?;
    let config = config::load_config(repo_root)?;
    let mouse = config.mouse.unwrap_or(true);

    let mut builder = tmux::TmuxSessionBuilder::new(&existing.project_name).mouse_mode(mouse);

    for entry in &existing.worktrees {
        builder = builder.add_pane(tmux::PaneSpec {
            branch: entry.branch.clone(),
            worktree: entry.worktree_path.to_string_lossy().to_string(),
            cli_command: entry.cli.clone(),
        });
    }

    let tmux_session = builder.build()?;
    tmux_session.execute()?;

    // Update session status
    let mut updated = existing.clone();
    updated.status = SessionStatus::Active;
    session::save_session(&updated)?;

    tmux::attach(&tmux_session.name)
}

// ---------------------------------------------------------------------------
// Command: __dashboard
// ---------------------------------------------------------------------------

/// Runs the broker and dashboard in pane 0 (internal command).
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
    let config = config::load_config(&repo_root)?;
    let broker_config = config.broker;

    let log_path = session::session_state_dir()?.join("broker.log");
    let broker_state = broker::BrokerState::new(Some(log_path));
    let handle = broker::start_broker(&broker_config, broker_state)?;
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

    git_paw::dashboard::run_dashboard(&state, handle, &shutdown)
}

// ---------------------------------------------------------------------------
// Command: stop
// ---------------------------------------------------------------------------

/// Stops the session: kills tmux but preserves worktrees and state.
fn cmd_stop() -> Result<(), PawError> {
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

/// Removes everything: tmux session, worktrees, and state.
fn cmd_purge(force: bool) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    let Some(existing) = session::find_session_for_repo(&repo_root)? else {
        println!("No session to purge for this repo.");
        return Ok(());
    };

    if !force {
        let confirmed = Confirm::new()
            .with_prompt(
                "This will remove the tmux session, all worktrees, and session state. Continue?",
            )
            .default(false)
            .interact()
            .map_err(|_| PawError::UserCancelled)?;

        if !confirmed {
            return Err(PawError::UserCancelled);
        }
    }

    // Kill tmux if alive
    if tmux::is_session_alive(&existing.session_name)? {
        tmux::kill_session(&existing.session_name)?;
    }

    // Remove worktrees
    for entry in &existing.worktrees {
        if let Err(e) = git::remove_worktree(&repo_root, &entry.worktree_path) {
            eprintln!(
                "warning: failed to remove worktree '{}': {e}",
                entry.worktree_path.display()
            );
        }
    }

    // Delete branches that git-paw created (best-effort)
    for entry in &existing.worktrees {
        if entry.branch_created
            && let Err(e) = git::delete_branch(&repo_root, &entry.branch)
        {
            eprintln!("warning: failed to delete branch '{}': {e}", entry.branch);
        }
    }

    // Delete broker log if it exists (best-effort)
    if let Some(ref log_path) = existing.broker_log_path {
        let _ = std::fs::remove_file(log_path);
    }

    // Delete session state
    session::delete_session(&existing.session_name)?;

    println!("Purged session '{}'.", existing.session_name);
    Ok(())
}

// ---------------------------------------------------------------------------
// Command: status
// ---------------------------------------------------------------------------

/// Shows session state for the current repo.
fn cmd_status() -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    let Some(existing) = session::find_session_for_repo(&repo_root)? else {
        println!("No session for this repo.");
        return Ok(());
    };

    let alive = tmux::is_session_alive(&existing.session_name)?;
    let effective = existing.effective_status(|name| tmux::is_session_alive(name).unwrap_or(false));

    let status_icon = match effective {
        SessionStatus::Active => "\u{1f7e2}",  // 🟢
        SessionStatus::Stopped => "\u{1f7e1}", // 🟡
    };

    println!("Session: {}", existing.session_name);
    println!("Status:  {status_icon} {effective}");
    println!("Tmux:    {}", if alive { "running" } else { "not running" });
    println!();

    // Broker info
    if let (Some(bind), Some(port)) = (&existing.broker_bind, existing.broker_port) {
        let url = format!("http://{bind}:{port}");
        match broker::probe_broker(&url) {
            broker::ProbeResult::LiveBroker => println!("Broker:  {url} (running)"),
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
    let config = config::load_config(&repo_root)?;
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
        let config = config::load_config(&repo_root)?;
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
