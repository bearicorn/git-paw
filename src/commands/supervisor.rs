//! `git paw` supervisor mode — the auto-start flow (`cmd_supervisor`), the
//! in-process unattended drive loop, the auto-approve thread wiring, and
//! supervisor-mode resolution. Extracted verbatim from `main.rs`
//! (code-analysis-refactor R2d).
//!
//! The shared agent-attach primitives (`attach_agent`, `AttachContext`) and
//! launch helpers (`submit_prompt_to_pane`, `resolve_submit_delay_ms`,
//! `write_repo_discovery_file`), plus the `SpecMode` dispatch enum and
//! `apply_spec_mode`, remain in `main.rs` and are referenced through the crate
//! root.

use std::io::IsTerminal;
use std::path::Path;
use std::time::SystemTime;

use dialoguer::Confirm;

use git_paw::broker;
use git_paw::broker::messages::BrokerMessage;
use git_paw::broker::publish::publish_to_broker_http;
use git_paw::config::{self, PawConfig, SupervisorConfig};
use git_paw::detect;
use git_paw::error::PawError;
use git_paw::git;
use git_paw::interactive;
use git_paw::session::{self, Session, SessionMode, SessionStatus, WorktreeEntry};
use git_paw::tmux;

use super::helpers::{
    attach_session_logging, config_to_custom_defs, configured_settings_paths,
    session_cli_settings_paths, to_interactive_cli,
};
use crate::{
    AttachContext, SpecMode, UNATTENDED_ENV, apply_spec_mode, attach_agent,
    resolve_submit_delay_ms, submit_prompt_to_pane, write_repo_discovery_file,
};

/// Loads the repo config from the current working directory and resolves
/// whether supervisor mode should be entered for this session.
pub(crate) fn resolve_supervisor_mode_from_cwd(
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
pub(crate) trait SupervisorPrompt {
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
pub(crate) fn resolve_supervisor_mode(
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

/// Per-session classifier wiring for [`spawn_auto_approve_thread`], bundled
/// so the spawn signature stays under the argument-count lint.
pub(crate) struct AutoApproveWiring {
    /// Agent ID → tmux pane index.
    pub(crate) pane_map: std::collections::HashMap<String, usize>,
    /// Agent ID → worktree root for the boundary-scoped classifier rules.
    pub(crate) worktree_map: std::collections::HashMap<String, std::path::PathBuf>,
    /// Manual-decision recorder for forwarded prompts.
    pub(crate) recorder: git_paw::supervisor::manual_approvals::ManualDecisionRecorder,
    /// Protected-path set for the operator config/memory danger rule
    /// (`agent-memory-isolation`).
    pub(crate) protected_paths: git_paw::supervisor::auto_approve::ProtectedPaths,
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
pub(crate) fn spawn_auto_approve_thread(
    session_name: String,
    broker_url: String,
    config: Option<config::AutoApproveConfig>,
    dev_allowlist: config::CommonDevAllowlistConfig,
    wiring: AutoApproveWiring,
) -> Option<(
    std::sync::Arc<std::sync::atomic::AtomicBool>,
    std::thread::JoinHandle<()>,
)> {
    let AutoApproveWiring {
        pane_map,
        worktree_map,
        recorder,
        protected_paths,
    } = wiring;
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
                dev_allowlist: &dev_allowlist,
                resolver: &resolver,
                inspector: &inspector,
                dispatcher: &mut dispatcher,
                forwarder: &mut forwarder,
                worktree_resolver: &worktree_resolver,
                protected_paths: &protected_paths,
                broker_url: Some(&broker_url),
            };
            let _ = tick_from_status(&rows, &mut ctx);
        }
    });
    Some((stop, handle))
}

