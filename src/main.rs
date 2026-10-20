//! git-paw — Parallel AI Worktrees.
//!
//! Orchestrates multiple AI coding CLI sessions across git worktrees
//! from a single terminal using tmux.

// `core-project-conventions`: no `unwrap()`/`expect()` in non-test code.
// `clippy.toml` exempts `#[cfg(test)]` code; `just lint`'s `-D warnings`
// turns a violation into a hard failure.
#![warn(clippy::unwrap_used, clippy::expect_used)]

use std::io::IsTerminal;
use std::path::Path;

use clap::Parser;
use dialoguer::Confirm;
use git_paw::cli_setup::facts::SetupItem;
use git_paw::cli_setup::render::{render_plan, render_undo_plan, render_unsupported};

use git_paw::agents;
use git_paw::broker;
#[cfg(test)]
use git_paw::broker::publish::build_status_message;
use git_paw::cli::{Cli, Command, SpecsFormat};
use git_paw::config::{self, SupervisorConfig, resolve_submit_delay_ms};
use git_paw::error::PawError;
use git_paw::git;
use git_paw::interactive;
use git_paw::session::{self, Session, WorktreeEntry};
use git_paw::tmux;

mod commands;

fn main() {
    let args = Cli::parse();

    let command = args.command.unwrap_or(Command::Start {
        cli: None,
        branches: None,
        from_all_specs: false,
        specs: None,
        specs_format: None,
        dry_run: false,
        preset: None,
        supervisor: false,
        no_supervisor: false,
        force: false,
        no_rebase: false,
        unattended: false,
    });

    if let Err(err) = run(command) {
        err.exit();
    }
}

/// Dispatch and execute the given CLI command.
// Length is inherent to a total dispatch match — it grows by one arm per
// subcommand, with no logic to extract.
#[allow(clippy::too_many_lines)]
fn run(command: Command) -> Result<(), PawError> {
    match command {
        Command::Start {
            cli: cli_flag,
            branches: branches_flag,
            from_all_specs,
            specs,
            specs_format,
            dry_run,
            preset,
            supervisor,
            no_supervisor,
            force,
            no_rebase,
            unattended,
        } => {
            // Resolve supervisor-mode-enabled state BEFORE dispatching so the
            // user's --supervisor flag (or [supervisor] config) is honoured
            // when combined with --from-all-specs (or its hidden alias).
            // `--no-supervisor` (added in no-supervisor-flag) wins over both
            // the flag and any [supervisor] config setting.
            //
            // `--unattended` engages supervisor mode (per the cli-parsing
            // delta): it is treated as an implicit `--supervisor` so the
            // dispatch routes to the supervisor launch path, where step 15
            // branches on the flag to drive the in-process loop. clap already
            // rejects `--unattended --no-supervisor`.
            let supervisor_enabled = commands::supervisor::resolve_supervisor_mode_from_cwd(
                no_supervisor,
                supervisor || unattended,
                dry_run,
            )?;
            let spec_mode = SpecMode::from_flags(from_all_specs, specs.as_deref());
            let specs_format_str = specs_format.map(SpecsFormat::as_str);
            match resolve_dispatch_target(&spec_mode, supervisor_enabled) {
                DispatchTarget::Supervisor { spec_mode } => {
                    let cwd = std::env::current_dir().map_err(|e| {
                        PawError::SessionError(format!("cannot read current directory: {e}"))
                    })?;
                    let repo_root = git::validate_repo(&cwd)?;
                    let config = config::load_config(&repo_root, None)?;
                    commands::supervisor::cmd_supervisor(
                        &repo_root,
                        &config,
                        cli_flag.as_deref(),
                        branches_flag.as_deref(),
                        &spec_mode,
                        specs_format_str,
                        dry_run,
                        no_rebase,
                        unattended,
                    )
                }
                DispatchTarget::StartWithSpecs(mode) => commands::start::cmd_start_with_specs(
                    cli_flag.as_deref(),
                    &mode,
                    specs_format_str,
                    dry_run,
                    force,
                    no_rebase,
                ),
                DispatchTarget::Start => commands::start::cmd_start(
                    cli_flag,
                    branches_flag,
                    dry_run,
                    preset.as_deref(),
                    no_supervisor,
                    no_rebase,
                ),
            }
        }
        Command::Add {
            branch,
            cli,
            from_spec,
        } => commands::add::cmd_add(branch.as_deref(), cli.as_deref(), from_spec.as_deref()),
        Command::Remove {
            branch,
            keep_worktree,
            force,
        } => commands::remove::cmd_remove(&branch, keep_worktree, force),
        Command::Pause => commands::pause::cmd_pause(),
        Command::Stop => commands::stop::cmd_stop(),
        Command::Purge {
            force,
            stale,
            all_repos,
        } => cmd_purge(force, stale, all_repos),
        Command::Status { json } => commands::status::cmd_status(json),
        Command::Attach => commands::attach::cmd_attach(),
        Command::ListClis => commands::clis::cmd_list_clis(),
        Command::AddCli {
            name,
            command,
            display_name,
        } => commands::clis::cmd_add_cli(&name, &command, display_name.as_deref()),
        Command::RemoveCli { name } => commands::clis::cmd_remove_cli(&name),
        Command::SetupCli {
            cli,
            allowlist,
            trust,
            hooks,
            dry_run,
            yes,
            undo,
        } => cmd_setup_cli(&SetupCliArgs {
            cli: &cli,
            allowlist,
            trust,
            hooks,
            dry_run,
            yes,
            undo,
        }),
        Command::Dashboard => cmd_dashboard(),
        Command::PromptProfile { cli, field } => cmd_prompt_profile(cli.as_deref(), &field),
        Command::Init => git_paw::init::run_init(),
        Command::Replay {
            branch,
            list,
            color,
            session,
        } => commands::replay::cmd_replay(branch, list, color, session.as_deref()),
        Command::Approvals {
            session,
            limit,
            json,
        } => commands::approvals::cmd_approvals(session.as_deref(), limit, json),
        Command::Mcp { repo, log_file } => {
            git_paw::mcp::cmd_mcp(repo.as_deref(), log_file.as_deref())
        }
        Command::Doctor { json, live } => git_paw::doctor::run(json, live),
        Command::Completions { shell } => commands::completions::cmd_completions(shell),
        Command::Selftest => git_paw::selftest::run(),
    }
}

// ---------------------------------------------------------------------------
// Dispatch routing
// ---------------------------------------------------------------------------

/// Spec selection mode resolved from the `--from-all-specs` and `--specs`
/// CLI flags. The mode controls how the discovered `SpecEntry` set is
/// filtered before launching worktrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecMode {
    /// No spec mode requested — fall through to the regular branch flow.
    None,
    /// `--from-all-specs` (or its hidden `--from-specs` alias) — launch
    /// every discovered spec.
    All,
    /// Bare `--specs` — open the multi-select picker (TTY required).
    Picker,
    /// `--specs NAME[,NAME...]` — narrow to the supplied names.
    Narrow(Vec<String>),
}

impl SpecMode {
    /// Converts the parsed `(from_all_specs, specs)` flag pair into a
    /// `SpecMode`. Mutual exclusion is enforced at the clap layer, so this
    /// translation is total.
    pub fn from_flags(from_all_specs: bool, specs: Option<&[String]>) -> Self {
        if from_all_specs {
            return Self::All;
        }
        match specs {
            None => Self::None,
            Some([]) => Self::Picker,
            Some(values) => Self::Narrow(values.to_vec()),
        }
    }
}

/// Concrete dispatch target for the `start` subcommand. Extracted as a pure
/// function (`resolve_dispatch_target`) so the routing decision is unit-testable
/// independently of any IO.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DispatchTarget {
    /// Route to `cmd_supervisor`, carrying the resolved spec selection mode.
    ///
    /// The mode is passed through verbatim (not collapsed to a boolean) so the
    /// supervisor launch path applies the same subset filtering as the
    /// non-supervisor `--specs` path: `Narrow(names)` launches only the named
    /// subset, `Picker` opens the multi-select picker, `All` launches every
    /// discovered spec, and `None` falls through to the `--branches` /
    /// branch-picker flow. Fixes the v0.6.0 dogfood bug where
    /// `--supervisor --specs a,b` launched every discovered spec.
    Supervisor { spec_mode: SpecMode },
    /// Route to `cmd_start_with_specs` with the resolved spec mode (one of
    /// `All`, `Picker`, `Narrow`).
    StartWithSpecs(SpecMode),
    /// Route to bare `cmd_start`.
    Start,
}

/// Pure routing function for `start` subcommand dispatch.
///
/// When supervisor mode is enabled, every spec mode routes to the supervisor
/// path carrying the resolved `SpecMode` so subset filtering (`Narrow`),
/// the interactive `Picker`, `All`, and the bare branch-picker (`None`) cases
/// behave identically to the non-supervisor path — just with supervisor mode
/// engaged. Without supervisor mode, a populated spec mode routes through
/// `cmd_start_with_specs` and `SpecMode::None` falls through to bare `cmd_start`.
fn resolve_dispatch_target(spec_mode: &SpecMode, supervisor_enabled: bool) -> DispatchTarget {
    match (supervisor_enabled, spec_mode) {
        (true, mode) => DispatchTarget::Supervisor {
            spec_mode: mode.clone(),
        },
        (false, SpecMode::All | SpecMode::Picker | SpecMode::Narrow(_)) => {
            DispatchTarget::StartWithSpecs(spec_mode.clone())
        }
        (false, SpecMode::None) => DispatchTarget::Start,
    }
}

/// Applies the `SpecMode` filter to the discovered `SpecEntry` set.
///
/// - `SpecMode::None` and `SpecMode::All` return the input unchanged.
/// - `SpecMode::Picker` requires an interactive stdin; with a non-TTY stdin
///   this returns a `PawError::SpecError` directing the user at the
///   explicit forms.
/// - `SpecMode::Narrow(names)` resolves each name against the discovered
///   set via `git_paw::specs::resolve::resolve_specs`.
fn apply_spec_mode(
    mode: &SpecMode,
    discovered: Vec<git_paw::specs::SpecEntry>,
    prompter: &dyn interactive::Prompter,
) -> Result<Vec<git_paw::specs::SpecEntry>, PawError> {
    match mode {
        SpecMode::None | SpecMode::All => Ok(discovered),
        SpecMode::Picker => {
            if !is_interactive_stdin() {
                return Err(PawError::SpecError(
                    "--specs without values requires an interactive terminal\n  \
                     Use `--specs NAME[,NAME...]` to narrow explicitly, or\n  \
                     `--from-all-specs` to launch every discovered spec."
                        .to_string(),
                ));
            }
            prompter.select_specs(&discovered)
        }
        SpecMode::Narrow(names) => git_paw::specs::resolve::resolve_specs(&discovered, names),
    }
}

// ---------------------------------------------------------------------------
// Non-TTY launch handling
// ---------------------------------------------------------------------------

/// Returns `true` when stdin is connected to an interactive terminal.
///
/// Used to gate `tmux::attach` and the foreground supervisor-CLI launch so
/// non-TTY invocations (CI, scripts, harness tools) exit cleanly with an
/// attach hint instead of "failed to attach" errors.
/// Fixes dogfood D2 in `from-specs-launch-fixes`.
fn is_interactive_stdin() -> bool {
    std::io::stdin().is_terminal()
}

/// Calls `tmux::attach` when stdin is interactive; otherwise prints an
/// attach-hint to stdout and returns `Ok(())` so the launch flow exits
/// cleanly without making the session look broken.
fn attach_or_print_hint(session_name: &str) -> Result<(), PawError> {
    if is_interactive_stdin() {
        tmux::attach(session_name)
    } else {
        println!("Session '{session_name}' started in detached mode.");
        println!("Attach with:  tmux attach -t {session_name}");
        Ok(())
    }
}

/// GP-13 reattach guard: called when a live session already exists for the
/// repository, right before a fresh launch would otherwise reach
/// `resolve_session_name` and fork a `-N`-suffixed parallel one.
///
/// Interactively, attaches to the existing session (identical to
/// [`attach_or_print_hint`]'s interactive branch). Non-interactively there is
/// no terminal to attach to, and silently printing a hint and exiting `Ok`
/// would let an automated caller (a script, the drive loop, a scanner
/// advancing a phase) believe a fresh launch happened when it did not — so
/// this refuses with an actionable error instead.
fn reattach_active_session_or_refuse(session_name: &str) -> Result<(), PawError> {
    if !is_interactive_stdin() {
        return Err(PawError::SessionError(format!(
            "session '{session_name}' is already running for this repository. Re-running \
             `git paw start` non-interactively would otherwise fork a second, parallel session. \
             Attach from an interactive terminal, or run: tmux attach -t {session_name}"
        )));
    }
    println!("Reattaching to session '{session_name}'...");
    tmux::attach(session_name)
}

/// Builds the per-agent task prompt that gets appended to the supervisor-mode
/// boot block before injection via `tmux send-keys`.
///
/// **Per-backend dispatch.** When `spec_entry` is `Some`, the helper switches
/// on `SpecEntry.backend` to pick the prompt shape. Every branch first points
/// the agent at the gitignored sidecar
/// ([`SIDECAR_REL_PATH`](git_paw::agents::SIDECAR_REL_PATH)) — the combined
/// view of the project's `AGENTS.md` plus the agent's assignment and
/// coordination rules — before its apply/begin step:
///
/// - `SpecBackendKind::OpenSpec` → read the sidecar, then run the
///   `/opsx:apply <id>` slash command. (Pre-`agents-md-sidecar-injection`
///   this branch was the bare slash command with deliberately *no* pointer,
///   because the combined view was auto-loaded from the worktree-root
///   `AGENTS.md`. The sidecar is no longer auto-loaded by the CLIs, so the
///   pointer is now required for the agent to receive its assignment and
///   inter-agent rules. This intentionally overrides that earlier
///   no-pointer decision.)
/// - `SpecBackendKind::Markdown` → read the sidecar (which carries the full
///   spec body), then locate sibling artifacts under `openspec/changes/<id>/`.
/// - `SpecBackendKind::SpecKit` → read the sidecar only. Unlike `Markdown`,
///   the Spec Kit decomposition (`speckit::build_prompt`) already embeds the
///   feature spec, implementation plan, and task phase inline, so there is
///   no `openspec/changes/<id>/` (or any other) sibling directory to point
///   at — naming one would send the agent down a path that does not exist
///   under Spec Kit (speckit-spec-path-pointer).
///
/// The match SHALL be exhaustive over `SpecBackendKind` (no `_ =>`
/// catch-all); the Rust compiler forces a decision for any new variant.
///
/// The returned string points the agent at the sidecar rather than embedding
/// any portion of the spec body. The combined view is written to the sidecar
/// by `setup_worktree_agents_md`; the supported CLIs only auto-load the
/// worktree-root `AGENTS.md`, so the prompt is what directs the agent to the
/// sidecar's combined content. Pointing at a file avoids the paste-buffer trap
/// and duplicate content.
///
/// **Prerequisite:** callers SHALL ensure `setup_worktree_agents_md` has run
/// for the worktree before the resulting prompt is injected. The prompt's
/// guidance is non-actionable if the sidecar is missing.
///
/// When a spec is associated with the branch, `OpenSpec` and `Markdown`
/// entries embed the spec ID so the agent can locate sibling artifacts
/// (proposal, design, specs, tasks) under `openspec/changes/<id>/`; `SpecKit`
/// and `Superpowers` entries need no such pointer since their sidecar
/// already carries the full assignment. When no spec is associated, the
/// prompt is the default fallback that points only at the sidecar.
/// Bundled per-backend templates for [`build_task_prompt`], carrying the
/// `{{SIDECAR_REL_PATH}}` placeholder and, where the backend needs it, the
/// `{{SPEC_ID}}` placeholder.
const TASK_PROMPT_NO_SPEC: &str = include_str!("../assets/boot-task-prompts/no-spec.md");
const TASK_PROMPT_OPENSPEC: &str = include_str!("../assets/boot-task-prompts/openspec.md");
const TASK_PROMPT_MARKDOWN: &str = include_str!("../assets/boot-task-prompts/markdown.md");
const TASK_PROMPT_SPECKIT: &str = include_str!("../assets/boot-task-prompts/speckit.md");
const TASK_PROMPT_SUPERPOWERS: &str = include_str!("../assets/boot-task-prompts/superpowers.md");

pub(crate) fn build_task_prompt(spec_entry: Option<&git_paw::specs::SpecEntry>) -> String {
    use git_paw::agents::SIDECAR_REL_PATH;
    use git_paw::specs::SpecBackendKind;
    match spec_entry {
        Some(s) => {
            let template = match s.backend {
                SpecBackendKind::OpenSpec => TASK_PROMPT_OPENSPEC,
                SpecBackendKind::Markdown => TASK_PROMPT_MARKDOWN,
                SpecBackendKind::SpecKit => TASK_PROMPT_SPECKIT,
                SpecBackendKind::Superpowers => TASK_PROMPT_SUPERPOWERS,
            };
            template
                .replace("{{SIDECAR_REL_PATH}}", SIDECAR_REL_PATH)
                .replace("{{SPEC_ID}}", &s.id)
        }
        None => TASK_PROMPT_NO_SPEC.replace("{{SIDECAR_REL_PATH}}", SIDECAR_REL_PATH),
    }
}

