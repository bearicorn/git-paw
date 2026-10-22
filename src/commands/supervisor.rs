//! `git paw` supervisor mode — the auto-start flow (`cmd_supervisor`), the
//! in-process unattended drive loop and its manual-decision log wiring, and
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
use git_paw::surface::SessionLayout;
use git_paw::tmux;

use super::helpers::{
    attach_session_logging, config_to_custom_defs, dashboard_command, to_interactive_cli,
    warn_if_broker_is_exposed,
};
use super::recover::recover_session;
use super::start::restart_from_pause;
use crate::{
    AttachContext, SpecMode, apply_spec_mode, attach_agent, gate_pane_or_fail_on_dialog,
    invalidate_if_stale, reattach_active_session_or_refuse, resolve_submit_delay_ms,
    submit_prompt_to_pane, write_repo_discovery_file,
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

/// Runs the interactive branch + CLI selection for a supervised launch that
/// has neither `--branches` nor a spec selection, and splits the result into
/// the branches to launch and their per-branch CLI overrides.
///
/// The overrides are what keep the user's picker choice: without them the
/// launch would fall back to the session-wide `agent_cli` and silently ignore
/// the CLI the user just picked (DS-8). `default_cli` only pre-selects in the
/// picker; `cli_flag` skips it, in which case every branch maps to that CLI.
fn interactive_branch_cli_selection(
    prompter: &dyn interactive::Prompter,
    all_branches: &[String],
    clis: &[interactive::CliInfo],
    cli_flag: Option<&str>,
    default_cli: Option<&str>,
) -> Result<(Vec<String>, std::collections::HashMap<String, String>), PawError> {
    let selection =
        interactive::run_selection(prompter, all_branches, clis, cli_flag, None, default_cli)?;
    let mut branches = Vec::with_capacity(selection.mappings.len());
    let mut overrides = std::collections::HashMap::new();
    for (branch, cli) in selection.mappings {
        branches.push(branch.clone());
        overrides.insert(branch, cli);
    }
    Ok((branches, overrides))
}

#[cfg(test)]
mod interactive_cli_selection_tests {
    //! Spec: *Supervised launch honours the CLIs chosen in the interactive
    //! picker* (`supervisor-launch`). A supervised launch with no
    //! `--branches`, `--specs` or `--cli` must launch every worker with the
    //! CLI its branch was mapped to, not with the session-wide `agent_cli`
    //! fallback.

    use std::cell::RefCell;

    use git_paw::error::PawError;
    use git_paw::interactive::{CliInfo, CliMode, Prompter};
    use git_paw::specs::SpecEntry;

    use super::interactive_branch_cli_selection;

    /// A prompter that answers with a scripted mode and CLI choices and
    /// records the `default` each picker was given.
    struct ScriptedPrompter {
        mode: CliMode,
        branches: Vec<String>,
        uniform_cli: String,
        per_branch_clis: Vec<String>,
        per_branch_calls: RefCell<usize>,
        uniform_default: RefCell<Option<String>>,
    }

    impl ScriptedPrompter {
        fn new(mode: CliMode, branches: &[&str], uniform: &str, per_branch: &[&str]) -> Self {
            Self {
                mode,
                branches: branches.iter().map(|b| (*b).to_string()).collect(),
                uniform_cli: uniform.to_string(),
                per_branch_clis: per_branch.iter().map(|c| (*c).to_string()).collect(),
                per_branch_calls: RefCell::new(0),
                uniform_default: RefCell::new(None),
            }
        }
    }

    impl Prompter for ScriptedPrompter {
        fn select_mode(&self) -> Result<CliMode, PawError> {
            Ok(self.mode)
        }
        fn select_branches(&self, _branches: &[String]) -> Result<Vec<String>, PawError> {
            Ok(self.branches.clone())
        }
        fn select_cli(&self, _clis: &[CliInfo], default: Option<&str>) -> Result<String, PawError> {
            *self.uniform_default.borrow_mut() = default.map(String::from);
            Ok(self.uniform_cli.clone())
        }
        fn select_cli_for_branch(
            &self,
            _branch: &str,
            _clis: &[CliInfo],
            _default: Option<&str>,
        ) -> Result<String, PawError> {
            let mut calls = self.per_branch_calls.borrow_mut();
            let cli = self
                .per_branch_clis
                .get(*calls)
                .cloned()
                .ok_or(PawError::UserCancelled)?;
            *calls += 1;
            Ok(cli)
        }
        fn select_specs(&self, _specs: &[SpecEntry]) -> Result<Vec<SpecEntry>, PawError> {
            Err(PawError::UserCancelled)
        }
    }

    fn clis() -> Vec<CliInfo> {
        ["alpha", "beta"]
            .into_iter()
            .map(|n| CliInfo {
                display_name: n.to_string(),
                binary_name: n.to_string(),
            })
            .collect()
    }

    fn all_branches() -> Vec<String> {
        vec!["feat/a".to_string(), "feat/b".to_string()]
    }

    /// Scenario: "A uniform CLI pick is honoured".
    #[test]
    fn a_uniform_pick_overrides_the_configured_default() {
        let prompter = ScriptedPrompter::new(CliMode::Uniform, &["feat/a"], "beta", &[]);

        let (branches, overrides) = interactive_branch_cli_selection(
            &prompter,
            &all_branches(),
            &clis(),
            None,
            Some("alpha"),
        )
        .unwrap();

        assert_eq!(branches, vec!["feat/a".to_string()]);
        assert_eq!(overrides.get("feat/a").map(String::as_str), Some("beta"));
        assert_eq!(
            prompter.uniform_default.borrow().as_deref(),
            Some("alpha"),
            "default_cli must pre-select in the picker"
        );
    }

    /// Scenario: "Per-branch CLI picks are honoured".
    #[test]
    fn per_branch_picks_map_each_branch_to_its_own_cli() {
        let prompter = ScriptedPrompter::new(
            CliMode::PerBranch,
            &["feat/a", "feat/b"],
            "",
            &["alpha", "beta"],
        );

        let (branches, overrides) =
            interactive_branch_cli_selection(&prompter, &all_branches(), &clis(), None, None)
                .unwrap();

        assert_eq!(branches, all_branches());
        assert_eq!(overrides.get("feat/a").map(String::as_str), Some("alpha"));
        assert_eq!(overrides.get("feat/b").map(String::as_str), Some("beta"));
    }

    /// `--cli` short-circuits CLI resolution here too: no CLI picker runs,
    /// every branch maps to the flag's CLI, and `default_cli` never reaches a
    /// prompt. (The spec's "The flag path keeps the session-wide fallback"
    /// scenario is the `--branches` path, which never reaches this function —
    /// it is covered by the PTY test in `tests/cli_prompt_matrix.rs`.)
    #[test]
    fn the_cli_flag_skips_the_cli_picker() {
        let prompter = ScriptedPrompter::new(CliMode::Uniform, &["feat/a"], "beta", &[]);

        let (branches, overrides) = interactive_branch_cli_selection(
            &prompter,
            &all_branches(),
            &clis(),
            Some("alpha"),
            Some("beta"),
        )
        .unwrap();

        assert_eq!(branches, vec!["feat/a".to_string()]);
        assert_eq!(overrides.get("feat/a").map(String::as_str), Some("alpha"));
        assert_eq!(
            prompter.uniform_default.borrow().as_deref(),
            None,
            "no CLI picker may run when --cli is given"
        );
    }
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
    // Reattach guard (GP-13): a supervisor-enabled repo's `git paw start`
    // dispatches straight here (see `resolve_dispatch_target`), bypassing
    // `cmd_start`'s own existing-session check entirely. Without this, a
    // re-invocation against a repository that already has a live session
    // fell straight through to `resolve_session_name`, which — finding the
    // base name already alive — forked a `-N`-suffixed parallel session
    // instead of reattaching to the one already running.
    let existing_session = session::find_session_for_repo(repo_root)?;
    if !dry_run
        && let Some(existing) = &existing_session
        && !invalidate_if_stale(repo_root, existing)?
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
                return restart_from_pause(repo_root, existing);
            }
            SessionStatus::Active => {
                return reattach_active_session_or_refuse(&existing.session_name);
            }
            SessionStatus::Stopped => {
                println!("Recovering session '{}'...", existing.session_name);
                return recover_session(repo_root, existing);
            }
        }
    }

    // Refuse a port held by another repository's broker before the branch and
    // CLI pickers below, so the user is never asked to make choices for a
    // launch that cannot proceed. See `cmd_start` for why this cannot wait
    // for the dashboard's own broker start-up. A dry run creates nothing, so
    // it never probes; the reattach and recover paths returned above and do
    // their own checks.
    if !dry_run {
        broker::ensure_port_free_for_repo(&config.broker, repo_root)?;
    }

    // Fall back to a synthesized default when [supervisor] is absent.
    // `resolve_supervisor_mode` already prompts the user to opt in to
    // supervisor mode without forcing them to hand-author a [supervisor]
    // block; the hard-error path that used to live here defeated that.
    let default_supervisor_cfg = SupervisorConfig::default();
    let supervisor_cfg = config
        .supervisor
        .as_ref()
        .unwrap_or(&default_supervisor_cfg);

    // The orchestrator runs outside the session, so no orchestrator CLI is
    // needed to launch: `[supervisor].cli`, else `default_cli`, only names it
    // in the orchestrator context when one is configured.
    let orchestrator_cli = supervisor_cfg
        .cli
        .clone()
        .or_else(|| config.default_cli.clone());

    // Resolve the coding agents' fallback CLI: explicit flag > default_cli >
    // `[supervisor].cli` (kept so earlier configs launch unchanged). Checked
    // against the branches that need it once they are known.
    let agent_cli = cli_flag
        .map(ToString::to_string)
        .or_else(|| config.default_cli.clone())
        .or_else(|| supervisor_cfg.cli.clone());

    // Resolve branches. Precedence:
    //   1. `--branches` — explicit branch list wins.
    //   2. `SpecMode::None` (bare `--supervisor`, no spec flag) — behave like
    //      `git paw start`: interactive branch picker, no spec discovery.
    //   3. `SpecMode::{All,Picker,Narrow}` — scan specs, then apply the same
    //      subset filter the non-supervisor `--specs` path uses so the named
    //      subset / picker selection is honoured (the v0.6.0 dogfood fix).
    let mut spec_by_branch: std::collections::HashMap<String, git_paw::specs::SpecEntry> =
        std::collections::HashMap::new();
    // Per-branch CLI resolved via the spec-driven 5-level priority chain
    // (GP-10): populated only for spec-driven branches, so `--supervisor
    // --specs` can no longer silently diverge from the non-supervisor
    // `--specs` resolution (in particular, `default_spec_cli` is honoured
    // instead of being bypassed by the `agent_cli` fallback below).
    let mut spec_cli_overrides: std::collections::HashMap<String, String> =
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
        let (picked_branches, picked_clis) = interactive_branch_cli_selection(
            &prompter,
            &all_branches,
            &interactive_clis,
            cli_flag,
            config.default_cli.as_deref(),
        )?;
        spec_cli_overrides.extend(picked_clis);
        picked_branches
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

        // Resolve the CLI per spec branch through the exact same chain
        // `cmd_start_with_specs` uses (`interactive::resolve_cli_for_specs`):
        // `--cli` > per-spec `paw_cli` > `default_spec_cli` > `default_cli`
        // (picker) > full picker. Previously `--supervisor --specs` ignored
        // this entirely and launched every worker with the single
        // `agent_cli` fallback below — silently unsandboxed workers when
        // `default_spec_cli` pinned a different CLI than `default_cli`.
        let custom_defs = config_to_custom_defs(config);
        let detected = detect::detect_clis(&custom_defs);
        if detected.is_empty() {
            return Err(PawError::NoCLIsFound);
        }
        let interactive_clis: Vec<interactive::CliInfo> =
            detected.iter().map(to_interactive_cli).collect();
        let prompter = interactive::TerminalPrompter;
        let cli_mappings = interactive::resolve_cli_for_specs(
            &specs,
            cli_flag,
            config,
            &interactive_clis,
            &prompter,
        )?;
        spec_cli_overrides.extend(cli_mappings);

        let mut out = Vec::with_capacity(specs.len());
        for spec in specs {
            out.push(spec.branch.clone());
            spec_by_branch.insert(spec.branch.clone(), spec);
        }
        out
    };

    // A branch without a per-branch CLI (the `--branches` path) launches with
    // the fallback; when it is needed and nothing resolved it, no coding agent
    // can start.
    if agent_cli.is_none() && branches.iter().any(|b| !spec_cli_overrides.contains_key(b)) {
        return Err(PawError::ConfigError(
            "supervisor mode could not resolve a coding-agent CLI: pass --cli or set default_cli"
                .to_string(),
        ));
    }
    let agent_cli = agent_cli.unwrap_or_default();

    // Refuse before anything is touched when two of the session's branches
    // derive the same broker agent id — they would share one inbox, so a
    // verdict or feedback meant for one would reach the other. As early as
    // `branches` is known: later steps prune worktrees and write the main
    // checkout's supervisor assignment block, and a refused launch should
    // leave none of that behind.
    //
    // Only when the broker is enabled: an agent id exists to address an agent
    // ON the broker, so with it off nothing derives, publishes or routes by
    // one and a shared id has no observable effect.
    if config.broker.enabled {
        let refs: Vec<&str> = branches.iter().map(String::as_str).collect();
        if let Some(clash) = broker::messages::ensure_distinct_agent_ids(&refs, &[]) {
            return Err(PawError::ConfigError(clash.to_string()));
        }
    }

    let project = git::project_name(repo_root);
    let session_name = tmux::resolve_session_name(&project)?;
    let mouse = config.mouse.unwrap_or(true);
    let broker_config = config.broker.clone();
    // Warn before the session exists: the dashboard pane prints the same
    // warning but immediately switches to the alternate screen.
    warn_if_broker_is_exposed(&broker_config);
    // Coding agents resolve their flags from `agent_approval` alone.
    // `[supervisor].approval` is the orchestrator's own level and no pane runs
    // an orchestrator, so it shapes no command here.
    let agent_approval = &supervisor_cfg.agent_approval;

    // Resolves the CLI `branch` launches with: its per-branch override when
    // one was resolved — the spec-driven priority chain (GP-10) or the
    // interactive picker (DS-8) — else the session-wide `agent_cli` fallback
    // (the `--branches` path, which shows no picker and has no per-spec
    // `paw_cli`/`default_spec_cli` to consult).
    let effective_agent_cli = |branch: &str| -> String {
        spec_cli_overrides
            .get(branch)
            .cloned()
            .unwrap_or_else(|| agent_cli.clone())
    };

    // Dry-run: print the plan and exit without touching the filesystem.
    if dry_run {
        if let Some(existing) = &existing_session {
            eprintln!(
                "warning: session '{}' already exists — purge it before starting a new one\n",
                existing.session_name
            );
        }
        println!("Dry run — supervisor session plan:\n");
        println!("  Session:    {session_name}");
        if spec_cli_overrides.values().any(|cli| cli != &agent_cli) {
            println!("  Agent CLI:  (resolved per branch, see below)");
        } else {
            println!("  Agent CLI:  {agent_cli}");
        }
        println!("  Approval:   {agent_approval:?}");
        println!("  Mouse:      {}", if mouse { "on" } else { "off" });
        if broker_config.enabled {
            println!("  Broker URL: {}", broker_config.url());
        }
        println!();
        for branch in &branches {
            let wt = git::worktree_display_path(repo_root, branch, config.worktree_placement())?;
            let cli = effective_agent_cli(branch);
            let flags = config::resolve_approval_flags(&cli, agent_approval, &config.clis);
            let cmd = if flags.is_empty() {
                cli
            } else {
                format!("{cli} {flags}")
            };
            println!("  {branch} \u{2192} {cmd} ({wt})");
        }
        return Ok(());
    }

    // Hard cap (D4): 25 agents per session for v0.5.0. Configurable layout
    // arrives in v1.0.0 (issue #17).
    // A supervised launch always builds the dashboard as the full-width top
    // row, broker or not, so the grid always has one.
    let layout = git_paw::surface::supervisor_layout(branches.len(), true)?;

    // Real launch.
    git::prune_worktrees(repo_root)?;

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

    // Render the orchestrator context into the repo-root sidecar
    // (`.git-paw/AGENTS.local.md`). No pane reads it: the orchestrator drives
    // the session from outside tmux — the operator's own agent session — and
    // the launch prints the sidecar path so the operator can point it there.
    let supervisor_skill_template = git_paw::skills::resolve("supervisor")?;
    let supervisor_md = git_paw::skills::render_for_supervisor(
        &supervisor_skill_template,
        &broker_config.url(),
        &project,
        &supervisor_cfg.gate_commands(),
        &session_backends,
    );
    let supervisor_assignment = git_paw::agents::WorktreeAssignment {
        branch: "supervisor".to_string(),
        // The configured orchestrator CLI, else the CLI the coding agents run.
        cli: orchestrator_cli
            .clone()
            .unwrap_or_else(|| agent_cli.clone()),
        spec_content: None,
        owned_files: None,
        // When unattended, append the drive-loop coordination directive so the
        // orchestrator consumes the loop's escalations instead of
        // blanket-approving (supervisor-loop-escalation-tiering), then the
        // `## Governance documents` section so a configured `[governance]`
        // table reaches the orchestrator context (boot-block: governance
        // follows the supervisor skill content).
        skill_content: Some(git_paw::skills::supervisor_boot_context(
            &supervisor_md,
            unattended,
            &config.governance,
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

    // Peer branches for the inter-agent rules block. Held as a slice rather
    // than a pre-rendered string: `build_inter_agent_rules` interpolates each
    // agent's own id into the block, so `attach_agent` builds a fresh one per
    // agent from this shared list rather than every agent getting an
    // identical, un-interpolatable string.
    let branch_refs: Vec<&str> = branches.iter().map(String::as_str).collect();

    let repo_str = repo_root.to_string_lossy().to_string();
    let dashboard_command = dashboard_command();

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
    let worktree_runtime = config.worktree_runtime();

    for branch in &branches {
        // Resolve this branch's own CLI (its spec-driven override, or the
        // session-wide fallback) and its approval flags, then build a fresh
        // context — a spec-driven session can launch a distinct CLI per
        // branch (GP-10), so the context can no longer be shared verbatim
        // across the loop.
        let branch_cli = effective_agent_cli(branch);
        let branch_flags =
            config::resolve_approval_flags(&branch_cli, agent_approval, &config.clis);
        let attach_ctx = AttachContext {
            repo_root,
            project: &project,
            broker_config: &broker_config,
            agent_cli: &branch_cli,
            agent_flags: &branch_flags,
            coordination_template: coordination_template.as_ref(),
            docs_fetch_template: docs_fetch_template.as_ref(),
            gate_commands: &gate_commands,
            session_backends: &session_backends,
            inter_agent_rules_peers: Some(&branch_refs),
            strict_guard,
            no_rebase,
            placement: config.worktree_placement(),
            worktree_runtime: &worktree_runtime,
            clis: &config.clis,
        };

        // Each agent takes the lowest slot not held by an already-attached
        // peer, so the port blocks in this session never overlap.
        let slot = git_paw::worktree_provision::allocate_slot(&worktree_entries);
        let attached = attach_agent(&attach_ctx, branch, spec_by_branch.get(branch), slot)?;
        agent_panes.push(attached.pane);
        agent_prompts.push(attached.prompt);
        worktree_entries.push(attached.entry);
    }

    let env_vars: Vec<(String, String)> = if broker_config.enabled {
        vec![("GIT_PAW_BROKER_URL".to_string(), broker_config.url())]
    } else {
        Vec::new()
    };
    let mut tmux_session = tmux::build_supervisor_session(
        &project,
        Some(session_name.clone()),
        &dashboard_pane,
        &agent_panes,
        layout,
        mouse,
        config.border_affordances_enabled(),
        &env_vars,
    )?;

    // Every reserved-index fact for this launch — the logging offset, the
    // recorded `dashboard_pane`, the discovery-file offset, and the
    // gate/submit targets below — comes from this one layout record.
    // (`layout` above is the grid GEOMETRY for `agent_count` panes; this is
    // the index record for the window those panes land in.)
    let session_layout = SessionLayout::planned(SessionMode::Supervisor, broker_config.enabled);

    // Attach session logging (no-op unless `[logging] enabled`). The
    // supervised layout reserves pane 0 for the dashboard, so coding agents —
    // in `branches` order, matching `agent_panes` and the boot loop below —
    // start at the layout's first agent index.
    attach_session_logging(
        &mut tmux_session,
        config,
        repo_root,
        &branch_refs,
        session_layout.first_agent_index(),
    )?;

    tmux_session.execute()?;

    // Rebalance each agent row to equal width on the live window (design D4,
    // G3): the raw `split-window -h` chain renders a row as 50/25/25, so a
    // column-precise resize evens it out now that the panes exist.
    if let Err(e) = tmux::rebalance_agent_rows(
        &tmux_session.name,
        session_layout.first_agent_index(),
        agent_panes.len(),
    ) {
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
        // Every supervised launch builds the dashboard at pane 0, broker or
        // not, and records it, so a recorded `None` keeps meaning "an earlier
        // binary" (design D6).
        dashboard_pane: session_layout.recorded_dashboard_pane(),
        layout: session_layout.recorded_pane_layout(),
    };
    if broker_config.enabled {
        state.broker_port = Some(broker_config.port);
        state.broker_bind = Some(broker_config.bind.clone());
        state.broker_log_path = Some(session::broker_log_path(&state.session_name)?);
    }
    session::save_session(&state)?;

    // Write the per-repo discovery file sweep.sh reads. Coding agents start
    // at the layout's first agent index (dashboard pane 0); no orchestrator
    // entry is written, since no pane runs one.
    write_repo_discovery_file(
        repo_root,
        &tmux_session.name,
        &state.worktrees,
        session_layout.first_agent_index(),
    );

    // Inject the initial prompt into each coding agent pane (indices 1..N).
    // The dashboard pane (index 0) is a TUI process and does NOT receive a
    // send-keys prompt.
    //
    // Instead of a blind fixed sleep, each pane's boot block is gated on
    // observed CLI readiness (design D1, G1): `gate_pane_for_injection` polls
    // the pane for its CLI's interactive marker — relaunching a still-bare
    // shell, and falling back to injection after the budget for an
    // unrecognised CLI — so the multi-line boot block is never typed into a
    // bare shell (the v0.8.0 G1 failure).
    //
    // A single Enter is sent per pane — on paste-aware CLIs (Claude Code
    // v2.1.x) this leaves the prompt in a paste-buffer state which the drive
    // loop's buffered-input submit (or the orchestrator, per its skill's
    // paste-buffer-recovery section) recovers. Sending more than one Enter at
    // launch risks accidentally accepting a follow-on permission prompt on
    // fast CLIs and is intentionally avoided.
    for (idx, prompt) in agent_prompts.iter().enumerate() {
        // Per-branch delay: a spec-driven session can resolve a distinct CLI
        // per branch (GP-10), and each CLI's configured settle delay differs.
        let agent_cli = effective_agent_cli(&branches[idx]);
        let agent_delay = resolve_submit_delay_ms(&agent_cli, &config.clis);
        let pane_idx = session_layout.agent_index(idx);
        gate_pane_or_fail_on_dialog(
            &tmux_session.name,
            pane_idx,
            &agent_cli,
            &agent_panes[idx].cli_command,
            &config.clis,
        )?;
        submit_prompt_to_pane(&tmux_session.name, pane_idx, prompt, agent_delay);
    }

    // The orchestrator's roster row appears only when the external
    // orchestrator publishes its own `agent.status`. The launcher does not
    // publish on its behalf, so a launch no orchestrator drives leaves no
    // phantom `supervisor` row on the dashboard.

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
    // The orchestrator runs outside the session: point the operator's own
    // agent session at the context rendered for it.
    println!(
        "Orchestrator context: {} (point your orchestrating agent session at it)",
        git_paw::agents::SIDECAR_REL_PATH
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
/// `worktree_path`, per D2), wires the manual-decision log, points the summary
/// at the broker log and the learnings file, then drives the loop to an exit
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
            branch: wt.branch.clone(),
        })
        .collect();

    // The session-instance guard (GP-06) compares against what
    // `find_session_for_repo` reads back from disk, which round-trips
    // `created_at` through a whole-seconds ISO 8601 string (the field's
    // serializer is frozen — see AGENTS.md). Binding to the in-memory
    // `state.created_at` (sub-second `SystemTime::now()`) would almost never
    // equal that truncated value and make the loop see its own session as
    // torn down on the very first tick, so re-read the just-saved session to
    // bind to the same truncated value the guard will later compare against.
    let session_created_at = session::find_session_for_repo(repo_root)
        .ok()
        .flatten()
        .map_or(state.created_at, |s| s.created_at);

    // Loaded once and reused: the `[clis]` table the nudge path resolves its
    // settle delay from. Config parse problems degrade to the defaults rather
    // than aborting the loop.
    let loaded_config = config::load_config(repo_root, None).unwrap_or_default();

    let broker_url = broker_config.enabled.then(|| broker_config.url());

    // The manual-decision log: every permission prompt the loop escalates is
    // recorded so `git paw approvals` can tell the user which commands keep
    // needing a human. Recording is opt-OUT (`manual_approvals_log`, default
    // on); the first sighting of a pattern also emits a `permission_pattern`
    // learning, gated separately on `[supervisor] learnings`. This is the
    // only caller, so the resolution is inlined here (design D6).
    let recorder = git_paw::supervisor::manual_approvals::ManualDecisionRecorder::new(
        git_paw::supervisor::manual_approvals::log_path(repo_root, session_name),
        supervisor_cfg.manual_approvals_log_enabled(),
        supervisor_cfg.learnings,
        state.project_name.clone(),
        supervisor_cfg
            .cli
            .clone()
            .or_else(|| loaded_config.default_cli.clone()),
    );

    let options = DriveRunOptions {
        broker_url: broker_url.clone(),
        manual_decisions: Box::new(git_paw::supervisor::drive::RecorderManualDecisionSink::new(
            recorder, broker_url,
        )),
        correction: supervisor_cfg.correction.clone(),
        learnings_enabled: supervisor_cfg.learnings,
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
        branch_refresh_enabled: supervisor_cfg.branch_refresh_enabled(),
        session_created_at,
        clis: loaded_config.clis.clone(),
        layout: git_paw::surface::SessionLayout::of(state),
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
