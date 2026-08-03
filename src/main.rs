//! git-paw — Parallel AI Worktrees.
//!
//! Orchestrates multiple AI coding CLI sessions across git worktrees
//! from a single terminal using tmux.

use std::io::IsTerminal;
use std::path::Path;
use std::process::Command as StdCommand;

use clap::Parser;
use dialoguer::Confirm;

use git_paw::agents;
use git_paw::broker;
#[cfg(test)]
use git_paw::broker::publish::build_status_message;
use git_paw::cli::{Cli, Command, SpecsFormat};
use git_paw::config::{self, SupervisorConfig};
use git_paw::error::PawError;
use git_paw::git;
use git_paw::interactive;
use git_paw::session::{self, Session, SessionMode, WorktreeEntry};
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
        Command::Stop { force } => commands::stop::cmd_stop(force),
        Command::Purge { force, stale } => cmd_purge(force, stale),
        Command::Status { json } => commands::status::cmd_status(json),
        Command::ListClis => commands::clis::cmd_list_clis(),
        Command::AddCli {
            name,
            command,
            display_name,
        } => commands::clis::cmd_add_cli(&name, &command, display_name.as_deref()),
        Command::RemoveCli { name } => commands::clis::cmd_remove_cli(&name),
        Command::Dashboard => cmd_dashboard(),
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
/// - `SpecBackendKind::SpecKit` → falls through to the same sidecar pointer
///   as `Markdown`. A `/speckit:apply`-style slash command may land later;
///   this change does not pre-empt its shape.
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
/// When a spec is associated with the branch, the prompt includes the spec
/// ID so the agent can locate sibling artifacts (proposal, design, specs,
/// tasks) under `openspec/changes/<id>/`. When no spec is associated, the
/// prompt is the default fallback that points only at the sidecar.
pub(crate) fn build_task_prompt(spec_entry: Option<&git_paw::specs::SpecEntry>) -> String {
    use git_paw::agents::SIDECAR_REL_PATH;
    use git_paw::specs::SpecBackendKind;
    match spec_entry {
        Some(s) => match s.backend {
            SpecBackendKind::OpenSpec => format!(
                "Read {SIDECAR_REL_PATH} first — it carries your assignment and \
                 coordination rules — then run /opsx:apply {id}.",
                id = s.id,
            ),
            SpecBackendKind::Markdown | SpecBackendKind::SpecKit => format!(
                "Begin your assigned task. Read {SIDECAR_REL_PATH} first — it carries the \
                 project rules, your full spec, and your assignment. Additional artifacts \
                 (proposal, design, specs, tasks) live under openspec/changes/{id}/ — read \
                 them all before starting.",
                id = s.id,
            ),
            SpecBackendKind::Superpowers => format!(
                "Begin your assigned task. Read {SIDECAR_REL_PATH} first — it carries the \
                 project rules and your full superpowers plan (goal, tasks, exact file \
                 paths, and per-step verification commands). Work the steps in order and \
                 flip `- [ ]` to `- [x]` in the plan as each one lands."
            ),
        },
        None => format!(
            "Read {SIDECAR_REL_PATH} first for your assignment, then begin your assigned task."
        ),
    }
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
    /// Inter-agent ownership rules block (`None` for the bare/no-rules case).
    inter_agent_rules: Option<&'a str>,
    /// Whether the broker git hooks enforce the strict branch guard.
    strict_guard: bool,
    /// Skip rebasing the branch onto the default branch on worktree create.
    no_rebase: bool,
    /// Worktree placement (child vs sibling) for `create_worktree`.
    placement: git_paw::config::WorktreePlacement,
    /// Resolved `[supervisor.common_dev_allowlist]` config for the
    /// per-worktree allowlist seeding (gating + stacks + extra).
    common_dev_allowlist: &'a git_paw::config::CommonDevAllowlistConfig,
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
fn attach_agent(
    ctx: &AttachContext,
    branch: &str,
    spec_entry: Option<&git_paw::specs::SpecEntry>,
) -> Result<AttachedAgent, PawError> {
    let wt = git::create_worktree(ctx.repo_root, branch, !ctx.no_rebase, ctx.placement)?;
    let wt_str = wt.path.to_string_lossy().to_string();

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

    // Seed the worktree-local allowlists next to the helper scripts they
    // authorize: a claude-format CLI resolves project settings from its
    // working directory (this worktree), so the repo-root seeding never
    // applies inside the agent pane. Same gates as the script provisioning
    // above plus `[supervisor.common_dev_allowlist]`; failures are non-fatal
    // warnings, matching the repo-root seeding.
    for (path, err) in git_paw::supervisor::worktree_allowlist::seed_worktree_allowlists(
        &wt.path,
        ctx.broker_config.enabled,
        ctx.docs_fetch_template.is_some(),
        Some(ctx.common_dev_allowlist),
    ) {
        eprintln!(
            "warning: failed to seed agent-worktree allowlist for {}: {err}",
            path.display()
        );
    }

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
        ctx.coordination_template.map(&render_tmpl),
        ctx.docs_fetch_template.map(&render_tmpl),
    );

    let spec_content = spec_entry.map(|s| s.prompt.clone());
    let owned_files = spec_entry.and_then(|s| s.owned_files.clone());

    let assignment = git_paw::agents::WorktreeAssignment {
        branch: branch.to_string(),
        cli: ctx.agent_cli.to_string(),
        spec_content,
        owned_files,
        skill_content: rendered_skill,
        inter_agent_rules: ctx.inter_agent_rules.map(str::to_string),
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

    let cli_command = if ctx.agent_flags.is_empty() {
        ctx.agent_cli.to_string()
    } else {
        format!("{} {}", ctx.agent_cli, ctx.agent_flags)
    };

    let boot_block = git_paw::skills::build_boot_block(branch, &ctx.broker_config.url());
    let task_prompt = build_task_prompt(spec_entry);

    Ok(AttachedAgent {
        pane: tmux::PaneSpec {
            branch: branch.to_string(),
            worktree: wt_str,
            cli_command,
        },
        prompt: format!("{boot_block}\n\n{task_prompt}"),
        entry: WorktreeEntry {
            branch: branch.to_string(),
            worktree_path: wt.path,
            cli: ctx.agent_cli.to_string(),
            branch_created: wt.branch_created,
            pending_boot_prompt: None,
        },
    })
}

/// Resolve the boot-prompt settle delay (ms) for `cli` from config,
/// falling back to [`git_paw::DEFAULT_SUBMIT_DELAY_MS`].
///
/// `cli` may carry flags (e.g. `"mycli --foo"`); the lookup keys on the
/// leading binary token. The delay is config-driven, never a hardcoded
/// CLI-name table, so the launcher stays CLI-agnostic — a CLI whose
/// large-paste handling needs more time sets `[clis.<name>].submit_delay_ms`
/// rather than requiring a code change (W15-1, 2026-05-31 dogfood).
#[must_use]
fn resolve_submit_delay_ms(cli: &str, config: &git_paw::config::PawConfig) -> u64 {
    let base = cli.split_whitespace().next().unwrap_or(cli);
    config
        .clis
        .get(base)
        .and_then(|c| c.submit_delay_ms)
        .unwrap_or(git_paw::DEFAULT_SUBMIT_DELAY_MS)
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
/// present. Best-effort: a write failure is surfaced as a warning and does
/// not abort the launch, since the global receipt remains the source of truth.
fn write_repo_discovery_file(
    repo_root: &Path,
    session_name: &str,
    worktrees: &[WorktreeEntry],
    pane_offset: usize,
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
    let file = session::RepoSessionFile {
        session_name: session_name.to_string(),
        agents,
    };
    if let Err(e) = session::write_repo_session_file(repo_root, &file) {
        eprintln!("warning: failed to write per-repo session discovery file: {e}");
    }
}

// ---------------------------------------------------------------------------
// Command: __dashboard
// ---------------------------------------------------------------------------

/// Session-environment flag the launcher sets for an `--unattended` supervisor
/// session. The `__dashboard` subprocess reads it to disable its auto-approve
/// thread (the in-process drive loop is the sole approver, D1).
pub(crate) const UNATTENDED_ENV: &str = "GIT_PAW_UNATTENDED";

/// Whether the dashboard subprocess should spawn its auto-approve thread.
///
/// It runs only for a broker-enabled supervisor session, and NOT for an
/// `--unattended` session — there the in-process drive loop is the sole
/// approver, so a dashboard-side approver would race it on the same pane.
fn dashboard_should_auto_approve(
    mode: SessionMode,
    broker_enabled: bool,
    unattended: bool,
) -> bool {
    mode == SessionMode::Supervisor && broker_enabled && !unattended
}

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

    let log_path = session::session_state_dir()?.join("broker.log");
    let mut broker_state = broker::BrokerState::new(Some(log_path))
        .with_verify_on_commit_nudge(verify_on_commit_nudge)
        .with_seeded_cli("supervisor", &supervisor_cli);

    // Look up the saved session once: needed for the broker watcher target
    // list AND to discover the supervisor pane_map for the auto-approve thread.
    let saved_session = session::find_session_for_repo(&repo_root)?;
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
        let learnings_path = repo_root.join(".git-paw").join("session-learnings.md");
        let mut aggregator = broker::learnings::LearningsAggregator::new(learnings_path);
        aggregator.set_broker_publish(
            sup.learnings_config
                .broker_publish
                .resolve(broker_config.enabled),
        );
        for target in &watch_targets {
            aggregator.register_agent(&target.agent_id);
        }
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

    // Auto-approve thread (supervisor mode only). v0.5.0 moves the thread
    // out of cmd_supervisor (which now returns immediately) into this long-
    // lived dashboard subprocess. Killing the dashboard pane terminates the
    // thread, matching user expectation that supervision stops when the
    // dashboard is gone.
    //
    // For an `--unattended` session the in-process drive loop is the sole
    // approver, so this thread is disabled to avoid two approvers racing on
    // the same pane (D1). The launcher signals that via `GIT_PAW_UNATTENDED`
    // in the session environment, which this pane inherits.
    let unattended = std::env::var(UNATTENDED_ENV).is_ok();
    let auto_approve_handle = saved_session
        .as_ref()
        .filter(|sess| dashboard_should_auto_approve(sess.mode, broker_config.enabled, unattended))
        .and_then(|sess| {
            let auto_approve_cfg = config
                .supervisor
                .as_ref()
                .and_then(|s| s.auto_approve.clone())?;
            let pane_map: std::collections::HashMap<String, usize> = sess
                .worktrees
                .iter()
                .enumerate()
                .map(|(idx, wt)| {
                    (
                        broker::messages::slugify_branch(&wt.branch),
                        idx + git_paw::supervisor::layout::SUPERVISOR_PANE_OFFSET,
                    )
                })
                .collect();
            // Map each agent to its worktree root so the file-op classifier
            // (bug 3) can resolve write/edit prompts against the boundary.
            let worktree_map: std::collections::HashMap<String, std::path::PathBuf> = sess
                .worktrees
                .iter()
                .map(|wt| {
                    (
                        broker::messages::slugify_branch(&wt.branch),
                        wt.worktree_path.clone(),
                    )
                })
                .collect();
            // Build the manual-decision recorder from supervisor config. It is
            // inert when `[supervisor] manual_approvals_log = false`; learnings
            // emission additionally requires `[supervisor] learnings = true`.
            let supervisor = config.supervisor.as_ref();
            let manual_enabled =
                supervisor.is_none_or(SupervisorConfig::manual_approvals_log_enabled);
            let learnings_enabled = supervisor.is_some_and(|s| s.learnings);
            let cli = supervisor.and_then(|s| s.cli.clone());
            let recorder = git_paw::supervisor::manual_approvals::ManualDecisionRecorder::new(
                git_paw::supervisor::manual_approvals::log_path(
                    &sess.repo_path,
                    &sess.session_name,
                ),
                manual_enabled,
                learnings_enabled,
                sess.project_name.clone(),
                cli,
            );
            // Protected-path set for the operator config/memory danger rule
            // (`agent-memory-isolation`), derived once before the thread
            // spawns.
            let protected_paths = git_paw::supervisor::auto_approve::ProtectedPaths::derive(
                &config,
                Some(&sess.repo_path),
            );
            commands::supervisor::spawn_auto_approve_thread(
                sess.session_name.clone(),
                broker_config.url(),
                Some(auto_approve_cfg),
                supervisor
                    .map(|s| s.common_dev_allowlist.clone())
                    .unwrap_or_default(),
                commands::supervisor::AutoApproveWiring {
                    pane_map,
                    worktree_map,
                    recorder,
                    protected_paths,
                },
            )
        });

    let dashboard_result = git_paw::dashboard::run_dashboard_with_panes(
        &state,
        handle,
        &shutdown,
        &std::collections::HashMap::new(),
        None,
        broker_log_cfg.max_messages,
        broker_log_cfg.default_visible,
        broker_log_cfg.height_lines,
    );

    if let Some((stop, join)) = auto_approve_handle {
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = join.join();
    }

    dashboard_result
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
/// With `stale = true`, purges only sessions whose tmux session is gone (a
/// stale receipt) across the whole machine, leaving live sessions untouched.
/// `--force` is redundant in that combination (a stale entry is never
/// prompted for) — passing both behaves identically to `--stale` alone.
fn cmd_purge(force: bool, stale: bool) -> Result<(), PawError> {
    if stale {
        return cmd_purge_stale();
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
/// claims active but the tmux session is gone), invalidates it — purging the
/// recorded worktrees, branches, and receipt equivalent to `purge --force` —
/// and emits a stderr notice naming the entry (design D5).
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
/// gone) across the whole machine. Live sessions are left untouched.
///
/// Stale is defined exactly as [`session::DisplayStatus::Stale`]: an `active`
/// receipt whose `tmux has-session` probe returns
/// [`tmux::SessionLiveness::Stale`]. Stopped receipts (intentionally stopped)
/// and sessions on a host with no tmux binary (Indeterminate probe) are NOT
/// touched. Exits 0 with a "nothing to purge" message when no stale receipt
/// exists.
fn cmd_purge_stale() -> Result<(), PawError> {
    let sessions_dir = session::session_state_dir()?;
    let all = session::load_all_sessions_in(&sessions_dir)?;

    let stale: Vec<session::Session> = all
        .into_iter()
        .filter(|s| {
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

/// Tears down a single agent's worktree: removes the worktree directory (with
/// the per-worktree `Removing worktree ...` / `...done (Xs)` progress markers)
/// and, when git-paw created the branch, deletes it afterwards.
///
/// Extracted from `cmd_purge`'s per-worktree loop (design D6, task 1.3) so
/// `cmd_remove` performs byte-identical removal for a single agent that
/// `git paw purge` performs for every agent. Best-effort: a failed
/// worktree-remove or branch-delete is surfaced as a `warning:` on `stderr`
/// and does not abort — matching purge's resilience on large or busy
/// worktrees.
fn detach_worktree(repo_root: &Path, entry: &WorktreeEntry, stderr: &mut dyn std::io::Write) {
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
            "Purge is irreversible. Continue?"
        };
        if !confirm(prompt_text)? {
            return Ok(PurgeOutcome::Cancelled);
        }
    }

    kill_tmux(&session.session_name)?;

    // Per-worktree teardown (worktree-remove + branch cleanup), delegated to
    // the shared `detach_worktree` helper so `cmd_remove` performs the exact
    // same per-worktree removal `purge` does (design D6, task 1.3). The helper
    // emits the per-worktree begin / `...done (Xs)` progress markers Bug D in
    // v0-5-0-audit-cleanup added.
    for entry in &session.worktrees {
        detach_worktree(repo_root, entry, stderr);
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
    use std::time::UNIX_EPOCH;
    use tempfile::TempDir;

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

    // Task 6.4 — the dashboard's auto-approve thread is disabled for an
    // `--unattended` session so the in-process drive loop is the sole approver
    // (no two approvers racing on the same pane).
    #[test]
    fn dashboard_auto_approve_disabled_under_unattended() {
        // Attended supervisor + broker → the dashboard thread runs.
        assert!(dashboard_should_auto_approve(
            SessionMode::Supervisor,
            true,
            false
        ));
        // Unattended → disabled even for a broker-enabled supervisor session.
        assert!(!dashboard_should_auto_approve(
            SessionMode::Supervisor,
            true,
            true
        ));
        // Never runs outside supervisor mode or without a broker.
        assert!(!dashboard_should_auto_approve(
            SessionMode::Bare,
            true,
            false
        ));
        assert!(!dashboard_should_auto_approve(
            SessionMode::Supervisor,
            false,
            false
        ));
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

    // Maps to scenario `Supervisor pane receives a boot block` from
    // supervisor-as-pane. The pane-0 prompt is constructed inline in
    // `cmd_supervisor` (it is not extracted into a helper); we reconstruct
    // it using the same public helpers production uses and lock in the
    // resulting prefix + framing. (test-coverage-v0-5-0 task 12.10)
    #[test]
    fn supervisor_pane_prompt_starts_with_boot_block() {
        let broker_url = "http://127.0.0.1:9119";
        let boot_block = git_paw::skills::build_boot_block("supervisor", broker_url);
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
                pending_boot_prompt: None,
            }],
            broker_port: None,
            broker_bind: None,
            broker_log_path: None,
            mode: SessionMode::Bare,
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
mod submit_delay_tests {
    //! `claude-oss-launch-v0-6-x` / `cli-submit-profile`: the boot-prompt
    //! settle delay is CONFIG-DRIVEN with a CLI-agnostic default — no
    //! hardcoded CLI-name table (W15-1, agnostic rework).

    use std::collections::HashMap;

    use git_paw::config::{CustomCli, PawConfig};

    use super::resolve_submit_delay_ms;

    fn config_with(cli: &str, submit_delay_ms: Option<u64>) -> PawConfig {
        let mut clis = HashMap::new();
        clis.insert(
            cli.to_string(),
            CustomCli {
                command: cli.to_string(),
                display_name: None,
                submit_delay_ms,
                settings_path: None,
                approval_args: HashMap::new(),
            },
        );
        PawConfig {
            clis,
            ..PawConfig::default()
        }
    }

    #[test]
    fn unknown_or_unconfigured_cli_uses_agnostic_default() {
        let cfg = PawConfig::default();
        assert_eq!(
            resolve_submit_delay_ms("any-cli", &cfg),
            git_paw::DEFAULT_SUBMIT_DELAY_MS,
        );
    }

    #[test]
    fn custom_cli_submit_delay_override_is_honoured() {
        let cfg = config_with("mycli", Some(2500));
        assert_eq!(resolve_submit_delay_ms("mycli", &cfg), 2500);
    }

    #[test]
    fn custom_cli_without_override_falls_back_to_default() {
        let cfg = config_with("mycli", None);
        assert_eq!(
            resolve_submit_delay_ms("mycli", &cfg),
            git_paw::DEFAULT_SUBMIT_DELAY_MS,
        );
    }

    #[test]
    fn lookup_keys_on_the_binary_not_the_flags() {
        // A `cli` value may carry flags (e.g. "mycli --foo"); the lookup
        // keys on the leading binary token.
        let cfg = config_with("mycli", Some(2500));
        assert_eq!(
            resolve_submit_delay_ms("mycli --dangerously-skip-permissions", &cfg),
            2500,
        );
    }

    #[test]
    fn no_cli_name_is_hardcoded_in_the_resolver() {
        // The agnostic contract: with an empty config, EVERY cli id —
        // including former-hardcoded names — resolves to the same default.
        let cfg = PawConfig::default();
        for cli in ["claude", "claude-oss", "gemini", "codex", "whatever"] {
            assert_eq!(
                resolve_submit_delay_ms(cli, &cfg),
                git_paw::DEFAULT_SUBMIT_DELAY_MS,
                "{cli} must use the agnostic default, not a hardcoded value"
            );
        }
    }
}
