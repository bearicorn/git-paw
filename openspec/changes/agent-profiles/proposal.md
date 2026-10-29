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
  recovery relaunches with it.
- **Delegate profiles** run **headless, one-shot** for the orchestrator through
  `git paw delegate <name> --task …` (the only interface; no MCP tool, so the MCP server still never
  launches an agent CLI), reusing v0.18.0's headless machinery (`headless_args`, context on stdin, timeout with
  process-group kill, logging). The delegate runs at the repository root with no pane, branch or
  worktree; its stdout (capped) is the result. Least privilege: `manual` by default, `full-auto`
  refused, root sessions only, no nesting, at most four read-only delegates at once (a writing
  delegate runs alone), and a run marker under which `git paw __authority` and the bundled
  `broker.sh` and `sweep.sh` refuse and the MCP server exposes no root-only tool — a delegate can never publish a
  verdict or any other broker message.
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
- `broker-agent-helper`: `git paw __authority` and the bundled `broker.sh` and `sweep.sh` refuse
  inside a delegate run.

## Impact

- **Code:** new `src/config/profiles.rs` (`AgentProfile`, `ProfileName`, `ProfileKind`, validation,
  merge); new `src/delegate.rs` (pure document / admission / output decisions) and
  `src/commands/delegate.rs` (shell); the headless spawn path of v0.18.0 `orchestrator-escalation-handoff`
  (`src/supervisor/escalation.rs`) extracted into a shared runner; `src/interactive/{mod,resolver,prompter}.rs`
  (a worker choice is a CLI or a profile); `src/specs/{mod,openspec,markdown}.rs` (`paw_profile`);
  launch paths (`src/commands/{start,add,supervisor,recover}.rs`, `src/main.rs::attach_agent`, or
  their v0.18.0 library home); `src/agents.rs` (role-skill section); `src/session.rs`
  (`WorktreeEntry.profile`); `src/commands/status.rs`; `src/supervisor/drive.rs` (summary);
  `src/mcp/mod.rs` (`RepoContext` root detection under the delegate marker); `src/skills.rs` (registry entries, gate-reference
  inclusion); `src/cli.rs`; `src/config/mod.rs` (template).
- **Assets:** `assets/agent-skills/{explorer,spec-auditor,doc-auditor,security-auditor}/SKILL.md`,
  `assets/delegate-framing.md`, the orchestrator `SKILL.md` delegation section.
- **Persisted state:** additive `WorktreeEntry.profile`; delegate run log and ledger under
  `.git-paw/logs/delegates/` (gitignored).
- **Enum-variant ripple:** none (`BrokerMessage`, `SpecBackendKind` untouched). `SpecEntry` gains a
  field, set by every backend constructor.
- **Security / safety:** a delegate is an agent with execution power started on another agent's
  request — reviewed with `security-and-safety-review` (see design D7–D9).
- **Dependencies:** none.
- **Order in v0.20.0:** last — after `skill-package-layout` → `skill-override-resolution` →
  `skill-package-deploy` → `per-cli-skill-templates` → `orchestrator-gate-references`. It extends the
  latter's delegation contract and renders its gate references.
- **Cross-version dependencies:**
  - v0.17.0: `status-and-picker-truth` (archived; `default_cli` pre-selects in both picker modes
    and a supervised launch honours the picked CLI — `default_profile` and profile picks follow the
    same rules), `cli-launch-identity` (archived; the resolved `command` is launched),
    `start-spec-launch-parity` (archived; spec-launch approval flags), `test-session-state-isolation`
    (test isolation pattern).
  - v0.18.0: `orchestrator-escalation-handoff` (headless machinery, reused and extracted);
    `mcp-orchestrator-write-tools` (root-session detection, root-only tools — its MODIFIED
    *bounded write surface* is left alone; this change ADDs a separate requirement);
    `broker-verdict-authority` (`__authority`); `orchestrator-skill-export` (the orchestrator
    skill); `cmd-library-surface` (launch paths move).
  - v0.19.0: `cli-provider-record` (`CliProfile` record, `[clis].args`, family-keyed headless
    template, the one launch builder — profile `args` extend its args slot); `worker-usage-budget`
    (usage facet, status JSON usage, exit-summary usage section, both extended, not modified);
    `worker-sandbox-mode` (its full-auto-without-sandbox doctor warning must evaluate each worker
    profile's effective level).
  - v0.21.0: `supervisor-vocabulary-split` introduces `[workers]`, `[orchestrator]`, `[drive]` —
    `[profiles]` stays a separate top-level table, outside their one-unit merge, so it stays clean;
    a worker profile's default approval becomes `[workers] approval`. `config-schema-versioning`
    carries `[profiles]` and `default_profile` unchanged in version 2. `dashboard-usage-view` should
    label a worker's tier with its profile and add a delegates row.