/// Combines a boot block with the agent's task prompt into the single string
/// submitted to its pane. Shared by [`attach_agent`] (the supervisor and
/// `add` paths) and `commands::start::cmd_start_with_specs` so a spec-driven
/// launch's prompt is byte-identical to the supervisor path's for the same
/// branch, broker config, and spec entry (`start-spec-launch-parity`).
pub(crate) fn combine_boot_and_task_prompt(boot_block: &str, task_prompt: &str) -> String {
    format!("{boot_block}\n\n{task_prompt}")
}

/// Per-session context for [`attach_agent`] — the values shared across every
/// agent attached to a single session. `cmd_supervisor` builds it once before
/// its per-branch loop; `cmd_add` builds it once for the single new agent.
struct AttachContext<'a> {
    /// Repository root the worktrees hang off.
    repo_root: &'a Path,
    /// Human-readable project name (worktree-path convention).
    project: &'a str,
    /// Broker config (drives skill rendering, hook install, boot block).
    broker_config: &'a git_paw::config::BrokerConfig,
    /// Coding-agent CLI id (no flags).
    agent_cli: &'a str,
    /// Resolved approval flags appended to `agent_cli` for the pane command.
    agent_flags: &'a str,
    /// Pre-resolved coordination skill template (`None` when broker disabled).
    coordination_template: Option<&'a git_paw::skills::SkillTemplate>,
    /// Pre-resolved docs-fetch skill template (`None` unless `docs_base_url` is
    /// explicitly configured). Gated on explicit config so a consumer that has
    /// not pointed git-paw at their own docs never has the skill (which defaults
    /// to git-paw's site) injected into their agents.
    docs_fetch_template: Option<&'a git_paw::skills::SkillTemplate>,
    /// Gate-command substitutions for skill rendering.
    gate_commands: &'a git_paw::skills::GateCommands<'a>,
    /// Distinct spec backends in the session (for `{{SPEC_PATH_DOCTRINE}}`).
    session_backends: &'a [git_paw::specs::SpecBackendKind],
    /// Peer branches to build the inter-agent rules block from (`None` for the
    /// bare/no-rules case). Held as the raw branch list rather than a
    /// pre-rendered string so `attach_agent` can build a fresh block per
    /// agent — `build_inter_agent_rules` interpolates each agent's own id
    /// into the block, so the same string cannot be shared across agents.
    inter_agent_rules_peers: Option<&'a [&'a str]>,
    /// Whether the broker git hooks enforce the strict branch guard.
    strict_guard: bool,
    /// Skip rebasing the branch onto the default branch on worktree create.
    no_rebase: bool,
    /// Worktree placement (child vs sibling) for `create_worktree`.
    placement: git_paw::config::WorktreePlacement,
    /// Resolved `[worktree]` runtime-provisioning config (env-file copies and
    /// port allocation). Inert when neither sub-table is configured.
    worktree_runtime: &'a git_paw::config::WorktreeConfig,
    /// The full `[clis.<name>]` table, consulted by
    /// [`config::resolve_session_id_args`] to decide whether `agent_cli`
    /// gets a fresh session id at launch (`worker-session-id-capture`).
    clis: &'a std::collections::HashMap<String, config::CustomCli>,
}

/// Artifacts produced by attaching one agent's worktree: the tmux pane spec to
/// splice into the session, the initial boot+task prompt to submit, and the
/// session-JSON worktree entry to register.
struct AttachedAgent {
    /// Pane spec (branch, worktree path, CLI command) for the tmux layout.
    pane: tmux::PaneSpec,
    /// Combined boot block + initial task prompt to inject into the pane.
    prompt: String,
    /// Session-JSON worktree record for `status` / `stop` / `purge`.
    entry: WorktreeEntry,
}

/// Performs the per-worktree setup `git paw start` (supervisor mode) does for a
/// single agent: create or reuse the worktree, render the coordination skill +
/// spec body into the worktree `AGENTS.md`, install the broker git hooks, and
/// build the pane spec, boot+task prompt, and session entry.
///
/// Factored out of `cmd_supervisor`'s per-branch loop (design D2, task 1.2) so
/// `cmd_supervisor` (looping over every branch) and `cmd_add` (once, for the
/// new branch) share one implementation — guaranteeing an added agent is
/// byte-identical to a start-time agent and that future boot-prompt fixes only
/// have to land here.
/// Builds the dashboard's learnings aggregator, seeded from the persisted
/// session.
///
/// `session` is the receipt `cmd_dashboard` loaded for this repository. When
/// it is present, [`LearningsAggregator::seed_from_session`] registers every
/// agent AND records the spec each was launched from — seeding the roster
/// alone leaves the agent → spec mapping empty, which reports every forward
/// conflict cross-spec even between two agents launched from the same spec.
/// When no receipt is found there is nothing to seed, and classification
/// falls back to cross-spec exactly as before.
///
/// Extracted from `cmd_dashboard` so the seeding is reachable from a test:
/// the wiring is the whole point of `supervisor-boot-wiring`, and a correct
/// helper nobody calls is the defect this change exists to fix.
fn build_learnings_aggregator(
    learnings_path: std::path::PathBuf,
    supervisor: &SupervisorConfig,
    session: Option<&Session>,
    broker_enabled: bool,
) -> broker::learnings::LearningsAggregator {
    let mut aggregator = broker::learnings::LearningsAggregator::new(learnings_path);
    aggregator.set_broker_publish(
        supervisor
            .learnings_config
            .broker_publish
            .resolve(broker_enabled),
    );
    if let Some(session) = session {
        aggregator.seed_from_session(session);
    }
    aggregator
}

/// Assembles an agent pane's CLI invocation: the launch command, then its
/// approval flags, then the resolved `session_id_args` — each appended only
/// when non-empty, so a CLI with neither is invoked bare.
///
/// `command` is the pane's *launch command* ([`config::launch_command`]),
/// not the CLI identity: flags and session-id args are resolved by identity
/// but appended after the command the pane actually runs.
fn agent_cli_command(command: &str, flags: &str, session_id_args: Option<&str>) -> String {
    let mut command = command.to_string();
    for part in [flags, session_id_args.unwrap_or_default()] {
        if !part.is_empty() {
            command.push(' ');
            command.push_str(part);
        }
    }
    command
}

fn attach_agent(
    ctx: &AttachContext,
    branch: &str,
    spec_entry: Option<&git_paw::specs::SpecEntry>,
    runtime_slot: u16,
) -> Result<AttachedAgent, PawError> {
    let wt = git::create_worktree(ctx.repo_root, branch, !ctx.no_rebase, ctx.placement)?;
    let wt_str = wt.path.to_string_lossy().to_string();

    // Provision the worktree's runtime state — the declared env-file copies,
    // this agent's port block in a generated `.env.local`, and the consumer's
    // `on_create` hook. This runs here, after the worktree exists and before the
    // pane spec this function returns is ever launched, so the agent's CLI
    // observes a fully provisioned checkout on its first read. A no-op unless
    // `[worktree.env]` / `[worktree.ports]` / `[worktree.hooks]` is configured.
    let provisioned = git_paw::worktree_provision::provision_worktree(
        ctx.repo_root,
        &wt.path,
        branch,
        ctx.worktree_runtime,
        runtime_slot,
    )?;
    for warning in &provisioned.warnings {
        eprintln!("warning: {branch}: {warning}");
    }
    if let Some(summary) = provisioned.hook_summary() {
        println!("  {branch}: {summary}");
    }

    // Provision the bundled helper scripts the agent's boot block invokes into
    // this worktree's `.git-paw/scripts/` (idempotent, executable). A fresh
    // worktree checkout has no `.git-paw/scripts/` (it is gitignored), so
    // without this the agent's relative `.git-paw/scripts/broker.sh` call fails
    // until it hand-copies from `assets/`. Sourced from the same embedded
    // assets `git paw init` uses so the worktree's helper matches the running
    // binary's version. `broker.sh` when the broker is enabled; `docs-fetch.sh`
    // when `docs_base_url` is configured (the same gate as the docs-fetch skill
    // injection — `ctx.docs_fetch_template` is `Some` iff it is configured).
    git_paw::init::provision_worktree_helpers(
        &wt.path,
        ctx.broker_config.enabled,
        ctx.docs_fetch_template.is_some(),
    )?;

    let render_tmpl = |tmpl: &git_paw::skills::SkillTemplate| {
        git_paw::skills::render(
            tmpl,
            branch,
            &ctx.broker_config.url(),
            ctx.project,
            ctx.gate_commands,
            ctx.session_backends,
        )
    };
    // Combine the coordination and docs-fetch skills into one managed block.
    // Either may be absent (coordination when broker is off; docs-fetch unless
    // `docs_base_url` is configured); the block carries whichever are present.
    let rendered_skill = git_paw::skills::combine_agent_skills(
        ctx.coordination_template.map(render_tmpl),
        ctx.docs_fetch_template.map(render_tmpl),
    );

    let spec_content = spec_entry.map(|s| s.prompt.clone());
    let owned_files = spec_entry.and_then(|s| s.owned_files.clone());

    let assignment = git_paw::agents::WorktreeAssignment {
        branch: branch.to_string(),
        cli: ctx.agent_cli.to_string(),
        spec_content,
        owned_files,
        skill_content: rendered_skill,
        inter_agent_rules: ctx
            .inter_agent_rules_peers
            .map(|peers| git_paw::agents::build_inter_agent_rules(branch, peers)),
    };
    git_paw::agents::setup_worktree_agents_md(ctx.repo_root, &wt.path, &assignment)?;

    if ctx.broker_config.enabled {
        let agent_id = git_paw::broker::messages::slugify_branch(branch);
        git_paw::agents::install_git_hooks(
            &wt.path,
            &ctx.broker_config.url(),
            &agent_id,
            branch,
            ctx.strict_guard,
        )?;
    }

    // Assign a fresh session id when this CLI resolves a `session_id_args`
    // template — generated (and thus persisted) before the CLI ever starts,
    // so recovery can resume it even if the pane dies moments later
    // (worker-session-id-capture design D1). Appended after the approval
    // flags, mirroring how those are appended after the bare CLI command.
    let has_session_id_template =
        config::resolve_session_id_args(ctx.agent_cli, "probe", ctx.clis).is_some();
    let cli_session_id = if has_session_id_template {
        Some(git_paw::random::generate_uuid_v4()?)
    } else {
        None
    };
    let session_id_args = cli_session_id
        .as_deref()
        .and_then(|id| config::resolve_session_id_args(ctx.agent_cli, id, ctx.clis));

    // The pane launches the identity's configured `[clis.<name>].command`
    // (falling back to the identity itself when unconfigured); every resolver
    // above and the `cli` recorded below stay keyed by `ctx.agent_cli`.
    let cli_command = agent_cli_command(
        &config::launch_command(ctx.agent_cli, ctx.clis),
        ctx.agent_flags,
        session_id_args.as_deref(),
    );

    let boot_block = git_paw::skills::build_boot_block(
        branch,
        &ctx.broker_config.url(),
        git_paw::skills::cli_speaks_mcp(ctx.agent_cli, ctx.clis),
    );
    let task_prompt = build_task_prompt(spec_entry);

    Ok(AttachedAgent {
        pane: tmux::PaneSpec {
            branch: branch.to_string(),
            worktree: wt_str,
            cli_command,
        },
        prompt: combine_boot_and_task_prompt(&boot_block, &task_prompt),
        entry: WorktreeEntry {
            branch: branch.to_string(),
            worktree_path: wt.path,
            cli: ctx.agent_cli.to_string(),
            branch_created: wt.branch_created,
            base_branch: created_branch_base(ctx.repo_root, wt.branch_created),
            pending_boot_prompt: None,
            runtime_slot: provisioned.runtime_slot,
            cli_session_id,
            spec_id: spec_entry.map(|s| s.id.clone()),
        },
    })
}

/// Gates a pane before boot-block injection via
/// [`tmux::gate_pane_for_injection`] and fails loudly when the pane is
/// blocked on an unanswered first-run acceptance dialog (GP-03b).
///
/// A [`tmux::ReadinessOutcome::DialogStuck`] pane SHALL NOT receive the boot
/// block on top of an unresolved dialog — a stuck pane is never reported a
/// healthy running agent — so this returns `Err` instead of injecting. Every
/// other outcome (`Ready` or `FellBack`) proceeds exactly as before; the
/// caller injects the prompt regardless.
///
/// `cli_id` is the CLI's identity (the `[clis.<name>]` key or a built-in /
/// pass-through name), which resolves the readiness markers; `cli_command` is
/// what the pane actually runs and is what a bare-shell relaunch types.
fn gate_pane_or_fail_on_dialog<S: std::hash::BuildHasher>(
    session_name: &str,
    pane_index: usize,
    cli_id: &str,
    cli_command: &str,
    clis: &std::collections::HashMap<String, config::CustomCli, S>,
) -> Result<(), PawError> {
    dialog_stuck_result(
        tmux::gate_pane_for_injection(session_name, pane_index, cli_id, cli_command, clis),
        session_name,
        pane_index,
        cli_command,
    )
}

/// The pure decision behind [`gate_pane_or_fail_on_dialog`]: `Err` exactly
/// when `outcome` is [`tmux::ReadinessOutcome::DialogStuck`], `Ok(())`
/// otherwise (`Ready` or `FellBack` proceed exactly as before). Split out so
/// the loud-failure wiring is unit-testable without a live tmux pane.
fn dialog_stuck_result(
    outcome: tmux::ReadinessOutcome,
    session_name: &str,
    pane_index: usize,
    cli_command: &str,
) -> Result<(), PawError> {
    if matches!(outcome, tmux::ReadinessOutcome::DialogStuck) {
        return Err(PawError::TmuxError(format!(
            "pane {pane_index} ({cli_command}) is blocked on an unanswered first-run \
             acceptance dialog; attach with `tmux attach -t {session_name}` and resolve it \
             manually, then retry"
        )));
    }
    Ok(())
}

/// Inject and submit an initial prompt into a tmux pane.
///
/// The boot block is injected literally, then — after `delay_ms` for a
/// paste-aware CLI to settle the (often large) paste — a separate `Enter`
/// submits it. Splitting the inject from the submit (rather than a
/// same-call trailing `Enter`) is what reliably submits a large paste
/// across CLIs (W15-1). `delay_ms` is resolved per-CLI from config so this
/// path carries no CLI-specific assumptions. Failures are swallowed —
/// best-effort by design.
fn submit_prompt_to_pane(session_name: &str, pane_idx: usize, prompt: &str, delay_ms: u64) {
    let target = format!("{session_name}:0.{pane_idx}");
    // 1. Inject the boot block literally (no Enter yet).
    let _ = std::process::Command::new("tmux")
        .args(["send-keys", "-t", &target, "-l", prompt])
        .status();
    // 2. Let a paste-aware CLI settle the paste before we submit.
    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    // 3. Submit with a separate Enter.
    let _ = std::process::Command::new("tmux")
        .args(["send-keys", "-t", &target, "Enter"])
        .status();
}

/// Writes the per-repo discovery file (`<repo>/.git-paw/sessions/<name>.json`)
/// the bundled `sweep.sh` helper reads (capability `session-json-location`).
///
/// Builds the sweep.sh-compatible agent roster from the launched worktrees:
/// each entry carries the broker agent id (`branch_id`), worktree path, CLI,
/// and the tmux `pane_index`. `pane_offset` is the index of the first coding
/// agent pane — `SUPERVISOR_PANE_OFFSET` for the supervisor layout, or
/// `1`/`0` for the bare layout depending on whether the dashboard pane is
/// present. `orchestrator` records the supervisor pane's own entry — `None`
/// for a bare-mode session, which has no orchestrator pane. Best-effort: a
/// write failure is surfaced as a warning and does not abort the launch,
/// since the global receipt remains the source of truth.
fn write_repo_discovery_file(
    repo_root: &Path,
    session_name: &str,
    worktrees: &[WorktreeEntry],
    pane_offset: usize,
    orchestrator: Option<session::RepoOrchestratorEntry>,
) {
    let agents = worktrees
        .iter()
        .enumerate()
        .map(|(idx, wt)| session::RepoAgentEntry {
            branch_id: git_paw::broker::messages::slugify_branch(&wt.branch),
            worktree_path: wt.worktree_path.clone(),
            cli: wt.cli.clone(),
            pane_index: pane_offset + idx,
        })
        .collect();
    write_repo_discovery_file_with_agents(repo_root, session_name, agents, orchestrator);
}