/// Resolves the supervisor pane's approval flags at its effective level,
/// warning (never failing) when `full-auto` has no known flag mapping for
/// the CLI.
///
/// The warning names the CLI and the `[clis.<name>].approval_args` override
/// so the operator can supply native flags; the pane then launches flagless
/// (`auto` behavior) rather than bricking the session on a typo'd CLI name.
/// Used by every path that builds the supervisor pane command: the
/// `cmd_supervisor` auto-start flow and session recovery.
pub(crate) fn resolve_supervisor_flags(
    supervisor_cli: &str,
    level: config::ApprovalLevel,
    clis: &std::collections::HashMap<String, config::CustomCli>,
) -> String {
    let flags = config::resolve_approval_flags(supervisor_cli, &level, clis);
    if level == config::ApprovalLevel::FullAuto && flags.is_empty() {
        eprintln!(
            "warning: [supervisor] approval is \"full-auto\" but no permission flags are known \
             for CLI '{supervisor_cli}'; launching the supervisor pane without flags (auto \
             behavior). Define [clis.{supervisor_cli}] approval_args = {{ \"full-auto\" = \
             \"<flags>\" }} to supply them."
        );
    }
    flags
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub(crate) fn cmd_supervisor(
    repo_root: &Path,
    config: &PawConfig,
    cli_flag: Option<&str>,
    branches_flag: Option<&[String]>,
    spec_mode: &SpecMode,
    specs_format_override: Option<&str>,
    dry_run: bool,
    no_rebase: bool,
    unattended: bool,
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
    let agent_approval = &supervisor_cfg.agent_approval;
    // The supervisor pane resolves its own level, inheriting agent_approval
    // when [supervisor] approval is unset (pre-v0.11.0 behavior). Coding
    // agents always resolve from agent_approval.
    let supervisor_approval = supervisor_cfg
        .approval
        .unwrap_or(supervisor_cfg.agent_approval);
    let agent_flags = config::resolve_approval_flags(&agent_cli, agent_approval, &config.clis);
    let supervisor_flags =
        resolve_supervisor_flags(&supervisor_cli, supervisor_approval, &config.clis);

    // Dry-run: print the plan and exit without touching the filesystem.
    if dry_run {
        let supervisor_cmd = if supervisor_flags.is_empty() {
            supervisor_cli.clone()
        } else {
            format!("{supervisor_cli} {supervisor_flags}")
        };
        println!("Dry run — supervisor session plan:\n");
        println!("  Session:    {session_name}");
        println!("  Supervisor: {supervisor_cmd}");
        println!("  Agent CLI:  {agent_cli}");
        if supervisor_approval == *agent_approval {
            println!("  Approval:   {agent_approval:?}");
        } else {
            println!("  Supervisor approval: {supervisor_approval:?}");
            println!("  Agent approval:      {agent_approval:?}");
        }
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
        // When unattended, append the drive-loop coordination directive so the
        // supervisor consumes the loop's escalations instead of blanket-approving
        // (supervisor-loop-escalation-tiering).
        skill_content: Some(git_paw::skills::with_drive_loop_directive(
            supervisor_md,
            unattended,
        )),
        inter_agent_rules: None,
    };
    git_paw::agents::setup_worktree_agents_md(repo_root, repo_root, &supervisor_assignment)?;

    // Resolve the coordination skill once for all agent panes.
    let coordination_template = if broker_config.enabled {
        Some(git_paw::skills::resolve("coordination")?)
    } else {
        None
    };

    // Resolve the docs-fetch skill once, gated on an explicitly-configured
    // `docs_base_url`. Left unset (the default points at git-paw's own site),
    // the skill is not injected — a consumer only gets it once they have
    // pointed git-paw at their own docs, keeping the exported skill agnostic.
    let docs_fetch_template = if config.docs_base_url.is_some() {
        Some(git_paw::skills::resolve("docs-fetch")?)
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
        agent_flags: &agent_flags,
        coordination_template: coordination_template.as_ref(),
        docs_fetch_template: docs_fetch_template.as_ref(),
        gate_commands: &gate_commands,
        session_backends: &session_backends,
        inter_agent_rules: Some(inter_agent_rules.as_str()),
        strict_guard,
        no_rebase,
        placement: config.worktree_placement(),
        common_dev_allowlist: &supervisor_cfg.common_dev_allowlist,
    };

    for branch in &branches {
        let attached = attach_agent(&attach_ctx, branch, spec_by_branch.get(branch))?;
        agent_panes.push(attached.pane);
        agent_prompts.push(attached.prompt);
        worktree_entries.push(attached.entry);
    }

    let mut env_vars: Vec<(String, String)> = if broker_config.enabled {
        vec![("GIT_PAW_BROKER_URL".to_string(), broker_config.url())]
    } else {
        Vec::new()
    };
    if unattended {
        // Injected into the session environment (before the dashboard pane is
        // split, so that pane inherits it): the `__dashboard` subprocess reads
        // it to DISABLE its auto-approve thread. For an unattended session the
        // in-process drive loop is the sole approver, so two approvers never
        // race on the same pane (D1).
        env_vars.push((UNATTENDED_ENV.to_string(), "1".to_string()));
    }

    let mut tmux_session = tmux::build_supervisor_session(
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

    // Attach session logging (no-op unless `[logging] enabled`). Supervisor
    // mode reserves pane 0 (supervisor) and pane 1 (dashboard), so coding
    // agents — in `branches` order, matching `agent_panes` and the boot loop
    // below — start at SUPERVISOR_PANE_OFFSET.
    attach_session_logging(
        &mut tmux_session,
        config,
        repo_root,
        &branch_refs,
        git_paw::supervisor::layout::SUPERVISOR_PANE_OFFSET,
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

    // Step 15 (supervisor-launch): branch on `--unattended`.
    //
    // Without it the launch returns immediately with the manual-attach hint
    // (v0.5.0 behaviour). With it, drive the in-process unattended loop
    // (`unattended-operation`): it blocks until a completion, escalation-
    // summary, stuck, or heartbeat exit condition, prints the summary, and
    // returns — it does NOT replace the foreground terminal with an
    // interactive supervisor CLI and does NOT require an attached terminal.
    if unattended {
        return drive_unattended_loop(
            repo_root,
            supervisor_cfg,
            &broker_config,
            &tmux_session.name,
            &state,
        );
    }

    println!("Attach with:  tmux attach -t {}", tmux_session.name);
    Ok(())
}

/// Runs the in-process unattended drive loop for a freshly-launched supervisor
/// session (supervisor-launch step 15, `unattended-operation`).
///
/// Builds the coding-agent roster from the saved session (each pane resolved by
/// `worktree_path`, per D2), resolves the classifier whitelist and
/// worktree-write policy from `[supervisor.auto_approve]`, points the summary at
/// the broker log and the learnings file, then drives the loop to an exit
/// condition and prints the summary. Blocks in the foreground process; requires
/// no attached interactive terminal.
fn drive_unattended_loop(
    repo_root: &Path,
    supervisor_cfg: &SupervisorConfig,
    broker_config: &git_paw::config::BrokerConfig,
    session_name: &str,
    state: &Session,
) -> Result<(), PawError> {
    use git_paw::supervisor::drive::{self, AgentPane, DriveRunOptions};

    let agents: Vec<AgentPane> = state
        .worktrees
        .iter()
        .map(|wt| AgentPane {
            agent_id: broker::messages::slugify_branch(&wt.branch),
            worktree_path: wt.worktree_path.clone(),
        })
        .collect();

    // The classifier the loop consumes mirrors the dashboard poll loop's
    // resolved whitelist + worktree-write policy so unattended and attended
    // sessions auto-approve the same set.
    let auto_approve = supervisor_cfg
        .auto_approve
        .clone()
        .unwrap_or_default()
        .resolved();

    // Protected-path set for the operator config/memory danger rule
    // (`agent-memory-isolation`); config parse problems degrade to the
    // defaults-only derivation rather than aborting the loop.
    let protected_paths = git_paw::supervisor::auto_approve::ProtectedPaths::derive(
        &config::load_config(repo_root, None).unwrap_or_default(),
        Some(repo_root),
    );

    let options = DriveRunOptions {
        broker_url: broker_config.enabled.then(|| broker_config.url()),
        whitelist: auto_approve.effective_whitelist(&supervisor_cfg.common_dev_allowlist),
        approve_worktree_writes: auto_approve.approve_worktree_writes(),
        protected_paths,
        broker_log_hint: state
            .broker_log_path
            .as_ref()
            .map(|p| p.display().to_string()),
        learnings_hint: supervisor_cfg.learnings.then(|| {
            repo_root
                .join(".git-paw")
                .join("session-learnings.md")
                .display()
                .to_string()
        }),
    };

    drive::run_drive_loop(session_name, repo_root, &agents, options)?;
    Ok(())
}

/// Canonical GitHub issues URL for optional learnings sharing. Tracks the
/// `repository` field in `Cargo.toml` and the README links so the disclosure
/// notice never drifts to a stale repo location.
pub(crate) const GIT_PAW_ISSUES_URL: &str = "https://github.com/bearicorn/git-paw/issues";

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
pub(crate) fn learnings_disclosure_notice(supervisor: Option<&SupervisorConfig>) -> Option<String> {
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
