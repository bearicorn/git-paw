//! Per-agent launch primitives shared by every launch path: attaching one
//! agent's worktree ([`attach_agent`]), composing its first prompt, gating
//! and submitting that prompt into its pane, and writing the per-repo
//! discovery file `sweep.sh` reads.

use std::path::Path;

use crate::config;
use crate::error::PawError;
use crate::git;
use crate::session::{self, Session, SessionStatus, WorktreeEntry};
use crate::tmux;

use super::Output;
use super::purge::created_branch_base;

/// What a launch left for its caller to do with the terminal.
///
/// A launch command never attaches, reattaches, refuses on a terminal check
/// or runs the drive loop; it reports which of those the caller owes.
#[derive(Debug)]
pub enum LaunchOutcome {
    /// Nothing to attach: a dry run, nothing to launch, or a supervised launch
    /// (which prints its own attach hint and returns).
    Done,
    /// A session is up — launched, restarted from pause, or recovered. The
    /// caller attaches when interactive, else prints the detached hint.
    Attach {
        /// The tmux session to attach to.
        session_name: String,
    },
    /// The repository already has a live session. The caller reattaches when
    /// interactive, else refuses rather than fork a parallel session.
    AlreadyRunning {
        /// The running tmux session.
        session_name: String,
    },
    /// `--unattended` launched a fresh supervised session. The caller takes
    /// the drive-loop binding and runs the loop on it.
    DriveFresh {
        /// The repository the session belongs to.
        repo_root: std::path::PathBuf,
        /// The config the launch ran with, which the loop runs with too.
        config: Box<config::PawConfig>,
        /// The session as saved.
        session: Box<Session>,
    },
    /// `--unattended` found an existing session and touched nothing. The
    /// caller refuses a bare session, takes the binding, revives, then runs
    /// the loop.
    DriveExisting {
        /// The repository the session belongs to.
        repo_root: std::path::PathBuf,
        /// The config the command ran with, which the loop runs with too.
        config: Box<config::PawConfig>,
        /// The existing session's receipt.
        existing: Box<Session>,
        /// Its effective status (active, paused or stopped).
        status: SessionStatus,
    },
}

/// An agent's rendered task prompt: the prose its first message carries, and
/// — when the spec format declares one — the workflow entry point submitted
/// as its own composer line once the pane goes idle.
///
/// The entry point is deliberately NOT part of `prose`. A command embedded in
/// a prose paragraph is a command a weaker model may narrate instead of
/// invoke (GP-54); submitted alone it is what a paste-aware CLI parses as a
/// slash-command invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskPrompt {
    /// The assignment prose appended to the boot block.
    pub(crate) prose: String,
    /// The single command line to submit after `prose`, when declared.
    pub(crate) entry_point: Option<String>,
}

/// Renders the task prompt for an agent's spec assignment (or the no-spec
/// fallback when its branch has none).
///
/// Pure: both templates come from
/// [`crate::task_prompts::templates_for`] — the one exhaustive
/// [`SpecBackendKind`](crate::specs::SpecBackendKind) match over the
/// bundled per-backend assets — and this only substitutes
/// `{{SIDECAR_REL_PATH}}` and `{{SPEC_ID}}` into them. No command name is
/// written here; a backend that declares no entry-point template yields
/// `entry_point: None` and its launch types no second line.
///
/// Every template points the agent at the gitignored sidecar
/// ([`SIDECAR_REL_PATH`](crate::agents::SIDECAR_REL_PATH)) — the combined
/// view of the project's `AGENTS.md` plus the agent's assignment and
/// coordination rules — rather than embedding any portion of the spec body,
/// which would both duplicate content and risk the paste-buffer trap. The
/// `Markdown` template additionally names the sibling-artifact directory
/// `openspec/changes/<id>/`; `SpecKit` and `Superpowers` name none, since
/// their sidecar already carries the full assignment inline
/// (speckit-spec-path-pointer).
///
/// **Prerequisite:** callers SHALL ensure `setup_worktree_agents_md` has run
/// for the worktree before the resulting prompt is injected. The prompt's
/// guidance is non-actionable if the sidecar is missing.
pub(crate) fn build_task_prompt(spec_entry: Option<&crate::specs::SpecEntry>) -> TaskPrompt {
    use crate::agents::SIDECAR_REL_PATH;
    let templates = crate::task_prompts::templates_for(spec_entry.map(|s| s.backend));
    let spec_id = spec_entry.map_or("", |s| s.id.as_str());
    let render = |template: &str| {
        template
            .replace("{{SIDECAR_REL_PATH}}", SIDECAR_REL_PATH)
            .replace("{{SPEC_ID}}", spec_id)
    };
    TaskPrompt {
        prose: render(templates.prose),
        entry_point: templates.entry_point.map(render),
    }
}

/// The bundled worktree rule every launch prompt ends with, carrying the
/// `{{WORKTREE_ROOT}}` placeholder.
pub(crate) const WORKTREE_ROOT_RULE: &str = include_str!("../../assets/worktree-root-rule.md");

