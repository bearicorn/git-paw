## Why

A `[clis.<name>]` entry says *which binary* to run, never *what it is for*. Users already fake
intent with one entry per model tier (the dogfood `claude-oss-sonnet` shims), and the
orchestrator has no git-paw-run way to hand bounded, context-heavy work — exploration, the three
audit gates — to a cheaper or isolated agent: `orchestrator-gate-references` ships the gate briefs
and a vendor-neutral delegation contract, but no executor git-paw controls. The user decided
(2026-10-19) that a persona is not a vendor subagent file: it is a CLI pinned to a model plus a
role skill with an explicit purpose, defined in config, for workers and for the orchestrator's
one-shot delegates alike, with as many of each as the user wants.

## What Changes

- **`[profiles.<name>]`** — a named agent profile: `kind = "worker" | "delegate"`, a one-line
  `description`, `cli` (a detected CLI or `[clis.<name>]` key), optional `args` (model, effort —
  appended after the CLI's own `args`), `skill` (a role skill), `approval`, and for delegates
  `timeout_seconds`. Profiles merge across the global and repository files by name. No profiles
  configured ⇒ behavior is unchanged.
- **Worker profiles** launch as normal workers (pane, branch, worktree). Their role skill is
  rendered into the worker's managed `AGENTS.md` block *after* the coordination skill (which keeps
  the broker protocol); their `approval` overrides the default for that worker only. Selection:
  `--profile <name>` on `start` / `add`, `default_profile` (ranked just above `default_cli`
  wherever that is consulted), the CLI picker (lists worker profiles with their descriptions beside
  plain CLIs), and `paw_profile` in spec frontmatter. The worktree entry records the profile;
  recovery relaunches with it. A worker profile builds through the one launch builder, so it gets
  its CLI's operator isolation (`worker-operator-isolation`) like any worker, and its approval level
  drives the native-sandbox refusal, the full-auto warning and `git paw doctor`'s sandbox check.
- **Delegate profiles** run **headless, one-shot** for the orchestrator through
  `git paw delegate <name> --task …` (the only interface; no MCP tool, so the MCP server still never
  launches an agent CLI), reusing the escalation one-shot's machinery (`HeadlessInvocation` argv over
  the CLI record, `CommandRunner::spawn` / `SpawnSpec` with stdin, timeout with process-group kill,
  logging). The delegate runs at the repository root with no pane, branch or worktree; its
  stdout (capped) is the result. It gets its CLI's operator isolation (arguments and environment),
  as a worker does. Least privilege: `manual` by default, `full-auto` refused, root sessions only,
  no nesting, at most four read-only delegates at once (a writing delegate runs alone), and a run
  marker under which `git paw __authority`, `git paw __send` and the bundled `broker.sh` and
  `sweep.sh` refuse and the MCP server exposes no root-only tool — a delegate can never publish a
  verdict or any other broker message, or type into a worker.
- **Bundled delegate role skills** `explorer`, `spec-auditor`, `doc-auditor`, `security-auditor`;
  the auditors render the orchestrator package's gate references (one source, no copy).
- **The orchestrator's delegation contract routes to delegate profiles first**, then the CLI's own
  isolated subagents, then inline.
- **Usage is attributed per profile** (status JSON, drive-loop exit summary), delegate runs
  included.
- **`git paw init`** ships commented profile examples with placeholder values (no vendor model id,
  no toolchain).

## Capabilities

### New Capabilities

- `cli-agent-profiles`: the `[profiles]` table and its validation, worker-profile launch shaping
  and selection, delegate invocation (contract, bounds, privileges, result, logging, listing), and
  the profiles documentation.

### Modified Capabilities

- `cli-resolution`: *CLI resolution chain for spec-driven launches* gains the profile levels
  (`--profile`, `paw_profile`, `default_profile`).
- `cli-parsing`: `--profile` on `start` / `add`; the `delegate` subcommand.
- `cli-interactive-selection`: the CLI picker lists worker profiles.
- `session-state`: the worktree entry records the profile; recovery uses it; status JSON exposes the
  profile and per-profile usage.
- `supervisor-unattended-operation`: the exit summary's usage section groups by profile.
- `mcp-server`: a server inside a delegate run is never a root session. *No agent CLI invocation as
  inference backend* is unchanged.
- `skill-packages`: bundled delegate role packages.
- `skill-agent`: the orchestrator delegates to delegate profiles first.
- `core-init`: commented profile examples in the generated config.
- `broker-agent-helper`: `git paw __authority`, `git paw __send` and the bundled `broker.sh` and
  `sweep.sh` refuse inside a delegate run.
