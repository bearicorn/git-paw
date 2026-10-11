//! `git paw start` recovery path — rebuilds the tmux session from saved
//! `session.json` state when the session is stopped or stale. Extracted
//! verbatim from `main.rs` (code-analysis-refactor R2c).
//!
//! `resolve_supervisor_flags` lives in [`super::supervisor`] (the supervisor
//! cluster, R2d) and `attach_or_print_hint` remains in `main.rs`; both are
//! referenced here across module boundaries.

use std::collections::HashMap;
use std::path::Path;

use git_paw::broker;
use git_paw::config::{self, ApprovalLevel, CustomCli, PawConfig, SupervisorConfig};
use git_paw::error::PawError;
use git_paw::session::{self, Session, SessionMode, SessionStatus, WorktreeEntry};
use git_paw::tmux;

use super::helpers::{dashboard_command, warn_if_broker_is_exposed};
use super::supervisor::resolve_supervisor_flags;
use crate::attach_or_print_hint;

/// Builds the recovery-time CLI launch command for `entry`: the bare CLI id,
/// the resolved approval flags for `approval` (`supervisor-config`'s "every
/// launch path that spawns a worker CLI" requirement — recovery is a launch
/// path like any other), and, when the entry carries an assigned
/// `cli_session_id` AND its CLI resolves a `resume_args` template, the
/// rendered resume arguments — restoring the worker's conversation context
/// (`worker-session-id-capture` design D4). Resume args are appended AFTER
/// the approval flags, mirroring how `attach_agent` appends session-id args
/// after the bare CLI command.
///
/// Recovery never fails because a resume template is absent: an entry
/// without an id, or a CLI without a resume template, launches with just its
/// approval flags, exactly as every other launch path does for an
/// unconfigured CLI.
fn cli_command_for_recovery<S: std::hash::BuildHasher>(
    entry: &WorktreeEntry,
    approval: ApprovalLevel,
    clis: &HashMap<String, CustomCli, S>,
) -> String {
    let agent_flags = config::resolve_approval_flags(&entry.cli, &approval, clis);
    let mut cli_command = if agent_flags.is_empty() {
        entry.cli.clone()
    } else {
        format!("{} {agent_flags}", entry.cli)
    };
    if let Some(id) = entry.cli_session_id.as_deref()
        && let Some(args) = config::resolve_resume_args(&entry.cli, id, clis)
    {
        cli_command.push(' ');
        cli_command.push_str(&args);
    }
    cli_command
}

/// Recovers a stopped/stale session by recreating the tmux session from saved state.
pub(crate) fn recover_session(repo_root: &Path, existing: &Session) -> Result<(), PawError> {
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

    // Recovery rebuilds the agent panes with `GIT_PAW_BROKER_URL` built from
    // the SAVED broker fields, which can differ from the current config when
    // `[broker] port` changed since the session was saved. Probe the port the
    // agents will actually be handed, not the configured one — checking the
    // wrong port would let the clash `cmd_start` refuses up front slip
    // through here.
    let recovery_broker_config = match (existing.broker_port, existing.broker_bind.as_ref()) {
        (Some(port), Some(bind)) => config::BrokerConfig {
            enabled: true,
            port,
            bind: bind.clone(),
            ..config.broker.clone()
        },
        // No saved broker fields: the rebuilt panes get no broker URL at all,
        // so there is nothing to clash over.
        _ => config::BrokerConfig {
            enabled: false,
            ..config.broker.clone()
        },
    };
    broker::ensure_port_free_for_repo(&recovery_broker_config, repo_root)?;

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

    // No `[supervisor]` section is required for this to apply — a bare
    // (non-supervisor) recovery has none at all, so the level falls back to
    // `SupervisorConfig`'s default (`agent_approval = "auto"`), exactly as
    // `cmd_start` and `cmd_add` do.
    let default_sup = SupervisorConfig::default();
    let bare_approval = config
        .supervisor
        .as_ref()
        .unwrap_or(&default_sup)
        .agent_approval;

    let (tmux_session, pane_offset, orchestrator) = match mode {
        SessionMode::Supervisor => {
            recover_supervisor_session(repo_root, existing, &config, broker_url.as_deref(), mouse)?
        }
        SessionMode::Bare => recover_bare_session(
            repo_root,
            existing,
            broker_url.as_deref(),
            mouse,
            config.border_affordances_enabled(),
            bare_approval,
            &config.clis,
        )?,
    };

    // Update session status + record the resolved mode.
    let mut updated = existing.clone();
    updated.status = SessionStatus::Active;
    updated.mode = mode;
    // Re-stamp the broker log path. A session saved by a git-paw that used a
    // single shared `broker.log` would otherwise have the dashboard write the
    // per-session name while purge deleted the old one, orphaning a log.
    if updated.broker_port.is_some() {
        updated.broker_log_path = Some(session::broker_log_path(&updated.session_name)?);
    }
    // Save BEFORE the dashboard pane starts. The dashboard resolves its log
    // path from the saved session, so launching first would let it read the
    // pre-re-stamp path and write there — the orphaned log this re-stamp
    // exists to prevent.
    session::save_session(&updated)?;

    // A recovery rebuilds the session, broker and all, so it is a launch for
    // the purposes of the exposure warning — and it is the common path after a
    // reboot or a tmux crash. The dashboard pane's own copy is hidden by the
    // alternate screen, so warn here, immediately before the pane that starts
    // the broker comes up.
    warn_if_broker_is_exposed(&config.broker);

    tmux_session.execute()?;

    // Rewrite the per-repo discovery file `sweep.sh` reads. Recovery REBUILDS
    // the layout, so a file written by the original launch records pane
    // indices that no longer describe the live session (and a session recovered
    // after the file was removed has none at all) — `session-state` requires
    // every path that creates or rebuilds the session's panes to (re)write it.
    // The offsets come from the layout just rebuilt above.
    crate::write_repo_discovery_file(
        repo_root,
        &tmux_session.name,
        &updated.worktrees,
        pane_offset,
        orchestrator,
    );

    attach_or_print_hint(&tmux_session.name)
}

