//! `git paw start` recovery path — rebuilds the tmux session from saved
//! `session.json` state when the session is stopped or stale. Recovery never
//! attaches the terminal; the caller does, from the launch outcome.

use std::collections::HashMap;
use std::path::Path;

use crate::agent_cli::{LaunchSession, resolve_for_launch};
use crate::broker;
use crate::config::{self, ApprovalLevel, CustomCli, PawConfig, SupervisorConfig};
use crate::error::PawError;
use crate::session::{self, Session, SessionMode, SessionStatus, WorktreeEntry};
use crate::surface::SessionLayout;
use crate::terminal::tmux::TmuxBackend;
use crate::terminal::{SessionPlan, SurfaceSpec, TerminalBackend};
use crate::tmux;

use super::helpers::{
    dashboard_command, launch_session, start_broker_host, warn_if_broker_is_exposed,
};
use super::launch::{LaunchOutcome, LaunchWarnings};
use super::{CommandContext, Output};

/// Builds the pane command that resumes `entry`'s recorded session id. A CLI
/// without a resume template launches fresh.
///
/// An entry recording a worker profile relaunches as that profile, resolved
/// from the current `profiles` (its CLI, `args` and approval level, the
/// profile's level replacing `approval`). A recorded profile that no longer
/// exists, or is no longer a worker profile, relaunches with the recorded CLI
/// at `approval` after one warning naming the branch and the profile.
///
/// # Errors
///
/// Refuses an ambiguous PATH shim.
fn cli_command_for_recovery<S: std::hash::BuildHasher>(
    entry: &WorktreeEntry,
    approval: ApprovalLevel,
    clis: &HashMap<String, CustomCli, S>,
    profiles: &std::collections::BTreeMap<crate::config::ProfileName, crate::config::ProfileEntry>,
    repo_root: &Path,
    warnings: &mut LaunchWarnings,
    out: &mut dyn Output,
) -> Result<String, PawError> {
    let session = entry
        .cli_session_id
        .as_deref()
        .map_or(LaunchSession::None, LaunchSession::Resume);
    let (worker, warning) = crate::worker::WorkerLaunch::recorded(
        &entry.branch,
        &entry.cli,
        entry.profile.as_deref(),
        profiles,
        approval,
    );
    if let Some(warning) = warning {
        out.warn(&warning);
    }
    let resolved = resolve_for_launch(&worker.cli, clis, &worker.approval, repo_root)?
        .with_profile_args(worker.profile_args.as_deref());
    let profile = warnings.admit(resolved, out);
    Ok(profile.launch_command(&worker.approval, session))
}

/// Options for [`recover`].
#[derive(Debug, Clone)]
pub struct RecoverOptions {
    /// The stopped session's receipt.
    pub session: Session,
}

/// Recovers the repository's stopped session from its saved receipt. Never
/// attaches: the outcome names the rebuilt session for the caller to attach
/// to.
///
/// # Errors
///
/// Returns an error when tmux is missing, the config cannot be loaded, the
/// recorded broker port is held by another repository, or the session cannot
/// be rebuilt or saved.
pub fn recover(
    ctx: &CommandContext,
    opts: RecoverOptions,
    out: &mut dyn Output,
) -> Result<LaunchOutcome, PawError> {
    let RecoverOptions { session } = opts;
    let session_name = recover_session_detached(ctx.repo_root(), &session, out)?;
    Ok(LaunchOutcome::Attach { session_name })
}

