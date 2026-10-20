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

use git_paw::broker;
use git_paw::config::{self, PawConfig, SupervisorConfig};
use git_paw::detect;
use git_paw::error::PawError;
use git_paw::git;
use git_paw::interactive;
use git_paw::session::{self, Session, SessionMode, SessionStatus, WorktreeEntry};
use git_paw::surface::SessionLayout;
use git_paw::tmux;

use super::helpers::{
    agent_pane_offset, attach_session_logging, config_to_custom_defs, dashboard_command,
    orchestrator_entry_for_mode, to_interactive_cli, warn_if_broker_is_exposed,
};
use super::recover::recover_session;
use super::supervisor::cmd_supervisor;
use crate::{
    SpecMode, apply_spec_mode, attach_or_print_hint, build_task_prompt,
    combine_boot_and_task_prompt, gate_pane_or_fail_on_dialog, invalidate_if_stale,
    reattach_active_session_or_refuse, resolve_submit_delay_ms, submit_prompt_to_pane,
    write_repo_discovery_file,
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
        let effective = existing.effective_status(|name| {
            matches!(
                tmux::session_liveness_for(name, existing.created_at, existing.agent_pane_offset()),
                tmux::SessionLiveness::Alive
            )
        });
        match effective {
            SessionStatus::Paused => {
                println!(
                    "Restarting paused session '{}' (broker + reattach)...",
                    existing.session_name
                );
                return restart_from_pause(&repo_root, existing);
            }
            SessionStatus::Active => {
                return reattach_active_session_or_refuse(&existing.session_name);
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

    // Refuse a port held by another repository's broker as soon as the config
    // is known — before the branch and CLI pickers, so the user is never asked
    // to make choices for a launch that cannot proceed. Every agent pane would
    // otherwise be handed a GIT_PAW_BROKER_URL pointing into that other
    // repository's broker. A dry run creates nothing, so it never probes.
    if !dry_run {
        broker::ensure_port_free_for_repo(&config.broker, &repo_root)?;
    }

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
        config.default_cli.as_deref(),
    )?;

    // Refuse before anything is created when two of the session's branches
    // derive the same broker agent id — they would share one inbox. Ahead of
    // the dry run too, so a previewed plan is one that could actually launch.
    //
    // Only when the broker is enabled: an agent id exists to address an agent
    // ON the broker, so with it off nothing derives, publishes or routes by
    // one and a shared id has no observable effect. Refusing anyway would
    // reject launches a broker-less session has always accepted.
    if config.broker.enabled {
        let session_branches: Vec<&str> = selection
            .mappings
            .iter()
            .map(|(branch, _)| branch.as_str())
            .collect();
        if let Some(clash) = broker::messages::ensure_distinct_agent_ids(&session_branches, &[]) {
            return Err(PawError::ConfigError(clash.to_string()));
        }
    }

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
            let wt = git::worktree_display_path(&repo_root, branch, config.worktree_placement())?;
            println!("  {branch} \u{2192} {cli} ({wt})");
        }
        return Ok(());
    }

    // Create worktrees and build tmux session
    // Prune stale worktree registrations from previous sessions
    git::prune_worktrees(&repo_root)?;

    let broker_config = config.broker.clone();
    // Warn before the session exists: the dashboard pane prints the same
    // warning but immediately switches to the alternate screen.
    warn_if_broker_is_exposed(&broker_config);

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
            cli_command: dashboard_command(),
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

    let worktree_runtime = config.worktree_runtime();

    // Resolve approval flags per branch's CLI (supervisor-config: resolved
    // approval flags apply on every worker-launch path, bare `start` included).
    // No `[supervisor]` section is required for this to apply — a bare launch
    // has no supervisor at all, so the level falls back to `SupervisorConfig`'s
    // default (`agent_approval = "auto"`) exactly as `cmd_add` does.
    let default_sup = SupervisorConfig::default();
    let approval = &config
        .supervisor
        .as_ref()
        .unwrap_or(&default_sup)
        .agent_approval;

    for (branch, cli) in &selection.mappings {
        let wt = git::create_worktree(&repo_root, branch, !no_rebase, config.worktree_placement())?;
        let wt_str = wt.path.to_string_lossy().to_string();

        // Provision runtime state (declared env copies, this agent's port
        // block, the `on_create` hook) after the worktree exists and before its
        // pane is built, so the agent's CLI starts against a fully provisioned
        // checkout. Each agent takes the lowest slot no already-created peer
        // holds.
        let slot = git_paw::worktree_provision::allocate_slot(&worktree_entries);
        let provisioned = git_paw::worktree_provision::provision_worktree(
            &repo_root,
            &wt.path,
            branch,
            &worktree_runtime,
            slot,
        )?;
        for warning in &provisioned.warnings {
            eprintln!("warning: {branch}: {warning}");
        }
        if let Some(summary) = provisioned.hook_summary() {
            println!("  {branch}: {summary}");
        }

        // Provision the bundled helper scripts the agent's boot block invokes
        // into this worktree's `.git-paw/scripts/` (idempotent, executable),
        // with the same gates `attach_agent` uses — `broker.sh` when the broker
        // is enabled, `docs-fetch.sh` when `docs_base_url` is configured. A
        // fresh worktree checkout has none (the directory is gitignored), so
        // without this the agent's relative `.git-paw/scripts/broker.sh` call
        // fails until it hand-copies from `assets/`.
        git_paw::init::provision_worktree_helpers(
            &wt.path,
            broker_config.enabled,
            config.docs_base_url.is_some(),
        )?;

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

        let agent_flags = config::resolve_approval_flags(cli, approval, &config.clis);
        builder = builder.add_pane(tmux::PaneSpec {
            branch: branch.clone(),
            worktree: wt_str,
            cli_command: apply_approval_flags(
                &config::launch_command(cli, &config.clis),
                &agent_flags,
            ),
        });

        worktree_entries.push(WorktreeEntry {
            branch: branch.clone(),
            worktree_path: wt.path,
            cli: cli.clone(),
            branch_created: wt.branch_created,
            base_branch: crate::created_branch_base(&repo_root, wt.branch_created),
            pending_boot_prompt: None,
            runtime_slot: provisioned.runtime_slot,
            cli_session_id: None,
            // Bare launch: no spec drove this branch.
            spec_id: None,
        });
    }

    let mut tmux_session = builder.build()?;

    // Every reserved-index fact for this launch — the logging offset below,
    // the recorded `dashboard_pane`, the discovery-file offset and the
    // boot-block pane targets — comes from this one layout record.
    let layout = SessionLayout::planned(SessionMode::Bare, broker_config.enabled);

    // Attach session logging (no-op unless `[logging] enabled`). Pane offset
    // matches the boot-block/discovery offset below: dashboard occupies pane 0
    // when the broker is enabled, so coding agents start at pane 1.
    let logging_branches: Vec<&str> = selection.mappings.iter().map(|(b, _)| b.as_str()).collect();
    attach_session_logging(
        &mut tmux_session,
        &config,
        &repo_root,
        &logging_branches,
        layout.first_agent_index(),
    )?;

    // Execute tmux session
    tmux_session.execute()?;

    // Save session state and write the per-repo discovery file BEFORE
    // gating/injecting — matching the supervisor path's order (save, then
    // gate). A stuck dialog fails the gate-and-inject step below with `?`,
    // but the session and worktrees it already created must still be
    // discoverable: `git paw stop`/`purge` need the saved session, and
    // `sweep.sh` needs the discovery file, so a user can clean up or re-run
    // without the next launch forking a parallel `paw-<project>-2` session.
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
        // The layout already answers `None` for a broker-less launch, so this
        // needs no broker guard of its own.
        dashboard_pane: layout.recorded_dashboard_pane(),
    };

    if broker_config.enabled {
        state.broker_port = Some(broker_config.port);
        state.broker_bind = Some(broker_config.bind.clone());
        state.broker_log_path = Some(session::broker_log_path(&state.session_name)?);
    }

    session::save_session(&state)?;

    // Write the per-repo discovery file sweep.sh reads. In the bare layout
    // the dashboard occupies pane 0 when the broker is enabled, so coding
    // agents start at pane 1; without the broker they start at pane 0.
    write_repo_discovery_file(
        &state.repo_path,
        &tmux_session.name,
        &state.worktrees,
        layout.first_agent_index(),
        None, // bare-mode session: the layout reserves no supervisor surface
    );

    // Inject boot blocks for manual broker mode (without supervisor).
    // The argv shape is determined by `tmux::build_boot_inject_args` so the
    // call shape (literal mode, no trailing Enter, `-l` before `-t`) has a
    // single source of truth that tests can verify directly.
    //
    // Gated on launch-readiness first (`tmux-orchestration`'s Launch-readiness
    // gate requirement applies uniformly to the start-time path): a pane still
    // showing an unanswered first-run trust/bypass dialog MUST NOT receive the
    // boot block on top of it, so the launch fails loudly instead.
    if broker_config.enabled {
        for (idx, (branch, cli)) in selection.mappings.iter().enumerate() {
            let pane_idx = layout.agent_index(idx);
            let agent_flags = config::resolve_approval_flags(cli, approval, &config.clis);
            let cli_command =
                apply_approval_flags(&config::launch_command(cli, &config.clis), &agent_flags);
            gate_pane_or_fail_on_dialog(
                &tmux_session.name,
                pane_idx,
                cli,
                &cli_command,
                &config.clis,
            )?;
            let boot_block = git_paw::skills::build_boot_block(
                branch,
                &broker_config.url(),
                git_paw::skills::cli_speaks_mcp(cli, &config.clis),
            );

            let args =
                git_paw::tmux::build_boot_inject_args(&tmux_session.name, pane_idx, &boot_block);
            let _ = std::process::Command::new("tmux").args(&args).status();
        }
    }

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
        let effective = existing.effective_status(|name| {
            matches!(
                tmux::session_liveness_for(name, existing.created_at, existing.agent_pane_offset()),
                tmux::SessionLiveness::Alive
            )
        });
        match effective {
            SessionStatus::Paused => {
                println!(
                    "Restarting paused session '{}' (broker + reattach)...",
                    existing.session_name
                );
                return restart_from_pause(&repo_root, existing);
            }
            SessionStatus::Active => {
                return reattach_active_session_or_refuse(&existing.session_name);
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
            let wt = git::worktree_display_path(&repo_root, branch, config.worktree_placement())?;
            println!("  {branch} \u{2192} {cli} ({wt})");
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

    // Refuse before anything is created when two of the session's branches
    // derive the same broker agent id — they would share one inbox. Gated on
    // the broker being enabled for the reason given in `cmd_start`.
    if config.broker.enabled {
        let session_branches: Vec<&str> =
            mappings.iter().map(|(branch, _)| branch.as_str()).collect();
        if let Some(clash) = broker::messages::ensure_distinct_agent_ids(&session_branches, &[]) {
            return Err(PawError::ConfigError(clash.to_string()));
        }
    }

    // Prune stale worktree registrations from previous sessions
    git::prune_worktrees(repo_root)?;

    let broker_config = config.broker.clone();
    // Warn before the session exists: the dashboard pane prints the same
    // warning but immediately switches to the alternate screen.
    warn_if_broker_is_exposed(&broker_config);

    // See `cmd_start`: the port clash has to be caught before any pane exists.
    broker::ensure_port_free_for_repo(&broker_config, repo_root)?;

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
            cli_command: dashboard_command(),
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

    let worktree_runtime = config.worktree_runtime();

    // Resolve approval flags per branch's CLI (supervisor-config: resolved
    // approval flags apply on every worker-launch path, the spec-driven path
    // included). No `[supervisor]` section is required — a spec-driven
    // launch without `--supervisor` has none at all, so the level falls back
    // to `SupervisorConfig`'s default (`agent_approval = "auto"`), exactly
    // as `cmd_add` does.
    let default_sup = SupervisorConfig::default();
    let approval = &config
        .supervisor
        .as_ref()
        .unwrap_or(&default_sup)
        .agent_approval;

    for (branch, cli) in mappings {
        let wt = git::create_worktree(repo_root, branch, !no_rebase, config.worktree_placement())?;
        let wt_str = wt.path.to_string_lossy().to_string();

        // Same seam as `cmd_start`: provision after create, before the pane is
        // built, taking the lowest slot no already-created peer holds.
        let slot = git_paw::worktree_provision::allocate_slot(&worktree_entries);
        let provisioned = git_paw::worktree_provision::provision_worktree(
            repo_root,
            &wt.path,
            branch,
            &worktree_runtime,
            slot,
        )?;
        for warning in &provisioned.warnings {
            eprintln!("warning: {branch}: {warning}");
        }
        if let Some(summary) = provisioned.hook_summary() {
            println!("  {branch}: {summary}");
        }

        // Same seam as `cmd_start` / `attach_agent`: provision the bundled
        // helper scripts the agent's boot block invokes into this worktree's
        // `.git-paw/scripts/`, gated on the broker being enabled (`broker.sh`)
        // and `docs_base_url` being configured (`docs-fetch.sh`).
        git_paw::init::provision_worktree_helpers(
            &wt.path,
            broker_config.enabled,
            config.docs_base_url.is_some(),
        )?;

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

        let agent_flags = config::resolve_approval_flags(cli, approval, &config.clis);
        builder = builder.add_pane(tmux::PaneSpec {
            branch: branch.clone(),
            worktree: wt_str,
            cli_command: apply_approval_flags(
                &config::launch_command(cli, &config.clis),
                &agent_flags,
            ),
        });

        worktree_entries.push(WorktreeEntry {
            branch: branch.clone(),
            worktree_path: wt.path,
            cli: cli.clone(),
            branch_created: wt.branch_created,
            base_branch: crate::created_branch_base(repo_root, wt.branch_created),
            pending_boot_prompt: None,
            runtime_slot: provisioned.runtime_slot,
            cli_session_id: None,
            spec_id: spec_by_branch
                .get(branch.as_str())
                .map(|entry| entry.id.clone()),
        });
    }

    let mut tmux_session = builder.build()?;

    // Every reserved-index fact for this launch — the logging offset, the
    // recorded `dashboard_pane`, the discovery-file offset and the per-agent
    // prompt targets — comes from this one layout record.
    let layout = SessionLayout::planned(SessionMode::Bare, broker_config.enabled);

    // Attach session logging (no-op unless `[logging] enabled`). Pane indices
    // shift by 1 when the broker's dashboard occupies pane 0.
    let logging_branches: Vec<&str> = mappings.iter().map(|(b, _)| b.as_str()).collect();
    attach_session_logging(
        &mut tmux_session,
        config,
        repo_root,
        &logging_branches,
        layout.first_agent_index(),
    )?;

    // Execute tmux session
    tmux_session.execute()?;

    // Save session state and write the per-repo discovery file BEFORE
    // gating/injecting — matching the supervisor path's order (save, then
    // gate). A stuck dialog fails the gate-and-submit step below with `?`,
    // but the session and worktrees it already created must still be
    // discoverable: `git paw stop`/`purge` need the saved session, and
    // `sweep.sh` needs the discovery file, so a user can clean up or re-run
    // without the next launch forking a parallel `paw-<project>-2` session.
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
        // The layout already answers `None` for a broker-less launch, so this
        // needs no broker guard of its own.
        dashboard_pane: layout.recorded_dashboard_pane(),
    };

    if broker_config.enabled {
        state.broker_port = Some(broker_config.port);
        state.broker_bind = Some(broker_config.bind.clone());
        state.broker_log_path = Some(session::broker_log_path(&state.session_name)?);
    }

    session::save_session(&state)?;

    // Write the per-repo discovery file sweep.sh reads. Mirrors `cmd_start`:
    // in the bare layout the dashboard occupies pane 0 when the broker is
    // enabled, so coding agents start at pane 1; without the broker they
    // start at pane 0.
    write_repo_discovery_file(
        &state.repo_path,
        &tmux_session.name,
        &state.worktrees,
        layout.first_agent_index(),
        None, // bare-mode session: the layout reserves no supervisor surface
    );

    // Inject and submit each agent's prompt: the broker boot block (when
    // enabled) plus the task prompt naming its spec assignment and apply
    // invocation — byte-identical to what the supervisor path builds for the
    // same spec (`combine_boot_and_task_prompt`). Unlike the bare `cmd_start`
    // pre-fill (no human types a spec-driven task, so leaving it unsubmitted
    // would strand the agent), this submits via the same split-send +
    // settle-delay mechanism the supervisor path uses, and runs even when the
    // broker is disabled — the boot block is broker-specific and correctly
    // suppressed without one, but the agent's assignment is not.
    //
    // Gated on launch-readiness first, exactly as the supervisor path gates
    // each agent pane: this path now sends a real `Enter` (unlike the bare
    // pre-fill above), so a blind submit into a pane still showing an
    // unanswered first-run trust/bypass dialog could accidentally accept it.
    for (idx, (branch, cli)) in mappings.iter().enumerate() {
        let pane_idx = layout.agent_index(idx);
        let agent_flags = config::resolve_approval_flags(cli, approval, &config.clis);
        let cli_command =
            apply_approval_flags(&config::launch_command(cli, &config.clis), &agent_flags);
        gate_pane_or_fail_on_dialog(
            &tmux_session.name,
            pane_idx,
            cli,
            &cli_command,
            &config.clis,
        )?;
        let task_prompt = build_task_prompt(spec_by_branch.get(branch.as_str()).copied());
        let prompt = if broker_config.enabled {
            let boot_block = git_paw::skills::build_boot_block(
                branch,
                &broker_config.url(),
                git_paw::skills::cli_speaks_mcp(cli, &config.clis),
            );
            combine_boot_and_task_prompt(&boot_block, &task_prompt)
        } else {
            task_prompt
        };
        let delay = resolve_submit_delay_ms(cli, &config.clis);
        submit_prompt_to_pane(&tmux_session.name, pane_idx, &prompt, delay);
    }

    attach_or_print_hint(&tmux_session.name)
}