- `preflight-diagnostics`: `git paw doctor`'s sandbox-mode check evaluates each worker profile at
  its own approval level; its spec-CLI check resolves `paw_profile`.

## Impact

- **Code:** new `src/config/profiles.rs` (`ProfileEntry`, `ProfileName`, `ProfileKind`, validation,
  merge); new `src/delegate.rs` (pure document / admission / output decisions) and
  `src/commands/delegate.rs` (shell); `src/supervisor/escalation.rs` (`HeadlessInvocation` /
  `resolve_handoff` generalised to take profile `args` and an optional operator-isolation facet —
  the escalation path passes none) and `src/command_runner.rs` (`SpawnSpec` gains environment
  removal and separate stdout capture; `RealCommandRunner::spawn` honours them);
  `src/agent_cli/profile.rs` (`CliProfile::launch_command` takes the profile's `args` in the args
  slot); `src/interactive/{mod,resolver,prompter}.rs` (a worker choice is a CLI or a profile);
  `src/specs/{mod,openspec,markdown}.rs` (`paw_profile`, read only inside `src/specs/` as `paw_cli`
  is, so `tests/source_audit.rs`'s spec-format audit is unaffected); launch paths
  (`src/commands/{start,add,supervisor,recover}.rs` and `src/commands/launch.rs`'s `attach_agent` /
  `dry_run_launch_line`); `src/agents.rs` (role-skill section); `src/session.rs`
  (`WorktreeEntry.profile`); `src/commands/status.rs`; `src/supervisor/drive/budget.rs` (exit
  summary usage section); `src/doctor.rs` (`probe_sandbox_mode`, `probe_spec_cli`);
  `src/commands/authority.rs` and `src/commands/send.rs` (delegate-marker refusals);
  `src/mcp/mod.rs` (`RepoContext` root detection under the delegate marker); `src/skills.rs`
  (registry entries, gate-reference inclusion); `src/cli.rs` (`--profile`, the `Delegate`
  subcommand); `src/main.rs` (dispatch); `src/config/mod.rs` (template).
- **Assets:** `assets/agent-skills/{explorer,spec-auditor,doc-auditor,security-auditor}/SKILL.md`,
  `assets/delegate-framing.md`, the orchestrator `SKILL.md` delegation section,
  `assets/agent-skills/coordination/scripts/broker.sh` and
  `assets/agent-skills/orchestrator/scripts/sweep.sh` (delegate-marker refusal; the pre-package
  `assets/scripts/` paths are the ones `skill-package-layout` moves).
- **Persisted state:** additive `WorktreeEntry.profile`; delegate run log and ledger under
  `.git-paw/logs/delegates/` (gitignored).
- **Enum-variant ripple:** none (`BrokerMessage`, `SpecBackendKind` untouched). The clap `Command`
  enum gains `Delegate` (exhaustive dispatch in `src/main.rs`).
- **Struct-literal ripple (GP-40)** — none of these derive `Default`, so every literal breaks; re-grep
  before implementing:
  - `WorktreeEntry {` (`profile`): `src/session.rs`, `src/worktree_provision_tests.rs`,
    `src/broker/learnings.rs`, `src/init.rs`, `src/commands/{purge,start,helpers,broker_host,recover,launch}.rs`,
    `tests/{status_json_cli_session_id,cli_tests,init_config_backfill_e2e,agent_pane_liveness_e2e,broker_integration,worktree_port_allocation,status_json_usage,dashboard_render,worktree_placement_integration}.rs`.
  - `SpecEntry {` (`profile`): `src/specs/{mod,openspec,markdown,speckit,superpowers,resolve}.rs`,
    `src/git.rs`, `src/commands/launch.rs`, `src/interactive/resolver.rs`.
  - `WorktreeAssignment {` (role section): `src/agents.rs`, `src/commands/{start,supervisor,launch}.rs`,
    `tests/git_integration.rs`.
  - `SelectionResult {` (`mappings` type): `src/interactive/{mod,resolver}.rs`.
  - `HeadlessInvocation {`: `src/supervisor/{escalation,drive}.rs`; `SpawnSpec {`:
    `src/command_runner.rs`, `src/supervisor/escalation.rs`.
  - `StartOptions {`, `StartWithSpecsOptions {`, `SupervisorOptions {` (`profile`): `src/main.rs`,
    `src/mcp/tools/session.rs`, `src/commands/{start,supervisor}.rs`,
    `tests/orchestrator_launch_in_process.rs`; `AddOptions {` (derives `Default`): `src/main.rs`,
    `src/commands/add.rs`; `AttachContext {`: `src/commands/{supervisor,add,launch}.rs`.
  - `SandboxModeProbe {` / `WorkerSandboxProbe {` / `SpecCliProbe {` if their shape changes:
    `src/doctor.rs`, `src/doctor_tests.rs`.
  - `PawConfig {` derives `Default`; only literals without `..` break (`src/config/{mod,tests}.rs`,
    `src/specs/mod.rs`, `src/interactive/resolver.rs`, `src/commands/mod.rs`, `src/main.rs`,
    `src/supervisor/escalation.rs`, `tests/{superpowers_integration,dashboard_render,speckit_integration}.rs`).
  - `Command::Start {` / `Command::Add {` patterns without `..`: `src/cli.rs`, `src/main.rs`.
- **Security / safety:** a delegate is an agent with execution power started on another agent's
  request — reviewed with `security-and-safety-review` (see design D7–D9).
- **Dependencies:** none.
- **Order in v0.20.0:** last — after `skill-package-layout` → `skill-override-resolution` →
  `skill-package-deploy` → `per-cli-skill-templates` → `orchestrator-gate-references`, and after
  `worker-operator-isolation` (independent of the skill chain). It extends
  `orchestrator-gate-references`' delegation contract and renders its gate references, and it
  applies `worker-operator-isolation`'s `operator_isolation()` facet to delegate runs (worker
  profiles get it through the shared launch builder with no extra work). Coordination point: the
  gate-references contract says the orchestrator runs gates inline "when the CLI cannot run isolated
  subagents"; this change's `skill-agent` requirement *A delegate profile counts as an isolated
  subagent* makes a matching delegate profile satisfy that condition, as an ADDED requirement that
  names the gate-references requirements it extends (they are not archived yet, so they cannot be
  MODIFIED here). Its vendor-neutrality rule is not restated: gate-references' *The delegation
  section names no vendor* covers the extended section.
