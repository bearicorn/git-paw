//! git-paw — Parallel AI Worktrees.
//!
//! Orchestrates multiple AI coding CLI sessions across git worktrees
//! from a single terminal using tmux.

use std::path::Path;
use std::time::SystemTime;

use clap::Parser;
use dialoguer::Confirm;

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
                eprintln!("error: --from-specs is not yet implemented");
                std::process::exit(1);
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
        Command::Init => {
            eprintln!("error: init is not yet implemented");
            std::process::exit(1);
        }
        Command::Replay { .. } => {
            eprintln!("error: replay is not yet implemented");
            std::process::exit(1);
        }
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
fn cmd_start(
    cli_flag: Option<String>,
    branches_flag: Option<Vec<String>>,
    dry_run: bool,
    preset: Option<&str>,
) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;

    // Check for existing session
    if let Some(existing) = session::find_session_for_repo(&repo_root)? {
        let alive = tmux::is_session_alive(&existing.session_name)?;

        if alive {
            // Active session — reattach
            println!("Reattaching to session '{}'...", existing.session_name);
            return tmux::attach(&existing.session_name);
        }

        // Stopped/stale session — recover
        println!("Recovering session '{}'...", existing.session_name);
        return recover_session(&repo_root, &existing);
    }

    // No session — fresh launch
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
    let mut builder = tmux::TmuxSessionBuilder::new(&project)
        .session_name(session_name)
        .mouse_mode(mouse);
    let mut worktree_entries = Vec::new();

    for (branch, cli) in &selection.mappings {
        let wt_path = git::create_worktree(&repo_root, branch)?;
        let wt_str = wt_path.to_string_lossy().to_string();

        builder = builder.add_pane(tmux::PaneSpec {
            branch: branch.clone(),
            worktree: wt_str,
            cli_command: cli.clone(),
        });

        worktree_entries.push(WorktreeEntry {
            branch: branch.clone(),
            worktree_path: wt_path,
            cli: cli.clone(),
        });
    }

    let tmux_session = builder.build()?;

    // Execute tmux session
    tmux_session.execute()?;

    // Save session state
    let state = Session {
        session_name: tmux_session.name.clone(),
        repo_path: repo_root,
        project_name: project,
        created_at: SystemTime::now(),
        status: SessionStatus::Active,
        worktrees: worktree_entries,
    };
    session::save_session(&state)?;

    // Attach
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