/// Restarts a paused session: recreates the dashboard pane (re-spawning
/// the broker), updates status to `Active`, and re-attaches the user's
/// client. Skips worktree creation, CLI spawning, and boot-prompt
/// injection — coding-agent panes are already running and retain their
/// in-memory conversation state.
pub(crate) fn restart_from_pause(repo_root: &Path, existing: &Session) -> Result<(), PawError> {
    tmux::ensure_tmux_installed()?;

    let dashboard_index = SessionLayout::of(existing).dashboard_index_or_default();

    let mut updated = existing.clone();
    // Re-stamp the broker log path, so a session paused under a git-paw that
    // used a single shared `broker.log` resumes onto the per-session name
    // instead of leaving the old file behind for purge to miss.
    //
    // This is saved BEFORE the dashboard pane starts below, because the
    // dashboard resolves its log path from the saved session: starting it
    // first would let it read the pre-re-stamp path and write there — the
    // orphaned log this re-stamp exists to prevent.
    //
    // `status` deliberately stays `Paused` for this write. Only the log path
    // is safe to publish before the dashboard exists; recording `Active` here
    // would mean a failure in the `split-window`/`send-keys` below left a
    // session marked active with tmux still alive, which the next
    // `git paw start` would reattach to instead of restarting the broker. The
    // flip to `Active` happens once the pane is up.
    if updated.broker_port.is_some() {
        updated.broker_log_path = Some(session::broker_log_path(&updated.session_name)?);
        session::save_session(&updated)?;
    }

    // Config is loaded BEST-EFFORT, once, for the three consumers below: the
    // broker-exposure warning, the pending-prompt submit delays, and the
    // discovery file's orchestrator entry. A resume recreates the dashboard
    // pane and re-spawns the broker, so a config that no longer parses must not
    // abort what follows — that would leave the session half-resumed, with no
    // saved state update and no discovery file. Every consumer has a sane
    // fallback (no warning, the built-in submit delay, no orchestrator entry),
    // so defaults are strictly better than an error here.
    let config = config::load_config(repo_root, None).unwrap_or_else(|e| {
        eprintln!(
            "warning: could not load config while resuming '{}' ({e}); \
             continuing with defaults for submit delays and the discovery file",
            updated.session_name
        );
        PawConfig::default()
    });

    // Recreate the dashboard pane only when the original session had a
    // broker enabled. Without a broker there is no dashboard pane to
    // recreate; pause+resume on a no-broker session is purely the tmux
    // detach/attach cycle.
    if existing.broker_port.is_some() {
        // Resuming re-spawns the broker, so the exposure warning belongs here
        // too — the dashboard pane's own copy is hidden by the alternate
        // screen. Best-effort, like the load above: a config that will not load
        // is the dashboard's problem to report, and must not break the resume.
        warn_if_broker_is_exposed(&config.broker);

        let dashboard_command = dashboard_command();
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

    // The dashboard pane is up (or there was never one to recreate), so the
    // session is genuinely running again. Every failure path above returned
    // early with the session still recorded `Paused`, which is what lets a
    // retry of `git paw start` restart the broker rather than reattach to a
    // session it believes is already active.
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
        for (idx, wt) in updated.worktrees.iter_mut().enumerate() {
            if let Some(pending) = wt.pending_boot_prompt.take() {
                let delay = resolve_submit_delay_ms(&wt.cli, &config.clis);
                submit_prompt_to_pane(&session_name, offset + idx, &pending, delay);
            }
        }
    }

    session::save_session(&updated)?;

    // Rewrite the per-repo discovery file `sweep.sh` reads. A resume does not
    // rebuild the layout: it splits a NEW dashboard pane into the live session
    // at whatever index tmux assigns, so every agent's pane index can shift
    // relative to what the original launch recorded. Read the live indices back
    // (`list-panes` with `pane_current_path`, matched to each worktree) rather
    // than assuming the launch-time offsets.
    let offset = agent_pane_offset(&updated);
    let agents = updated
        .worktrees
        .iter()
        .enumerate()
        .map(|(idx, wt)| session::RepoAgentEntry {
            branch_id: git_paw::broker::messages::slugify_branch(&wt.branch),
            worktree_path: wt.worktree_path.clone(),
            cli: wt.cli.clone(),
            pane_index: tmux::resolve_pane_index_for_worktree(
                &updated.session_name,
                &wt.worktree_path,
            )
            .ok()
            .flatten()
            .unwrap_or(offset + idx),
        })
        .collect();
    // The orchestrator entry is DERIVED from the resumed session's mode, as
    // recovery derives it — reading it back from the existing discovery file
    // would lose it exactly when the file is missing, which is the case this
    // rewrite exists for.
    let orchestrator = orchestrator_entry_for_mode(updated.mode, &config);
    crate::write_repo_discovery_file_with_agents(
        repo_root,
        &updated.session_name,
        agents,
        orchestrator,
    );

    attach_or_print_hint(&existing.session_name)
}

/// Appends `flags` to `command` with a single separating space, or returns
/// `command` unchanged when `flags` is empty — no trailing separator —
/// matching the supervisor path's (`attach_agent`'s) pane-command
/// construction.
///
/// `command` is the pane's *launch command* (see
/// [`config::launch_command`]), not the CLI identity: approval flags are
/// resolved by identity but appended after the command the pane runs.
fn apply_approval_flags(command: &str, flags: &str) -> String {
    if flags.is_empty() {
        command.to_string()
    } else {
        format!("{command} {flags}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_approval_flags_empty_yields_bare_cli_no_trailing_separator() {
        assert_eq!(apply_approval_flags("echo", ""), "echo");
    }

    #[test]
    fn apply_approval_flags_non_empty_joins_with_single_space() {
        assert_eq!(
            apply_approval_flags("echo", "--full-auto"),
            "echo --full-auto"
        );
    }
}