/// [`write_repo_discovery_file`] for a caller that has already resolved each
/// agent's pane index.
///
/// The offset form above assumes agents occupy a contiguous run of panes
/// starting at `pane_offset` — true of every path that BUILDS the layout.
/// Restart-from-pause does not build it: it splits a new dashboard pane into a
/// live session at whatever index tmux assigns, so the agent indices are read
/// back from the running session instead of derived. Best-effort, like the
/// offset form: a write failure is a warning, not a launch failure.
fn write_repo_discovery_file_with_agents(
    repo_root: &Path,
    session_name: &str,
    agents: Vec<session::RepoAgentEntry>,
    orchestrator: Option<session::RepoOrchestratorEntry>,
) {
    let file = session::RepoSessionFile {
        session_name: session_name.to_string(),
        agents,
        orchestrator,
    };
    if let Err(e) = session::write_repo_session_file(repo_root, &file) {
        eprintln!("warning: failed to write per-repo session discovery file: {e}");
    }
}

// ---------------------------------------------------------------------------
// Command: __prompt-profile
// ---------------------------------------------------------------------------

/// Regex-escapes `marker` so it can be joined into an extended-regex
/// alternation without its literal characters (parentheses, brackets, …)
/// being read as regex syntax.
fn escape_for_regex_alternation(marker: &str) -> String {
    let mut escaped = String::with_capacity(marker.len());
    for c in marker.chars() {
        if ".^$*+?()[]{}|\\".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// Renders `broad_grant_marker` as a `grep -E` pattern that matches the
/// marker whether the terminal drew an ASCII apostrophe or the Unicode right
/// single quote.
///
/// Claude's marker is `don't ask again`, and a terminal may render that
/// apostrophe either way, so a literal match misses half the time. The
/// tolerance belongs here rather than in `sweep.sh` for two reasons: the
/// shell helper must stay free of apostrophes (they desync quoting inside its
/// `$(cat <<'PY' …)` heredocs), and relaxing the ESCAPED form in shell is
/// subtly wrong — a marker containing a regex metacharacter escapes to `\(`,
/// and relaxing both characters would then demand two characters where the
/// pane renders one, making the guard MISS. Escaping after the substitution
/// keeps every other character literal.
fn broad_grant_marker_regex(marker: &str) -> String {
    let mut out = String::with_capacity(marker.len() + 8);
    for c in marker.chars() {
        if c == '\'' || c == '\u{2019}' {
            // A character class of both quote forms; neither needs escaping
            // inside brackets.
            out.push_str("['\u{2019}]");
        } else if ".^$*+?()[]{}|\\".contains(c) {
            out.push('\\');
            out.push(c);
        } else {
            out.push(c);
        }
    }
    out
}

/// Prints one resolved [`config::CliPromptProfile`] field for the bundled
/// `sweep.sh` helper (internal command): a marker-list field prints as a
/// regex-escaped `|`-joined alternation (ready for `grep -E`); a pattern
/// field prints its raw regex source.
///
/// Resolves the profile for `cli` (falling back to `[supervisor].cli`, then
/// `default_cli`, matching how the boot-prompt flags resolve the session
/// CLI) against the config loaded from the CURRENT WORKING DIRECTORY — MUST
/// be invoked from inside the target repository, as `sweep.sh` does.
///
/// `cli-prompt-profiles` capability: this is the single source `sweep.sh`
/// reads instead of carrying its own copies of the markers/regexes.
fn cmd_prompt_profile(cli: Option<&str>, field: &str) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git::validate_repo(&cwd)?;
    let config = config::load_config(&repo_root, None).unwrap_or_default();
    let resolved_cli = cli
        .map(str::to_string)
        .or_else(|| config.supervisor.as_ref().and_then(|s| s.cli.clone()))
        .or_else(|| config.default_cli.clone())
        .unwrap_or_default();
    let profile = config::resolve_prompt_profile(&resolved_cli, &config.clis);

    let marker_alternation = |markers: &[String]| {
        markers
            .iter()
            .map(|m| escape_for_regex_alternation(m))
            .collect::<Vec<_>>()
            .join("|")
    };

    let value = match field {
        "readiness_markers" => marker_alternation(&profile.readiness_markers),
        "approval_markers" => marker_alternation(&profile.approval_markers),
        "live_prompt_markers" => marker_alternation(&profile.live_prompt_markers),
        "mid_response_markers" => marker_alternation(&profile.mid_response_markers),
        "prompt_boilerplate_markers" => marker_alternation(&profile.prompt_boilerplate_markers),
        "command_header_markers" => marker_alternation(&profile.command_header_markers),
        "mode_accept_edits_markers" => marker_alternation(&profile.mode_accept_edits_markers),
        "mode_interactive_markers" => marker_alternation(&profile.mode_interactive_markers),
        "stream_error_markers" => marker_alternation(&profile.stream_error_markers),
        "input_sigils" => marker_alternation(&profile.input_sigils),
        "file_prompt_pattern" => profile.file_prompt_pattern.clone(),
        "option_line_pattern" => profile.option_line_pattern.clone(),
        "broad_grant_marker" => broad_grant_marker_regex(&profile.broad_grant_marker),
        "context_bloat_pattern" => profile.context_bloat_pattern.clone(),
        "paste_buffer_pattern" => profile.paste_buffer_pattern.clone(),
        "cli_unavailable_markers" => marker_alternation(&profile.cli_unavailable_markers),
        other => {
            return Err(PawError::SessionError(format!(
                "unknown prompt-profile field '{other}'"
            )));
        }
    };
    println!("{value}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Command: __dashboard
// ---------------------------------------------------------------------------

/// Runs the broker and dashboard in pane 0 (internal command).
#[allow(clippy::too_many_lines)]
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
    let config = config::load_config(&repo_root, None)?;
    let broker_config = config.broker.clone();
    // Broker log panel: ring-buffer cap + initial visibility from
    // `[dashboard.broker_log]`. An absent `[dashboard]` section uses the
    // documented defaults (cap 500, visible).
    let broker_log_cfg = config
        .dashboard
        .as_ref()
        .map(|d| d.broker_log.clone())
        .unwrap_or_default();

    // The conflict detector subsystem runs only when supervisor mode is
    // enabled — its outputs flow through the supervisor inbox and are
    // tagged for the supervisor agent's attention.
    let conflict_cfg = config
        .supervisor
        .as_ref()
        .filter(|s| s.enabled)
        .map(|s| s.conflict.clone());

    // Learnings flush cadence (seconds). The aggregator itself is attached
    // separately to `broker_state` when `[supervisor] learnings = true`;
    // here we just pass the interval into start_broker_with so the flush
    // thread spawns with the user-configured cadence.
    let learnings_interval_seconds = config
        .supervisor
        .as_ref()
        .map_or(60, |s| s.learnings_config.flush_interval_seconds);

    // Per-commit verification nudge: when an agent commits, the broker pings
    // the supervisor to verify that commit immediately (default on; opt out
    // with `[supervisor] verify_on_commit_nudge = false`). Resolves to `true`
    // when no `[supervisor]` section is present.
    let verify_on_commit_nudge = config
        .supervisor
        .as_ref()
        .is_none_or(SupervisorConfig::verify_on_commit_nudge_enabled);

    // Resolve the supervisor's CLI the same way the launcher did
    // (`[supervisor].cli` > `default_cli`) and seed it authoritatively. The
    // supervisor is not a filesystem watch target, so this is the only
    // deterministic source for its dashboard CLI column — relying on the
    // supervisor to self-report via `agent.status` is unreliable (W15-15).
    let supervisor_cli = config
        .supervisor
        .as_ref()
        .and_then(|s| s.cli.clone())
        .or_else(|| config.default_cli.clone())
        .unwrap_or_default();

    // Look up the saved session once: needed for the per-session broker log
    // name, the broker watcher target list, the learnings aggregator's
    // agent -> spec mapping AND to discover the supervisor pane_map for the
    // auto-approve thread.
    //
    // This read happens once, with no retry, and `cmd_supervisor` writes the
    // receipt *after* spawning this pane (deliberately: a session that failed
    // to start must not be recorded). This pane therefore races a receipt that
    // is normally already on disk by the time the binary has started. The
    // watcher targets have always depended on winning that race; the learnings
    // seeding now does too, and losing it degrades silently — no receipt means
    // no agent -> spec mapping, so every forward conflict reports cross-spec.
    // If that degradation is ever observed, the fix is a bounded retry here,
    // not reordering the write.
    let saved_session = session::find_session_for_repo(&repo_root)?;

    // One log per session, so two repositories never write — or purge — the
    // same file. See `session::resolve_broker_log_path` for why the saved
    // path wins over the derived one.
    let log_path = session::resolve_broker_log_path(saved_session.as_ref())?;
    let mut broker_state = broker::BrokerState::new(Some(log_path))
        .with_repo_root(&repo_root)
        .with_verify_on_commit_nudge(verify_on_commit_nudge)
        .with_seeded_cli("supervisor", &supervisor_cli);

    let watch_targets = saved_session
        .as_ref()
        .map(|sess| {
            sess.worktrees
                .iter()
                .map(|wt| broker::WatchTarget {
                    agent_id: broker::messages::slugify_branch(&wt.branch),
                    cli: wt.cli.clone(),
                    worktree_path: wt.worktree_path.clone(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Attach the opsx role-gating context. The guard is scoped to the OpenSpec
    // spec engine: under speckit/markdown (or no spec source) `engine_is_openspec`
    // is false and the guard stays inert regardless of the configured mode. The
    // roster maps each coding agent's worktree plus the supervisor's repo root,
    // so the guard can attribute a committing worktree and clear the supervisor's
    // own archives.
    {
        let engine_is_openspec =
            git_paw::specs::resolved_spec_type(&config, &repo_root).as_deref() == Some("openspec");
        let mut roster: Vec<(String, std::path::PathBuf)> = watch_targets
            .iter()
            .map(|t| (t.agent_id.clone(), t.worktree_path.clone()))
            .collect();
        roster.push((
            git_paw::opsx::SUPERVISOR_AGENT_ID.to_string(),
            repo_root.clone(),
        ));
        broker_state = broker_state.with_role_gating(git_paw::opsx::RoleGatingContext {
            mode: config.role_gating_mode(),
            engine_is_openspec,
            roster,
        });
    }

    // Attach the learnings aggregator when supervisor mode + learnings are
    // both enabled (mirrors the `should_attach` predicate pinned by the
    // learnings unit test). The aggregator appends to
    // `.git-paw/session-learnings.md` and — when broker publish resolves to
    // active — additionally emits `agent.learning` records through the
    // broker (the `agent-learning-variant` dual-output path).
    if let Some(sup) = config
        .supervisor
        .as_ref()
        .filter(|s| s.enabled && s.learnings)
    {
        let aggregator = build_learnings_aggregator(
            repo_root.join(".git-paw").join("session-learnings.md"),
            sup,
            saved_session.as_ref(),
            broker_config.enabled,
        );
        broker_state.attach_learnings(std::sync::Arc::new(std::sync::Mutex::new(aggregator)));
    }

    // Start the in-process broker. A bind/probe failure — most often a stale
    // dashboard still squatting the port — is terminal: emit a diagnostic and
    // exit non-zero rather than fall through to the render loop. A second
    // dashboard busy-rendering against a contended port is exactly the
    // CPU-pegging leak this guards against, so we never retry or loop here.
    let handle = match broker::start_broker_with(
        &broker_config,
        broker_state,
        watch_targets,
        conflict_cfg,
        learnings_interval_seconds,
    ) {
        Ok(handle) => handle,
        Err(err) => {
            eprintln!(
                "git-paw dashboard: broker failed to start ({err}); exiting instead of busy-looping"
            );
            return Err(err.into());
        }
    };
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
        broker_log_cfg.max_messages,
        broker_log_cfg.default_visible,
        broker_log_cfg.height_lines,
    )
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
///
/// With `stale = true`, purges only stale sessions (receipt claims active but
/// the tmux session is gone), scoped to the current repository by default;
/// `all_repos = true` (only meaningful with `stale`) broadens the sweep to
/// every repository on the machine. Live sessions are always left untouched.
/// `--force` is redundant in the `--stale` combination (a stale entry is
/// never prompted for) — passing both behaves identically to `--stale` alone.
fn cmd_purge(force: bool, stale: bool, all_repos: bool) -> Result<(), PawError> {
    if stale {
        return cmd_purge_stale(all_repos);
    }

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

/// Probes the existing receipt for staleness and, when stale (the receipt
/// claims active but the tmux session is gone), invalidates it — removing the
/// recorded worktrees and the receipt equivalent to `purge --force` — and emits
/// a stderr notice naming the entry (design D5).
///
/// Like every other teardown path it goes through `detach_worktree`, so a
/// branch is deleted only when git-paw created it and it is already contained
/// in the default branch or the branch it was created from; anything else is
/// kept.
///
/// Returns `true` when invalidation fired; the caller SHALL then proceed with
/// a fresh launch as if no prior session existed. Returns `false` for a live
/// (`active` + alive), paused, stopped, or indeterminate receipt, leaving the
/// caller's normal reattach/recover decision intact.
fn invalidate_if_stale(repo_root: &Path, existing: &Session) -> Result<bool, PawError> {
    let liveness = tmux::session_liveness(&existing.session_name);
    if session::DisplayStatus::from_receipt(&existing.status, liveness)
        != session::DisplayStatus::Stale
    {
        return Ok(false);
    }

    let when = existing
        .created_at_iso8601()
        .map(|t| format!(", last seen {t}"))
        .unwrap_or_default();
    eprintln!(
        "notice: removed stale session receipt\n  ({}{}, tmux session no longer exists)",
        existing.session_name, when
    );

    let sessions_dir = session::session_state_dir()?;
    let mut confirm = |_: &str| -> Result<bool, PawError> { Ok(true) };
    let mut kill_tmux = |name: &str| -> Result<(), PawError> {
        if tmux::is_session_alive(name)? {
            tmux::kill_session(name)?;
        }
        Ok(())
    };
    purge_with_prompt(
        repo_root,
        &sessions_dir,
        existing,
        true,
        &mut confirm,
        &mut kill_tmux,
        &mut std::io::stderr(),
    )?;
    Ok(true)
}

/// Purges only stale sessions (receipt claims active but the tmux session is
/// gone), scoped to the current repository by default; `all_repos = true`
/// broadens the sweep to every repository on the machine. Live sessions are
/// always left untouched.
///
/// Stale is defined exactly as [`session::DisplayStatus::Stale`]: an `active`
/// receipt whose `tmux has-session` probe returns
/// [`tmux::SessionLiveness::Stale`]. Stopped receipts (intentionally stopped)
/// and sessions on a host with no tmux binary (Indeterminate probe) are NOT
/// touched — the fail-safe guarantee: a receipt is purged only when the probe
/// positively confirms staleness, never on a live or indeterminate liveness
/// read. Exits 0 with a "nothing to purge" message when no stale receipt is
/// in scope.
fn cmd_purge_stale(all_repos: bool) -> Result<(), PawError> {
    let sessions_dir = session::session_state_dir()?;
    let all = session::load_all_sessions_in(&sessions_dir)?;

    let repo_root = if all_repos {
        None
    } else {
        let cwd = std::env::current_dir()
            .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
        Some(git::validate_repo(&cwd)?)
    };

    let stale: Vec<session::Session> = all
        .into_iter()
        .filter(|s| {
            if let Some(root) = &repo_root
                && !session::session_belongs_to_repo(s, root)
            {
                return false;
            }
            let liveness = tmux::session_liveness(&s.session_name);
            session::DisplayStatus::from_receipt(&s.status, liveness)
                == session::DisplayStatus::Stale
        })
        .collect();

    if stale.is_empty() {
        println!("No stale sessions to purge.");
        return Ok(());
    }

    let mut confirm = |_: &str| -> Result<bool, PawError> { Ok(true) };
    let mut kill_tmux = |name: &str| -> Result<(), PawError> {
        if tmux::is_session_alive(name)? {
            tmux::kill_session(name)?;
        }
        Ok(())
    };

    for session_entry in &stale {
        // force = true: stale entries are orphaned, nothing to confirm.
        let outcome = purge_with_prompt(
            &session_entry.repo_path,
            &sessions_dir,
            session_entry,
            true,
            &mut confirm,
            &mut kill_tmux,
            &mut std::io::stderr(),
        )?;
        if outcome == PurgeOutcome::Purged {
            println!("Purged stale session '{}'.", session_entry.session_name);
        }
    }
    Ok(())
}

/// Tears down a single agent's worktree: runs the consumer's `on_remove` hook,
/// removes the worktree directory (with the per-worktree
/// `Removing worktree ...` / `...done (Xs)` progress markers) and, when git-paw
/// created the branch, deletes it afterwards.
///
/// Extracted from `cmd_purge`'s per-worktree loop (design D6, task 1.3) so
/// `cmd_remove` performs byte-identical removal for a single agent that
/// `git paw purge` performs for every agent. Both teardown paths therefore run
/// the hook by construction — `on_remove` lives here rather than being repeated
/// at each command handler. Best-effort throughout: a failed hook, a failed
/// worktree-remove, or a failed branch-delete is surfaced as a `warning:` on
/// `stderr` and does not abort — a resource git-paw could not drop must never
/// cost the operator a stranded worktree.
///
/// `on_remove` is the configured `[worktree.hooks] on_remove` command, or `None`
/// when the consumer declared no hook (in which case nothing is run).
fn detach_worktree(
    repo_root: &Path,
    entry: &WorktreeEntry,
    on_remove: Option<&str>,
    stderr: &mut dyn std::io::Write,
) {
    // Before the directory is deleted, so the hook can still read the checkout.
    if let Some(command) = on_remove
        && let Some(warning) = git_paw::worktree_provision::run_remove_hook(
            command,
            &entry.branch,
            &entry.worktree_path,
        )
    {
        let _ = writeln!(stderr, "warning: {warning}");
    }

    let _ = writeln!(
        stderr,
        "Removing worktree {}...",
        entry.worktree_path.display()
    );
    let _ = stderr.flush();
    let started = std::time::Instant::now();
    let result = git::remove_worktree(repo_root, &entry.worktree_path);
    let elapsed_secs = started.elapsed().as_secs_f64();
    match result {
        Ok(()) => {
            let _ = writeln!(stderr, "  ...done ({elapsed_secs:.1}s)");
        }
        Err(e) => {
            let _ = writeln!(
                stderr,
                "warning: failed to remove worktree '{}' after {:.1}s: {e}",
                entry.worktree_path.display(),
                elapsed_secs
            );
        }
    }
    let _ = stderr.flush();

    if entry.branch_created {
        retire_created_branch(repo_root, entry, stderr);
    }
}

/// The start point to record on a worktree entry git-paw just created a branch
/// for — the main checkout's current branch, which is what
/// `git worktree add -b <branch> <path>` branches from.
///
/// `None` when the branch already existed (there is no git-paw-chosen start
/// point to record) or when `HEAD` is detached. Teardown treats a missing value
/// as "compare against the default branch only", which is the pre-existing
/// behaviour, so an unrecorded entry is never less safe.
pub(crate) fn created_branch_base(repo_root: &Path, branch_created: bool) -> Option<String> {
    if !branch_created {
        return None;
    }
    git::current_branch(repo_root).ok()
}

/// Where a branch's commits stand relative to the branches it could have
/// landed in.
enum MergeState {
    /// Contained in the default branch, or in the branch it was created from.
    Merged,
    /// Carries commits that are in neither. `count` is `None` when the
    /// comparison succeeded but counting the commits did not.
    Unmerged {
        count: Option<usize>,
        compared_against: String,
    },
    /// No comparison could be made; the wrapped text says why.
    Undeterminable(String),
}

/// Compares `entry`'s branch against the default branch and, when one was
/// recorded, the branch it was created from.
///
/// Both comparisons matter because `git worktree add -b` takes no start point:
/// a branch git-paw created starts at whatever the main checkout had checked
/// out. Launched from an integration branch, an agent branch that has been
/// merged *into that integration branch* is finished work, but it is not yet an
/// ancestor of the default branch — comparing against the default alone would
/// keep it and report the entire integration backlog as its "unmerged" count.
/// Containment in **either** branch therefore means merged, and the count is
/// taken against the recorded base when there is one, so the number describes
/// the branch's own commits.
///
/// Fails closed: when every comparison errors the result is
/// [`MergeState::Undeterminable`], which callers treat as "retain".
fn branch_merge_state(
    repo_root: &Path,
    entry: &WorktreeEntry,
    default_branch: Option<&str>,
) -> MergeState {
    // The recorded base first: it is the more specific question, and its label
    // is the one a count should be reported against.
    let mut targets: Vec<&str> = Vec::new();
    if let Some(base) = entry.base_branch.as_deref() {
        targets.push(base);
    }
    if let Some(default) = default_branch
        && !targets.contains(&default)
    {
        targets.push(default);
    }

    let mut first_comparable: Option<&str> = None;
    let mut failures = Vec::new();
    for target in targets {
        match git::branch_is_merged_into(repo_root, &entry.branch, target) {
            Ok(true) => return MergeState::Merged,
            Ok(false) => first_comparable.get_or_insert(target),
            Err(e) => {
                failures.push(format!("{target}: {e}"));
                continue;
            }
        };
    }

    // At least one target answered "not contained" — report against the first
    // one, which is the recorded base when there is one. If none answered at
    // all, nothing is known and the branch must be kept.
    let Some(compared_against) = first_comparable else {
        return MergeState::Undeterminable(if failures.is_empty() {
            "the default branch could not be resolved and no base branch was recorded".to_string()
        } else {
            failures.join("; ")
        });
    };

    MergeState::Unmerged {
        count: git::unmerged_commit_count(repo_root, &entry.branch, compared_against).ok(),
        compared_against: compared_against.to_string(),
    }
}

/// `-d` when git would accept the safe delete for `branch`, `-D` otherwise.
///
/// `git branch -d` refuses unless the branch is contained in the checked-out
/// branch, so suggesting it blindly hands the operator a command that fails.
/// Suggesting `-D` blindly is worse: it is the irreversible form, and an
/// operator who pastes it loses the branch even in the cases where git would
/// have vetoed the delete. The check is the same containment question git
/// itself asks.
fn discard_flag(repo_root: &Path, branch: &str) -> &'static str {
    let safe = git::current_branch(repo_root)
        .ok()
        .and_then(|head| git::branch_is_merged_into(repo_root, branch, &head).ok())
        .unwrap_or(false);
    if safe { "-d" } else { "-D" }
}

/// Decides the fate of a branch git-paw created, once its worktree is gone.
///
/// `git branch -D` is the only irreversible operation in the teardown: it drops
/// the branch reflog with the ref, and the worktree whose `HEAD` reflog was the
/// other recovery path has just been removed. So the deletion is gated on the
/// read-only check in [`branch_merge_state`] — the branch is deleted only when
/// its tip is an ancestor of the default branch or of the branch it was created
/// from.
///
/// The gate fails **closed**: a branch carrying commits in neither, a branch
/// whose comparison errors, and a branch in a repository whose default branch
/// cannot be resolved are all retained. Every outcome is printed with the SHA
/// that matters — `Deleted branch <name> (was <sha>)` on deletion, and on
/// retention the tip plus how to resume (`git worktree add`) or discard
/// (`git branch -d` / `-D`) the branch by hand.
fn retire_created_branch(repo_root: &Path, entry: &WorktreeEntry, stderr: &mut dyn std::io::Write) {
    let tip =
        git::branch_tip_short(repo_root, &entry.branch).unwrap_or_else(|_| "unknown".to_string());

    let default_branch = git::default_branch(repo_root).ok();
    let retention_reason = match branch_merge_state(repo_root, entry, default_branch.as_deref()) {
        MergeState::Merged => None,
        MergeState::Unmerged {
            count: Some(count),
            compared_against,
        } => Some(format!("{count} commit(s) not in {compared_against}")),
        MergeState::Unmerged {
            count: None,
            compared_against,
        } => Some(format!(
            "commits not in {compared_against}; count unavailable"
        )),
        MergeState::Undeterminable(why) => {
            Some(format!("merge state could not be determined: {why}"))
        }
    };

    if let Some(reason) = retention_reason {
        let _ = writeln!(stderr, "Kept branch {} at {tip} ({reason})", entry.branch);
        let _ = writeln!(
            stderr,
            "  resume it with: git worktree add '{}' {}",
            entry.worktree_path.display(),
            entry.branch
        );
        let _ = writeln!(
            stderr,
            "  discard it with: git branch {} {}",
            discard_flag(repo_root, &entry.branch),
            entry.branch
        );
        let _ = stderr.flush();
        return;
    }

    match git::delete_branch(repo_root, &entry.branch) {
        Ok(()) => {
            let _ = writeln!(stderr, "Deleted branch {} (was {tip})", entry.branch);
        }
        Err(e) => {
            let _ = writeln!(
                stderr,
                "warning: failed to delete branch '{}': {e}",
                entry.branch
            );
        }
    }
    let _ = stderr.flush();
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
    // The single default-branch resolver (`git-operations`), not a purge-local
    // lookup: a purge-local fallback to a literal `main` made every comparison
    // fail in a `master`-default repository without `origin/HEAD`, and the
    // branches were deleted without a warning. When the resolver itself fails,
    // nothing is comparable, so every branch is reported as undeterminable and
    // `retire_created_branch` keeps all of them.
    let default_branch = git::default_branch(repo_root);
    if let Err(ref e) = default_branch {
        let _ = writeln!(
            stderr,
            "Warning: the default branch could not be resolved ({e}) — no branch will be deleted."
        );
    }
    let unmerged = collect_unmerged_branches(repo_root, session, default_branch.as_deref().ok());

    if !unmerged.is_empty() {
        let _ = writeln!(
            stderr,
            "Warning: {} branch(es) have unmerged commits and will be kept:",
            unmerged.len()
        );
        for retained in &unmerged {
            match (retained.unmerged_count, &retained.compared_against) {
                (Some(count), Some(target)) => {
                    let _ = writeln!(
                        stderr,
                        "  {} ({}): {count} commit(s) not in {target}",
                        retained.branch, retained.tip
                    );
                }
                (None, Some(target)) => {
                    let _ = writeln!(
                        stderr,
                        "  {} ({}): commits not in {target}; count unavailable",
                        retained.branch, retained.tip
                    );
                }
                (_, None) => {
                    let _ = writeln!(
                        stderr,
                        "  {} ({}): merge state could not be determined",
                        retained.branch, retained.tip
                    );
                }
            }
        }
        let _ = writeln!(
            stderr,
            "Their worktrees (including any uncommitted changes) and the session state \
             will be removed; the branches themselves are kept."
        );
        // Flush the warning block so it lands on the terminal before the
        // dialoguer prompt reads from stdin. Without this flush the
        // warning can race against the prompt — on some TTYs the prompt's
        // stdin read fires while the stderr buffer is still draining,
        // dialoguer sees an unexpected I/O state, and the `y` + Enter
        // never reaches `Confirm::interact`. Bug C in v0-5-0-audit-cleanup.
        let _ = stderr.flush();
    }

    if !force {
        let prompt_text = if unmerged.is_empty() {
            "This will remove the tmux session, all worktrees, and session state. Continue?"
        } else {
            "Worktrees (including any uncommitted changes) and session state will be removed; \
             the branches listed above are kept. Continue?"
        };
        if !confirm(prompt_text)? {
            return Ok(PurgeOutcome::Cancelled);
        }
    }

    kill_tmux(&session.session_name)?;

    // Per-worktree teardown (`on_remove` hook + worktree-remove + branch
    // cleanup), delegated to the shared `detach_worktree` helper so `cmd_remove`
    // performs the exact same per-worktree removal `purge` does (design D6, task
    // 1.3). The helper emits the per-worktree begin / `...done (Xs)` progress
    // markers Bug D in v0-5-0-audit-cleanup added. The hook is resolved once
    // rather than per entry — purge tears down every agent, and re-reading the
    // config in the loop would buy nothing. A config that cannot be read leaves
    // it unset, so an unreadable config never blocks a purge.
    let worktree_runtime = config::load_config(repo_root, None)
        .unwrap_or_default()
        .worktree_runtime();
    let on_remove = worktree_runtime.on_remove_hook();
    for entry in &session.worktrees {
        detach_worktree(repo_root, entry, on_remove, stderr);
    }

    if let Some(ref log_path) = session.broker_log_path {
        let _ = std::fs::remove_file(log_path);
    }

    // Bug E (v0-5-0-audit-cleanup §9c) — strip the supervisor-pane boot
    // block from AGENTS.md so it does not accumulate across sessions.
    // Idempotent: missing block / missing AGENTS.md is a no-op.
    if let Err(e) = agents::remove_session_boot_block(repo_root) {
        let _ = writeln!(
            stderr,
            "warning: failed to clean session boot block from AGENTS.md: {e}"
        );
    }

    // Deleting the session file releases every worktree's runtime port block
    // along with its entry — the slots live on the entries and
    // `worktree_provision::allocate_slot` reads the live roster, so the next
    // session starts from slot 0 again.
    session::delete_session_in(&session.session_name, sessions_dir)?;

    // Remove the per-repo discovery file sweep.sh reads (capability
    // `session-json-location`). Idempotent — a missing file is not an error.
    if let Err(e) = session::remove_repo_session_file(repo_root, &session.session_name) {
        let _ = writeln!(
            stderr,
            "warning: failed to remove per-repo session discovery file: {e}"
        );
    }

    Ok(PurgeOutcome::Purged)
}

/// A session branch that purge will report as unmerged, and therefore keep.
struct RetainedBranch {
    /// The branch name as recorded in the session file.
    branch: String,
    /// Short SHA of the branch tip, or `"unknown"` if it could not be read.
    tip: String,
    /// The branch the count is against — the recorded base when the entry has
    /// one, else the default branch. `None` when no comparison could be made.
    compared_against: Option<String>,
    /// `Some(n)` — `n` commits are not in `compared_against`; `None` — the
    /// count could not be taken.
    unmerged_count: Option<usize>,
}

/// The session's branches that are contained in neither the default branch nor
/// the branch they were created from.
///
/// `default_branch` is `None` when the resolver itself failed; an entry with no
/// recorded base is then uncomparable and is reported as such. A comparison
/// that errors is likewise reported rather than skipped: the old
/// `continue`-on-failure silently classified an uncomparable branch as merged,
/// which is the direction that loses work.
fn collect_unmerged_branches(
    repo_root: &Path,
    session: &Session,
    default_branch: Option<&str>,
) -> Vec<RetainedBranch> {
    let mut out = Vec::new();
    for entry in &session.worktrees {
        if Some(entry.branch.as_str()) == default_branch {
            continue;
        }
        let (compared_against, unmerged_count) =
            match branch_merge_state(repo_root, entry, default_branch) {
                MergeState::Merged => continue,
                MergeState::Unmerged {
                    count,
                    compared_against,
                } => (Some(compared_against), count),
                MergeState::Undeterminable(_) => (None, None),
            };
        let tip = git::branch_tip_short(repo_root, &entry.branch)
            .unwrap_or_else(|_| "unknown".to_string());
        out.push(RetainedBranch {
            branch: entry.branch.clone(),
            tip,
            compared_against,
            unmerged_count,
        });
    }
    out
}

// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::supervisor::{
        GIT_PAW_ISSUES_URL, SupervisorPrompt, learnings_disclosure_notice, resolve_supervisor_mode,
    };
    use git_paw::config::{PawConfig, SupervisorConfig};
    use git_paw::session::SessionStatus;
    use serial_test::serial;
    use std::path::PathBuf;
    use std::process::Command as StdCommand;
    use std::time::UNIX_EPOCH;
    use tempfile::TempDir;

    /// `agent_cli_command` appends only the parts that carry text, so a CLI
    /// with no flags and no session-id args is invoked bare, and a resolved
    /// but empty `session_id_args` template adds no trailing space.
    #[test]
    fn agent_cli_command_appends_only_non_empty_parts() {
        assert_eq!(agent_cli_command("claude", "", None), "claude");
        assert_eq!(
            agent_cli_command("claude", "--dangerous", None),
            "claude --dangerous"
        );
        assert_eq!(
            agent_cli_command("claude", "", Some("--session-id abc")),
            "claude --session-id abc",
            "session-id args attach even when the CLI has no approval flags"
        );
        assert_eq!(
            agent_cli_command("claude", "--dangerous", Some("--session-id abc")),
            "claude --dangerous --session-id abc",
            "flags precede the session-id args"
        );
        // A `session_id_args` template that resolves to the empty string
        // (`session_id_args = ""`) must not leave a dangling separator.
        assert_eq!(agent_cli_command("claude", "", Some("")), "claude");
        assert_eq!(
            agent_cli_command("claude", "--dangerous", Some("")),
            "claude --dangerous"
        );
    }

    /// A session receipt whose worktree entries carry the given
    /// `(branch, spec_id)` pairs — the shape a spec-driven launch saves.
    fn receipt_with(entries: &[(&str, Option<&str>)]) -> Session {
        Session {
            session_name: "paw-repo".to_string(),
            repo_path: PathBuf::from("/repo"),
            project_name: "repo".to_string(),
            created_at: UNIX_EPOCH,
            status: SessionStatus::Active,
            worktrees: entries
                .iter()
                .map(|(branch, spec_id)| WorktreeEntry {
                    branch: (*branch).to_string(),
                    worktree_path: PathBuf::from(format!("/repo/wt/{branch}")),
                    cli: "claude".to_string(),
                    branch_created: false,
                    base_branch: None,
                    pending_boot_prompt: None,
                    runtime_slot: None,
                    cli_session_id: None,
                    spec_id: spec_id.map(str::to_string),
                })
                .collect(),
            broker_port: None,
            broker_bind: None,
            broker_log_path: None,
            mode: git_paw::session::SessionMode::Bare,
            dashboard_pane: None,
        }
    }

    /// A `[conflict-detector]`-tagged forward-conflict feedback message
    /// naming `from` and `other`, as the detector emits it.
    fn forward_conflict(from: &str, other: &str) -> git_paw::broker::messages::BrokerMessage {
        git_paw::broker::messages::BrokerMessage::Feedback {
            agent_id: from.to_string(),
            payload: git_paw::broker::messages::FeedbackPayload {
                from: "conflict-detector".to_string(),
                errors: vec![format!(
                    "[conflict-detector] forward conflict with {other} on src/main.rs"
                )],
            },
        }
    }

    /// `supervisor-learnings`' "The spec mapping is seeded from the persisted
    /// session": attaching the aggregator for a session SHALL be the only call
    /// needed for it to learn the mapping.
    ///
    /// This asserts the *wiring*, not the helper. `seed_from_session` has its
    /// own unit tests, but those would stay green if `cmd_dashboard` never
    /// called it — which is exactly the class of defect this change fixes.
    #[test]
    fn dashboard_aggregator_is_seeded_from_the_session_receipt() {
        let tmp = TempDir::new().expect("tempdir");
        let receipt = receipt_with(&[
            ("feat/x", Some("003-user-list")),
            ("feat/y", Some("003-user-list")),
        ]);
        let mut aggregator = build_learnings_aggregator(
            tmp.path().join("session-learnings.md"),
            &SupervisorConfig::default(),
            Some(&receipt),
            false,
        );

        aggregator.record_detector_message(&forward_conflict("feat-x", "feat-y"));
        aggregator.record_detector_message(&forward_conflict("feat-y", "feat-x"));
        aggregator.flush().expect("flush learnings");

        let md = std::fs::read_to_string(aggregator.file_path()).expect("learnings file written");
        assert!(
            md.contains("forward-conflict-intra-spec"),
            "the dashboard must seed the agent -> spec mapping from the receipt, so a \
             same-spec forward conflict classifies intra-spec; got:\n{md}"
        );
        assert!(
            md.contains("003-user-list"),
            "the flushed bullet must name the spec both agents were launched from; got:\n{md}"
        );
    }

    /// No receipt means nothing to seed: the aggregator is still built and
    /// still classifies, falling back to cross-spec as it did before.
    #[test]
    fn dashboard_aggregator_without_a_receipt_falls_back_to_cross_spec() {
        let tmp = TempDir::new().expect("tempdir");
        let mut aggregator = build_learnings_aggregator(
            tmp.path().join("session-learnings.md"),
            &SupervisorConfig::default(),
            None,
            false,
        );

        aggregator.record_detector_message(&forward_conflict("feat-x", "feat-y"));
        aggregator.record_detector_message(&forward_conflict("feat-y", "feat-x"));
        aggregator.flush().expect("flush learnings");

        let md = std::fs::read_to_string(aggregator.file_path()).expect("learnings file written");
        assert!(
            !md.contains("forward-conflict-intra-spec"),
            "with no receipt there is no mapping, so nothing may classify intra-spec; got:\n{md}"
        );
    }

    // Byte-identical safety net (D1 / task 1.3): each per-backend boot-prompt
    // framing sentence, relocated from a compiled match arm into its own
    // asset, must render exactly as before.
    #[test]
    fn build_task_prompt_per_backend_is_byte_identical_to_ground_truth() {
        use git_paw::specs::{SpecBackendKind, SpecEntry};
        let mk = |backend: SpecBackendKind| SpecEntry {
            id: "my-change".to_string(),
            backend,
            branch: "feat/my-change".to_string(),
            cli: None,
            prompt: String::new(),
            owned_files: None,
        };
        assert_eq!(
            build_task_prompt(Some(&mk(SpecBackendKind::OpenSpec))),
            "Read .git-paw/AGENTS.local.md first — it carries your assignment and \
             coordination rules — then run /opsx:apply my-change."
        );
        assert_eq!(
            build_task_prompt(Some(&mk(SpecBackendKind::Markdown))),
            "Begin your assigned task. Read .git-paw/AGENTS.local.md first — it carries the \
             project rules, your full spec, and your assignment. Additional artifacts \
             (proposal, design, specs, tasks) live under openspec/changes/my-change/ — read \
             them all before starting."
        );
        assert_eq!(
            build_task_prompt(Some(&mk(SpecBackendKind::SpecKit))),
            "Begin your assigned task. Read .git-paw/AGENTS.local.md first — it already \
             carries the project rules and your full Spec Kit assignment (feature \
             spec, implementation plan, and task phase) embedded inline; no other \
             artifact directory is needed to start."
        );
        assert_eq!(
            build_task_prompt(Some(&mk(SpecBackendKind::Superpowers))),
            "Begin your assigned task. Read .git-paw/AGENTS.local.md first — it carries the \
             project rules and your full superpowers plan (goal, tasks, exact file \
             paths, and per-step verification commands). Work the steps in order and \
             flip `- [ ]` to `- [x]` in the plan as each one lands."
        );
    }

    // -----------------------------------------------------------------------
    // resolve_dispatch_target — pure routing for the `start` subcommand
    // -----------------------------------------------------------------------

    #[test]
    fn dispatch_from_all_specs_with_supervisor_routes_to_supervisor_with_all() {
        let target = resolve_dispatch_target(&SpecMode::All, true);
        assert_eq!(
            target,
            DispatchTarget::Supervisor {
                spec_mode: SpecMode::All
            }
        );
    }

    #[test]
    fn dispatch_from_all_specs_without_supervisor_routes_to_start_with_specs() {
        let target = resolve_dispatch_target(&SpecMode::All, false);
        assert_eq!(target, DispatchTarget::StartWithSpecs(SpecMode::All));
    }

    #[test]
    fn dispatch_supervisor_without_specs_routes_to_supervisor_with_none() {
        // Bare `--supervisor` (no spec flag) carries `SpecMode::None`, which
        // cmd_supervisor resolves to the `--branches` / branch-picker flow —
        // NOT spec auto-discovery.
        let target = resolve_dispatch_target(&SpecMode::None, true);
        assert_eq!(
            target,
            DispatchTarget::Supervisor {
                spec_mode: SpecMode::None
            }
        );
    }

    #[test]
    fn dispatch_neither_flag_routes_to_start() {
        let target = resolve_dispatch_target(&SpecMode::None, false);
        assert_eq!(target, DispatchTarget::Start);
    }

    #[test]
    fn dispatch_picker_routes_to_start_with_specs_picker() {
        let target = resolve_dispatch_target(&SpecMode::Picker, false);
        assert_eq!(target, DispatchTarget::StartWithSpecs(SpecMode::Picker));
    }

    #[test]
    fn dispatch_narrow_routes_to_start_with_specs_narrow() {
        let names = vec!["add-auth".to_string()];
        let target = resolve_dispatch_target(&SpecMode::Narrow(names.clone()), false);
        assert_eq!(
            target,
            DispatchTarget::StartWithSpecs(SpecMode::Narrow(names))
        );
    }

    // Task 1.4 — `--supervisor --specs a,b` routes to the supervisor path
    // carrying the named subset, NOT `use_specs`-collapses-to-all.
    #[test]
    fn dispatch_narrow_with_supervisor_routes_to_supervisor_with_named_subset() {
        let names = vec!["a".to_string(), "b".to_string()];
        let target = resolve_dispatch_target(&SpecMode::Narrow(names.clone()), true);
        assert_eq!(
            target,
            DispatchTarget::Supervisor {
                spec_mode: SpecMode::Narrow(names)
            }
        );
    }

    // Task 1.5 — `--supervisor --from-all-specs` routes to the supervisor path
    // carrying `All` (every discovered spec).
    #[test]
    fn dispatch_picker_with_supervisor_routes_to_supervisor_with_picker() {
        let target = resolve_dispatch_target(&SpecMode::Picker, true);
        assert_eq!(
            target,
            DispatchTarget::Supervisor {
                spec_mode: SpecMode::Picker
            }
        );
    }

    // Task 2.3 — `--unattended --branches feat/a,feat/b` routes to the
    // supervisor launch path. `--unattended` engages supervisor mode via the
    // `supervisor || unattended` coupling `run()` applies before resolving the
    // dispatch target, so the wave routes to cmd_supervisor even with
    // `--supervisor` absent and `[supervisor]` config disabled.
    #[test]
    fn dispatch_unattended_routes_to_supervisor_launch_path() {
        let cfg = cfg_with_supervisor(false);
        let mut prompt = StubPrompt {
            answer: false,
            called: false,
        };
        let (supervisor_flag, unattended) = (false, true);
        // `run()` passes `supervisor || unattended` as the supervisor flag.
        let supervisor_enabled = resolve_supervisor_mode(
            false,
            supervisor_flag || unattended,
            false,
            &cfg,
            &mut prompt,
        )
        .unwrap();
        assert!(
            supervisor_enabled,
            "--unattended resolves supervisor mode active"
        );
        assert!(!prompt.called, "the flag short-circuits before any prompt");
        // With `--branches` the spec mode is None; the target is the supervisor
        // launch path carrying that mode.
        let target = resolve_dispatch_target(&SpecMode::None, supervisor_enabled);
        assert_eq!(
            target,
            DispatchTarget::Supervisor {
                spec_mode: SpecMode::None
            }
        );
    }

    /// GP-03b: a pane stuck on an unanswered dialog fails the launch loudly
    /// (`Err`) instead of proceeding to inject the boot block; every other
    /// outcome proceeds exactly as before.
    #[test]
    fn dialog_stuck_result_fails_loudly_only_for_dialog_stuck() {
        let err = dialog_stuck_result(
            git_paw::tmux::ReadinessOutcome::DialogStuck,
            "paw-proj",
            2,
            "claude --dangerously-skip-permissions",
        )
        .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("pane 2"));
        assert!(message.contains("acceptance dialog"));
        assert!(message.contains("paw-proj"));

        assert!(
            dialog_stuck_result(
                git_paw::tmux::ReadinessOutcome::Ready,
                "paw-proj",
                2,
                "claude"
            )
            .is_ok()
        );
        assert!(
            dialog_stuck_result(
                git_paw::tmux::ReadinessOutcome::FellBack,
                "paw-proj",
                2,
                "claude"
            )
            .is_ok()
        );
    }

    #[test]
    fn spec_mode_from_flags_translates_each_combination() {
        assert_eq!(SpecMode::from_flags(true, None), SpecMode::All);
        assert_eq!(SpecMode::from_flags(false, None), SpecMode::None);
        let empty: Vec<String> = Vec::new();
        assert_eq!(SpecMode::from_flags(false, Some(&empty)), SpecMode::Picker);
        let names = vec!["add-auth".to_string(), "fix-session".to_string()];
        assert_eq!(
            SpecMode::from_flags(false, Some(&names)),
            SpecMode::Narrow(names)
        );
    }

    // -----------------------------------------------------------------------
    // build_task_prompt — pure helper that constructs the per-agent task
    // prompt appended to the supervisor-mode boot block. Replaces the old
    // first-line-of-spec-body truncation (MILESTONE drift item 29).
    // -----------------------------------------------------------------------

    fn make_spec_entry(id: &str, prompt_body: &str) -> git_paw::specs::SpecEntry {
        // Default to the `Markdown` backend so the regression tests
        // (`task_prompt_with_spec_points_at_sidecar_and_includes_id`,
        // `task_prompt_does_not_include_spec_body_first_line`) keep exercising
        // the sidecar-pointer branch of `build_task_prompt`.
        // Tests that want the OpenSpec branch construct a `SpecEntry`
        // literal directly with `backend = SpecBackendKind::OpenSpec`.
        git_paw::specs::SpecEntry {
            id: id.to_string(),
            backend: git_paw::specs::SpecBackendKind::Markdown,
            branch: format!("feat/{id}"),
            cli: None,
            prompt: prompt_body.to_string(),
            owned_files: None,
        }
    }

    #[test]
    fn task_prompt_with_spec_points_at_sidecar_and_includes_id() {
        let entry = make_spec_entry("my-change", "## 1. First section\n\nbody body body");
        let prompt = build_task_prompt(Some(&entry));
        assert!(
            prompt.contains(git_paw::agents::SIDECAR_REL_PATH),
            "spec-derived task prompt should point at the sidecar, got: {prompt}"
        );
        assert!(
            prompt.contains("openspec/changes/my-change"),
            "spec-derived task prompt should include the spec id directory, got: {prompt}"
        );
    }

    #[test]
    fn task_prompt_without_spec_points_at_sidecar() {
        let prompt = build_task_prompt(None);
        assert_eq!(
            prompt,
            "Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task.",
            "no-spec task prompt should point at the sidecar verbatim"
        );
    }

    #[test]
    fn task_prompt_openspec_backend_points_at_sidecar_then_invokes_opsx_apply() {
        // OVERRIDE (agents-md-sidecar-injection): pre-sidecar this branch was
        // the bare `/opsx:apply <id>` with deliberately NO pointer, because the
        // combined view was auto-loaded from the worktree-root AGENTS.md. The
        // managed block now lives in a gitignored sidecar the CLIs do not
        // auto-load, so the prompt MUST point the agent at the sidecar first,
        // then run the slash command. This intentionally reverses the earlier
        // "no AGENTS.md pointer" decision.
        let entry = git_paw::specs::SpecEntry {
            id: "my-change".to_string(),
            backend: git_paw::specs::SpecBackendKind::OpenSpec,
            branch: "feat/my-change".to_string(),
            cli: None,
            prompt: String::new(),
            owned_files: None,
        };
        let prompt = build_task_prompt(Some(&entry));
        assert!(
            prompt.contains(git_paw::agents::SIDECAR_REL_PATH),
            "OpenSpec branch must point the agent at the sidecar, got: {prompt}"
        );
        assert!(
            prompt.contains("/opsx:apply my-change"),
            "OpenSpec branch must still invoke the opsx:apply slash command, got: {prompt}"
        );
        assert!(
            !prompt.contains("openspec/changes/"),
            "OpenSpec branch must suppress the openspec/changes/ path prose, got: {prompt}"
        );
    }

    #[test]
    fn task_prompt_markdown_backend_uses_sidecar_pointer() {
        let entry = git_paw::specs::SpecEntry {
            id: "my-feature".to_string(),
            backend: git_paw::specs::SpecBackendKind::Markdown,
            branch: "feat/my-feature".to_string(),
            cli: None,
            prompt: String::new(),
            owned_files: None,
        };
        let prompt = build_task_prompt(Some(&entry));
        assert!(
            prompt.contains(git_paw::agents::SIDECAR_REL_PATH),
            "Markdown-backed task prompt should point at the sidecar, got: {prompt}"
        );
        assert!(
            prompt.contains("openspec/changes/my-feature"),
            "Markdown-backed task prompt should include the spec id directory, got: {prompt}"
        );
        assert!(
            !prompt.contains("/opsx:apply"),
            "Markdown branch must NOT invoke the OpenSpec slash command, got: {prompt}"
        );
    }

    #[test]
    fn task_prompt_without_spec_points_at_sidecar_verbatim() {
        // Regression: the `None` branch must produce the exact sidecar-pointer
        // fallback string byte-for-byte, even after the per-backend dispatch.
        let prompt = build_task_prompt(None);
        assert_eq!(
            prompt,
            "Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task.",
            "no-spec task prompt should be the sidecar-pointer fallback verbatim"
        );
    }

    #[test]
    fn task_prompt_does_not_include_spec_body_first_line() {
        // Regression test for the original bug: the task prompt used to be
        // `spec_entry.prompt.lines().next()`, which produced exactly this
        // truncated heading. The fix replaces it with an AGENTS.md pointer
        // that does NOT contain the heading.
        let entry = make_spec_entry(
            "prompt-submit-fix",
            "## 1. Code fix in cmd_supervisor\n\n- [ ] 1.1 Add a constant.\n",
        );
        let prompt = build_task_prompt(Some(&entry));
        assert!(
            !prompt.contains("## 1. Code fix in cmd_supervisor"),
            "task prompt MUST NOT include the spec body's first heading in raw form, got: {prompt}"
        );
        // The body should not leak in either.
        assert!(
            !prompt.contains("- [ ] 1.1"),
            "task prompt MUST NOT include spec body content, got: {prompt}"
        );
    }

    /// Standing guard (spec-traceability-audit C3): every `build_task_prompt`
    /// arm — including `Superpowers`, which lacked a dedicated test — points the
    /// agent at the gitignored sidecar, since the CLIs no longer auto-load it.
    #[test]
    fn every_build_task_prompt_arm_points_at_the_sidecar() {
        use git_paw::agents::SIDECAR_REL_PATH;
        use git_paw::specs::{SpecBackendKind, SpecEntry};
        for backend in [
            SpecBackendKind::OpenSpec,
            SpecBackendKind::Markdown,
            SpecBackendKind::SpecKit,
            SpecBackendKind::Superpowers,
        ] {
            let entry = SpecEntry {
                id: "x".to_string(),
                backend,
                branch: "feat/x".to_string(),
                cli: None,
                prompt: String::new(),
                owned_files: None,
            };
            let prompt = build_task_prompt(Some(&entry));
            assert!(
                prompt.contains(SIDECAR_REL_PATH),
                "spec-backed arm must point at the sidecar, got: {prompt}"
            );
        }
        assert!(
            build_task_prompt(None).contains(SIDECAR_REL_PATH),
            "no-spec arm must point at the sidecar"
        );
    }

    // Maps to scenario "Spec-derived task prompt points at the sidecar and
    // includes spec id" — fixes the named-test coverage gap from
    // boot-prompt-full-body. (test-coverage-v0-5-0 task 2.1)
    #[test]
    fn build_task_prompt_spec_entry_contains_sidecar_and_spec_id() {
        let entry = make_spec_entry("governance-config", "## 1. Struct definitions\n\nBody.");
        let prompt = build_task_prompt(Some(&entry));
        assert!(
            prompt.contains(git_paw::agents::SIDECAR_REL_PATH),
            "spec-derived prompt should point at the sidecar, got: {prompt}"
        );
        assert!(
            prompt.contains("openspec/changes/governance-config"),
            "spec-derived prompt should embed the spec id directory, got: {prompt}"
        );
        assert!(
            !prompt.contains("## 1. Struct definitions"),
            "spec body's first heading MUST NOT leak into the prompt, got: {prompt}"
        );
    }

    // Maps to scenario "Spec Kit doctrine names the Spec Kit spec location"
    // from speckit-spec-path-pointer: the SpecKit arm of `build_task_prompt`
    // must not equate SpecKit with the OpenSpec artifact layout the way the
    // Markdown arm does.
    #[test]
    fn build_task_prompt_speckit_does_not_reference_openspec_changes_path() {
        use git_paw::specs::{SpecBackendKind, SpecEntry};
        let entry = SpecEntry {
            id: "user-auth-T009".to_string(),
            backend: SpecBackendKind::SpecKit,
            branch: "task/x".to_string(),
            cli: None,
            prompt: String::new(),
            owned_files: None,
        };
        let prompt = build_task_prompt(Some(&entry));
        assert!(
            prompt.contains(git_paw::agents::SIDECAR_REL_PATH),
            "Spec Kit task prompt should still point at the sidecar, got: {prompt}"
        );
        assert!(
            !prompt.contains("openspec/changes/"),
            "Spec Kit task prompt must not direct the worker to the nonexistent \
             openspec/changes/ path, got: {prompt}"
        );
    }

    // Maps to scenario `Supervisor pane receives a boot block` from
    // supervisor-as-pane. The pane-0 prompt is constructed inline in
    // `cmd_supervisor` (it is not extracted into a helper); we reconstruct
    // it using the same public helpers production uses and lock in the
    // resulting prefix + framing. (test-coverage-v0-5-0 task 12.10)
    #[test]
    fn supervisor_pane_prompt_starts_with_boot_block() {
        let broker_url = "http://127.0.0.1:9119";
        // The supervisor's OWN entry point, which is what `cmd_supervisor`
        // calls. `build_boot_block` is the coding-agent one and slugifies its
        // argument, so passing "supervisor" there would (correctly) yield
        // `agent-supervisor` — a coding agent must never be handed the
        // reserved id.
        let boot_block = git_paw::skills::build_supervisor_boot_block(broker_url);
        let supervisor_framing = "Begin observing the spec implementation session. Your skill (.git-paw/AGENTS.local.md) describes \
             your role — read it, then start the autonomous loop. The user can type questions or \
             directives directly into your pane; handle them per the 'When the user types in your \
             pane' section of your skill.";
        let supervisor_prompt = format!("{boot_block}\n\n{supervisor_framing}");

        assert!(
            supervisor_prompt.starts_with(&boot_block),
            "supervisor pane prompt should begin with the boot block; got:\n{supervisor_prompt}"
        );
        // The boot-block template uses `{{BRANCH_ID}}` as a placeholder that
        // `build_boot_block` substitutes to the slugified agent id. For the
        // supervisor pane the substituted value is the literal string
        // `supervisor`, which appears in every bundled-helper call as
        // `broker.sh --agent supervisor ...` (the boot block calls the helper,
        // never a raw broker curl). That is the production shape of
        // "BRANCH_ID=supervisor" from the spec scenario.
        assert!(
            boot_block.contains(".git-paw/scripts/broker.sh --agent supervisor status booting"),
            "boot block must carry the supervisor agent_id substitution; got:\n{boot_block}"
        );
        let framing_idx = supervisor_prompt
            .find("Begin observing")
            .expect("`Begin observing` framing must follow the boot block");
        assert!(
            framing_idx > boot_block.len(),
            "framing must follow the boot block, not precede it; framing_idx={framing_idx}"
        );
    }

    // Maps to scenario `Boot block withheld until the pane is CLI-ready` from
    // session-orchestration-robustness (G1). The blind pre-`send-keys`
    // fixed sleep was replaced by the launch-readiness gate: `cmd_supervisor`
    // MUST gate every prompt-bearing pane via `tmux::gate_pane_for_injection`
    // before injecting its boot block, rather than relying on a wall-clock
    // sleep that the v0.8.0 dogfood proved racy.
    #[test]
    fn supervisor_launch_gates_injection_on_readiness() {
        let src = include_str!("commands/supervisor.rs");
        let cmd_start = src
            .find("fn cmd_supervisor(")
            .expect("cmd_supervisor signature present");
        let body_start = src[cmd_start..]
            .find('{')
            .map(|o| cmd_start + o)
            .expect("opening brace");
        let mut depth: i32 = 0;
        let mut end = body_start;
        for (i, ch) in src[body_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = body_start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let body = &src[body_start..end];

        let gate_idx = body.find("gate_pane_for_injection").expect(
            "cmd_supervisor must gate boot-block injection on CLI readiness \
             (tmux::gate_pane_for_injection) instead of a blind fixed sleep",
        );
        let submit_idx = body
            .find("submit_prompt_to_pane")
            .expect("cmd_supervisor must still inject the boot block via submit_prompt_to_pane");
        assert!(
            gate_idx < submit_idx,
            "the readiness gate must run BEFORE the first boot-block injection"
        );
        assert!(
            !body.contains("from_secs(2)"),
            "the blind 2s pre-injection sleep must be gone (replaced by the readiness gate)"
        );
    }

    // Maps to scenario "build_task_prompt is a pure function" — pairs a
    // determinism assertion with a static-source check that the function body
    // contains no filesystem / process IO calls. (test-coverage-v0-5-0 task 2.2)
    #[test]
    fn build_task_prompt_is_deterministic_and_io_free() {
        let entry = make_spec_entry("governance-config", "## body\n\nmore body");
        let a = build_task_prompt(Some(&entry));
        let b = build_task_prompt(Some(&entry));
        assert_eq!(a.as_bytes(), b.as_bytes(), "must be deterministic");

        let src = include_str!("main.rs");
        let needle = "pub(crate) fn build_task_prompt";
        let start = src
            .find(needle)
            .unwrap_or_else(|| panic!("build_task_prompt signature not found in main.rs"));
        let body_start = src[start..].find('{').map_or_else(
            || panic!("opening brace not found after signature"),
            |o| start + o,
        );
        // Walk braces to find the matching closing brace.
        let mut depth: i32 = 0;
        let mut end = body_start;
        for (i, ch) in src[body_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = body_start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        assert!(end > body_start, "did not find closing brace");
        let body = &src[body_start..end];

        for needle in [
            "std::fs::",
            "File::open",
            "File::create",
            "Command::new",
            "tokio::fs::",
        ] {
            assert!(
                !body.contains(needle),
                "build_task_prompt body must not contain `{needle}`; body:\n{body}"
            );
        }
    }

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
        let result = resolve_supervisor_mode(false, true, false, &cfg, &mut prompt).unwrap();
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
        let result = resolve_supervisor_mode(false, false, false, &cfg, &mut prompt).unwrap();
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
        let result = resolve_supervisor_mode(false, false, false, &cfg, &mut prompt).unwrap();
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
        let result = resolve_supervisor_mode(false, false, true, &cfg, &mut prompt).unwrap();
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
        let result = resolve_supervisor_mode(false, false, false, &cfg, &mut prompt).unwrap();
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
        let result = resolve_supervisor_mode(false, true, true, &cfg, &mut prompt).unwrap();
        assert!(result);
    }

    // --- resolve_supervisor_mode: --no-supervisor short-circuit (Step 0) ---

    #[test]
    fn resolve_no_supervisor_overrides_config_enabled() {
        let cfg = cfg_with_supervisor(true);
        let mut prompt = StubPrompt {
            answer: true,
            called: false,
        };
        // no_supervisor_flag = true, supervisor_flag = false (mutex enforced by clap)
        let result = resolve_supervisor_mode(true, false, false, &cfg, &mut prompt).unwrap();
        assert!(
            !result,
            "--no-supervisor must override config enabled = true"
        );
        assert!(!prompt.called);
    }

    #[test]
    fn resolve_no_supervisor_with_config_disabled_is_idempotent() {
        let cfg = cfg_with_supervisor(false);
        let mut prompt = StubPrompt {
            answer: true,
            called: false,
        };
        let result = resolve_supervisor_mode(true, false, false, &cfg, &mut prompt).unwrap();
        assert!(!result);
        assert!(!prompt.called);
    }

    #[test]
    fn resolve_no_supervisor_with_no_section_skips_prompt() {
        // No [supervisor] section + --no-supervisor → off, no prompt regardless
        // of TTY (StubPrompt would have returned true if asked).
        let cfg = PawConfig::default();
        let mut prompt = StubPrompt {
            answer: true,
            called: false,
        };
        let result = resolve_supervisor_mode(true, false, false, &cfg, &mut prompt).unwrap();
        assert!(!result);
        assert!(
            !prompt.called,
            "--no-supervisor must short-circuit before the prompt"
        );
    }

    #[test]
    fn resolve_no_supervisor_with_dry_run_disables() {
        let cfg = cfg_with_supervisor(true);
        let mut prompt = StubPrompt {
            answer: true,
            called: false,
        };
        let result = resolve_supervisor_mode(true, false, true, &cfg, &mut prompt).unwrap();
        assert!(!result);
        assert!(!prompt.called);
    }

    // (merge-ordering tests previously here have been removed alongside
    // src/merge_loop.rs — supervisor-as-pane moves merge orchestration into
    // the supervisor skill as a v0.5.0 design change.)

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

    /// The tip SHA of `branch`, or `None` when the branch does not exist.
    fn branch_tip(repo: &Path, branch: &str) -> Option<String> {
        let out = StdCommand::new("git")
            .current_dir(repo)
            .args(["rev-parse", "--verify", "--quiet", branch])
            .output()
            .expect("git spawn");
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn setup_purge_sandbox(with_unmerged_commit: bool) -> PurgeSandbox {
        setup_purge_sandbox_on("main", with_unmerged_commit)
    }

    /// `setup_purge_sandbox` with an explicit default-branch name, so the
    /// `master`-default / no-`origin/HEAD` path can be exercised.
    fn setup_purge_sandbox_on(default_branch: &str, with_unmerged_commit: bool) -> PurgeSandbox {
        let sandbox = TempDir::new().expect("tempdir");
        let repo = sandbox.path().join("repo");
        std::fs::create_dir(&repo).expect("mkdir repo");
        let sessions_dir = sandbox.path().join("sessions");

        git(&repo, &["init", "-q", "-b", default_branch]);
        git(&repo, &["config", "user.email", "test@test.com"]);
        git(&repo, &["config", "user.name", "Test"]);
        std::fs::write(repo.join("README.md"), "# test").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "initial"]);

        let wt = git::create_worktree(
            &repo,
            "feature/test",
            false,
            git_paw::config::WorktreePlacement::Sibling,
        )
        .expect("create worktree");
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
                base_branch: created_branch_base(&repo, wt.branch_created),
                pending_boot_prompt: None,
                runtime_slot: None,
                cli_session_id: None,
                spec_id: None,
            }],
            broker_port: None,
            broker_bind: None,
            broker_log_path: None,
            mode: git_paw::session::SessionMode::Bare,
            dashboard_pane: None,
        };
        session::save_session_in(&session, &sessions_dir).expect("save session");

        PurgeSandbox {
            _sandbox: sandbox,
            repo,
            sessions_dir,
            session,
        }
    }

    /// A sandbox whose main checkout sits on an integration branch that is
    /// ahead of the default branch, with one agent worktree branched off it —
    /// the shape `git paw start` produces when the operator launches a wave
    /// from an integration branch rather than from `main`.
    ///
    /// When `merged_into_integration` the agent's commit is merged back into
    /// the integration branch before the session struct is built.
    fn setup_integration_sandbox(merged_into_integration: bool) -> PurgeSandbox {
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

        // The integration branch runs ahead of main by two commits — exactly
        // the backlog a default-branch-only comparison would miscount as the
        // agent's own work.
        git(&repo, &["checkout", "-q", "-b", "integration"]);
        for n in 1..=2 {
            std::fs::write(repo.join(format!("integration-{n}.txt")), "x").unwrap();
            git(&repo, &["add", "."]);
            git(&repo, &["commit", "-q", "-m", &format!("integration {n}")]);
        }

        let wt = git::create_worktree(
            &repo,
            "feature/test",
            false,
            git_paw::config::WorktreePlacement::Sibling,
        )
        .expect("create worktree");
        std::fs::write(wt.path.join("agent.txt"), "agent work").unwrap();
        git(&wt.path, &["add", "."]);
        git(&wt.path, &["commit", "-q", "-m", "agent work"]);

        if merged_into_integration {
            git(
                &repo,
                &["merge", "-q", "--no-ff", "-m", "merge", "feature/test"],
            );
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
                base_branch: created_branch_base(&repo, wt.branch_created),
                pending_boot_prompt: None,
                runtime_slot: None,
                cli_session_id: None,
                spec_id: None,
            }],
            broker_port: None,
            broker_bind: None,
            broker_log_path: None,
            mode: git_paw::session::SessionMode::Bare,
            dashboard_pane: None,
        };
        session::save_session_in(&session, &sessions_dir).expect("save session");

        PurgeSandbox {
            _sandbox: sandbox,
            repo,
            sessions_dir,
            session,
        }
    }

    // --- shared teardown: branch retention (`git-add-remove-branch`) ---

    /// Spec `git-add-remove-branch` — "A same-named tag never stands in for the
    /// branch". `rev-parse` tries `refs/tags/<name>` before `refs/heads/<name>`,
    /// so an unqualified comparison would answer on the tag's history: the tag
    /// here sits on merged history while the branch carries unmerged work.
    #[test]
    #[serial]
    fn detach_worktree_is_not_fooled_by_a_tag_sharing_the_branch_name() {
        let sb = setup_purge_sandbox(true);
        // A tag with the branch's exact name, pointing at fully merged history.
        git(&sb.repo, &["tag", "feature/test", "main"]);
        let entry = &sb.session.worktrees[0];
        let branch_tip_before = branch_tip(&sb.repo, "refs/heads/feature/test")
            .expect("branch ref resolves before teardown");
        let tag_tip = branch_tip(&sb.repo, "refs/tags/feature/test").expect("tag ref resolves");
        assert_ne!(
            branch_tip_before, tag_tip,
            "the fixture must put the tag on different history than the branch"
        );

        let mut stderr = Vec::<u8>::new();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = String::from_utf8(stderr).unwrap();
        assert_eq!(
            branch_tip(&sb.repo, "refs/heads/feature/test").as_deref(),
            Some(branch_tip_before.as_str()),
            "the unmerged branch must survive despite the merged same-named tag: {err_text:?}"
        );
        assert!(
            err_text.contains("Kept branch feature/test at")
                && err_text.contains("1 commit(s) not in"),
            "the comparison must run on refs/heads, not the tag: {err_text:?}"
        );
        assert!(
            err_text.contains(&branch_tip_before[..7]),
            "the reported tip must be the branch's, not the tag's: {err_text:?}"
        );
    }

    /// Spec `git-add-remove-branch` — "A branch merged into the branch it was
    /// created from is deleted". Launched from an integration branch, work that
    /// has landed in that branch is finished even though it is not yet in the
    /// default branch.
    #[test]
    #[serial]
    fn detach_worktree_deletes_a_branch_merged_into_its_recorded_base() {
        let sb = setup_integration_sandbox(true);
        let entry = &sb.session.worktrees[0];
        assert_eq!(
            entry.base_branch.as_deref(),
            Some("integration"),
            "the session must record the branch git-paw branched from"
        );

        let mut stderr = Vec::<u8>::new();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = String::from_utf8(stderr).unwrap();
        assert!(
            branch_tip(&sb.repo, "feature/test").is_none(),
            "a branch merged into its base should be deleted; stderr: {err_text:?}"
        );
        assert!(
            err_text.contains("Deleted branch feature/test (was "),
            "deletion must print the branch and its SHA: {err_text:?}"
        );
    }

    /// Spec `git-add-remove-branch` — "An unmerged branch is counted against
    /// the branch it was created from". The integration branch is two commits
    /// ahead of `main`; the agent added one. Counting against `main` would
    /// report three and describe the integration backlog as the agent's work.
    #[test]
    #[serial]
    fn detach_worktree_counts_an_unmerged_branch_against_its_recorded_base() {
        let sb = setup_integration_sandbox(false);
        let entry = &sb.session.worktrees[0];
        let tip_before = branch_tip(&sb.repo, "feature/test").expect("branch exists");

        let mut stderr = Vec::<u8>::new();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = String::from_utf8(stderr).unwrap();
        assert_eq!(
            branch_tip(&sb.repo, "feature/test").as_deref(),
            Some(tip_before.as_str()),
            "an unmerged branch must survive: {err_text:?}"
        );
        assert!(
            err_text.contains("1 commit(s) not in integration"),
            "the count must be against the recorded base, not the default branch: {err_text:?}"
        );
        assert!(
            !err_text.contains("3 commit(s)"),
            "the integration backlog must not be counted as the agent's work: {err_text:?}"
        );
    }

    /// Spec `git-add-remove-branch` — the retention hint quotes the worktree
    /// path (a path with a space would otherwise produce a broken command) and
    /// offers `git branch -d` when git would accept the safe delete.
    #[test]
    #[serial]
    fn retention_hint_quotes_the_path_and_prefers_the_safe_delete_when_it_would_work() {
        // HEAD is `integration`, which contains the branch — so `-d` succeeds —
        // but the entry records no base, so teardown still compares against
        // `main` only and keeps it (the pre-`base_branch` session shape).
        let mut sb = setup_integration_sandbox(true);
        sb.session.worktrees[0].base_branch = None;
        let entry = &sb.session.worktrees[0];
        let worktree_path = entry.worktree_path.display().to_string();

        let mut stderr = Vec::<u8>::new();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = String::from_utf8(stderr).unwrap();
        assert!(
            err_text.contains(&format!("git worktree add '{worktree_path}' feature/test")),
            "the resume hint must quote the worktree path: {err_text:?}"
        );
        assert!(
            err_text.contains("git branch -d feature/test"),
            "`-d` must be offered when git would accept it: {err_text:?}"
        );
    }

    /// The `-D` fallback: nothing contains the branch, so the safe delete would
    /// be refused and the hint must say so.
    #[test]
    #[serial]
    fn retention_hint_falls_back_to_force_delete_when_the_safe_delete_would_fail() {
        let sb = setup_purge_sandbox(true);
        let entry = &sb.session.worktrees[0];

        let mut stderr = Vec::<u8>::new();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = String::from_utf8(stderr).unwrap();
        assert!(
            err_text.contains("git branch -D feature/test"),
            "`-D` is the only form that would work here: {err_text:?}"
        );
    }

    /// Spec `git-add-remove-branch` — "remove keeps a created branch with
    /// unmerged commits".
    #[test]
    #[serial]
    fn detach_worktree_keeps_a_created_branch_with_unmerged_commits() {
        let sb = setup_purge_sandbox(true);
        let entry = &sb.session.worktrees[0];
        let tip_before = branch_tip(&sb.repo, &entry.branch).expect("branch exists");
        let worktree_path = entry.worktree_path.clone();

        let mut stderr = Vec::<u8>::new();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = String::from_utf8(stderr).unwrap();
        assert!(!worktree_path.exists(), "worktree should be removed");
        assert_eq!(
            branch_tip(&sb.repo, &entry.branch).as_deref(),
            Some(tip_before.as_str()),
            "the branch must survive at the same tip"
        );
        assert!(
            err_text.contains("Kept branch feature/test at ")
                && err_text.contains("1 commit(s) not in main"),
            "retention must name the branch, its tip, and the unmerged count: {err_text:?}"
        );
        assert!(
            err_text.contains(&format!(
                "git worktree add '{}' feature/test",
                worktree_path.display()
            )),
            "retention must print the resume hint: {err_text:?}"
        );
        assert!(
            err_text.contains("git branch -D feature/test"),
            "retention must print the discard hint: {err_text:?}"
        );
    }

    /// Spec `git-add-remove-branch` — "remove deletes a created branch that is
    /// fully merged".
    #[test]
    #[serial]
    fn detach_worktree_deletes_a_created_branch_that_is_fully_merged() {
        let sb = setup_purge_sandbox(false);
        let entry = &sb.session.worktrees[0];
        let tip_before = branch_tip(&sb.repo, &entry.branch).expect("branch exists");
        let short_tip = &tip_before[..7];

        let mut stderr = Vec::<u8>::new();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = String::from_utf8(stderr).unwrap();
        assert!(
            branch_tip(&sb.repo, &entry.branch).is_none(),
            "a fully merged created branch should be deleted"
        );
        assert!(
            err_text.contains(&format!("Deleted branch feature/test (was {short_tip}")),
            "deletion must print the branch and the SHA it was at: {err_text:?}"
        );
    }

    /// Spec `git-add-remove-branch` — "A pre-existing branch is never deleted".
    #[test]
    #[serial]
    fn detach_worktree_leaves_a_pre_existing_branch_untouched() {
        let mut sb = setup_purge_sandbox(false);
        sb.session.worktrees[0].branch_created = false;
        let entry = &sb.session.worktrees[0];
        let tip_before = branch_tip(&sb.repo, &entry.branch).expect("branch exists");

        let mut stderr = Vec::<u8>::new();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = String::from_utf8(stderr).unwrap();
        assert_eq!(
            branch_tip(&sb.repo, &entry.branch).as_deref(),
            Some(tip_before.as_str()),
            "a branch git-paw did not create must remain even when merged"
        );
        assert!(
            !err_text.contains("Deleted branch"),
            "no deletion should be reported: {err_text:?}"
        );
    }

    /// Spec `git-add-remove-branch` — "A failed merge comparison retains the
    /// branch". The session entry names a branch that is not in the repository,
    /// so `git merge-base` cannot compare it.
    #[test]
    #[serial]
    fn detach_worktree_retains_a_branch_whose_merge_state_is_undeterminable() {
        let mut sb = setup_purge_sandbox(true);
        sb.session.worktrees[0].branch = "feature/ghost".to_string();
        let entry = &sb.session.worktrees[0];

        let mut stderr = Vec::<u8>::new();
        detach_worktree(&sb.repo, entry, None, &mut stderr);

        let err_text = String::from_utf8(stderr).unwrap();
        assert!(
            err_text.contains("Kept branch feature/ghost")
                && err_text.contains("merge state could not be determined"),
            "an uncomparable branch must be retained with the reason stated: {err_text:?}"
        );
        assert!(
            !err_text.contains("Deleted branch"),
            "nothing may be deleted on a failed comparison: {err_text:?}"
        );
        assert!(
            branch_tip(&sb.repo, "feature/test").is_some(),
            "the real branch must be untouched"
        );
    }

    #[test]
    #[serial]
    fn purge_no_unmerged_commits_runs_without_warning() {
        let sb = setup_purge_sandbox(false);
        let worktree_path = sb.session.worktrees[0].worktree_path.clone();
        let short_tip =
            branch_tip(&sb.repo, "feature/test").expect("branch exists")[..7].to_string();

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
        assert!(
            branch_tip(&sb.repo, "feature/test").is_none(),
            "a fully merged created branch should be deleted"
        );
        assert!(
            err_text.contains(&format!("Deleted branch feature/test (was {short_tip}")),
            "each deletion should print the SHA the branch was at: {err_text:?}"
        );
    }

    #[test]
    #[serial]
    fn purge_with_unmerged_commits_emits_warning_to_stderr() {
        let sb = setup_purge_sandbox(true);
        let tip_before = branch_tip(&sb.repo, "feature/test").expect("branch exists before purge");

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
            err_text.contains("will be kept"),
            "the warning must say the branches are kept, not lost: {err_text:?}"
        );
        assert!(
            err_text.contains("1 commit(s) not in main"),
            "the warning must report the unmerged count: {err_text:?}"
        );
        assert_eq!(
            branch_tip(&sb.repo, "feature/test").as_deref(),
            Some(tip_before.as_str()),
            "an unmerged branch must survive the purge at the same tip"
        );
    }

    /// Spec `supervisor-launch` — "Confirmed purge also retains unmerged
    /// branches".
    #[test]
    #[serial]
    fn purge_confirmed_by_the_user_still_retains_an_unmerged_branch() {
        let sb = setup_purge_sandbox(true);
        let tip_before = branch_tip(&sb.repo, "feature/test").expect("branch exists before purge");
        let worktree_path = sb.session.worktrees[0].worktree_path.clone();

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
        assert!(!worktree_path.exists(), "worktree should still be removed");
        assert_eq!(
            branch_tip(&sb.repo, "feature/test").as_deref(),
            Some(tip_before.as_str()),
            "answering Y must not delete an unmerged branch"
        );
    }

    /// Spec `supervisor-launch` — "An undeterminable merge state is treated as
    /// unmerged". The session names a branch that no longer exists, so every
    /// ref comparison against it errors.
    #[test]
    #[serial]
    fn purge_reports_an_undeterminable_merge_state_and_deletes_nothing() {
        let mut sb = setup_purge_sandbox(true);
        let tip_before = branch_tip(&sb.repo, "feature/test").expect("branch exists before purge");
        sb.session.worktrees[0].branch = "feature/ghost".to_string();

        let mut stderr = Vec::<u8>::new();
        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            true, // force
            &mut |_: &str| Ok(true),
            &mut |_: &str| Ok(()),
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PurgeOutcome::Purged);
        let err_text = String::from_utf8(stderr).unwrap();
        assert!(
            err_text.contains("merge state could not be determined"),
            "an uncomparable branch must be reported, not silently treated as merged: {err_text:?}"
        );
        assert_eq!(
            branch_tip(&sb.repo, "feature/test").as_deref(),
            Some(tip_before.as_str()),
            "no branch may be deleted when the comparison cannot be made"
        );
    }

    /// Spec `supervisor-launch` — "Purge in a master-default repository without
    /// origin/HEAD compares against master". The purge-local resolver used to
    /// fall back to a literal `main` here, so the comparison failed and the
    /// branch was deleted without a warning.
    #[test]
    #[serial]
    fn purge_in_a_master_default_repo_without_origin_head_reports_the_unmerged_commit() {
        let sb = setup_purge_sandbox_on("master", true);
        let tip_before = branch_tip(&sb.repo, "feature/test").expect("branch exists before purge");

        let mut stderr = Vec::<u8>::new();
        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            true, // force
            &mut |_: &str| Ok(true),
            &mut |_: &str| Ok(()),
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PurgeOutcome::Purged);
        let err_text = String::from_utf8(stderr).unwrap();
        assert!(
            err_text.contains("1 commit(s) not in master"),
            "the comparison must run against master, not a literal main: {err_text:?}"
        );
        assert_eq!(
            branch_tip(&sb.repo, "feature/test").as_deref(),
            Some(tip_before.as_str()),
            "the unmerged branch must survive"
        );
    }

    #[test]
    #[serial]
    fn purge_force_skips_confirm_but_still_warns() {
        let sb = setup_purge_sandbox(true);
        let tip_before = branch_tip(&sb.repo, "feature/test").expect("branch exists before purge");

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
        assert!(
            err_text.contains("will be kept"),
            "--force must not imply the branches are deleted: {err_text:?}"
        );
        assert_eq!(
            branch_tip(&sb.repo, "feature/test").as_deref(),
            Some(tip_before.as_str()),
            "--force must still retain a branch with unmerged commits"
        );
        assert!(
            err_text.contains("git branch -D feature/test"),
            "the retention must print how to discard the branch by hand: {err_text:?}"
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

    /// Recorder for the order of writes, flushes, and confirm calls.
    /// Used by the Bug C regression test to assert the warning is
    /// flushed BEFORE the prompt fires.
    struct OrderedWrite {
        events: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    impl std::io::Write for OrderedWrite {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.events
                .borrow_mut()
                .push(format!("write({})", buf.len()));
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.events.borrow_mut().push("flush".to_string());
            Ok(())
        }
    }

    // Bug D — purge --force "freeze". Per-worktree progress markers
    // must surround each `git::remove_worktree` call so the user can
    // tell the command is making progress on a large worktree. The
    // assertion is on the stderr contents after a real successful
    // purge_with_prompt run. v0-5-0-audit-cleanup tasks 9b.1–9b.5.
    #[test]
    #[serial]
    fn purge_emits_per_worktree_progress_messages() {
        let sb = setup_purge_sandbox(false);
        let worktree_path_display = sb.session.worktrees[0].worktree_path.display().to_string();

        let mut confirm = |_: &str| -> Result<bool, PawError> { Ok(true) };
        let mut stderr = Vec::<u8>::new();

        let outcome = purge_with_prompt(
            &sb.repo,
            &sb.sessions_dir,
            &sb.session,
            true, // force — bypass confirm to isolate the progress assertion
            &mut confirm,
            &mut |_: &str| Ok(()),
            &mut stderr,
        )
        .unwrap();

        assert_eq!(outcome, PurgeOutcome::Purged);
        let err_text = String::from_utf8(stderr).unwrap();
        assert!(
            err_text.contains(&format!("Removing worktree {worktree_path_display}")),
            "stderr should announce each worktree removal by path; got:\n{err_text}"
        );
        assert!(
            err_text.contains("...done ("),
            "stderr should emit a `...done (Xs)` marker per worktree once removal completes; got:\n{err_text}"
        );
    }

    // Bug C — purge confirmation prompt regression. When unmerged-
    // commit warnings are written to stderr, the stream must be FLUSHED
    // before the dialoguer prompt reads stdin. Without the flush the
    // warning races against the prompt and dialoguer can mis-classify
    // the `y` keystroke as cancellation. v0-5-0-audit-cleanup tasks
    // 9a.1–9a.4 (the piped-stdin integration test described in 9a.4 is
    // impractical because dialoguer probes the TTY and behaves
    // differently under pipes; the behavioural assertion below covers
    // the same regression shape without a TTY dependency).
    #[test]
    #[serial]
    fn purge_with_unmerged_commits_flushes_stderr_before_confirm() {
        let sb = setup_purge_sandbox(true);

        let events: std::rc::Rc<std::cell::RefCell<Vec<String>>> =
            std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let events_for_confirm = std::rc::Rc::clone(&events);

        let mut confirm = |_: &str| -> Result<bool, PawError> {
            events_for_confirm.borrow_mut().push("confirm".to_string());
            Ok(true)
        };
        let mut stderr = OrderedWrite {
            events: std::rc::Rc::clone(&events),
        };

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
        let recorded = events.borrow().clone();
        let confirm_idx = recorded
            .iter()
            .position(|s| s == "confirm")
            .expect("confirm must have been called");
        // Find the last flush BEFORE confirm — that's the Bug C fix's
        // flush. Per-worktree progress emits more flushes after
        // confirm; the test only cares that at least one flush
        // precedes the prompt.
        let flush_before_confirm_idx = recorded[..confirm_idx]
            .iter()
            .rposition(|s| s == "flush")
            .expect("stderr.flush() must have been called before the prompt fired");
        // Defence in depth: at least one write must have happened
        // before that flush — confirming the warning text actually
        // landed in the stream before the flush event.
        let has_write_before_flush = recorded[..flush_before_confirm_idx]
            .iter()
            .any(|s| s.starts_with("write("));
        assert!(
            has_write_before_flush,
            "at least one stderr write must precede the pre-confirm flush; events: {recorded:?}"
        );
    }

    // -----------------------------------------------------------------------
    // learnings_disclosure_notice — session-start privacy disclosure gating
    // (spec: learnings-mode "Session-start learnings disclosure notice")
    // -----------------------------------------------------------------------

    #[test]
    fn disclosure_notice_prints_when_learnings_enabled() {
        // GIVEN [supervisor] enabled = true AND learnings = true.
        let cfg = SupervisorConfig {
            enabled: true,
            learnings: true,
            ..SupervisorConfig::default()
        };
        let notice =
            learnings_disclosure_notice(Some(&cfg)).expect("notice must print when opted in");
        // THEN it names the local path, the no-telemetry stance, and the
        // optional share-via-issue invitation with the review/anonymise caveat.
        assert!(
            notice.contains(".git-paw/session-learnings.md"),
            "notice must name the local path; got: {notice}"
        );
        assert!(
            notice.contains("no telemetry") && notice.contains("nothing is sent anywhere"),
            "notice must state the no-telemetry stance; got: {notice}"
        );
        assert!(
            notice.contains(GIT_PAW_ISSUES_URL),
            "notice must invite sharing via the canonical issues URL; got: {notice}"
        );
        assert!(
            notice.contains("anonymise"),
            "notice must carry the review/anonymise caveat; got: {notice}"
        );
    }

    #[test]
    fn no_disclosure_notice_when_learnings_disabled() {
        // GIVEN [supervisor] enabled = true but learnings = false.
        let cfg = SupervisorConfig {
            enabled: true,
            learnings: false,
            ..SupervisorConfig::default()
        };
        assert!(
            learnings_disclosure_notice(Some(&cfg)).is_none(),
            "no notice when learnings is disabled"
        );
    }

    #[test]
    fn no_disclosure_notice_when_supervisor_section_absent() {
        // GIVEN no [supervisor] section at all — output identical to pre-change.
        assert!(
            learnings_disclosure_notice(None).is_none(),
            "no notice when the [supervisor] section is absent"
        );
    }

    #[test]
    fn no_disclosure_notice_when_supervisor_disabled() {
        // Defence in depth: learnings = true but supervisor disabled mirrors
        // the aggregator attach predicate (enabled && learnings) — no file is
        // produced, so no notice.
        let cfg = SupervisorConfig {
            enabled: false,
            learnings: true,
            ..SupervisorConfig::default()
        };
        assert!(
            learnings_disclosure_notice(Some(&cfg)).is_none(),
            "no notice when supervisor mode itself is disabled"
        );
    }

    // Spec scenario: "Learnings doc carries the privacy and sharing section".
    // Guards the user-guide chapter against silently losing the no-telemetry
    // stance or the optional-share invitation with its review/anonymise caveat.
    #[test]
    fn learnings_doc_carries_privacy_and_sharing_section() {
        let doc = include_str!("../docs/src/user-guide/learnings.md");
        assert!(
            doc.contains("## Privacy & Sharing"),
            "learnings doc must carry a Privacy & Sharing section"
        );
        assert!(
            doc.contains("no telemetry"),
            "section must state the no-telemetry / local / opt-in stance"
        );
        assert!(
            doc.contains(GIT_PAW_ISSUES_URL),
            "section must link to the GitHub issue tracker for optional sharing"
        );
        assert!(
            doc.contains("anonymise"),
            "section must carry the review-and-anonymise caveat"
        );
    }

    #[test]
    fn learnings_doc_documents_broker_publication_and_file_sourced_get_learnings() {
        let doc = include_str!("../docs/src/user-guide/learnings.md");
        assert!(
            doc.contains("agent.learning"),
            "learnings doc must mention agent.learning broker messages"
        );
        assert!(
            doc.contains("get_learnings") || doc.contains("get_learnings()"),
            "learnings doc must mention that get_learnings() reads the file"
        );
        assert!(
            !doc.contains("deferred"),
            "learnings doc must not claim the feature is deferred"
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
            None,
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
            None,
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
            None,
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
mod permission_seeding_removed_tests {
    //! `approval-command-safety` — "No permission grants are seeded into
    //! agent CLI settings files": `attach_agent` is the per-worktree
    //! provisioning primitive shared by `git paw start` (`cmd_supervisor`)
    //! and `git paw add`, and touches no tmux state — so it is exercised
    //! directly here rather than through a full session launch. The
    //! machinery that used to write a settings file / vendor directory
    //! (the curl-allowlist, dev-allowlist and worktree-allowlist seeders)
    //! has been deleted; these tests pin the resulting behaviour.

    use std::path::Path;
    use std::process::Command as StdCommand;

    use tempfile::TempDir;

    use git_paw::config::{BrokerConfig, CustomCli, WorktreeConfig, WorktreePlacement};
    use git_paw::skills::GateCommands;

    use super::{AttachContext, attach_agent};

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

    fn init_repo(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["config", "user.email", "test@test.com"]);
        git(dir, &["config", "user.name", "Test"]);
        std::fs::write(dir.join("README.md"), "# test").unwrap();
        git(dir, &["add", "."]);
        git(dir, &["commit", "-q", "-m", "initial"]);
    }

    fn attach_ctx<'a>(
        repo_root: &'a Path,
        gate_commands: &'a GateCommands<'a>,
        broker_config: &'a BrokerConfig,
        worktree_runtime: &'a WorktreeConfig,
        clis: &'a std::collections::HashMap<String, CustomCli>,
    ) -> AttachContext<'a> {
        AttachContext {
            repo_root,
            project: "test",
            broker_config,
            agent_cli: "claude",
            agent_flags: "",
            coordination_template: None,
            docs_fetch_template: None,
            gate_commands,
            session_backends: &[],
            inter_agent_rules_peers: None,
            strict_guard: true,
            no_rebase: true,
            placement: WorktreePlacement::Sibling,
            worktree_runtime,
            clis,
        }
    }

    /// Scenario "No vendor settings directory is created in a worktree": a
    /// freshly attached worktree gets no `.claude/` directory and no
    /// `.claude/` line in its worktree-local `info/exclude`.
    #[test]
    fn attach_agent_creates_no_claude_directory_or_exclude_entry() {
        let sandbox = TempDir::new().unwrap();
        let repo = sandbox.path().join("repo");
        init_repo(&repo);

        let gate_commands = GateCommands::default();
        let broker_config = BrokerConfig::default();
        let worktree_runtime = WorktreeConfig::default();
        let clis = std::collections::HashMap::new();
        let ctx = attach_ctx(
            &repo,
            &gate_commands,
            &broker_config,
            &worktree_runtime,
            &clis,
        );

        let attached = attach_agent(&ctx, "feat-a", None, 0).expect("attach_agent succeeds");
        let wt_path = attached.entry.worktree_path.clone();

        assert!(
            !wt_path.join(".claude").exists(),
            "attach_agent must not create a .claude/ directory in the worktree"
        );

        let exclude_out = StdCommand::new("git")
            .current_dir(&wt_path)
            .args(["rev-parse", "--git-path", "info/exclude"])
            .output()
            .expect("git rev-parse --git-path info/exclude");
        assert!(exclude_out.status.success());
        let raw = String::from_utf8_lossy(&exclude_out.stdout)
            .trim()
            .to_string();
        let exclude_path = if Path::new(&raw).is_absolute() {
            std::path::PathBuf::from(raw)
        } else {
            wt_path.join(raw)
        };
        let exclude_contents = std::fs::read_to_string(&exclude_path).unwrap_or_default();
        assert!(
            !exclude_contents.lines().any(|l| l.trim() == ".claude/"),
            "attach_agent must not add a .claude/ exclude entry; got: {exclude_contents:?}"
        );
    }

    /// Scenario "Pre-existing settings files are left untouched": a settings
    /// file that predates this change (or was hand-maintained by a user)
    /// survives a session re-attach byte-identical.
    #[test]
    fn attach_agent_leaves_a_pre_existing_settings_file_byte_identical() {
        let sandbox = TempDir::new().unwrap();
        let repo = sandbox.path().join("repo");
        init_repo(&repo);

        let gate_commands = GateCommands::default();
        let broker_config = BrokerConfig::default();
        let worktree_runtime = WorktreeConfig::default();
        let clis = std::collections::HashMap::new();
        let ctx = attach_ctx(
            &repo,
            &gate_commands,
            &broker_config,
            &worktree_runtime,
            &clis,
        );

        // First attach creates the worktree.
        let attached = attach_agent(&ctx, "feat-b", None, 0).expect("first attach succeeds");
        let wt_path = attached.entry.worktree_path.clone();

        // A user (or a pre-removal git-paw) wrote a settings file by hand.
        let settings_dir = wt_path.join(".claude");
        std::fs::create_dir_all(&settings_dir).unwrap();
        let settings = settings_dir.join("settings.json");
        let sentinel = r#"{"allowed_bash_prefixes":["my-custom-tool"]}"#;
        std::fs::write(&settings, sentinel).unwrap();

        // Re-attach the SAME branch — `create_worktree` is idempotent for an
        // existing worktree/branch pair, mirroring the recovery/resume path.
        attach_agent(&ctx, "feat-b", None, 0).expect("second attach succeeds");

        let after = std::fs::read_to_string(&settings).unwrap();
        assert_eq!(
            after, sentinel,
            "a pre-existing settings file must be left byte-identical"
        );
    }

    /// Asserts `id` matches the v4 UUID shape
    /// (`xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx`).
    fn assert_v4_uuid_shape(id: &str) {
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            vec![8, 4, 4, 4, 12],
            "not a v4 UUID shape: {id}"
        );
        assert!(parts[2].starts_with('4'), "version nibble must be 4: {id}");
    }

    /// Spec scenario "Claude worker is launched with an assigned id"
    /// (`worker-session-id-capture`).
    #[test]
    fn attach_agent_assigns_a_claude_worker_a_session_id() {
        let sandbox = TempDir::new().unwrap();
        let repo = sandbox.path().join("repo");
        init_repo(&repo);

        let gate_commands = GateCommands::default();
        let broker_config = BrokerConfig::default();
        let worktree_runtime = WorktreeConfig::default();
        let clis = std::collections::HashMap::new();
        let ctx = attach_ctx(
            &repo,
            &gate_commands,
            &broker_config,
            &worktree_runtime,
            &clis,
        );

        let attached = attach_agent(&ctx, "feat-x", None, 0).expect("attach_agent succeeds");

        let id = attached
            .entry
            .cli_session_id
            .as_deref()
            .expect("cli_session_id is assigned for claude");
        assert_v4_uuid_shape(id);
        assert!(
            attached
                .pane
                .cli_command
                .contains(&format!("--session-id {id}")),
            "pane command should carry --session-id <id>, got: {}",
            attached.pane.cli_command
        );
    }

    /// Spec scenario "Each worker gets a distinct id".
    #[test]
    fn attach_agent_assigns_distinct_ids_to_each_worker() {
        let sandbox = TempDir::new().unwrap();
        let repo = sandbox.path().join("repo");
        init_repo(&repo);

        let gate_commands = GateCommands::default();
        let broker_config = BrokerConfig::default();
        let worktree_runtime = WorktreeConfig::default();
        let clis = std::collections::HashMap::new();
        let ctx = attach_ctx(
            &repo,
            &gate_commands,
            &broker_config,
            &worktree_runtime,
            &clis,
        );

        let ids: Vec<String> = ["feat-1", "feat-2", "feat-3"]
            .iter()
            .map(|branch| {
                attach_agent(&ctx, branch, None, 0)
                    .expect("attach_agent succeeds")
                    .entry
                    .cli_session_id
                    .expect("cli_session_id is assigned")
            })
            .collect();

        let unique: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(unique.len(), 3, "each worker must get a distinct id");
    }

    /// Spec scenario "CLI without an assignment template launches unchanged".
    #[test]
    fn attach_agent_leaves_a_cli_without_a_template_unchanged() {
        let sandbox = TempDir::new().unwrap();
        let repo = sandbox.path().join("repo");
        init_repo(&repo);

        let gate_commands = GateCommands::default();
        let broker_config = BrokerConfig::default();
        let worktree_runtime = WorktreeConfig::default();
        let clis = std::collections::HashMap::new();
        let ctx = AttachContext {
            repo_root: &repo,
            project: "test",
            broker_config: &broker_config,
            agent_cli: "codex",
            agent_flags: "",
            coordination_template: None,
            docs_fetch_template: None,
            gate_commands: &gate_commands,
            session_backends: &[],
            inter_agent_rules_peers: None,
            strict_guard: true,
            no_rebase: true,
            placement: WorktreePlacement::Sibling,
            worktree_runtime: &worktree_runtime,
            clis: &clis,
        };

        let attached = attach_agent(&ctx, "feat-codex", None, 0).expect("attach_agent succeeds");

        assert_eq!(attached.pane.cli_command, "codex");
        assert_eq!(attached.entry.cli_session_id, None);
    }

    /// Spec scenario "Assignment follows approval flags".
    #[test]
    fn attach_agent_appends_session_id_after_approval_flags() {
        let sandbox = TempDir::new().unwrap();
        let repo = sandbox.path().join("repo");
        init_repo(&repo);

        let gate_commands = GateCommands::default();
        let broker_config = BrokerConfig::default();
        let worktree_runtime = WorktreeConfig::default();
        let clis = std::collections::HashMap::new();
        let ctx = AttachContext {
            repo_root: &repo,
            project: "test",
            broker_config: &broker_config,
            agent_cli: "claude",
            agent_flags: "--dangerously-skip-permissions",
            coordination_template: None,
            docs_fetch_template: None,
            gate_commands: &gate_commands,
            session_backends: &[],
            inter_agent_rules_peers: None,
            strict_guard: true,
            no_rebase: true,
            placement: WorktreePlacement::Sibling,
            worktree_runtime: &worktree_runtime,
            clis: &clis,
        };

        let attached =
            attach_agent(&ctx, "feat-full-auto", None, 0).expect("attach_agent succeeds");

        let flags_pos = attached
            .pane
            .cli_command
            .find("--dangerously-skip-permissions")
            .expect("approval flag present");
        let session_pos = attached
            .pane
            .cli_command
            .find("--session-id")
            .expect("session-id flag present");
        assert!(
            flags_pos < session_pos,
            "--dangerously-skip-permissions must precede --session-id, got: {}",
            attached.pane.cli_command
        );
    }
}

