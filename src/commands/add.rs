//! `git paw add <branch>` — hot-attach a worktree + agent pane to a running
//! supervisor-mode session. Extracted verbatim from `main.rs`
//! (code-analysis-refactor R2c).
//!
//! The shared attach primitives (`attach_agent`, `AttachContext`,
//! `AttachedAgent`) and the launch helpers (`resolve_submit_delay_ms`,
//! `submit_prompt_to_pane`, `write_repo_discovery_file`) remain in `main.rs`
//! (relocated with the supervisor cluster in R2d) and are referenced through
//! the crate root.

use git_paw::broker;
use git_paw::config::{self, SupervisorConfig};
use git_paw::detect;
use git_paw::error::PawError;
use git_paw::git;
use git_paw::session::{self, SessionMode, SessionStatus};
use git_paw::tmux;

use super::helpers::{agent_pane_offset, bare_mode_unsupported, config_to_custom_defs};
use crate::{
    AttachContext, AttachedAgent, attach_agent, resolve_submit_delay_ms, submit_prompt_to_pane,
    write_repo_discovery_file,
};

/// `git paw add <branch>` — hot-attach a worktree + agent pane to a running
/// session (capability `add-branch`).
#[allow(clippy::too_many_lines)]
pub(crate) fn cmd_add(
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
    let agent_flags = config::resolve_approval_flags(&agent_cli, approval, &config.clis);
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
    // Gated on explicit `docs_base_url` (see cmd_supervisor) so an added agent
    // is byte-identical to a start-time one.
    let docs_fetch_template = if config.docs_base_url.is_some() {
        Some(git_paw::skills::resolve("docs-fetch")?)
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
        agent_flags: &agent_flags,
        coordination_template: coordination_template.as_ref(),
        docs_fetch_template: docs_fetch_template.as_ref(),
        gate_commands: &gate_commands,
        session_backends: &session_backends,
        inter_agent_rules: Some(inter_agent_rules.as_str()),
        strict_guard,
        no_rebase: false,
        placement: config.worktree_placement(),
        common_dev_allowlist: &supervisor_cfg.common_dev_allowlist,
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
             `git paw start`.",
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