/// Recovers a stopped/stale session by recreating the tmux session from saved
/// state, and returns the rebuilt session's name once it is up. It never
/// attaches the terminal: `git paw start` attaches from the launch outcome,
/// and an unattended re-attach binds a drive loop to the session instead —
/// the recovery itself stays one implementation.
///
/// # Errors
///
/// Returns an error when tmux is missing, the recorded broker port is held by
/// another repository, or the session cannot be rebuilt or saved.
#[allow(clippy::too_many_lines)]
pub(crate) fn recover_session_detached(
    repo_root: &Path,
    existing: &Session,
    out: &mut dyn Output,
) -> Result<String, PawError> {
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
        out.warn(&format!(
            "warning: session '{}' was created with a v0.4 layout but [supervisor] is enabled \
             in current config — rebuilding with v0.5 supervisor-as-pane layout.",
            existing.session_name
        ));
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
    // wrong port would let the clash `start` refuses up front slip
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
    let backend = TmuxBackend::new();
    if backend
        .session_alive(&existing.session_name)
        .unwrap_or(false)
        && let Err(e) = backend.close_session(&existing.session_name)
    {
        out.warn(&format!(
            "warning: could not tear down stale tmux session '{}' before recovery: {e}",
            existing.session_name
        ));
    }

    // No `[supervisor]` section is required for this to apply — a bare
    // (non-supervisor) recovery has none at all, so the level falls back to
    // `SupervisorConfig`'s default (`agent_approval = "auto"`), exactly as
    // `start` and `add` do.
    let default_sup = SupervisorConfig::default();
    let bare_approval = config
        .supervisor
        .as_ref()
        .unwrap_or(&default_sup)
        .agent_approval;

    let (plan, layout) = match mode {
        SessionMode::Supervisor => recover_supervisor_session(
            repo_root,
            existing,
            &config,
            broker_url.as_deref(),
            mouse,
            out,
        )?,
        SessionMode::Bare => recover_bare_session(
            repo_root,
            existing,
            broker_url.as_deref(),
            mouse,
            config.border_affordances_enabled(),
            bare_approval,
            &config.clis,
            &config.profiles,
            out,
        )?,
    };

    // Update session status + record the resolved mode.
    let mut updated = existing.clone();
    updated.status = SessionStatus::Active;
    updated.mode = mode;
    // Record the dashboard index of the layout just rebuilt — `Some(0)` for
    // every supervised recovery, including one of a session an earlier binary
    // saved with its dashboard at pane 1 (design D6).
    updated.dashboard_pane = layout.recorded_dashboard_pane();
    // Recovery REBUILDS the window, so whatever layout the stopped session was
    // saved with, what is live now is the agent grid — including a bare session
    // an earlier binary saved with the whole-window tiled layout, which is how
    // such a session becomes hot-attachable again.
    updated.layout = layout.recorded_pane_layout();
    // Re-stamp the broker log path. A session saved by a git-paw that used a
    // single shared `broker.log` would otherwise have the dashboard write the
    // per-session name while purge deleted the old one, orphaning a log.
    if updated.broker_port.is_some() {
        updated.broker_log_path = Some(session::broker_log_path(&updated.session_name)?);
    }
    // Save BEFORE the broker host starts: the host seeds its agents from the
    // saved receipt, and the dashboard resolves the broker URL from it.
    session::save_session(&updated)?;

    // A recovery rebuilds the session, broker and all, so it is a launch for
    // the purposes of the exposure warning — and it is the common path after a
    // reboot or a tmux crash. The broker host's own copy lands in its detached
    // service surface, where nobody reads it, so warn here, before the host
    // comes up.
    // Judged on the address the host will bind — the saved one — not the
    // current config's.
    warn_if_broker_is_exposed(&recovery_broker_config, out);

    // Rebuild the session and even each agent row's width, as a fresh start
    // does (design D4). Recovery attaches no session logging.
    launch_session(&backend, &plan, &layout, None, out)?;

    // Recreate the broker host in its service surface, with the session name
    // and the re-stamped log path; it seeds its watch targets and agent ->
    // spec mapping from the receipt saved above.
    if let (true, Some(log_path)) = (
        recovery_broker_config.enabled,
        updated.broker_log_path.as_ref(),
    ) {
        // A host that fails leaves no live session recorded `Active` with no
        // broker: close what was rebuilt and restore the receipt as it was,
        // so the next `git paw start` recovers again.
        if let Err(e) = start_broker_host(
            &backend,
            &plan.name,
            repo_root,
            &recovery_broker_config,
            log_path,
            updated.broker_authority.as_deref(),
        ) {
            let _ = backend.close_session(&plan.name);
            session::save_session(existing)?;
            return Err(e);
        }
    }

    // Rewrite the per-repo discovery file `sweep.sh` reads. Recovery REBUILDS
    // the layout, so a file written by the original launch records pane
    // indices that no longer describe the live session (and a session recovered
    // after the file was removed has none at all) — `session-state` requires
    // every path that creates or rebuilds the session's panes to (re)write it.
    // The offsets come from the layout just rebuilt above.
    super::launch::write_repo_discovery_file(
        repo_root,
        &plan.name,
        &updated.worktrees,
        layout.first_agent_index(),
        out,
    );

    Ok(plan.name)
}