/// Rebuild a bare-mode (non-supervisor) session: dashboard at pane 0 (when
/// broker enabled), coding agents at pane 1+.
///
/// Returns the built session together with the agent pane offset of the
/// rebuilt layout and its orchestrator entry (`None` — a bare session has no
/// orchestrator pane), so [`recover_session`] can rewrite the per-repo
/// discovery file against the layout it actually produced.
fn recover_bare_session<S: std::hash::BuildHasher>(
    repo_root: &Path,
    existing: &Session,
    broker_url: Option<&str>,
    mouse: bool,
    border_affordances: bool,
    approval: ApprovalLevel,
    clis: &HashMap<String, CustomCli, S>,
) -> Result<
    (
        tmux::TmuxSession,
        usize,
        Option<session::RepoOrchestratorEntry>,
    ),
    PawError,
> {
    let mut builder = tmux::TmuxSessionBuilder::new(&existing.project_name)
        .session_name(existing.session_name.clone())
        .mouse_mode(mouse)
        .border_affordances(border_affordances);

    if let Some(url) = broker_url {
        let repo_str = repo_root.to_string_lossy().to_string();
        builder = builder.add_pane(tmux::PaneSpec {
            branch: "dashboard".to_string(),
            worktree: repo_str,
            cli_command: dashboard_command(),
        });
        builder = builder.set_environment("GIT_PAW_BROKER_URL", url);
    }

    for entry in &existing.worktrees {
        builder = builder.add_pane(tmux::PaneSpec {
            branch: entry.branch.clone(),
            worktree: entry.worktree_path.to_string_lossy().to_string(),
            cli_command: cli_command_for_recovery(entry, approval, clis),
        });
    }

    // The dashboard occupies pane 0 when the broker is enabled, so coding
    // agents start at pane 1; without it they start at pane 0.
    let pane_offset = usize::from(broker_url.is_some());
    Ok((builder.build()?, pane_offset, None))
}

/// Rebuild a supervisor-mode session with the v0.5 layout: supervisor at pane
/// 0, dashboard at pane 1, coding agents at pane 2+.
///
/// Returns the built session together with the agent pane offset of the
/// rebuilt layout and the orchestrator entry describing the supervisor's own
/// pane, so [`recover_session`] can rewrite the per-repo discovery file
/// against the layout it actually produced — including the orchestrator entry
/// a supervisor-mode discovery file carries.
fn recover_supervisor_session(
    repo_root: &Path,
    existing: &Session,
    config: &PawConfig,
    broker_url: Option<&str>,
    mouse: bool,
) -> Result<
    (
        tmux::TmuxSession,
        usize,
        Option<session::RepoOrchestratorEntry>,
    ),
    PawError,