/// Composes the single prompt git-paw injects into one agent pane at launch:
/// the boot block (when the broker is enabled), the task prompt's prose (when
/// the agent has an assignment), and — always, last — the worktree rule
/// naming `worktree_root`, joined by blank lines.
///
/// Every injecting path goes through here (the bare pre-fill with no prose,
/// spec-driven and supervised `start`, and `git paw add`), so a spec-driven
/// launch's prompt stays byte-identical to the supervisor path's for the same
/// branch, broker config and spec entry (`start-spec-launch-parity`) and no
/// path can forget the rule. The rule is placed last for recency: it is the
/// constraint the agent must still be holding when it starts editing
/// (`boot-block`, "Launch prompts name the agent's worktree root").
pub(crate) fn compose_launch_prompt(
    boot_block: Option<&str>,
    prose: Option<&str>,
    worktree_root: &Path,
) -> String {
    let rule = WORKTREE_ROOT_RULE.replace("{{WORKTREE_ROOT}}", &worktree_root.to_string_lossy());
    let mut parts: Vec<&str> = Vec::with_capacity(3);
    parts.extend(boot_block);
    parts.extend(prose);
    parts.push(&rule);
    parts.join("\n\n")
}

/// Per-session context for [`attach_agent`] — the values shared across every
/// agent attached to a single session. `supervisor` builds it once before
/// its per-branch loop; `add` builds it once for the single new agent.
pub(crate) struct AttachContext<'a> {
    /// Repository root the worktrees hang off.
    pub(crate) repo_root: &'a Path,
    /// Human-readable project name (worktree-path convention).
    pub(crate) project: &'a str,
    /// Broker config (drives skill rendering, hook install, boot block).
    pub(crate) broker_config: &'a crate::config::BrokerConfig,
    /// Coding-agent CLI id (no flags).
    pub(crate) agent_cli: &'a str,
    /// Resolved approval flags appended to `agent_cli` for the pane command.
    pub(crate) agent_flags: &'a str,
    /// Pre-resolved coordination skill template (`None` when broker disabled).
    pub(crate) coordination_template: Option<&'a crate::skills::SkillTemplate>,
    /// Pre-resolved docs-fetch skill template (`None` unless `docs_base_url` is
    /// explicitly configured). Gated on explicit config so a consumer that has
    /// not pointed git-paw at their own docs never has the skill (which defaults
    /// to git-paw's site) injected into their agents.
    pub(crate) docs_fetch_template: Option<&'a crate::skills::SkillTemplate>,
    /// Gate-command substitutions for skill rendering.
    pub(crate) gate_commands: &'a crate::skills::GateCommands<'a>,
    /// Distinct spec backends in the session (for `{{SPEC_PATH_DOCTRINE}}`).
    pub(crate) session_backends: &'a [crate::specs::SpecBackendKind],
    /// Peer branches to build the inter-agent rules block from (`None` for the
    /// bare/no-rules case). Held as the raw branch list rather than a
    /// pre-rendered string so `attach_agent` can build a fresh block per
    /// agent — `build_inter_agent_rules` interpolates each agent's own id
    /// into the block, so the same string cannot be shared across agents.
    pub(crate) inter_agent_rules_peers: Option<&'a [&'a str]>,
    /// Whether the broker git hooks enforce the strict branch guard.
    pub(crate) strict_guard: bool,
    /// Skip rebasing the branch onto the default branch on worktree create.
    pub(crate) no_rebase: bool,
    /// Worktree placement (child vs sibling) for `create_worktree`.
    pub(crate) placement: crate::config::WorktreePlacement,
    /// Resolved `[worktree]` runtime-provisioning config (env-file copies and
    /// port allocation). Inert when neither sub-table is configured.
    pub(crate) worktree_runtime: &'a crate::config::WorktreeConfig,
    /// The full `[clis.<name>]` table, consulted by
    /// [`config::resolve_session_id_args`] to decide whether `agent_cli`
    /// gets a fresh session id at launch (`worker-session-id-capture`).
    pub(crate) clis: &'a std::collections::HashMap<String, config::CustomCli>,
}

/// Artifacts produced by attaching one agent's worktree: the tmux pane spec to
/// splice into the session, the initial boot+task prompt to submit, and the
/// session-JSON worktree entry to register.
pub(crate) struct AttachedAgent {
    /// Surface spec (label, owning worktree, launch command) for the layout.
    pub(crate) pane: crate::terminal::SurfaceSpec,
    /// Composed boot block + task prose + worktree rule to inject into the
    /// pane as its first message.
    pub(crate) prompt: String,
    /// The workflow entry point to submit as a second composer line once the
    /// pane goes idle, when this agent's spec format declares one.
    pub(crate) entry_point: Option<String>,
    /// Session-JSON worktree record for `status` / `stop` / `purge`.
    pub(crate) entry: WorktreeEntry,
}