/// The parsed `setup-cli` invocation, kept as one struct so the entry
/// function takes a single argument rather than seven loose parameters.
///
/// The booleans mirror the command-line flags one for one; collapsing them
/// into enums here would only re-encode what clap already parsed, and would
/// hide which flag a given branch is reacting to.
#[allow(clippy::struct_excessive_bools)]
struct SetupCliArgs<'a> {
    /// CLI identity named on the command line.
    cli: &'a str,
    /// `--allowlist`.
    allowlist: bool,
    /// `--trust`.
    trust: bool,
    /// `--hooks`.
    hooks: bool,
    /// `--dry-run`.
    dry_run: bool,
    /// `--yes`.
    yes: bool,
    /// `--undo`.
    undo: bool,
}

impl SetupCliArgs<'_> {
    /// Returns the requested items, in flag order.
    fn items(&self) -> Vec<SetupItem> {
        let mut items = Vec::new();
        if self.allowlist {
            items.push(SetupItem::Allowlist);
        }
        if self.trust {
            items.push(SetupItem::Trust);
        }
        if self.hooks {
            items.push(SetupItem::Hooks);
        }
        items
    }
}

/// Entry layer for `git paw setup-cli`.
///
/// Owns everything the use case deliberately does not: printing the plan, the
/// terminal check, the confirmation, and the mapping from outcome to exit
/// code. Writing is gated three ways — the item must be supported, its facts
/// must be verified, and the user must have confirmed (or passed `--yes`).
fn cmd_setup_cli(args: &SetupCliArgs<'_>) -> Result<(), PawError> {
    let cwd = std::env::current_dir()
        .map_err(|e| PawError::SessionError(format!("cannot read current directory: {e}")))?;
    let repo_root = git_paw::git::validate_repo(&cwd)?;
    let config = git_paw::config::load_config(&repo_root, None)?;
    let runner = git_paw::command_runner::RealCommandRunner;

    let request = commands::setup_cli::SetupRequest {
        cli: args.cli.to_string(),
        items: args.items(),
        undo: args.undo,
    };

    match commands::setup_cli::prepare(&runner, &cwd, &config, &request)? {
        commands::setup_cli::Prepared::Unsupported { cli, items } => {
            print!("{}", render_unsupported(&cli, &items));
            Err(PawError::NotAttempted(format!(
                "nothing was written: {} not supported for `{cli}`",
                if items.len() == 1 {
                    "that setup item is"
                } else {
                    "those setup items are"
                }
            )))
        }
        commands::setup_cli::Prepared::Setup { plan, snapshots } => {
            print!("{}", render_plan(&plan));
            if args.dry_run {
                return Ok(());
            }
            if plan.is_empty() {
                // Nothing to do: no prompt, no write, success.
                return Ok(());
            }
            if !plan.verified {
                return Err(PawError::NotAttempted(format!(
                    "nothing was written: these facts have not been verified against an \
                     installed copy of `{}`, so writing is disabled — `--dry-run` still shows \
                     the plan",
                    plan.cli
                )));
            }
            confirm_setup(args, "Apply this plan?")?;
            let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
            let outcome = commands::setup_cli::apply_setup(&plan, &snapshots, &now)?;
            println!(
                "\nApplied {} entr{} across {} file{}.",
                outcome.entries_added,
                if outcome.entries_added == 1 {
                    "y"
                } else {
                    "ies"
                },
                outcome.written.len(),
                if outcome.written.len() == 1 { "" } else { "s" }
            );
            Ok(())
        }
        commands::setup_cli::Prepared::Undo {
            plan,
            record,
            snapshots,
        } => {
            print!("{}", render_undo_plan(&plan));
            if args.dry_run {
                return Ok(());
            }
            if plan.is_empty() {
                return Ok(());
            }
            confirm_setup(args, "Remove these entries?")?;
            let outcome = commands::setup_cli::apply_undo(&plan, &record, &snapshots)?;
            println!(
                "\nRemoved {} entr{}.",
                outcome.entries_removed,
                if outcome.entries_removed == 1 {
                    "y"
                } else {
                    "ies"
                }
            );
            Ok(())
        }
    }
}

/// The confirmation gate shared by setup and undo.
///
/// `--yes` proceeds. On a terminal the user is asked, defaulting to No. Off a
/// terminal there is nobody to ask, so nothing is written and the message
/// names the flags that would proceed — a bare "Cancelled." would leave the
/// user with no idea what to do next.
fn confirm_setup(args: &SetupCliArgs<'_>, prompt: &str) -> Result<(), PawError> {
    if args.yes {
        return Ok(());
    }
    if !is_interactive_stdin() {
        return Err(PawError::NotAttempted(
            "nothing was written: the confirmation could not be shown because stdin is not a \
             terminal — re-run with `--yes` to apply the printed plan, or `--dry-run` to preview \
             it"
            .into(),
        ));
    }
    let approved = Confirm::new()
        .with_prompt(prompt)
        .default(false)
        .interact()
        .map_err(|e| PawError::SessionError(format!("confirmation prompt failed: {e}")))?;
    if approved {
        Ok(())
    } else {
        Err(PawError::NotAttempted(
            "nothing was written: the plan was not approved".into(),
        ))
    }
}