> {
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
    // Same effective-level resolution as the auto-start flow: the
    // supervisor's own `approval` when set, else `agent_approval`.
    let supervisor_approval = supervisor_cfg
        .approval
        .unwrap_or(supervisor_cfg.agent_approval);
    let supervisor_flags =
        resolve_supervisor_flags(&supervisor_cli, supervisor_approval, &config.clis);
    let supervisor_cli_command = if supervisor_flags.is_empty() {
        supervisor_cli.clone()
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
        cli_command: dashboard_command(),
    };
    let agent_panes: Vec<tmux::PaneSpec> = existing
        .worktrees
        .iter()
        .map(|entry| tmux::PaneSpec {
            branch: entry.branch.clone(),
            worktree: entry.worktree_path.to_string_lossy().to_string(),
            cli_command: cli_command_for_recovery(
                entry,
                supervisor_cfg.agent_approval,
                &config.clis,
            ),
        })
        .collect();

    let env_vars: Vec<(String, String)> = broker_url
        .map(|url| vec![("GIT_PAW_BROKER_URL".to_string(), url.to_string())])
        .unwrap_or_default();

    let session = tmux::build_supervisor_session(
        &existing.project_name,
        Some(existing.session_name.clone()),
        &supervisor_pane,
        &dashboard_pane,
        &agent_panes,
        layout,
        mouse,
        config.border_affordances_enabled(),
        &env_vars,
    )?;

    Ok((
        session,
        git_paw::supervisor::layout::SUPERVISOR_PANE_OFFSET,
        Some(session::RepoOrchestratorEntry {
            cli: supervisor_cli,
            pane_index: 0,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn entry(cli: &str, cli_session_id: Option<&str>) -> WorktreeEntry {
        WorktreeEntry {
            branch: "feat/x".to_string(),
            worktree_path: PathBuf::from("/tmp/wt-feat-x"),
            cli: cli.to_string(),
            branch_created: false,
            base_branch: None,
            pending_boot_prompt: None,
            runtime_slot: None,
            cli_session_id: cli_session_id.map(String::from),
            spec_id: None,
        }
    }

    /// Spec scenario "Recovered Claude worker is resumed". `Manual` resolves
    /// to no flags for every CLI (built-in table), keeping this test's
    /// assertion scoped to resume-arg behaviour alone — approval-flag
    /// composition is covered separately below.
    #[test]
    fn recovered_claude_worker_command_carries_resume_args() {
        let e = entry("claude", Some("3f2c1a90-aaaa-4bbb-8ccc-1234567890ab"));
        let clis = HashMap::new();
        assert_eq!(
            cli_command_for_recovery(&e, ApprovalLevel::Manual, &clis),
            "claude --resume 3f2c1a90-aaaa-4bbb-8ccc-1234567890ab"
        );
    }

    /// Spec scenario "Entry without an id launches fresh".
    #[test]
    fn entry_without_an_id_launches_fresh() {
        let e = entry("claude", None);
        let clis = HashMap::new();
        assert_eq!(
            cli_command_for_recovery(&e, ApprovalLevel::Manual, &clis),
            "claude"
        );
    }

    /// Spec scenario "CLI without a resume template launches fresh".
    #[test]
    fn cli_without_a_resume_template_launches_fresh() {
        let e = entry("my-agent", Some("3f2c1a90-aaaa-4bbb-8ccc-1234567890ab"));
        let clis = HashMap::new();
        assert_eq!(
            cli_command_for_recovery(&e, ApprovalLevel::Manual, &clis),
            "my-agent"
        );
    }

    /// `supervisor-config`'s ADDED requirement ("resolved approval flags
    /// apply on every worker-launch path") covers recovery too: a CLI whose
    /// resolved flags for `approval` are non-empty carries them in the
    /// recovered pane's command.
    #[test]
    fn recovered_worker_command_carries_resolved_approval_flags() {
        let e = entry("claude", None);
        let clis = HashMap::new();
        assert_eq!(
            cli_command_for_recovery(&e, ApprovalLevel::Auto, &clis),
            "claude --permission-mode acceptEdits"
        );
    }

    /// A per-CLI `approval_args` override reaches recovery, and combines
    /// with resume args in the same order `attach_agent` uses: CLI, approval
    /// flags, then session-id/resume args.
    #[test]
    fn recovered_worker_combines_override_approval_flags_with_resume_args() {
        let e = entry("my-agent", Some("3f2c1a90-aaaa-4bbb-8ccc-1234567890ab"));
        let mut clis = HashMap::new();
        clis.insert(
            "my-agent".to_string(),
            CustomCli {
                command: "my-agent".to_string(),
                display_name: None,
                submit_delay_ms: None,
                settings_path: None,
                approval_args: HashMap::from([("full-auto".to_string(), "--my-flag".to_string())]),
                prompt_profile: None,
                session_id_args: None,
                resume_args: Some("--continue {{SESSION_ID}}".to_string()),
            },
        );
        assert_eq!(
            cli_command_for_recovery(&e, ApprovalLevel::FullAuto, &clis),
            "my-agent --my-flag --continue 3f2c1a90-aaaa-4bbb-8ccc-1234567890ab"
        );
    }

    /// Empty resolved flags yield the bare CLI name with no trailing
    /// separator (`supervisor-config`'s "Empty resolved flags launch the
    /// bare command" scenario).
    #[test]
    fn recovered_worker_with_empty_flags_has_no_trailing_separator() {
        let e = entry("unmapped-cli", None);
        let clis = HashMap::new();
        assert_eq!(
            cli_command_for_recovery(&e, ApprovalLevel::Auto, &clis),
            "unmapped-cli"
        );
    }
}
