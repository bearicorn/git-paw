## Context

Today a worker is chosen by CLI id only. `config::PawConfig` has `default_cli`, `default_spec_cli`,
`clis: HashMap<String, CustomCli>` and `presets` (`Preset { branches, cli }`); `merged_with` merges
`clis` / `presets` by key. Selection is split across paths: `interactive::run_selection` returns
`SelectionResult.mappings: Vec<(String, String)>` (branch, CLI id); `interactive::resolve_cli_for_specs`
runs the 5-level spec chain (`SpecEntry.cli` comes from `paw_cli` frontmatter, read only by the
OpenSpec and Markdown backends); `commands/add.rs` falls back `--cli` > `spec.cli` > first worktree's
`cli` > `default_cli`; `commands/supervisor.rs` computes `agent_cli` (`--cli` > `default_cli` >
supervisor CLI). The worker's managed block comes from `agents::WorktreeAssignment.skill_content`
(the rendered `coordination` skill) via `setup_worktree_agents_md`. `session::WorktreeEntry` records
`cli` and `cli_session_id`. There is no "what is this agent for" anywhere.

The pieces this change composes land earlier: v0.19.0 `cli-provider-record` gives each CLI a
resolved `CliProfile` with `args`, a family, a family-keyed headless template and one launch builder
(`CliProfile::launch_command`); v0.18.0 `orchestrator-escalation-handoff` spawns a headless one-shot
through `CommandRunner` (stdin context, process-group timeout, log) in `src/supervisor/escalation.rs`;
v0.18.0 `mcp-orchestrator-write-tools` detects root sessions and gates root-only tools;
`broker-verdict-authority` adds `git paw __authority`; v0.20.0 `orchestrator-gate-references` moves the
three audit gates into `references/gates/*.md` and adds a vendor-neutral delegation contract;
`skill-override-resolution` makes `skills::resolve(name, repo_root)` reach repository and user skills.

## Goals / Non-Goals

**Goals:**
- A config entry that carries intent: CLI + launch args + role skill + purpose, any number of them.
- Worker profiles selectable everywhere a CLI is selectable; recorded and recovered.
- Delegate profiles: an executor git-paw controls for the delegation contract, least-privilege by
  default, run through `git paw delegate`.
- Per-profile usage attribution.
- Nothing changes for a config without profiles.

**Non-Goals:**
- Vendor agent-definition files (no `.claude/agents/` or equivalents; v0.18.0 writes into no vendor
  directory).
- Profiles in `[presets]`, in MCP `start_session`, or a `default_spec_profile` (Open Questions).
- Delegates that act on a worktree or a branch, or that publish broker messages.
- Budget enforcement on delegate runs (Open Questions).
- MCP tools for delegates (D10): the command is the only way to run one.
- Per-CLI fragments for role skills (only the coordination package has them).

## Decisions

### D1 — `[profiles.<name>]`, one table, a required `kind`

One table keeps one namespace, so `--profile`, `paw_profile` and `default_profile` are unambiguous,
and `kind` is required so intent is explicit and a worker can never be run as a delegate by accident.
The table is top-level and merges by name like `[clis]`.
*Naming:* `[workers.<name>]` is ruled out — v0.21.0 `supervisor-vocabulary-split` creates `[workers]`
with its own sub-tables (such as `[workers.tell]`) and merges `[workers]` / `[orchestrator]` /
`[drive]` as one unit, so profiles there would collide and a repository `[workers]` table would hide
every global profile. `[agents]` is ruled out because "agent" already means a running worker
(`agent_id`, `agent.status`). `[profiles]` stays clean after v0.21.0. "Profile" is already used for
the prompt-shape profile (`[clis.<name>].prompt_profile`), v0.19.0's internal `CliProfile` and v1.1.0's
repository profile DTO; the Rust type is `AgentProfile`, docs say "agent profile", and the user-facing
key `profiles` does not clash with any existing key. *Alternatives:* two tables
(`[worker_profiles]`, `[delegate_profiles]`) — rejected, two namespaces for one concept; `[personas]` —
rejected, carries the vendor-file meaning the user decision retired. **Decision for the user.**

### D2 — The second kind is `delegate`, not `helper`

"Helper" already names the bundled helper scripts (`broker.sh`, `sweep.sh`, the `broker-agent-helper`
capability, the `git paw setup-cli` helper grant table, `{{BROKER_HELPER}}` placeholders). `git paw helper`
next to `.git-paw/skills/*/scripts/` helpers would be read as a script runner. "Delegate" is the verb
`orchestrator-gate-references`' contract already uses. Command `git paw delegate`, marker
`GIT_PAW_DELEGATE`. **Decided 2026-10-19.**

### D3 — A profile references a CLI and adds its own `args`

