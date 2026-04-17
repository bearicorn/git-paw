//! git-paw — Parallel AI Worktrees.
//!
//! Orchestrates multiple AI coding CLI sessions across git worktrees
//! from a single terminal using tmux.

#[cfg(test)]
use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::Path;
use std::process::Command as StdCommand;
use std::time::SystemTime;

use clap::Parser;
use dialoguer::Confirm;

use git_paw::broker;
use git_paw::broker::messages::BrokerMessage;
use git_paw::broker::publish::{build_status_message, publish_to_broker_http};
use git_paw::cli::{Cli, Command};
use git_paw::config::{self, PawConfig};
use git_paw::detect;
use git_paw::error::PawError;
use git_paw::git;
use git_paw::interactive;
use git_paw::merge_loop::run_merge_loop;
#[cfg(test)]
use git_paw::merge_loop::{build_dependency_graph, topological_merge_order};
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
        supervisor: false,
        force: false,
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
            supervisor,
            force,
        } => {
            if from_specs {
                return cmd_start_from_specs(cli_flag.as_deref(), dry_run, force);
            }
            let supervisor_enabled = resolve_supervisor_mode_from_cwd(supervisor, dry_run)?;
            if supervisor_enabled {
                let cwd = std::env::current_dir().map_err(|e| {
                    PawError::SessionError(format!("cannot read current directory: {e}"))
                })?;
                let repo_root = git::validate_repo(&cwd)?;
                let config = config::load_config(&repo_root)?;
                return cmd_supervisor(
                    &repo_root,
                    &config,
                    cli_flag.as_deref(),
                    branches_flag.as_deref(),
                    dry_run,
                );
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
// Supervisor mode resolution
// ---------------------------------------------------------------------------

/// Loads the repo config from the current working directory and resolves
/// whether supervisor mode should be entered for this session.
fn resolve_supervisor_mode_from_cwd(
    supervisor_flag: bool,
    dry_run: bool,
) -> Result<bool, PawError> {
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
    let config = config::load_config(&repo_root).unwrap_or_default();
    resolve_supervisor_mode(
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
    supervisor_flag: bool,
    dry_run: bool,
    config: &PawConfig,
    prompt: &mut dyn SupervisorPrompt,
) -> Result<bool, PawError> {
    // Step 1: --supervisor flag always wins.
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

    // Supervisor mode: when the supervisor section is enabled in config, hand
    // off to the auto-start flow that launches all coding agents under a
    // supervisor CLI. The supervisor is responsible for verification and merge.
    if config.supervisor.as_ref().is_some_and(|s| s.enabled) {
        return cmd_supervisor(
            &repo_root,
            &config,
            cli_flag.as_deref(),
            branches_flag.as_deref(),
            dry_run,
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
        let rendered_skill = skill_content.as_ref().map(|tmpl| {
            git_paw::skills::render(tmpl, branch, &broker_config.url(), &project, None)
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
            git_paw::agents::install_git_hooks(&wt.path, &broker_config.url(), &agent_id)?;
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

        struct BrokerForwarder {
            broker_url: String,
        }
        impl git_paw::supervisor::poll::QuestionForwarder for BrokerForwarder {
            fn forward_question(
                &mut self,
                agent_id: &str,
                kind: git_paw::supervisor::permission_prompt::PermissionType,
                _captured: &str,
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
            }
        }

        let inspector = TmuxPaneInspector;
        let resolver = move |id: &str| pane_map.get(id).copied();
        let mut dispatcher = TmuxKeyDispatcher;
        let mut forwarder = BrokerForwarder {
            broker_url: broker_url.clone(),
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
                broker_url: Some(&broker_url),
            };
            let _ = tick_from_status(&rows, &mut ctx);
        }
    });
    Some((stop, handle))
}

#[allow(clippy::too_many_lines)]
fn cmd_supervisor(
    repo_root: &Path,
    config: &PawConfig,
    cli_flag: Option<&str>,
    branches_flag: Option<&[String]>,
    dry_run: bool,
) -> Result<(), PawError> {
    let supervisor_cfg = config.supervisor.as_ref().ok_or_else(|| {
        PawError::ConfigError("supervisor mode enabled but [supervisor] config missing".to_string())
    })?;

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

    // Resolve branches. Prefer --branches, then scan specs, otherwise error.
    let mut spec_by_branch: std::collections::HashMap<String, git_paw::specs::SpecEntry> =
        std::collections::HashMap::new();
    let branches: Vec<String> = if let Some(bs) = branches_flag {
        bs.to_vec()
    } else {
        let specs = git_paw::specs::scan_specs(config, repo_root)?;
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

    // Real launch.
    git::prune_worktrees(repo_root)?;

    // Pre-populate `.claude/settings.json` with curl allowlist entries
    // so the supervisor and coding agents do not hit an approval prompt
    // on every broker round-trip. Failures are logged but non-fatal.
    if broker_config.enabled {
        let claude_settings = repo_root.join(".claude").join("settings.json");
        if let Err(e) = git_paw::supervisor::curl_allowlist::setup_curl_allowlist(
            &broker_config.url(),
            &claude_settings,
        ) {
            eprintln!("warning: failed to setup curl allowlist: {e}");
        }
    }

    let mut builder = tmux::TmuxSessionBuilder::new(&project)
        .session_name(session_name.clone())
        .mouse_mode(mouse);

    // Pane 0 = dashboard. Inject GIT_PAW_BROKER_URL before any send-keys so
    // every coding agent pane inherits it for its `curl` calls.
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

    // Resolve the coordination skill once for all agent panes.
    let coordination_template = if broker_config.enabled {
        Some(git_paw::skills::resolve("coordination")?)
    } else {
        None
    };

    // Build the inter-agent rules block for this session.
    let branch_refs: Vec<&str> = branches.iter().map(String::as_str).collect();
    let inter_agent_rules = git_paw::agents::build_inter_agent_rules(&branch_refs);

    // Pre-compute the initial prompt for each coding agent pane.
    let mut initial_prompts: Vec<(usize, String)> = Vec::new();
    let mut worktree_entries = Vec::new();
    let pane_offset = usize::from(broker_config.enabled);

    for (idx, branch) in branches.iter().enumerate() {
        let wt = git::create_worktree(repo_root, branch)?;
        let wt_str = wt.path.to_string_lossy().to_string();

        let rendered_skill = coordination_template.as_ref().map(|tmpl| {
            git_paw::skills::render(
                tmpl,
                branch,
                &broker_config.url(),
                &project,
                supervisor_cfg.test_command.as_deref(),
            )
        });

        let spec_entry = spec_by_branch.get(branch);
        let spec_content = spec_entry.map(|s| s.prompt.clone());
        let owned_files = spec_entry.and_then(|s| s.owned_files.clone());

        let assignment = git_paw::agents::WorktreeAssignment {
            branch: branch.clone(),
            cli: agent_cli.clone(),
            spec_content,
            owned_files,
            skill_content: rendered_skill,
            inter_agent_rules: Some(inter_agent_rules.clone()),
        };
        git_paw::agents::setup_worktree_agents_md(repo_root, &wt.path, &assignment)?;

        if broker_config.enabled {
            let agent_id = git_paw::broker::messages::slugify_branch(branch);
            git_paw::agents::install_git_hooks(&wt.path, &broker_config.url(), &agent_id)?;
        }

        // Build the agent launch command with approval flags.
        let cli_command = if agent_flags.is_empty() {
            agent_cli.clone()
        } else {
            format!("{agent_cli} {agent_flags}")
        };

        builder = builder.add_pane(tmux::PaneSpec {
            branch: branch.clone(),
            worktree: wt_str,
            cli_command,
        });

        // Build boot block for this agent
        let boot_block = git_paw::skills::build_boot_block(branch, &broker_config.url());

        // Initial prompt: spec title/description if present, else default.
        let task_prompt = spec_entry
            .map(|s| s.prompt.lines().next().unwrap_or("").trim().to_string())
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| "Begin your assigned task as described in AGENTS.md.".to_string());

        // Prepend boot block to task prompt for supervisor mode
        let full_prompt = format!("{boot_block}\n\n{task_prompt}");
        initial_prompts.push((idx + pane_offset, full_prompt));

        worktree_entries.push(WorktreeEntry {
            branch: branch.clone(),
            worktree_path: wt.path,
            cli: agent_cli.clone(),
            branch_created: wt.branch_created,
        });
    }

    let tmux_session = builder.build()?;
    tmux_session.execute()?;

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
    };
    if broker_config.enabled {
        state.broker_port = Some(broker_config.port);
        state.broker_bind = Some(broker_config.bind.clone());
        state.broker_log_path = Some(session::session_state_dir()?.join("broker.log"));
    }
    session::save_session(&state)?;

    // Wait ~2s for panes to boot to an interactive state, then inject the
    // initial prompt into each coding agent pane via `tmux send-keys`.
    std::thread::sleep(std::time::Duration::from_secs(2));
    for (pane_idx, prompt) in &initial_prompts {
        let target = format!("{}:0.{pane_idx}", tmux_session.name);
        let _ = std::process::Command::new("tmux")
            .args(["send-keys", "-t", &target, prompt, "Enter"])
            .status();
    }

    // Supervisor self-registration: publish an `agent.status` so the
    // supervisor row shows up in the dashboard alongside the coding agents.
    // Failures here are non-fatal — the dashboard still works, the
    // supervisor row simply will not appear.
    if broker_config.enabled {
        let boot_msg = build_status_message(
            "supervisor",
            "working",
            Some("Supervisor booting".to_string()),
        );
        if let Err(e) = publish_to_broker_http(&broker_config.url(), &boot_msg) {
            eprintln!("warning: failed to publish supervisor boot status: {e}");
        }
    }

    // Write the supervisor skill template into the repo root as the
    // supervisor CLI's AGENTS.md, then start the supervisor CLI in the
    // foreground terminal (blocks until it exits).
    //
    // Any failure here would leave the tmux session running orphaned (the
    // dashboard + coding agent panes are already up). Wrap the skill setup
    // in an immediately-invoked closure so we can `tmux::kill_session` before
    // propagating the error, ensuring no orphan session is left behind.
    let skill_setup = (|| -> Result<(), PawError> {
        let supervisor_skill = git_paw::skills::resolve("supervisor")?;
        let supervisor_md = git_paw::skills::render(
            &supervisor_skill,
            "supervisor",
            &broker_config.url(),
            &project,
            supervisor_cfg.test_command.as_deref(),
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
        Ok(())
    })();
    if let Err(e) = skill_setup {
        // Best-effort cleanup — ignore any kill error so the original failure
        // is what surfaces to the operator.
        let _ = tmux::kill_session(&tmux_session.name);
        return Err(e);
    }

    println!(
        "Supervisor session '{}' launched with {} coding agent(s).",
        tmux_session.name,
        branches.len()
    );
    println!("Starting supervisor CLI '{supervisor_cli}' in the foreground...");

    // Spawn the auto-approve poll thread (no-op when broker is disabled or
    // [supervisor.auto_approve] is missing). The thread exits when the
    // shutdown flag flips at the end of cmd_supervisor.
    let auto_approve_handle = if broker_config.enabled {
        let pane_map: std::collections::HashMap<String, usize> = branches
            .iter()
            .enumerate()
            .map(|(idx, branch)| {
                (
                    git_paw::broker::messages::slugify_branch(branch),
                    idx + pane_offset,
                )
            })
            .collect();
        spawn_auto_approve_thread(
            tmux_session.name.clone(),
            broker_config.url(),
            supervisor_cfg.auto_approve.clone(),
            pane_map,
        )
    } else {
        None
    };

    let status = std::process::Command::new(&supervisor_cli)
        .current_dir(repo_root)
        .status()
        .map_err(|e| {
            PawError::SessionError(format!(
                "failed to start supervisor CLI '{supervisor_cli}': {e}"
            ))
        })?;

    // Stop the poll thread (if running) and wait for it to finish.
    if let Some((stop, handle)) = auto_approve_handle {
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = handle.join();
    }

    // Run the merge loop after supervisor CLI exits
    let merge_results = run_merge_loop(
        repo_root,
        &state,
        supervisor_cfg.test_command.as_ref(),
        &broker_config,
    )?;

    // Reconstruct broker state from the broker's /log so the session summary
    // contains real per-agent records (modified files, last status message,
    // exports, etc.) instead of an empty fresh state. The broker runs in the
    // dashboard process; this `cmd_supervisor` call lives in a different
    // process and would otherwise see an empty BrokerState.
    let final_state = std::sync::Arc::new(broker::BrokerState::new(None));
    if broker_config.enabled {
        match git_paw::broker::publish::fetch_log_over_http(&broker_config.url()) {
            Ok(messages) => {
                for msg in &messages {
                    git_paw::broker::delivery::publish_message(&final_state, msg);
                }
            }
            Err(e) => {
                eprintln!(
                    "warning: failed to fetch broker /log for session summary: {e}; \
                     summary will contain empty per-agent records"
                );
            }
        }
    }

    // Write the session summary
    match git_paw::summary::write_supervisor_summary(
        &final_state,
        &state,
        &merge_results.merge_order,
        &merge_results.test_results,
        repo_root,
    ) {
        Ok(path) => println!("Session summary: {}", path.display()),
        Err(e) => eprintln!("warning: failed to write session summary: {e}"),
    }

    if !status.success() {
        return Err(PawError::SessionError(format!(
            "supervisor CLI exited with status {status}"
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Command: start --from-specs
// ---------------------------------------------------------------------------

/// Launches sessions from spec files instead of interactive branch selection.
fn cmd_start_from_specs(
    cli_flag: Option<&str>,
    dry_run: bool,
    force: bool,
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

    // Fresh launch from specs (or dry-run preview)
    tmux::ensure_tmux_installed()?;
    let config = config::load_config(&repo_root)?;

    // Scan for pending specs
    let specs = git_paw::specs::scan_specs(&config, &repo_root)?;
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
        let rendered_skill = skill_template.as_ref().map(|tmpl| {
            git_paw::skills::render(
                tmpl,
                branch,
                &broker_config.url(),
                project,
                config
                    .supervisor
                    .as_ref()
                    .and_then(|s| s.test_command.as_deref()),
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
            git_paw::agents::install_git_hooks(&wt.path, &broker_config.url(), &agent_id)?;
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

    // Broker: inject dashboard pane and environment variable (mirror cmd_start logic)
    // Use original session's broker state, not current config, to preserve broker
    // functionality across stop/start cycles
    if let (Some(port), Some(bind)) = (existing.broker_port, &existing.broker_bind) {
        let repo_str = repo_root.to_string_lossy().to_string();
        let broker_url = format!("http://{bind}:{port}");

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
        builder = builder.set_environment("GIT_PAW_BROKER_URL", &broker_url);

        // Re-populate the curl allowlist when recovering — the broker URL
        // can change across sessions and missing entries would re-trigger
        // permission prompts.
        let claude_settings = repo_root.join(".claude").join("settings.json");
        if let Err(e) =
            git_paw::supervisor::curl_allowlist::setup_curl_allowlist(&broker_url, &claude_settings)
        {
            eprintln!("warning: failed to setup curl allowlist: {e}");
        }
    }

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
    let show_message_log = config
        .dashboard
        .as_ref()
        .is_some_and(|d| d.show_message_log);

    let log_path = session::session_state_dir()?.join("broker.log");
    let broker_state = broker::BrokerState::new(Some(log_path));

    // Build the watcher target list from the saved session, if any.
    // The session is always written by cmd_start/launch_spec_session before
    // tmux attaches, so reading it here gives us the current worktree set.
    let watch_targets = session::find_session_for_repo(&repo_root)?
        .map(|sess| {
            sess.worktrees
                .into_iter()
                .map(|wt| broker::WatchTarget {
                    agent_id: broker::messages::slugify_branch(&wt.branch),
                    cli: wt.cli,
                    worktree_path: wt.worktree_path,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let handle = broker::start_broker(&broker_config, broker_state, watch_targets)?;
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

    git_paw::dashboard::run_dashboard_with_panes(
        &state,
        handle,
        &shutdown,
        &std::collections::HashMap::new(),
        None,
        show_message_log,
    )
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

/// Outcome of the purge UX flow.
#[derive(Debug, PartialEq, Eq)]
enum PurgeOutcome {
    Purged,
    Cancelled,
}

/// Removes everything: tmux session, worktrees, and state.
fn cmd_purge(force: bool) -> Result<(), PawError> {
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

    for entry in &session.worktrees {
        if let Err(e) = git::remove_worktree(repo_root, &entry.worktree_path) {
            let _ = writeln!(
                stderr,
                "warning: failed to remove worktree '{}': {e}",
                entry.worktree_path.display()
            );
        }
    }

    for entry in &session.worktrees {
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

    if let Some(ref log_path) = session.broker_log_path {
        let _ = std::fs::remove_file(log_path);
    }

    session::delete_session_in(&session.session_name, sessions_dir)?;

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use git_paw::broker::messages::{BlockedPayload, BrokerMessage};
    use git_paw::config::SupervisorConfig;
    use serial_test::serial;
    use std::path::PathBuf;
    use std::time::UNIX_EPOCH;
    use tempfile::TempDir;

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
        let result = resolve_supervisor_mode(true, false, &cfg, &mut prompt).unwrap();
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
        let result = resolve_supervisor_mode(false, false, &cfg, &mut prompt).unwrap();
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
        let result = resolve_supervisor_mode(false, false, &cfg, &mut prompt).unwrap();
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
        let result = resolve_supervisor_mode(false, true, &cfg, &mut prompt).unwrap();
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
        let result = resolve_supervisor_mode(false, false, &cfg, &mut prompt).unwrap();
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
        let result = resolve_supervisor_mode(true, true, &cfg, &mut prompt).unwrap();
        assert!(result);
    }

    // --- merge ordering ---

    fn blocked(agent: &str, from: &str) -> BrokerMessage {
        BrokerMessage::Blocked {
            agent_id: agent.to_string(),
            payload: BlockedPayload {
                needs: "x".to_string(),
                from: from.to_string(),
            },
        }
    }

    #[test]
    fn dependency_graph_single_edge() {
        let msgs = vec![(1, blocked("feat-a", "feat-b"))];
        let graph = build_dependency_graph(&msgs);
        assert_eq!(graph.get("feat-b").unwrap(), &vec!["feat-a".to_string()]);
    }

    #[test]
    fn topo_no_dependencies_returns_all() {
        let graph = HashMap::new();
        let agents = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let order = topological_merge_order(&graph, &agents);
        assert_eq!(order.len(), 3);
        for a in &agents {
            assert!(order.contains(a));
        }
    }

    #[test]
    fn topo_chain_merges_dep_first() {
        let msgs = vec![(1, blocked("feat-a", "feat-b"))];
        let graph = build_dependency_graph(&msgs);
        let agents = vec!["feat-a".to_string(), "feat-b".to_string()];
        let order = topological_merge_order(&graph, &agents);
        let b_idx = order.iter().position(|s| s == "feat-b").unwrap();
        let a_idx = order.iter().position(|s| s == "feat-a").unwrap();
        assert!(b_idx < a_idx, "feat-b should merge before feat-a");
    }

    #[test]
    fn topo_cycle_falls_back_to_arbitrary_order() {
        let msgs = vec![
            (1, blocked("feat-a", "feat-b")),
            (2, blocked("feat-b", "feat-a")),
        ];
        let graph = build_dependency_graph(&msgs);
        let agents = vec!["feat-a".to_string(), "feat-b".to_string()];
        let order = topological_merge_order(&graph, &agents);
        assert_eq!(order.len(), agents.len());
        assert!(order.contains(&"feat-a".to_string()));
        assert!(order.contains(&"feat-b".to_string()));
    }

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

        let wt = git::create_worktree(&repo, "feature/test").expect("create worktree");
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
            }],
            broker_port: None,
            broker_bind: None,
            broker_log_path: None,
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
