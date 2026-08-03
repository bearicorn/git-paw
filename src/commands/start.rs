//! `git paw start` and `git paw start --from-specs` — the session-launch
//! orchestration: fresh launch, reattach, recover, restart-from-pause, and the
//! spec-driven launch path. Extracted verbatim from `main.rs`
//! (code-analysis-refactor R2c).
//!
//! `cmd_supervisor` lives in [`super::supervisor`] (the supervisor cluster,
//! R2d). The remaining shared helpers — `apply_spec_mode`,
//! `attach_or_print_hint`, `invalidate_if_stale` (purge cluster, R2b),
//! `resolve_submit_delay_ms`, `submit_prompt_to_pane`,
//! `write_repo_discovery_file`, and the `SpecMode` dispatch enum — remain in
//! `main.rs` and are referenced through the crate root.

use std::path::Path;
use std::process::Command as StdCommand;
use std::time::SystemTime;

use git_paw::config::{self, PawConfig, SupervisorConfig};
use git_paw::detect;
use git_paw::error::PawError;
use git_paw::git;
use git_paw::interactive;
use git_paw::session::{self, Session, SessionMode, SessionStatus, WorktreeEntry};
use git_paw::tmux;

use super::helpers::{
    agent_pane_offset, attach_session_logging, config_to_custom_defs, to_interactive_cli,
};
use super::recover::recover_session;
use super::supervisor::cmd_supervisor;
use crate::{
    SpecMode, apply_spec_mode, attach_or_print_hint, invalidate_if_stale, resolve_submit_delay_ms,
    submit_prompt_to_pane, write_repo_discovery_file,
};

/// Smart start: reattach if active, recover if stale, launch fresh if new.
#[allow(clippy::too_many_lines)]
pub(crate) fn cmd_start(
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
        // This config-driven handoff is only reachable when `--unattended` was
        // NOT passed: `--unattended` resolves supervisor mode active in the
        // dispatch, which routes straight to `cmd_supervisor` and never falls
        // through to `cmd_start`. So the drive loop is off (`unattended: false`)
        // on this path.
        return cmd_supervisor(
            &repo_root,
            &config,
            cli_flag.as_deref(),
            branches_flag.as_deref(),
            &SpecMode::None,
            None,
            dry_run,
            no_rebase,
            false,
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

    let mut tmux_session = builder.build()?;

    // Attach session logging (no-op unless `[logging] enabled`). Pane offset
    // matches the boot-block/discovery offset below: dashboard occupies pane 0
    // when the broker is enabled, so coding agents start at pane 1.
    let logging_branches: Vec<&str> = selection.mappings.iter().map(|(b, _)| b.as_str()).collect();
    attach_session_logging(
        &mut tmux_session,
        &config,
        &repo_root,
        &logging_branches,
        usize::from(broker_config.enabled),
    )?;

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

/// Launches sessions from spec files instead of interactive branch selection.
pub(crate) fn cmd_start_with_specs(
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

    // Attach session logging (no-op unless `[logging] enabled`). Pane indices
    // shift by 1 when the broker's dashboard occupies pane 0.
    let logging_branches: Vec<&str> = mappings.iter().map(|(b, _)| b.as_str()).collect();
    attach_session_logging(
        &mut tmux_session,
        config,
        repo_root,
        &logging_branches,
        usize::from(broker_config.enabled),
    )?;

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