`cli` names a detected CLI or a `[clis.<name>]` key; the profile's `args` are appended right after the
CLI's own `args` in v0.19.0's one launch builder (the same slot, extended), so sandbox arguments,
approval flags and session-id / headless arguments keep their order. One `[clis.claude]` can serve
`routine`, `review` and `explorer` profiles with different models. *Alternative:* the profile only
names a `[clis.*]` entry that carries the model — rejected: every intent would need two entries, and
the CLI entry would again encode intent.

### D4 — Role skills: additive for workers, required for delegates, never the orchestrator

A worker's `coordination` skill carries the broker protocol the boot block relies on, so a worker role
skill is rendered *after* it, never instead of it; `skill` absent or `coordination` adds nothing. A
delegate needs a role to be anything but "an agent with a task", so `skill` is required. Role skills
resolve through `skills::resolve(name, repo_root)` (repository `.agents/skills/`, user config dir,
embedded), text only. `orchestrator` / `supervisor` is refused for both kinds: the orchestrator package
never reaches a worktree, and a headless orchestrator is the escalation one-shot's job.

### D5 — Selection mirrors `default_cli` exactly

A worker choice becomes `WorkerChoice::{Cli(CliId), Profile(ProfileName)}` and every selection
function returns it (`SelectionResult.mappings: Vec<(String, WorkerChoice)>`), so a profile and a CLI
of the same name stay distinct. Precedence: flag (`--profile` xor `--cli`) > spec (`paw_profile` xor
`paw_cli`) > `default_spec_cli` > picker pre-selecting `default_profile`, else `default_cli`; path
fallbacks without a picker use `default_profile` before `default_cli`; `add` inherits the first
worktree's profile. This follows v0.17.0 `status-and-picker-truth` (pending), which makes
`default_cli` pre-select in both picker modes and makes a supervised launch honour the pick.
*Alternative:* `default_profile` skips the picker — rejected as a second, different meaning of
"default"; `--profile` or `default_spec_cli` already skip it. `paw_profile` is read where `paw_cli`
is (OpenSpec, Markdown); Spec Kit and Superpowers read neither.

### D6 — Delegates reuse the escalation one-shot's runner

Extract from `src/supervisor/escalation.rs` a shared headless runner: a pure `HeadlessInvocation`
(argv, env additions and removals, cwd, stdin document, timeout) built from the `CliProfile`, and a
shell that spawns it through `CommandRunner` with a new process group, stdin, captured stdout,
stderr to a log, and process-group kill on timeout. The escalation path keeps its behaviour (it
still discards stdout and runs in the background); the delegate path waits and returns stdout. Pure
decisions live in `src/delegate.rs` — document assembly, admission, output truncation, ledger line;
`src/commands/delegate.rs` is the shell. Argv order: executable, CLI `args`, profile `args`, v0.19.0
sandbox args when declared (a delegate is never less confined than a worker on the same CLI),
approval flags, session-id args (when the CLI has a template — so usage is attributable), headless
args.

### D7 — Least privilege: what a delegate can and cannot do

A delegate is an agent with execution power, started on another agent's request, at the repository
root (outside every worktree), with no pane for the drive loop to watch. It also runs where the
repository's `.claude/settings.local.json` applies, so any helper grant the user applied with
`git paw setup-cli` (v0.17.0) reaches it too. So:
- **Level:** `manual` by default — git-paw passes no permission-widening flag, and the CLI's own
  non-interactive mode decides what runs. `auto` is the opt-in for a role that edits. `full-auto` is
  refused at load: an orchestrator could otherwise launder full execution power through a delegate.
- **Where:** main working tree only (refused from linked worktrees, so a worker cannot use a delegate
  to reach outside its worktree); no nesting.
- **Authority:** the run carries `GIT_PAW_DELEGATE`; `git paw __authority` refuses under it, the
  bundled `broker.sh` and `sweep.sh` refuse under it (at the root they would otherwise read the
  broker address from `.git-paw/config.toml`, and a `setup-cli` grant lets them run without a
  prompt, so a delegate could publish as any agent, publish orchestrator status or learnings, or
  type into a worker's pane), and an MCP server started under it is never a root session (no verdict or
  lifecycle tools); no broker URL is passed. This is defence in depth: a delegate allowed to run shell commands could unset the
  variable, so the documented boundary is the approval level plus, where configured, v0.19.0's
  `[clis.<name>].sandbox`. The posture chapter says so.
- **Input:** the task travels on stdin, never argv; profile names are validated (`ProfileName`) before
  they reach a log path.
- **No prompt relay:** no pane, so the drive loop never sees, escalates or answers a delegate's
  permission prompts; git-paw approves nothing for any agent (v0.17.0 `classifier-retirement`), and
  under `manual` the CLI's own non-interactive mode decides what runs.