/// Performs the per-worktree setup `git paw start` (supervisor mode) does for a
/// single agent: create or reuse the worktree, render the coordination skill +
/// spec body into the worktree `AGENTS.md`, install the broker git hooks, and
/// build the pane spec, boot+task prompt, and session entry.
///
/// Factored out of `supervisor`'s per-branch loop (design D2, task 1.2) so
/// `supervisor` (looping over every branch) and `add` (once, for the
/// new branch) share one implementation — guaranteeing an added agent is
/// byte-identical to a start-time agent and that future boot-prompt fixes only
/// have to land here.
/// Assembles an agent pane's CLI invocation: the launch command, then its
/// approval flags, then the resolved `session_id_args` — each appended only
/// when non-empty, so a CLI with neither is invoked bare.
///
/// `command` is the pane's *launch command* ([`config::launch_command`]),
/// not the CLI identity: flags and session-id args are resolved by identity
/// but appended after the command the pane actually runs.
pub(crate) fn agent_cli_command(
    command: &str,
    flags: &str,
    session_id_args: Option<&str>,
) -> String {
    let mut command = command.to_string();
    for part in [flags, session_id_args.unwrap_or_default()] {
        if !part.is_empty() {
            command.push(' ');
            command.push_str(part);
        }
    }
    command
}