- **Cross-version dependencies:**
  - v0.17.0: `status-and-picker-truth` (archived; `default_cli` pre-selects in both picker modes
    and a supervised launch honours the picked CLI — `default_profile` and profile picks follow the
    same rules), `cli-launch-identity` (archived; the resolved `command` is launched),
    `start-spec-launch-parity` (archived; spec-launch approval flags), `test-session-state-isolation`
    (test isolation pattern).
  - v0.18.0: `orchestrator-escalation-handoff` (headless machinery — `HeadlessInvocation`,
    `HandoffSetup::spawn_spec`, `CommandRunner::spawn` — reused and generalised);
    `mcp-orchestrator-write-tools` (root-session detection, root-only tools — its MODIFIED
    *bounded write surface* is left alone; this change ADDs a separate requirement);
    `broker-verdict-authority` (`__authority`); `orchestrator-skill-export` (the orchestrator
    skill); `cmd-library-surface` (launch paths move).
  - v0.19.0 (shipped): `cli-provider-record` (`CliProfile` record, `[clis].args`, family-keyed
    headless template, the one launch builder `CliProfile::launch_command` — profile `args` extend
    its args slot); `cli-profile-threading` (`agent_cli::AgentProfiles`, the per-agent recorded-CLI
    map: a profiled worker records its profile's resolved CLI id in `cli`, so the drive loop keeps
    resolving its prompt profile unchanged); `worker-usage-budget` (usage facet, status JSON
    `usage` / `usage_total`, `[supervisor] budget_max_*`, the exit-summary usage section in
    `src/supervisor/drive/budget.rs` — extended, not modified; delegate runs are not counted toward
    the budget, see Open Questions); `worker-sandbox-mode` (`[clis].sandbox` / `sandbox_args`,
    `resolve_for_launch`'s refusal and full-auto warning, and `git paw doctor`'s sandbox-mode check —
    each now evaluated at a worker profile's own level, `preflight-diagnostics` delta);
    `verified-agent-send` (`git paw __send`, the backend of `sweep.sh tell`, guarded only by its
    linked-worktree refusal — a delegate at the root could call it directly, so it refuses under the
    delegate marker too); `spec-backend-strategy` (`paw_profile` is parsed inside the OpenSpec and
    Markdown backends next to `paw_cli`, never by a per-format match outside `src/specs/`).
  - v0.20.0: `worker-operator-isolation` (the `operator_isolation()` provider facet and
    `[clis.<name>].inherit_operator_config`; lands first).
  - v0.21.0: `supervisor-vocabulary-split` introduces `[workers]`, `[orchestrator]`, `[drive]` —
    `[profiles]` stays a separate top-level table, outside their one-unit merge, so it stays clean;
    a worker profile's default approval becomes `[workers] approval`. `config-schema-versioning`
    carries `[profiles]` and `default_profile` unchanged in version 2. `dashboard-usage-view` should
    label a worker's tier with its profile and add a delegates row.