### D8 — Concurrency: four readers or one writer, refused not queued

Gate fan-out needs the three audits at once, plus an explorer, so up to four `manual` runs run
together; an `auto` run can edit the main checkout and so runs alone (the escalation one-shot
serialises for the same reason). A refused run fails immediately: queueing would hold a CLI call open
for an unbounded time. Active runs are records under
`.git-paw/logs/delegates/active/` (already gitignored) holding the PID and level, taken and released under the `lock::SessionLock`-style `create_new` lock; dead PIDs
are ignored. `admit(active, level) -> Admit | Refuse(reason)` is pure. The limits are constants, not
configuration. *Alternatives:* serialize every run like the escalation one-shot — rejected, it defeats
the gate fan-out; unbounded — rejected, a rogue orchestrator could spawn without limit.

### D9 — The result is stdout, capped

The point of a delegate is to keep raw output out of the orchestrator's context, so the returned
result is stdout capped at 64 KiB with a pointer to the full log; stderr only goes to the log. The
ledger (`runs.jsonl`) holds metadata only — no task or result — and is what usage attribution reads.
`.git-paw/logs/` is already gitignored.

### D10 — No MCP delegate tools; the command is the interface

`mcp-server`'s *No agent CLI invocation as inference backend* forbids any tool from spawning an agent
CLI, and it stays unchanged: there is no `list_delegates` / `run_delegate`. An orchestrator delegates
by running `git paw delegate` from its own shell, which it already uses for the helpers. **Decided
2026-10-19** (the user chose the command-only option). *Rejected alternative:* a root-only
`run_delegate` tool as one named exception to the guardrail. It would let an orchestrator driving
through MCP alone (phone, remote control) delegate, at the cost of the guardrail and of MCP client
tool-call timeouts on long audits. It can be added later without changing the command. The only MCP
change is the delegate-run marker making a server non-root (D7).

### D11 — Auditor packages include the gate references, they do not copy them

`spec-auditor`, `doc-auditor`, `security-auditor` are thin packages: frontmatter, a role preamble
(read-only, compressed verdict, never publish) and an inclusion the registry resolves to the rendered
orchestrator reference. One source per gate procedure; a scan test enforces it. `explorer` is
self-contained. All four are installed at the root like every bundled package
(`skill-package-deploy`) and never selected for a worktree.

### D12 — Usage attribution

Workers: `WorktreeEntry.profile` groups v0.19.0's per-worktree `usage`. Delegates: each ledger line
carries the CLI session id git-paw assigned, resolved through the provider's usage facet. Status JSON
gains `usage_by_profile`; the exit summary gains per-profile lines only when a profile was used, so
v0.19.0's section is otherwise unchanged.

## Risks / Trade-offs

- [A CLI's headless mode runs tools without asking even with no flags] → the posture chapter and the
  profiles chapter state the per-family behaviour as verified at implementation (**to verify**:
  Claude Code `-p` and Codex `exec` default permission behaviour); recommend a declared sandbox.
- [A delegate's MCP server does not inherit the environment] → then the root-only tools would be
  advertised to it; **to verify** per CLI how stdio MCP servers inherit the environment; the
  `__authority` refusal and the delegate's approval level still apply.
- [A stale run record blocks admission after a crash] → PID liveness check; records of dead PIDs are
  dropped on the next admission.
- [Profile `approval = "full-auto"` on workers widens one worker] → same trust as `agent_approval`
  (operator config); v0.19.0 `worker-sandbox-mode`'s doctor warning evaluates each worker profile.
- [Profile/CLI name overlap confuses users] → picker shows profiles with their description and CLI;
  flags are separate.

## Migration Plan

1. Config types, validation, merge, `default_profile` (no behaviour change without profiles).
2. `WorkerChoice` through the selection functions and the spec chain; picker items.
3. Launch shaping (args, approval, role skill), `WorktreeEntry.profile`, recovery.
4. Extract the headless runner from the escalation path (behaviour-preserving).
5. `git paw delegate`, admission, ledger, `__authority` and MCP root-gating under the marker.
6. Delegate role packages; orchestrator delegation section; usage attribution; init template; docs.
Rollback: revert. Session files with `profile` load in older binaries (unknown field ignored, the
recorded `cli` still launches).

## Open Questions

- Names: `[profiles]` + `kind`, capability `cli-agent-profiles` (D1) — confirm.
- Should delegate runs count toward `[supervisor] budget_max_*` and be refused after a breach?
  Recommended yes, as a follow-up once v0.19.0's budget lands.
- `profile` in `[presets]` and in v0.18.0's MCP `start_session`: recommended follow-ups.
- Concurrency constants (4 readers, 1 writer) and the 600 s / 64 KiB defaults — confirm.
