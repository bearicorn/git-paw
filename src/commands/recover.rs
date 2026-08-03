//! `git paw start` recovery path — rebuilds the tmux session from saved
//! `session.json` state when the session is stopped or stale. Extracted
//! verbatim from `main.rs` (code-analysis-refactor R2c).
//!
//! `resolve_supervisor_flags` and `attach_or_print_hint` remain in `main.rs`
//! (supervisor-flow / dispatch helpers relocated in later waves) and are
//! referenced through the crate root.

use std::path::Path;

use git_paw::config::{self, PawConfig, SupervisorConfig};
use git_paw::error::PawError;
use git_paw::session::{self, Session, SessionMode, SessionStatus};
use git_paw::tmux;

use super::helpers::configured_settings_paths;
use crate::{attach_or_print_hint, resolve_supervisor_flags};

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

    // Re-seed every restored agent worktree's local allowlists (the same
    // seeding `attach_agent` performs at start/add) so restored panes pick up
    // preset updates before their CLIs boot. Gates mirror the repo-root
    // re-seeds above; failures are non-fatal warnings.
    let default_supervisor_cfg = SupervisorConfig::default();
    let recovery_dev_allowlist = (mode == SessionMode::Supervisor).then(|| {
        &config
            .supervisor
            .as_ref()
            .unwrap_or(&default_supervisor_cfg)
            .common_dev_allowlist
    });
    for wt in &existing.worktrees {
        for (path, err) in git_paw::supervisor::worktree_allowlist::seed_worktree_allowlists(
            &wt.worktree_path,
            broker_url.is_some(),
            config.docs_base_url.is_some(),
            recovery_dev_allowlist,
        ) {
            eprintln!(
                "warning: failed to seed agent-worktree allowlist for {}: {err}",
                path.display()
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
    // Same effective-level resolution as the auto-start flow: the
    // supervisor's own `approval` when set, else `agent_approval`.
    let supervisor_approval = supervisor_cfg
        .approval
        .unwrap_or(supervisor_cfg.agent_approval);
    let supervisor_flags =
        resolve_supervisor_flags(&supervisor_cli, supervisor_approval, &config.clis);
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