/// Rebuild a bare-mode (non-supervisor) session: dashboard at pane 0 (when
/// broker enabled), coding agents at pane 1+.
///
/// Returns the built session together with the layout record of what it
/// rebuilt, so [`recover_session_detached`] can re-save the session and rewrite the
/// per-repo discovery file against the layout it actually produced.
#[allow(clippy::too_many_arguments)]
fn recover_bare_session<S: std::hash::BuildHasher>(
    repo_root: &Path,
    existing: &Session,
    broker_url: Option<&str>,
    mouse: bool,
    border_affordances: bool,
    approval: ApprovalLevel,
    clis: &HashMap<String, CustomCli, S>,
    profiles: &std::collections::BTreeMap<crate::config::ProfileName, crate::config::ProfileEntry>,
    out: &mut dyn Output,
) -> Result<(SessionPlan, SessionLayout), PawError> {
    let mut warnings = LaunchWarnings::default();
    let plan = SessionPlan {
        name: existing.session_name.clone(),
        mode: SessionMode::Bare,
        dashboard: broker_url.map(|_| SurfaceSpec {
            label: "dashboard".to_string(),
            owner: repo_root.to_path_buf(),
            launch_command: dashboard_command(),
        }),
        surfaces: existing
            .worktrees
            .iter()
            .map(|entry| {
                Ok(SurfaceSpec {
                    label: entry.branch.clone(),
                    owner: entry.worktree_path.clone(),
                    launch_command: cli_command_for_recovery(
                        entry,
                        approval,
                        clis,
                        profiles,
                        repo_root,
                        &mut warnings,
                        out,
                    )?,
                })
            })
            .collect::<Result<_, PawError>>()?,
        env: broker_url
            .map(|url| vec![("GIT_PAW_BROKER_URL".to_string(), url.to_string())])
            .unwrap_or_default(),
        mouse,
        show_labels: border_affordances,
    };

    // Reject a plan the backend cannot build BEFORE the caller re-saves the
    // session as active: a session with no surface at all, or more agents
    // than the grid holds.
    if plan.dashboard.is_none() && plan.surfaces.is_empty() {
        return Err(PawError::TmuxError(
            "cannot create a session with no panes".to_owned(),
        ));
    }
    crate::surface::layout_for(plan.surfaces.len(), plan.dashboard.is_some())?;

    // The dashboard occupies pane 0 when the broker is enabled, so coding
    // agents start at pane 1; without it they start at pane 0.
    let layout = SessionLayout::planned(SessionMode::Bare, broker_url.is_some());
    Ok((plan, layout))
}