pub(crate) fn attach_agent(
    ctx: &AttachContext,
    branch: &str,
    spec_entry: Option<&crate::specs::SpecEntry>,
    runtime_slot: u16,
    out: &mut dyn Output,
) -> Result<AttachedAgent, PawError> {
    let wt = git::create_worktree(ctx.repo_root, branch, !ctx.no_rebase, ctx.placement)?;

    // Provision the worktree's runtime state — the declared env-file copies,
    // this agent's port block in a generated `.env.local`, and the consumer's
    // `on_create` hook. This runs here, after the worktree exists and before the
    // pane spec this function returns is ever launched, so the agent's CLI
    // observes a fully provisioned checkout on its first read. A no-op unless
    // `[worktree.env]` / `[worktree.ports]` / `[worktree.hooks]` is configured.
    let provisioned = crate::worktree_provision::provision_worktree(
        ctx.repo_root,
        &wt.path,
        branch,
        ctx.worktree_runtime,
        runtime_slot,
    )?;
    for warning in &provisioned.warnings {
        out.warn(&format!("warning: {branch}: {warning}"));
    }
    if let Some(summary) = provisioned.hook_summary() {
        out.line(&format!("  {branch}: {summary}"));
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
    crate::init::provision_worktree_helpers(
        &wt.path,
        ctx.broker_config.enabled,
        ctx.docs_fetch_template.is_some(),
    )?;

    let render_tmpl = |tmpl: &crate::skills::SkillTemplate| {
        crate::skills::render(
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
    let rendered_skill = crate::skills::combine_agent_skills(
        ctx.coordination_template.map(render_tmpl),
        ctx.docs_fetch_template.map(render_tmpl),
    );

    let spec_content = spec_entry.map(|s| s.prompt.clone());
    let owned_files = spec_entry.and_then(|s| s.owned_files.clone());

    let assignment = crate::agents::WorktreeAssignment {
        branch: branch.to_string(),
        cli: ctx.agent_cli.to_string(),
        spec_content,
        owned_files,
        skill_content: rendered_skill,
        inter_agent_rules: ctx
            .inter_agent_rules_peers
            .map(|peers| crate::agents::build_inter_agent_rules(branch, peers)),
    };
    crate::agents::setup_worktree_agents_md(ctx.repo_root, &wt.path, &assignment)?;

    if ctx.broker_config.enabled {
        let agent_id = crate::broker::messages::slugify_branch(branch);
        crate::agents::install_git_hooks(
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
        Some(crate::random::generate_uuid_v4()?)
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

    let (prompt, entry_point) = compose_agent_launch(ctx, branch, spec_entry, &wt.path);

    Ok(AttachedAgent {
        pane: crate::terminal::SurfaceSpec {
            label: branch.to_string(),
            owner: wt.path.clone(),
            launch_command: cli_command,
        },
        prompt,
        entry_point,
        entry: WorktreeEntry {
            branch: branch.to_string(),
            worktree_path: wt.path,
            cli: ctx.agent_cli.to_string(),
            branch_created: wt.branch_created,
            base_branch: created_branch_base(ctx.repo_root, wt.branch_created),
            pending_boot_prompt: None,
            pending_entry_point: None,
            runtime_slot: provisioned.runtime_slot,
            cli_session_id,
            spec_id: spec_entry.map(|s| s.id.clone()),
        },
    })
}

/// Builds one agent's first message and, when its spec format declares one,
/// the workflow entry point to submit after it.
///
/// Split out of [`attach_agent`] so the pure composition — boot block, task
/// prose, worktree rule — sits apart from the worktree/hook/pane side effects
/// around it, and so the supervisor and `add` paths cannot drift from it.
pub(crate) fn compose_agent_launch(
    ctx: &AttachContext,
    branch: &str,
    spec_entry: Option<&crate::specs::SpecEntry>,
    worktree_root: &Path,
) -> (String, Option<String>) {
    let boot_block = crate::skills::build_boot_block(
        branch,
        &ctx.broker_config.url(),
        crate::skills::cli_speaks_mcp(ctx.agent_cli, ctx.clis),
    );
    let task_prompt = build_task_prompt(spec_entry);
    (
        compose_launch_prompt(Some(&boot_block), Some(&task_prompt.prose), worktree_root),
        task_prompt.entry_point,
    )
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
pub(crate) fn gate_pane_or_fail_on_dialog<S: std::hash::BuildHasher>(
    backend: &dyn crate::terminal::TerminalBackend,
    session_name: &str,
    pane_index: usize,
    cli_id: &str,
    cli_command: &str,
    clis: &std::collections::HashMap<String, config::CustomCli, S>,
) -> Result<(), PawError> {
    dialog_stuck_result(
        tmux::gate_pane_for_injection(backend, session_name, pane_index, cli_id, cli_command, clis),
        session_name,
        pane_index,
        cli_command,
    )
}

/// The pure decision behind [`gate_pane_or_fail_on_dialog`]: `Err` exactly
/// when `outcome` is [`tmux::ReadinessOutcome::DialogStuck`], `Ok(())`
/// otherwise (`Ready` or `FellBack` proceed exactly as before). Split out so
/// the loud-failure wiring is unit-testable without a live tmux pane.
pub(crate) fn dialog_stuck_result(
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

/// Inject and submit an initial prompt into an agent surface through the
/// terminal port.
///
/// The boot block is injected literally, then — after `delay_ms` for a
/// paste-aware CLI to settle the (often large) paste — a separate `Enter`
/// submits it. Splitting the inject from the submit (rather than a
/// same-call trailing `Enter`) is what reliably submits a large paste
/// across CLIs (W15-1). `delay_ms` is resolved per-CLI from config so this
/// path carries no CLI-specific assumptions. Failures are swallowed —
/// best-effort by design.
pub(crate) fn submit_prompt_to_pane(
    backend: &dyn crate::terminal::TerminalBackend,
    session_name: &str,
    pane_idx: usize,
    prompt: &str,
    delay_ms: u64,
) {
    let surface = backend.surface_at(session_name, pane_idx);
    // 1. Inject the boot block literally (no Enter yet).
    let _ = backend.send_text(&surface, prompt);
    // 2. Let a paste-aware CLI settle the paste before we submit.
    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    // 3. Submit with a separate Enter.
    let _ = backend.send_key(&surface, crate::terminal::Key::Enter);
}

/// Writes the per-repo discovery file (`<repo>/.git-paw/sessions/<name>.json`)
/// the bundled `sweep.sh` helper reads (capability `session-json-location`).
///
/// Builds the sweep.sh-compatible agent roster from the launched worktrees:
/// each entry carries the broker agent id (`branch_id`), worktree path, CLI,
/// and the tmux `pane_index`. `pane_offset` is the index of the first coding
/// agent pane, answered by the session's
/// [`SessionLayout`](crate::surface::SessionLayout). No orchestrator entry
/// is written: the orchestrator runs outside the session, so no pane is its
/// own. Best-effort: a write failure is surfaced as a warning and does not
/// abort the launch, since the global receipt remains the source of truth.
pub(crate) fn write_repo_discovery_file(
    repo_root: &Path,
    session_name: &str,
    worktrees: &[WorktreeEntry],
    pane_offset: usize,
    out: &mut dyn Output,
) {
    let agents = worktrees
        .iter()
        .enumerate()
        .map(|(idx, wt)| session::RepoAgentEntry {
            branch_id: crate::broker::messages::slugify_branch(&wt.branch),
            worktree_path: wt.worktree_path.clone(),
            cli: wt.cli.clone(),
            pane_index: pane_offset + idx,
        })
        .collect();
    write_repo_discovery_file_with_agents(repo_root, session_name, agents, out);
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
pub(crate) fn write_repo_discovery_file_with_agents(
    repo_root: &Path,
    session_name: &str,
    agents: Vec<session::RepoAgentEntry>,
    out: &mut dyn Output,
) {
    let file = session::RepoSessionFile {
        session_name: session_name.to_string(),
        agents,
        orchestrator: None,
    };
    if let Err(e) = session::write_repo_session_file(repo_root, &file) {
        out.warn(&format!(
            "warning: failed to write per-repo session discovery file: {e}"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    // Byte-identical safety net (D1 / task 1.3): each per-backend boot-prompt
    // framing sentence, relocated from a compiled match arm into its own
    // asset, must render exactly as before.
    #[test]
    fn build_task_prompt_per_backend_is_byte_identical_to_ground_truth() {
        use crate::specs::{SpecBackendKind, SpecEntry};
        let mk = |backend: SpecBackendKind| SpecEntry {
            id: "my-change".to_string(),
            backend,
            branch: "feat/my-change".to_string(),
            cli: None,
            prompt: String::new(),
            owned_files: None,
        };
        let openspec = build_task_prompt(Some(&mk(SpecBackendKind::OpenSpec)));
        assert_eq!(
            openspec.prose,
            "Read .git-paw/AGENTS.local.md first — it carries your assignment and \
             coordination rules. Your workflow command follows as the next message; run it \
             once you have read the sidecar."
        );
        assert_eq!(
            openspec.entry_point.as_deref(),
            Some("/opsx:apply my-change")
        );
        assert_eq!(
            build_task_prompt(Some(&mk(SpecBackendKind::Markdown))).prose,
            "Begin your assigned task. Read .git-paw/AGENTS.local.md first — it carries the \
             project rules, your full spec, and your assignment. Additional artifacts \
             (proposal, design, specs, tasks) live under openspec/changes/my-change/ — read \
             them all before starting."
        );
        assert_eq!(
            build_task_prompt(Some(&mk(SpecBackendKind::SpecKit))).prose,
            "Begin your assigned task. Read .git-paw/AGENTS.local.md first — it already \
             carries the project rules and your full Spec Kit assignment (feature \
             spec, implementation plan, and task phase) embedded inline; no other \
             artifact directory is needed to start."
        );
        assert_eq!(
            build_task_prompt(Some(&mk(SpecBackendKind::Superpowers))).prose,
            "Begin your assigned task. Read .git-paw/AGENTS.local.md first — it carries the \
             project rules and your full superpowers plan (goal, tasks, exact file \
             paths, and per-step verification commands). Work the steps in order and \
             flip `- [ ]` to `- [x]` in the plan as each one lands."
        );
    }

    /// Maps to `supervisor-launch` scenario "Launch flow sends one Enter per
    /// submitted line": only a backend with an apply workflow declares an
    /// entry point, so only its pane is ever typed into twice.
    #[test]
    fn only_the_openspec_backend_declares_an_entry_point() {
        use crate::specs::{SpecBackendKind, SpecEntry};
        let mk = |backend: SpecBackendKind| SpecEntry {
            id: "my-change".to_string(),
            backend,
            branch: "feat/my-change".to_string(),
            cli: None,
            prompt: String::new(),
            owned_files: None,
        };
        for backend in [
            SpecBackendKind::Markdown,
            SpecBackendKind::SpecKit,
            SpecBackendKind::Superpowers,
        ] {
            assert_eq!(
                build_task_prompt(Some(&mk(backend))).entry_point,
                None,
                "{backend:?} has no apply workflow and must declare no entry point"
            );
        }
        assert_eq!(
            build_task_prompt(None).entry_point,
            None,
            "an agent with no spec has no workflow to enter"
        );
    }

    // -----------------------------------------------------------------------
    // compose_launch_prompt — the single composition every injecting launch
    // path goes through (`boot-block`, "Launch prompts name the agent's
    // worktree root")
    // -----------------------------------------------------------------------

    fn worktree_root() -> std::path::PathBuf {
        std::path::PathBuf::from("/repo/.git-paw/worktrees/feat-x")
    }

    /// Maps to `supervisor-launch` scenario "The composed prompt ends with
    /// the worktree rule".
    #[test]
    fn composed_prompt_is_boot_then_prose_then_the_rule_on_blank_lines() {
        let composed = compose_launch_prompt(Some("BOOT"), Some("PROSE"), &worktree_root());
        let (head, rule) = composed
            .split_once("\n\nYour worktree root is ")
            .expect("the rule is the last blank-line-separated part; got:\n{composed}");
        assert_eq!(head, "BOOT\n\nPROSE");
        assert!(
            rule.starts_with("/repo/.git-paw/worktrees/feat-x."),
            "the rule names the agent's absolute worktree root; got: {rule}"
        );
    }

    /// Maps to `boot-block` scenario "A child worktree's prompt names its
    /// absolute root": the rule states the confinement, including `../`.
    #[test]
    fn the_worktree_rule_forbids_climbing_out_of_the_root() {
        let composed = compose_launch_prompt(None, Some("PROSE"), &worktree_root());
        assert!(
            composed.contains("/repo/.git-paw/worktrees/feat-x"),
            "got:\n{composed}"
        );
        assert!(composed.contains("../"), "got:\n{composed}");
        assert!(
            composed.ends_with(')') || composed.ends_with('.'),
            "the rule is last, so the prompt ends with its final sentence; got:\n{composed}"
        );
    }

    /// Maps to `boot-block` scenario "Boot block pre-filled without Enter":
    /// the bare pre-fill has no task prose, so the rule follows the boot
    /// block directly — with no empty paragraph where the prose would be.
    #[test]
    fn the_bare_pre_fill_joins_boot_block_straight_to_the_rule() {
        let composed = compose_launch_prompt(Some("BOOT"), None, &worktree_root());
        assert!(
            composed.starts_with("BOOT\n\nYour worktree root is "),
            "got:\n{composed}"
        );
        assert!(
            !composed.contains("\n\n\n"),
            "no blank part may be joined in; got:\n{composed}"
        );
    }

    /// A broker-less spec-driven launch injects no boot block; the prose and
    /// the rule still compose cleanly.
    #[test]
    fn a_missing_boot_block_leaves_prose_then_rule() {
        let composed = compose_launch_prompt(None, Some("PROSE"), &worktree_root());
        assert!(
            composed.starts_with("PROSE\n\nYour worktree root is "),
            "got:\n{composed}"
        );
    }

    /// GP-03b: a pane stuck on an unanswered dialog fails the launch loudly
    /// (`Err`) instead of proceeding to inject the boot block; every other
    /// outcome proceeds exactly as before.
    #[test]
    fn dialog_stuck_result_fails_loudly_only_for_dialog_stuck() {
        let err = dialog_stuck_result(
            crate::tmux::ReadinessOutcome::DialogStuck,
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
                crate::tmux::ReadinessOutcome::Ready,
                "paw-proj",
                2,
                "claude"
            )
            .is_ok()
        );
        assert!(
            dialog_stuck_result(
                crate::tmux::ReadinessOutcome::FellBack,
                "paw-proj",
                2,
                "claude"
            )
            .is_ok()
        );
    }

    // -----------------------------------------------------------------------
    // build_task_prompt — pure helper that constructs the per-agent task
    // prompt appended to the supervisor-mode boot block. Replaces the old
    // first-line-of-spec-body truncation (MILESTONE drift item 29).
    // -----------------------------------------------------------------------

    fn make_spec_entry(id: &str, prompt_body: &str) -> crate::specs::SpecEntry {
        // Default to the `Markdown` backend so the regression tests
        // (`task_prompt_with_spec_points_at_sidecar_and_includes_id`,
        // `task_prompt_does_not_include_spec_body_first_line`) keep exercising
        // the sidecar-pointer branch of `build_task_prompt`.
        // Tests that want the OpenSpec branch construct a `SpecEntry`
        // literal directly with `backend = SpecBackendKind::OpenSpec`.
        crate::specs::SpecEntry {
            id: id.to_string(),
            backend: crate::specs::SpecBackendKind::Markdown,
            branch: format!("feat/{id}"),
            cli: None,
            prompt: prompt_body.to_string(),
            owned_files: None,
        }
    }

    #[test]
    fn task_prompt_with_spec_points_at_sidecar_and_includes_id() {
        let entry = make_spec_entry("my-change", "## 1. First section\n\nbody body body");
        let prompt = build_task_prompt(Some(&entry)).prose;
        assert!(
            prompt.contains(crate::agents::SIDECAR_REL_PATH),
            "spec-derived task prompt should point at the sidecar, got: {prompt}"
        );
        assert!(
            prompt.contains("openspec/changes/my-change"),
            "spec-derived task prompt should include the spec id directory, got: {prompt}"
        );
    }

    #[test]
    fn task_prompt_without_spec_points_at_sidecar() {
        let prompt = build_task_prompt(None).prose;
        assert_eq!(
            prompt,
            "Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task.",
            "no-spec task prompt should point at the sidecar verbatim"
        );
    }

    /// Maps to `supervisor-launch` scenario "`OpenSpec` task prompt declares
    /// opsx:apply as its entry point".
    ///
    /// OVERRIDE (launch-prompt-entry-point): this test previously required
    /// the prose to CONTAIN `/opsx:apply <id>`. A Haiku worker narrated that
    /// embedded command instead of running it (GP-54), so the command moved
    /// out of the prose entirely and became the declared entry point the
    /// launch submits as its own composer line. The prose must now be free of
    /// it — a command still in the sentence is a command that can be
    /// described rather than invoked.
    #[test]
    fn task_prompt_openspec_backend_declares_opsx_apply_outside_the_prose() {
        let entry = crate::specs::SpecEntry {
            id: "my-change".to_string(),
            backend: crate::specs::SpecBackendKind::OpenSpec,
            branch: "feat/my-change".to_string(),
            cli: None,
            prompt: String::new(),
            owned_files: None,
        };
        let task_prompt = build_task_prompt(Some(&entry));
        let prose = &task_prompt.prose;
        assert!(
            prose.contains(crate::agents::SIDECAR_REL_PATH),
            "OpenSpec branch must point the agent at the sidecar, got: {prose}"
        );
        assert!(
            !prose.contains("/opsx:apply"),
            "the command must not be embedded in the prose, got: {prose}"
        );
        assert!(
            !prose.contains("openspec/changes/"),
            "OpenSpec branch must suppress the openspec/changes/ path prose, got: {prose}"
        );
        assert_eq!(
            task_prompt.entry_point.as_deref(),
            Some("/opsx:apply my-change"),
            "the command is declared as the entry point instead"
        );
    }

    #[test]
    fn task_prompt_markdown_backend_uses_sidecar_pointer() {
        let entry = crate::specs::SpecEntry {
            id: "my-feature".to_string(),
            backend: crate::specs::SpecBackendKind::Markdown,
            branch: "feat/my-feature".to_string(),
            cli: None,
            prompt: String::new(),
            owned_files: None,
        };
        let task_prompt = build_task_prompt(Some(&entry));
        let prompt = &task_prompt.prose;
        assert!(
            prompt.contains(crate::agents::SIDECAR_REL_PATH),
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
        assert_eq!(
            task_prompt.entry_point, None,
            "Markdown has no apply workflow to enter"
        );
    }

    #[test]
    fn task_prompt_without_spec_points_at_sidecar_verbatim() {
        // Regression: the `None` branch must produce the exact sidecar-pointer
        // fallback string byte-for-byte, even after the per-backend dispatch.
        let task_prompt = build_task_prompt(None);
        assert_eq!(
            task_prompt.prose,
            "Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task.",
            "no-spec task prompt should be the sidecar-pointer fallback verbatim"
        );
        assert_eq!(task_prompt.entry_point, None);
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
        let prompt = build_task_prompt(Some(&entry)).prose;
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
        use crate::agents::SIDECAR_REL_PATH;
        use crate::specs::{SpecBackendKind, SpecEntry};
        // Driven off `SpecBackendKind::ALL`, not a hand-written list: a new
        // variant must be covered here, not silently skipped.
        for backend in SpecBackendKind::ALL {
            let entry = SpecEntry {
                id: "x".to_string(),
                backend,
                branch: "feat/x".to_string(),
                cli: None,
                prompt: String::new(),
                owned_files: None,
            };
            let prompt = build_task_prompt(Some(&entry)).prose;
            assert!(
                prompt.contains(SIDECAR_REL_PATH),
                "spec-backed arm must point at the sidecar, got: {prompt}"
            );
        }
        assert!(
            build_task_prompt(None).prose.contains(SIDECAR_REL_PATH),
            "no-spec arm must point at the sidecar"
        );
    }

    // Maps to scenario "Spec-derived task prompt points at the sidecar and
    // includes spec id" — fixes the named-test coverage gap from
    // boot-prompt-full-body. (test-coverage-v0-5-0 task 2.1)
    #[test]
    fn build_task_prompt_spec_entry_contains_sidecar_and_spec_id() {
        let entry = make_spec_entry("governance-config", "## 1. Struct definitions\n\nBody.");
        let prompt = build_task_prompt(Some(&entry)).prose;
        assert!(
            prompt.contains(crate::agents::SIDECAR_REL_PATH),
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
        use crate::specs::{SpecBackendKind, SpecEntry};
        let entry = SpecEntry {
            id: "user-auth-T009".to_string(),
            backend: SpecBackendKind::SpecKit,
            branch: "task/x".to_string(),
            cli: None,
            prompt: String::new(),
            owned_files: None,
        };
        let prompt = build_task_prompt(Some(&entry)).prose;
        assert!(
            prompt.contains(crate::agents::SIDECAR_REL_PATH),
            "Spec Kit task prompt should still point at the sidecar, got: {prompt}"
        );
        assert!(
            !prompt.contains("openspec/changes/"),
            "Spec Kit task prompt must not direct the worker to the nonexistent \
             openspec/changes/ path, got: {prompt}"
        );
    }

    // Maps to boot-block scenario `No boot block is built for a supervisor
    // identity`. The only boot-block entry point a launch has is the
    // coding-agent one, which slugifies its argument — so even a branch
    // literally named `supervisor` boots as `agent-supervisor` and is never
    // handed the orchestrator's reserved id.
    #[test]
    fn no_boot_block_is_built_with_the_supervisor_identity() {
        let broker_url = "http://127.0.0.1:9119";
        for mcp_capable in [false, true] {
            let block = crate::skills::build_boot_block("supervisor", broker_url, mcp_capable);
            assert!(
                !block.contains("--agent supervisor "),
                "no boot block may publish as the reserved id; got:\n{block}"
            );
        }
        let helper_form = crate::skills::build_boot_block("supervisor", broker_url, false);
        assert!(
            helper_form
                .contains(".git-paw/scripts/broker.sh --agent agent-supervisor status booting"),
            "a branch named supervisor boots under its slugified id; got:\n{helper_form}"
        );
    }

    // Maps to scenario `Boot block withheld until the pane is CLI-ready` from
    // session-orchestration-robustness (G1). The blind pre-`send-keys`
    // fixed sleep was replaced by the launch-readiness gate: `supervisor`
    // MUST gate every prompt-bearing pane via `tmux::gate_pane_for_injection`
    // before injecting its boot block, rather than relying on a wall-clock
    // sleep that the v0.8.0 dogfood proved racy.
    #[test]
    fn supervisor_launch_gates_injection_on_readiness() {
        let src = include_str!("supervisor.rs");
        let cmd_start = src
            .find("pub fn supervisor(")
            .expect("supervisor signature present");
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
        assert_eq!(a, b, "must be deterministic");

        let src = include_str!("launch.rs");
        let needle = "pub(crate) fn build_task_prompt";
        let start = src.find(needle).unwrap_or_else(|| {
            panic!("build_task_prompt signature not found in commands/launch.rs")
        });
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

        // The selector `build_task_prompt` delegates to now holds the match,
        // so its source is part of this function's effective body — checking
        // only the three lines here would guard nothing.
        let selector = include_str!("../task_prompts.rs");
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
            assert!(
                !selector.contains(needle),
                "the task-prompt selector it delegates to must not contain `{needle}` either"
            );
        }
    }

    /// Standing guard for `boot-block` scenario "Every launch path carries
    /// the rule". The rule is appended in exactly one place
    /// ([`compose_launch_prompt`]), so the way a path can lose it is to build
    /// its prompt some other way. This pins the funnel: no launch path may
    /// reach [`submit_prompt_to_pane`] or `build_boot_inject_args` with a
    /// string it composed itself.
    ///
    /// A source check rather than five more tmux end-to-end tests: the claim
    /// is about which code path builds the prompt, and a grep answers that
    /// for every path at once — including ones a future change adds.
    #[test]
    fn every_injecting_path_composes_its_prompt_through_one_helper() {
        for (name, src) in [
            ("main.rs", include_str!("../main.rs")),
            ("commands/launch.rs", include_str!("launch.rs")),
            ("commands/start.rs", include_str!("start.rs")),
            ("commands/supervisor.rs", include_str!("supervisor.rs")),
            ("commands/add.rs", include_str!("add.rs")),
        ] {
            // The pre-change composition helper must be gone everywhere: it
            // joined boot block and prose with no worktree rule. Spelled in
            // two halves so this file's own source does not match the needle
            // it searches for.
            let retired = concat!("combine_boot_and", "_task_prompt");
            assert!(
                !src.contains(retired),
                "{name} still composes a prompt with `{retired}`, which carries \
                 no worktree rule"
            );
            // Any file that injects must also compose.
            let injects =
                src.contains("submit_prompt_to_pane(") || src.contains("build_boot_inject_args(");
            if injects {
                // Either it composes directly, or it takes a prompt from
                // `attach_agent`, which composes via `compose_agent_launch`.
                let composes = [
                    "compose_launch_prompt(",
                    "compose_agent_launch(",
                    "attach_agent(",
                ]
                .iter()
                .any(|needle| src.contains(needle));
                assert!(
                    composes,
                    "{name} injects a prompt but never composes one — every injecting \
                     path must go through compose_launch_prompt so it carries the \
                     worktree rule"
                );
            }
        }
    }
}

#[cfg(test)]
mod permission_seeding_removed_tests {
    //! `approval-command-safety` — "No permission grants are seeded into
    //! agent CLI settings files": `attach_agent` is the per-worktree
    //! provisioning primitive shared by `git paw start` (`supervisor`)
    //! and `git paw add`, and touches no tmux state — so it is exercised
    //! directly here rather than through a full session launch. The
    //! machinery that used to write a settings file / vendor directory
    //! (the curl-allowlist, dev-allowlist and worktree-allowlist seeders)
    //! has been deleted; these tests pin the resulting behaviour.

    use std::path::Path;
    use std::process::Command as StdCommand;

    use tempfile::TempDir;

    use crate::config::{BrokerConfig, CustomCli, WorktreeConfig, WorktreePlacement};
    use crate::skills::GateCommands;

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

        let attached = attach_agent(
            &ctx,
            "feat-a",
            None,
            0,
            &mut crate::commands::CaptureOutput::default(),
        )
        .expect("attach_agent succeeds");
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
        let attached = attach_agent(
            &ctx,
            "feat-b",
            None,
            0,
            &mut crate::commands::CaptureOutput::default(),
        )
        .expect("first attach succeeds");
        let wt_path = attached.entry.worktree_path.clone();

        // A user (or a pre-removal git-paw) wrote a settings file by hand.
        let settings_dir = wt_path.join(".claude");
        std::fs::create_dir_all(&settings_dir).unwrap();
        let settings = settings_dir.join("settings.json");
        let sentinel = r#"{"allowed_bash_prefixes":["my-custom-tool"]}"#;
        std::fs::write(&settings, sentinel).unwrap();

        // Re-attach the SAME branch — `create_worktree` is idempotent for an
        // existing worktree/branch pair, mirroring the recovery/resume path.
        attach_agent(
            &ctx,
            "feat-b",
            None,
            0,
            &mut crate::commands::CaptureOutput::default(),
        )
        .expect("second attach succeeds");

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

        let attached = attach_agent(
            &ctx,
            "feat-x",
            None,
            0,
            &mut crate::commands::CaptureOutput::default(),
        )
        .expect("attach_agent succeeds");

        let id = attached
            .entry
            .cli_session_id
            .as_deref()
            .expect("cli_session_id is assigned for claude");
        assert_v4_uuid_shape(id);
        assert!(
            attached
                .pane
                .launch_command
                .contains(&format!("--session-id {id}")),
            "pane command should carry --session-id <id>, got: {}",
            attached.pane.launch_command
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
                attach_agent(
                    &ctx,
                    branch,
                    None,
                    0,
                    &mut crate::commands::CaptureOutput::default(),
                )
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

        let attached = attach_agent(
            &ctx,
            "feat-codex",
            None,
            0,
            &mut crate::commands::CaptureOutput::default(),
        )
        .expect("attach_agent succeeds");

        assert_eq!(attached.pane.launch_command, "codex");
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

        let attached = attach_agent(
            &ctx,
            "feat-full-auto",
            None,
            0,
            &mut crate::commands::CaptureOutput::default(),
        )
        .expect("attach_agent succeeds");

        let flags_pos = attached
            .pane
            .launch_command
            .find("--dangerously-skip-permissions")
            .expect("approval flag present");
        let session_pos = attached
            .pane
            .launch_command
            .find("--session-id")
            .expect("session-id flag present");
        assert!(
            flags_pos < session_pos,
            "--dangerously-skip-permissions must precede --session-id, got: {}",
            attached.pane.launch_command
        );
    }
}