/// Rebuild a supervisor-mode session with the current layout: dashboard at
/// pane 0, coding agents at panes 1..N — `N + 1` panes, whatever layout the
/// stopped session was saved with. No orchestrator pane is rebuilt, so no
/// orchestrator CLI is needed.
///
/// Returns the built session together with the layout record of what it
/// rebuilt, so [`recover_session_detached`] can re-save `dashboard_pane` and rewrite
/// the per-repo discovery file against the layout it actually produced.
fn recover_supervisor_session(
    repo_root: &Path,
    existing: &Session,
    config: &PawConfig,
    broker_url: Option<&str>,
    mouse: bool,
    out: &mut dyn Output,
) -> Result<(SessionPlan, SessionLayout), PawError> {
    let mut warnings = LaunchWarnings::default();
    // Fall back to a default config when [supervisor] is absent so a
    // re-launched session does not error on a config the prior boot
    // already accepted.
    let default_supervisor_cfg = SupervisorConfig::default();
    let supervisor_cfg = config
        .supervisor
        .as_ref()
        .unwrap_or(&default_supervisor_cfg);

    // Reject more agents than the grid holds before the caller re-saves the
    // session as active.
    crate::surface::supervisor_layout(existing.worktrees.len(), true)?;

    let dashboard_pane = SurfaceSpec {
        label: "dashboard".to_string(),
        owner: repo_root.to_path_buf(),
        launch_command: dashboard_command(),
    };
    let agent_panes: Vec<SurfaceSpec> = existing
        .worktrees
        .iter()
        .map(|entry| {
            Ok(SurfaceSpec {
                label: entry.branch.clone(),
                owner: entry.worktree_path.clone(),
                launch_command: cli_command_for_recovery(
                    entry,
                    supervisor_cfg.agent_approval,
                    &config.clis,
                    &config.profiles,
                    repo_root,
                    &mut warnings,
                    out,
                )?,
            })
        })
        .collect::<Result<_, PawError>>()?;

    let env_vars: Vec<(String, String)> = broker_url
        .map(|url| vec![("GIT_PAW_BROKER_URL".to_string(), url.to_string())])
        .unwrap_or_default();

    let plan = SessionPlan {
        name: existing.session_name.clone(),
        mode: SessionMode::Supervisor,
        dashboard: Some(dashboard_pane),
        surfaces: agent_panes,
        env: env_vars,
        mouse,
        show_labels: config.border_affordances_enabled(),
    };

    Ok((
        plan,
        SessionLayout::planned(SessionMode::Supervisor, broker_url.is_some()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recovery_command(
        e: &WorktreeEntry,
        level: ApprovalLevel,
        clis: &HashMap<String, CustomCli>,
    ) -> String {
        cli_command_for_recovery(
            e,
            level,
            clis,
            &std::collections::BTreeMap::new(),
            Path::new("/"),
            &mut LaunchWarnings::default(),
            &mut crate::commands::CaptureOutput::default(),
        )
        .unwrap()
    }
    use std::path::PathBuf;

    fn entry(cli: &str, cli_session_id: Option<&str>) -> WorktreeEntry {
        WorktreeEntry {
            branch: "feat/x".to_string(),
            worktree_path: PathBuf::from("/tmp/wt-feat-x"),
            cli: cli.to_string(),
            profile: None,
            branch_created: false,
            base_branch: None,
            pending_boot_prompt: None,
            pending_entry_point: None,
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
            recovery_command(&e, ApprovalLevel::Manual, &clis),
            "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --setting-sources project,local --resume 3f2c1a90-aaaa-4bbb-8ccc-1234567890ab"
        );
    }

    fn routine_profiles()
    -> std::collections::BTreeMap<crate::config::ProfileName, crate::config::ProfileEntry> {
        std::collections::BTreeMap::from([(
            crate::config::ProfileName::new("routine").unwrap(),
            crate::config::ProfileEntry {
                kind: crate::config::ProfileKind::Worker,
                description: "Routine".into(),
                cli: "my-agent".into(),
                args: Some("--model small".into()),
                skill: None,
                approval: None,
                timeout_seconds: None,
            },
        )])
    }

    fn recover_with_profiles(
        e: &WorktreeEntry,
        profiles: &std::collections::BTreeMap<
            crate::config::ProfileName,
            crate::config::ProfileEntry,
        >,
    ) -> (String, crate::commands::CaptureOutput) {
        let mut out = crate::commands::CaptureOutput::default();
        let command = cli_command_for_recovery(
            e,
            ApprovalLevel::Manual,
            &HashMap::new(),
            profiles,
            Path::new("/"),
            &mut LaunchWarnings::default(),
            &mut out,
        )
        .unwrap();
        (command, out)
    }

    /// Spec scenario "A recovered worker keeps its profile's arguments".
    #[test]
    fn a_recovered_worker_keeps_its_profiles_arguments() {
        let mut e = entry("my-agent", None);
        e.profile = Some("routine".into());
        let (command, out) = recover_with_profiles(&e, &routine_profiles());
        assert!(command.contains("--model small"), "{command}");
        assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    }

    /// Spec scenario "A removed profile falls back with a warning".
    #[test]
    fn a_removed_profile_falls_back_with_a_warning() {
        let mut e = entry("beta", None);
        e.profile = Some("gone".into());
        let (command, out) = recover_with_profiles(&e, &routine_profiles());
        assert_eq!(command, "beta");
        let warned: Vec<_> = out
            .warnings
            .iter()
            .filter(|w| w.contains("feat/x") && w.contains("gone"))
            .collect();
        assert_eq!(warned.len(), 1, "{:?}", out.warnings);
    }

    /// Spec scenario "Entry without an id launches fresh".
    #[test]
    fn entry_without_an_id_launches_fresh() {
        let e = entry("claude", None);
        let clis = HashMap::new();
        assert_eq!(
            recovery_command(&e, ApprovalLevel::Manual, &clis),
            "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --setting-sources project,local"
        );
    }

    /// Spec scenario "CLI without a resume template launches fresh".
    #[test]
    fn cli_without_a_resume_template_launches_fresh() {
        let e = entry("my-agent", Some("3f2c1a90-aaaa-4bbb-8ccc-1234567890ab"));
        let clis = HashMap::new();
        assert_eq!(
            recovery_command(&e, ApprovalLevel::Manual, &clis),
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
            recovery_command(&e, ApprovalLevel::Auto, &clis),
            "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --setting-sources project,local --permission-mode acceptEdits"
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
                mcp: None,
                headless_args: None,
                family: None,
                args: None,
                sandbox: None,
                sandbox_args: None,
                inherit_operator_config: false,
            },
        );
        assert_eq!(
            recovery_command(&e, ApprovalLevel::FullAuto, &clis),
            "my-agent --my-flag --continue 3f2c1a90-aaaa-4bbb-8ccc-1234567890ab"
        );
    }

    /// Spec scenario "Recovery builds the full command" (`cli-provider`).
    #[test]
    fn recovery_carries_approval_flags_then_resume_args() {
        let e = entry("claude", Some("abc-123"));
        assert_eq!(
            recovery_command(&e, ApprovalLevel::Auto, &HashMap::new()),
            "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --setting-sources project,local --permission-mode acceptEdits --resume abc-123"
        );
    }

    /// A recovered worker keeps its entry's `args`, before the approval
    /// flags, so a per-profile model survives recovery.
    #[test]
    fn recovery_carries_the_entry_args() {
        let e = entry("claude", Some("abc-123"));
        let clis = HashMap::from([(
            "claude".to_string(),
            CustomCli {
                command: "claude".to_string(),
                args: Some("--model sonnet".to_string()),
                ..CustomCli::default()
            },
        )]);
        assert_eq!(
            recovery_command(&e, ApprovalLevel::Auto, &clis),
            "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --model sonnet --setting-sources project,local --permission-mode acceptEdits --resume abc-123"
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
            recovery_command(&e, ApprovalLevel::Auto, &clis),
            "unmapped-cli"
        );
    }

    /// `code-architecture` scenario "Launch plan is asserted without tmux": a
    /// two-worktree plan with the broker enabled, opened through the fake
    /// backend, records one dashboard surface and two coding surfaces with
    /// their labels, owners and launch commands, then evens the agent grid.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn launch_plan_is_asserted_without_tmux() {
        use crate::terminal::fake::{FakeBackend, Operation};
        use crate::terminal::{GridGeometry, TerminalBackend};

        let mut a = entry("claude", None);
        a.branch = "feat/a".to_string();
        a.worktree_path = PathBuf::from("/repo/.git-paw/worktrees/feat-a");
        let mut b = entry("codex", None);
        b.branch = "feat/b".to_string();
        b.worktree_path = PathBuf::from("/repo/.git-paw/worktrees/feat-b");
        let existing = Session {
            session_name: "paw-proj".to_string(),
            repo_path: PathBuf::from("/repo"),
            project_name: "proj".to_string(),
            created_at: std::time::SystemTime::now(),
            status: SessionStatus::Stopped,
            worktrees: vec![a, b],
            broker_port: Some(9119),
            broker_bind: Some("127.0.0.1".to_string()),
            broker_log_path: None,
            mode: SessionMode::Bare,
            dashboard_pane: Some(0),
            layout: None,
            broker_authority: None,
        };
        let clis = HashMap::new();
        let (plan, layout) = recover_bare_session(
            Path::new("/repo"),
            &existing,
            Some("http://127.0.0.1:9119"),
            true,
            true,
            ApprovalLevel::Manual,
            &clis,
            &std::collections::BTreeMap::new(),
            &mut crate::commands::CaptureOutput::default(),
        )
        .expect("a two-agent plan builds");

        let fake = FakeBackend::new();
        launch_session(
            &fake,
            &plan,
            &layout,
            None,
            &mut crate::commands::CaptureOutput::default(),
        )
        .expect("the fake opens it");

        let ops = fake.operations();
        let Operation::OpenSession(opened) = &ops[0] else {
            panic!("the launch opens the session first, got {ops:?}");
        };
        let dashboard = opened.dashboard.as_ref().expect("broker → dashboard");
        assert_eq!(dashboard.label, "dashboard");
        assert_eq!(dashboard.owner, PathBuf::from("/repo"));
        assert!(dashboard.launch_command.ends_with("__dashboard"));
        let coding: Vec<(&str, &Path, &str)> = opened
            .surfaces
            .iter()
            .map(|s| {
                (
                    s.label.as_str(),
                    s.owner.as_path(),
                    s.launch_command.as_str(),
                )
            })
            .collect();
        assert_eq!(
            coding,
            vec![
                (
                    "feat/a",
                    Path::new("/repo/.git-paw/worktrees/feat-a"),
                    "env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --setting-sources project,local"
                ),
                (
                    "feat/b",
                    Path::new("/repo/.git-paw/worktrees/feat-b"),
                    "codex"
                ),
            ]
        );
        // Three surfaces now exist, owned as planned, in layout order.
        let owners: Vec<_> = fake
            .list_surfaces("paw-proj")
            .unwrap()
            .into_iter()
            .map(|s| (s.position, s.owner))
            .collect();
        assert_eq!(
            owners,
            vec![
                (0, Some(PathBuf::from("/repo"))),
                (1, Some(PathBuf::from("/repo/.git-paw/worktrees/feat-a"))),
                (2, Some(PathBuf::from("/repo/.git-paw/worktrees/feat-b"))),
            ]
        );
        assert_eq!(
            ops[1],
            Operation::Layout {
                session: "paw-proj".to_string(),
                grid: GridGeometry {
                    first_agent: 1,
                    agent_count: 2,
                    has_top_row: true,
                },
            }
        );
        // The fake spawns no process: nothing here touched a terminal.
        assert!(ops.iter().all(|op| !op.is_keystroke()));
    }
}
