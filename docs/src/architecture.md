# Architecture

This chapter covers git-paw's internal architecture: module structure, data flow, and key design decisions.

It describes the code as it is today. For the structure the code is moving toward — and the rules new code follows — see [Target Architecture](architecture-direction.md).

## Module Diagram

```
┌───────────────────────────────────────────────────────────────────────┐
│                    main.rs  (entry point, dispatch)                   │
│                  commands/  (per-subcommand handlers)                 │
├──────────┬──────────────┬──────────────┬─────────────┬────────────────┤
│  cli.rs  │ interactive/ │   config/    │   error.rs  │    dirs.rs     │
│  (clap)  │  (prompts,   │    (TOML)    │  (PawError) │     (XDG)      │
│          │   pickers)   │              │             │                │
├──────────┴──────────────┴──────────────┴─────────────┴────────────────┤
│                                                                       │
│  detect.rs    git.rs       session.rs   logging.rs   agents.rs        │
│  (PATH scan)  (worktrees)  (JSON state) (pane logs)  (AGENTS.md)      │
│                                                                       │
│  skills.rs    init.rs      replay.rs    selftest.rs  doctor.rs        │
│  (skill       (project     (log         (smoke       (preflight       │
│   templates)   bootstrap)   playback)    check)       diagnostics)    │
│                                                                       │
│  domain.rs    lock.rs      random.rs    command_runner.rs             │
│  (newtypes)   (add/remove  (UUID v4)    (process-execution seam)      │
│                lock)                                                  │
│                                                                       │
│  worktree_provision.rs       dashboard.rs + dashboard/                │
│  (env copies, ports, hooks)  (TUI: model, view, log panel)            │
├───────────────────────────────────────────────────────────────────────┤
│  broker/               supervisor/               specs/               │
│  ├── mod.rs            ├── mod.rs                ├── mod.rs           │
│  ├── server.rs         ├── approval_gate.rs      ├── openspec.rs      │
│  ├── messages.rs       ├── approve.rs            ├── markdown.rs      │
│  ├── delivery.rs       ├── auto_approve.rs       ├── speckit.rs       │
│  ├── conflict.rs       ├── branch_refresh.rs     ├── superpowers.rs   │
│  ├── learnings.rs      ├── claim.rs              └── resolve.rs       │
│  ├── watcher.rs        ├── dev_allowlist.rs                           │
│  └── publish.rs        ├── drive.rs              opsx/                │
│                        ├── layout.rs             ├── mod.rs           │
│                        ├── manual_approvals.rs   └── role_guard.rs    │
│                        ├── permission_prompt.rs                       │
│                        ├── poll.rs               coordination/        │
│                        └── stall.rs              ├── mod.rs           │
│                                                  ├── inventory.rs     │
│                                                  └── tell.rs          │
├───────────────────────────────────────────────────────────────────────┤
│  tmux/                 mcp/                                           │
│  ├── mod.rs            ├── mod.rs                                     │
│  ├── command.rs        ├── server.rs                                  │
│  ├── layout.rs         ├── logging.rs                                 │
│  ├── readiness.rs      ├── query/   (data-layer reads)                │
│  └── session.rs        └── tools/   (MCP tool surfaces)               │
└───────────────────────────────────────────────────────────────────────┘
```

### Module Responsibilities

| Module | File | Purpose |
|--------|------|---------|
| **Entry point** | `src/main.rs`, `src/lib.rs` | `main.rs` parses arguments and dispatches each subcommand to its handler; it still holds the `purge` handler and the hidden `__dashboard` / `__classify` / `__prompt-profile` subcommands. `lib.rs` is the library target the binary and the integration tests share — internal, not a stable API. |
| **Commands** | `src/commands/` | One handler module per subcommand: `start.rs` (fresh launch, reattach, restart-from-pause, spec-driven launch), `supervisor.rs` (`cmd_supervisor`, the unattended drive-loop wiring, the dashboard auto-approve thread), `recover.rs`, `add.rs`, `remove.rs`, `pause.rs`, `stop.rs`, `status.rs`, `attach.rs`, `approvals.rs`, `replay.rs`, `clis.rs` (`list-clis` / `add-cli` / `remove-cli`), `completions.rs`, plus shared `helpers.rs`. |
| **CLI** | `src/cli.rs` | Argument parsing with clap v4 derive macros. Defines all subcommands, flags (`--from-all-specs`, `--specs`, `--specs-format`, `--supervisor`, `--no-supervisor`, `--force`, ...), and help text. |
| **Detection** | `src/detect.rs` | Scans `PATH` for known AI CLI binaries (`KNOWN_CLIS`). Resolves custom CLIs from config. Merges and deduplicates. |
| **Git** | `src/git.rs` | Validates git repos, lists branches (local + remote, deduplicated), creates/removes worktrees, derives safe directory names. |
| **Tmux** | `src/tmux/` | Tmux orchestration. `command.rs` holds the `TmuxSessionBuilder` plus the supervisor-session, add-agent, and remove-retile command builders (pure argv assembly, pane titles, the supervisor-as-pane layout); `layout.rs` the equal-width agent-row rebalance; `readiness.rs` the launch-readiness gate; `session.rs` runtime operations on live sessions (liveness probes, session-name resolution, attach/detach, pane and session teardown, pane-to-worktree reconciliation). |
| **Session** | `src/session.rs` | Persists session state to JSON files under `~/.local/share/git-paw/sessions/`. Atomic writes, crash recovery. Also writes the per-repo discovery file (see below). |
| **Config** | `src/config/` | Parses TOML from global (`~/.config/git-paw/config.toml`) and per-repo (`.git-paw/config.toml`). Merges with repo-wins semantics. `mod.rs` holds `PawConfig` and loading; one file per section: `broker.rs`, `cli.rs` (custom CLIs and presets), `dashboard.rs`, `layout.rs`, `prompt_profile.rs` (per-CLI prompt-shape profiles), `specs.rs`, `supervisor.rs`. |
| **Interactive** | `src/interactive/` | Terminal prompts, split by altitude behind the `Prompter` trait seam: `resolver.rs` (terminal-free selection logic, `PickerState`, spec grouping), `prompter.rs` (the dialoguer `TerminalPrompter`), `picker.rs` (the ratatui/crossterm fuzzy multi-select and its terminal lifecycle). The branch and spec multi-selects are fuzzy-filter pickers (type to filter, space to toggle); the mode and CLI single-selects use dialoguer. Skips prompts when flags are provided. |
| **Error** | `src/error.rs` | `PawError` enum with thiserror. Actionable error messages and distinct exit codes. |
| **Dirs** | `src/dirs.rs` | In-tree platform XDG path helper. Replaces the upstream `dirs` crate (removed in v0.5.0 for license reasons); see `AGENTS.md § Dependencies`. |
| **Agents** | `src/agents.rs` | Generates the gitignored `.git-paw/AGENTS.local.md` sidecar (the combined view) per worktree; manages the `<!-- git-paw:start … end -->` marker region; leaves the tracked `AGENTS.md` committable. |
| **Skills** | `src/skills.rs` | Resolves agent skills following the [agentskills.io specification](https://agentskills.io) — `.agents/skills/<name>/SKILL.md` first, then a user override in the config directory, then the default embedded in the binary — and renders their placeholders. Injects coordination + supervisor instructions into the worktree sidecar. |
| **Init** | `src/init.rs` | `git paw init` bootstrap. Creates `.git-paw/` (with `logs/`, `tmp/`, `scripts/`), installs the bundled helper scripts into `.git-paw/scripts/`, writes the default config, and adds gitignore entries. Prompts for the spec system and records `[specs]` (no filesystem auto-detection). |
| **Replay** | `src/replay.rs` | `git paw replay`. Reads pane logs from `.git-paw/logs/` and either strips ANSI or pipes through `less -R`. |
| **Doctor** | `src/doctor.rs` | `git paw doctor`. Read-only preflight diagnostics: a probe layer gathers environment/CLI/config/spec/script/broker/supervisor/hygiene facts, and pure check functions map them to grouped ✓/⚠/✗ results with remedies. Renders human or `--json`; exits non-zero only when a check fails (warnings exit 0). `--live` folds in the `selftest` verdict. |
| **Selftest** | `src/selftest.rs` | `git paw selftest`. Isolated end-to-end lifecycle smoke check (start → add → remove → stop) against a throwaway repo and a dummy CLI (`cat`) — private tmux socket, ephemeral broker port, isolated `HOME`, no LLM backend. The shipped form of the dogfood isolation recipe. |
| **Logging** | `src/logging.rs` | Per-pane log capture via `tmux pipe-pane`. Files at `.git-paw/logs/<session>/<branch>.log`. |
| **Worktree provisioning** | `src/worktree_provision.rs` | Per-worktree runtime isolation driven by `[worktree]` config: copies declared env files into a new worktree, allocates its port block (`base + slot * stride`) into a managed `.env.local` block, and runs the consumer's `on_create` / `on_remove` hooks. |
| **Process seam** | `src/command_runner.rs` | The `CommandRunner` trait every `git` / `tmux` invocation goes through: production wires `RealCommandRunner`, tests inject a fake that records argv and returns scripted output. |
| **Domain** | `src/domain.rs` | Newtypes for injection-prone constructed strings — `SessionName`, `BranchSlug`, `WorktreePath` — plus `shell_quote` for paths interpolated into shell commands. |
| **Lock** | `src/lock.rs` | Advisory `<repo>/.git-paw/.add-remove.lock` taken by `git paw add` and `git paw remove` so two concurrent pane splices cannot interleave. |
| **Random** | `src/random.rs` | Version-4 UUIDs built from OS randomness (no `uuid` dependency), used for CLI session ids. |
| **Dashboard** | `src/dashboard.rs`, `src/dashboard/` | The `git paw __dashboard` TUI (status table + Broker log panel). Detail below. |
| **Broker** | `src/broker/` | HTTP coordination server (axum) with watcher + conflict detector + learnings subsystems. Detail below. |
| **Supervisor** | `src/supervisor/` | Supervisor-mode subsystems (unattended drive loop, auto-approve classifier and dispatch, approval send gate and per-pane claim, stall detection, permission prompts, manual-approval log, branch refresh, dev-command presets, pane layout). Detail below. |
| **Specs** | `src/specs/` | Spec scanning. Four backends (`openspec`, `markdown`, `speckit`, `superpowers`); `mod.rs` holds the dispatch entry point and `resolve.rs` resolves `--specs` names. |
| **opsx** | `src/opsx/` | OpenSpec role-gating guard (`role_guard.rs`): run by the broker on every committed `agent.artifact`, it flags archive activity by a non-supervisor agent per `[opsx] role_gating`. Inert unless the spec engine is OpenSpec. |
| **MCP** | `src/mcp/` | `git paw mcp`. Model Context Protocol server over stdio. Read-only tools expose coordination, governance, docs, specs, session, learnings, skills, git, and source-browsing state to MCP-aware clients; the one write surface is four agent-scoped publish tools that publish only as the calling worktree's own agent. Runs standalone (no session/broker/supervisor required; broker-backed reads degrade to empty results). Detail below. |
| **Coordination** | `src/coordination/` | User→agent coordination helpers (inventory + target validation) backing the `/agents` and `/tell` supervisor commands. Distinct from `src/broker/` peer-to-peer (agent↔agent) coordination. Detail below. |

### `src/broker/` modules

| File | Purpose |
|------|---------|
| `src/broker/mod.rs` | Public surface (start/stop entry points, shared state types). |
| `src/broker/server.rs` | `axum` HTTP server: `/publish`, `/watch`, `/messages/:agent_id`, `/status`, `/log`. Validates each incoming `/publish` body (message schema, `agent_id` pattern, unfilled-placeholder rejection) before handing it to delivery. |
| `src/broker/messages.rs` | `BrokerMessage` enum + payload types + slug validation. Source of truth for the wire format used in user-facing examples. |
| `src/broker/publish.rs` | Client-side helpers for callers outside the broker process: build `BrokerMessage` values and publish them over HTTP, register a live `/watch` target, and fetch `/log`. Shared by `cmd_supervisor` and the MCP publish tools. |
| `src/broker/delivery.rs` | Delivery layer: assigns each message its sequence number, updates the sender's record, appends it to the message log, and routes it into inboxes (broadcast, supervisor inbox, targeted delivery). Also cursor-based polling, the dashboard snapshot, and the `broker.log` flush loop. |
| `src/broker/watcher.rs` | Git-status polling watcher: one task per worktree runs `git status --porcelain` every 2 s and publishes `agent.status` (with `modified_files`) when the reported path set changes; prunes a worktree that has vanished. |
| `src/broker/conflict.rs` | Forward / in-flight / ownership conflict detection. Auto-emits `[conflict-detector]`-tagged `agent.feedback` and escalates via `agent.question`. |
| `src/broker/learnings.rs` | Opt-in learnings subsystem. Aggregates the deterministic signals (stuck durations, recovery cycles, conflict events, permission patterns) plus published qualitative `agent.learning` records, and flushes them to `.git-paw/session-learnings.md`. |

`src/dashboard.rs` (top-level, not inside `src/broker/`) renders the dashboard
pane — the live status table and the optional message-log panel — by reading
the shared broker state. The dashboard pane sits at pane index 1 in supervisor
mode (see the layout diagram below) and at pane 0 in non-supervisor broker
mode.

The dashboard is structured as **Model-View-Update**:

| File | Purpose |
|------|---------|
| `src/dashboard.rs` | The **Model** (`Model` — the redraw state: derived agent rows, footer status line, Broker log buffer, quit flag) and the **Update** (`Msg` + a pure `update(&mut Model, Msg)`), plus what MVU cannot own: terminal setup/teardown, the SIGHUP/`poll_tty` FFI, the `getppid() == 1` orphan-exit gate, and the event loop. |
| `src/dashboard/view.rs` | The **View** — a pure function of the `Model` producing the rendered frame (`render`, `render_dashboard`, `draw_frame`, and the `status_symbol` / `format_age` / `format_agent_rows` / `arrange_with_supervisor_pinned` / `format_status_line` helpers). No I/O, so frames are asserted against a `TestBackend` without a live pty. |
| `src/dashboard/broker_log.rs` | The Broker log panel: ring buffer, filter chips, selection/overlay state, key handling, and its own rendering. |

The event loop drains crossterm key presses, the per-tick status snapshot, and
new broker-log entries into `Msg` values, routes each through `update`, and
renders the resulting `Model` via the View — rather than mutating local state
inline. The View re-exports keep `crate::dashboard::{render_dashboard,
format_agent_rows, …}` resolving from the parent module.

### `src/supervisor/` modules

| File | Purpose |
|------|---------|
| `src/supervisor/mod.rs` | Module root declaring the submodules below. The supervisor launch itself (`cmd_supervisor`) lives in `src/commands/supervisor.rs`. |
| `src/supervisor/approval_gate.rs` | The send gate every in-binary approver passes through: refuses pane 0, takes a fresh capture of the target pane immediately before sending, and dispatches keys only if the prompt is still live. |
| `src/supervisor/approve.rs` | Auto-approval keystroke dispatch: selects the prompt's option index, publishes an `agent.status` audit entry, then sends the option digit and `Enter` as two separate `tmux send-keys` calls. |
| `src/supervisor/auto_approve.rs` | Safe-command classifier: command-slice extraction and normalisation, the danger-list, `.git/` and protected-path write checks, worktree-confined rules, the composed whitelist, and option selection. Backs the hidden `git paw __classify` subcommand. |
| `src/supervisor/branch_refresh.rs` | Pure precondition check for the opt-in post-merge branch refresh (`[supervisor] branch_refresh`): a branch is rebased only when every gate passes. The drive loop resolves the inputs. |
| `src/supervisor/claim.rs` | Exclusive per-pane approval claim. Every approver — the drive loop, the dashboard auto-approver, and the bundled `sweep.sh approve` in its own process — atomically creates `<repo>/.git-paw/tmp/approve-pane-<N>.claim` before sending a keystroke, so two can never both approve one pane. Non-blocking (a held pane is skipped, not waited on), released by RAII, and stealable once untouched past a 10s TTL so a hard-killed approver cannot wedge a pane. |
| `src/supervisor/dev_allowlist.rs` | `[supervisor.common_dev_allowlist]` preset data: the always-on universal tier (non-destructive git verbs plus read-only `find` / `grep` / `sed -n`) and the opt-in `rust` / `node` / `python` / `go` stack presets, composed into the supervisor's own auto-approve whitelist and rendered into the bundled supervisor skill's permission guidance. |
| `src/supervisor/drive.rs` | The `--unattended` drive loop, run in-process by `cmd_supervisor`: ~15 s sweeps of every pane, auto-approval, non-blocking escalation, hand-offs to the supervisor pane, the optional correction loop and branch refresh, completion detection, and the exit summary. |
| `src/supervisor/layout.rs` | Supervisor-as-pane tmux layout: pane 0 supervisor, pane 1 dashboard, agent panes 2 onwards in the bottom-row grid (row-height proportions documented below). |
| `src/supervisor/manual_approvals.rs` | Records each command forwarded for a manual decision to `.git-paw/sessions/<session>.manual-approvals.jsonl`, aggregates the log, and suggests a promotion target for `git paw approvals`. |
| `src/supervisor/permission_prompt.rs` | Pane classification for permission-prompt detection (`tmux capture-pane` parsing). |
| `src/supervisor/poll.rs` | One tick of the dashboard-side auto-approve loop: detect stalled agents, capture and classify each pane, approve safe live prompts, forward the rest as questions. The loop thread runs in the `__dashboard` process. |
| `src/supervisor/stall.rs` | Stall detection: agents whose status is not terminal (`done` / `verified` / `blocked` / `committed`) and whose `last_seen` exceeds the threshold. |

### `src/specs/` modules

| File | Purpose |
|------|---------|
| `src/specs/mod.rs` | Public surface (`SpecBackend` trait, `SpecEntry`) and the dispatch entry point (`scan_specs` / `scan_specs_with_override`). Picks the backend from the `--specs-format` CLI override or the `[specs] type` config — the only two sources (no filesystem auto-detection). |
| `src/specs/resolve.rs` | Resolves `--specs NAME[,NAME...]` against the scanned entries: exact id match, then Spec Kit feature-name and numeric-prefix matches; an unknown or ambiguous name fails the whole resolution. |
| `src/specs/openspec.rs` | OpenSpec backend: scans `<dir>/<change>/tasks.md` directories, skips `<dir>/archive/`. |
| `src/specs/markdown.rs` | Markdown backend: scans flat `.md` files with YAML frontmatter; only `paw_status: pending` is picked up. |
| `src/specs/speckit.rs` | Spec Kit backend: scans `.specify/specs/<feature>/`, decomposes the current phase into `[P]`-task worktrees plus one consolidated `phase/…` worktree; probes `<dir>/../memory/constitution.md` for the governance auto-wire. |
| `src/specs/superpowers.rs` | Superpowers backend: scans `docs/superpowers/plans/*.md` plan files; each plan with incomplete steps becomes one entry (one worktree), never fanned out per task. |

### `src/mcp/` modules

The MCP subsystem follows a strict one-way dependency direction — `query` knows
nothing about MCP, `tools` knows about MCP and `query`, and `server` only wires
`tools` onto a transport — so a future HTTP transport stays additive.

| File | Purpose |
|------|---------|
| `src/mcp/mod.rs` | Entry point: `cmd_mcp()`, `RepoContext`, and repository resolution (`--repo` wins, else nearest `.git` ancestor). |
| `src/mcp/server.rs` | stdio transport setup, tool-registry wiring, and process lifecycle (exits when stdin closes). |
| `src/mcp/logging.rs` | Minimal in-tree logger (no tracing dependency) — diagnostics to stderr at a `RUST_LOG`-controlled level (default `warn`) and, with `--log-file`, tee'd to a file; stdout stays reserved for the JSON-RPC stream. |
| `src/mcp/query/*` | Data-layer reads (no MCP types): `conflicts.rs`, `docs.rs`, `git.rs`, `governance.rs`, `intents.rs`, `learnings.rs`, `session.rs`, `source.rs`, `specs.rs`, plus `mod.rs`. Built from broker HTTP state, files on disk, and git output. |
| `src/mcp/tools/*` | MCP tool surfaces (one file per category): `coordination.rs`, `docs.rs`, `git.rs`, `governance.rs`, `project.rs`, `publish.rs`, `session.rs`, `source.rs`, plus `mod.rs`. Each read tool maps a `query` reader onto an MCP tool definition. `publish.rs` is the only write surface: `publish_status`, `publish_artifact`, `publish_blocked`, and `publish_question`, built with the `src/broker/publish.rs` helpers, with the `agent_id` derived from the session's worktree branch (no tool accepts one) and no `agent.verified` / `agent.feedback` verbs. |

### `src/coordination/` modules

| File | Purpose |
|------|---------|
| `src/coordination/mod.rs` | Public surface for the user→agent coordination helpers. |
| `src/coordination/inventory.rs` | Agent inventory + target-validation helpers (unknown-target rejection) shared by the supervisor routing commands. |
| `src/coordination/tell.rs` | Pure logic behind the `/tell` supervisor command — argument parsing and delivery-mode selection. The send itself (`tmux send-keys` or a broker publish) is driven by the supervisor skill. |

## Start Flow

The `start` command is the primary flow. `main.rs` dispatches it to one of three
handlers in `src/commands/`: `supervisor::cmd_supervisor` when supervisor mode
resolves on (`--supervisor` or `--unattended`, else `[supervisor] enabled`, else
an interactive prompt when no `[supervisor]` section exists; `--no-supervisor`
always wins), `start::cmd_start_with_specs` for a spec-driven launch
(`--from-all-specs` / `--specs`) without a supervisor, and `start::cmd_start`
otherwise. All three run the same existing-session check first. Here's what
`cmd_start` does step by step:

```
git paw start
     │
     ▼
┌─ Validate git repo ──────────────────────────────────────┐
│  git::validate_repo() → repo root path                   │
└──────────────────────────────────────────────────────────┘
     │
     ▼
┌─ Check for existing session ─────────────────────────────┐
│                                                          │
│  Active, tmux session gone?  ──yes──► Invalidate stale   │
│         │ no                           receipt, then     │
│         │                              fresh start       │
│  Paused + tmux alive?        ──yes──► Restart broker,    │
│         │ no                           reattach          │
│  Active + tmux alive?        ──yes──► Reattach           │
│         │ no                                             │
│  Stopped session?            ──yes──► Recover            │
│         │ no                                             │
│  No session                  ──────► Fresh start         │
└──────────────────────────────────────────────────────────┘
     │
     ▼ (fresh start)
┌─ Load config ────────────────────────────────────────────┐
│  config::load_config() → merged PawConfig                │
│  ([supervisor] enabled → hand off to cmd_supervisor)     │
└──────────────────────────────────────────────────────────┘
     │
     ▼
┌─ Detect CLIs ────────────────────────────────────────────┐
│  detect::detect_clis() → Vec<CliInfo>                    │
│  (auto-detected + custom, deduplicated)                  │
└──────────────────────────────────────────────────────────┘
     │
     ▼
┌─ Interactive selection ──────────────────────────────────┐
│  interactive::run_selection()                            │
│  → Vec<(branch, cli)> mappings                           │
│  (skipped if --cli + --branches provided)                │
└──────────────────────────────────────────────────────────┘
     │
     ▼
┌─ Create worktrees ───────────────────────────────────────┐
│  git::create_worktree() for each branch                  │
│  → .git-paw/worktrees/<slug>/   (child placement)        │
│    or ../<project>-<slug>/       (sibling placement)     │
│  worktree_provision::provision_worktree()                │
│  → env-file copies, port slot, on_create hook            │
└──────────────────────────────────────────────────────────┘
     │
     ▼
┌─ Build tmux session ─────────────────────────────────────┐
│  TmuxSessionBuilder::new(project)                        │
│    .session_name("paw-project")                          │
│    .mouse_mode(true)                                     │
│    .border_affordances(true)                             │
│    .add_pane(PaneSpec { branch, worktree,                │
│                         cli_command }) × N               │
│    .build() → TmuxSession with command sequence          │
└──────────────────────────────────────────────────────────┘
     │
     ▼
┌─ Pane layout by mode ────────────────────────────────────┐
│  supervisor   → Pane 0 = supervisor CLI                  │
│                 Pane 1 = `git paw __dashboard`           │
│                 Pane 2..N = per-agent CLIs               │
│                 (cmd_supervisor builds it with           │
│                  tmux::build_supervisor_session)         │
│  broker-only  → Pane 0 = `git paw __dashboard`           │
│                 Pane 1..N = per-branch agent CLIs        │
│  no broker    → Pane 0..N = per-branch agent CLIs        │
│                                                          │
│  With the broker on, the tmux session environment        │
│  carries GIT_PAW_BROKER_URL into every pane, and the     │
│  dashboard pane:                                         │
│   ├─ Starts the axum HTTP broker on the configured       │
│   │  bind address and port                               │
│   └─ Renders the ratatui status table                    │
└──────────────────────────────────────────────────────────┘
     │
     ▼
┌─ Save session state ─────────────────────────────────────┐
│  session::save_session() → atomic JSON write             │
│  + per-repo discovery file (.git-paw/sessions/)          │
└──────────────────────────────────────────────────────────┘
     │
     ▼
┌─ Attach ─────────────────────────────────────────────────┐
│  attach_or_print_hint() → user enters tmux session       │
│  (prints an attach hint when stdin is not a TTY)         │
└──────────────────────────────────────────────────────────┘
```

### Per-repo session discovery file

Alongside the global receipt under `~/.local/share/git-paw/sessions/`, `git paw
start` writes a **per-repo** discovery file at
`<repo>/.git-paw/sessions/<session>.json`. This is the surface the bundled
`sweep.sh` supervisor helper reads to find the active session and its agent
roster from inside the repo (without reaching into the XDG state dir). `git paw
add` / `remove` rewrite it with the updated roster; `purge` removes it.

Its shape — stable for sweep.sh and forward-compatible (consumers ignore
unknown keys):

```json
{
  "session_name": "paw-myproject",
  "agents": [
    {
      "branch_id": "feat-add-auth",
      "worktree_path": "/abs/path/to/myproject/.git-paw/worktrees/feat-add-auth",
      "cli": "claude",
      "pane_index": 2
    }
  ],
  "orchestrator": {
    "cli": "fable",
    "pane_index": 0
  }
}
```

`branch_id` is the broker agent id (slugified branch); `pane_index` is the
agent's tmux pane within the session window. When the file is absent (e.g. a
supervisor attached to a pre-existing `paw-*` session), `sweep.sh` falls back to
resolving the session name from `$TMUX` / `tmux display-message -p '#S'`, so the
file never needs to be hand-authored.

`orchestrator` records the supervisor pane's own entry — distinct from the
coding-agent roster in `agents`, so tooling can see the full tiered-model
split (e.g. a `fable`-tier orchestrator driving `sonnet` workers). It is
present only for a supervisor-mode session; a bare-mode session (no
orchestrator pane) and a file written before this field existed both omit it.
The field is additive and back-compatible: it deserializes with
`#[serde(default)]`, so an older file loads unchanged, and `sweep.sh` — which
reads `agents` and `session_name` by name — ignores it.

## Broker Architecture

When `[broker] enabled = true`, the dashboard pane runs `git paw __dashboard`. This single process hosts both the HTTP broker and the dashboard TUI. The dashboard pane sits at pane 1 in supervisor mode and at pane 0 in non-supervisor broker mode.

```
Dashboard pane process (git paw __dashboard):
├── tokio runtime (background threads)
│   ├── axum HTTP server on 127.0.0.1:9119 (default [broker] bind / port)
│   │   ├── POST /publish
│   │   ├── POST /watch   (register a hot-added worktree as a live watch target)
│   │   ├── GET /messages/:agent_id?since=N
│   │   ├── GET /status
│   │   └── GET /log?since=N   (full message log, read by cmd_supervisor and the MCP server)
│   ├── Worktree watchers (src/broker/watcher.rs), one task per worktree
│   │   └── `git status --porcelain` every 2s; publishes agent.status on change;
│   │       prunes a vanished worktree
│   └── Conflict detector (src/broker/conflict.rs), supervisor mode only
│       └── Forward / in-flight / ownership shapes
├── Log flush thread (std::thread, 5s interval)
│   └── Appends new messages to broker.log
├── Learnings flush thread (std::thread; opt-in via [supervisor] learnings = true)
│   └── Flushes the aggregator (src/broker/learnings.rs), fed on every publish,
│       to .git-paw/session-learnings.md (default every 60s, plus once at shutdown)
├── Auto-approve thread (supervisor mode with [supervisor.auto_approve] enabled,
│   │                    not --unattended)
│   └── Runs src/supervisor/poll.rs ticks against stalled panes
└── Main thread
    └── ratatui dashboard (800ms idle tick; a keystroke wakes it immediately)
```

The main-thread render loop is bound to the session's lifecycle: it checks a single exit gate every iteration (on all paths, including error/degraded branches) and terminates on a clean `SIGHUP`, on reparent-to-init (`getppid() == 1`, Unix only), or when the controlling terminal is gone (a poll error or a failed terminal write — catching reparent-to-a-lingering-shell). If the in-process broker fails to bind its port at startup, the process emits a diagnostic and exits non-zero instead of busy-looping. Together these keep an orphaned `__dashboard` from lingering and pegging CPU. See the [Dashboard chapter](user-guide/dashboard.md#lifecycle-and-exit-conditions) for the user-facing description.

### Broker state

The broker state is an `Arc<BrokerState>` defined in `src/broker/mod.rs`: an
`RwLock` around the agent records, inboxes, and message log, plus a
lock-free `AtomicU64` sequence counter. Lock guards are never held across an
`.await` (the `clippy::await_holding_lock` lint is set to deny). The state is
shared between the axum server handlers, the watchers, the conflict detector,
the learnings aggregator, and the ratatui dashboard render loop. The server
validates incoming messages (`src/broker/server.rs`) and hands them to
`src/broker/delivery.rs`, which sequences, logs, and routes them; the dashboard
reads the latest snapshot each tick.

The log flush thread periodically appends new messages to `broker.log` in the
session-state directory (`~/.local/share/git-paw/sessions/broker.log`, recorded
as `broker_log_path` in the session file), one human-readable line per message
(`[seq] <unix-seconds> [agent_id] <summary>`). This runs on a plain
`std::thread` to avoid contention with the tokio runtime.

### Environment injection

When the broker is enabled, git-paw sets `GIT_PAW_BROKER_URL=http://<bind>:<port>` (default `http://127.0.0.1:9119`) in the tmux environment for the session when it builds it. Each agent pane inherits this variable and can use it to communicate with the broker.

## Supervisor Mode Layout

When `--supervisor` is active (or `[supervisor] enabled = true`), the tmux
session is laid out as a 50/50 top row plus a row-major agent grid below.
This is the canonical v0.5.0 supervisor-as-pane layout established by the
`supervisor-as-pane` archive.

```
┌──────────────────────────┬──────────────────────────┐
│  pane 0: supervisor      │  pane 1: dashboard       │
├──┬──┬──┬──┬──┬───────────┴──────────────────────────┤
│ 2│ 3│ 4│ 5│ 6│  agent grid (row 1)                  │
├──┴──┴──┴──┴──┤                                      │
│ 7│..│..│..│ N│  agent grid (row 2..M)               │
└──┴──┴──┴──┴──┴──────────────────────────────────────┘
```

Pane 0 always hosts the supervisor CLI; pane 1 always hosts the dashboard.
Pane indices 2 onwards host one CLI per agent. The supervisor reads agent
state via the broker and the dashboard; the dashboard reads the same broker
state for its status table.

### Row-height proportions

The top row is always half the session height — the supervisor and dashboard
panes split its width 50/50 — and the agent rows share the remaining 50%
evenly, each taking `50 / agent_rows` percent. This is a fixed rule rather
than a per-agent-count table: adding agents shrinks the agent rows, never the
top row.

| Agent rows | Top row | Each agent row |
|------------|---------|----------------|
| 1 | 50% | 50% |
| 2 | 50% | 25% |
| 3 | 50% | 16.7% |
| 4 | 50% | 12.5% |
| 5 | 50% | 10% |

### Equal-width agent columns

Agent panes are spliced into a row by successive `tmux split-window -h`, and
each `-h` split halves the *current* pane — so a row populated by raw splits
renders unequal widths (a 3-agent row would render 50/25/25, not equal
thirds). `select-layout tiled` is deliberately **not** used for the whole
window because it would scramble the predictable pane-index ordering the rest
of the system relies on. Instead, after the panes for a row exist, git-paw
rebalances that row to equal width: `tmux::rebalance_agent_rows` queries the
live window width and issues `tmux resize-pane -x <cols>` so each pane in the
row gets an equal column share (the last pane absorbs the rounding remainder),
leaving the row equal-width within a one-column tolerance. The rebalance runs
on every path that changes the grid — `git paw start`, `git paw add`, and
`git paw remove` — so an incrementally-built grid matches a start-time grid of
the same agent count. It never touches the top row's supervisor/dashboard
50/50 split nor the per-row vertical heights. No agent row exceeds five panes
(`SUPERVISOR_AGENTS_PER_ROW`), bounding the smallest equal-width target to
~20% of the window. `src/supervisor/layout.rs` and the rebalance in
`src/tmux/layout.rs` are the source of truth.

### Launch-readiness gate

Before injecting an agent's boot block (the initial prompt, paste-handling
notes, and `/opsx:apply …` task) into a pane, git-paw verifies the pane's CLI
has actually reached an interactive state rather than relying on a fixed
wall-clock sleep. `tmux::gate_pane_for_injection` polls the pane with
`tmux capture-pane` for a CLI-readiness marker; only once the marker appears is
the boot block injected. If the readiness budget elapses while the pane is
still a bare shell (the CLI never started), git-paw relaunches the CLI command
into that pane and polls again, up to a small relaunch budget, before falling
back to injection. The gate is conservative: an unrecognised CLI whose UI
matches no known marker simply falls back to injecting after the budget, so
launch behaviour is never worse than the previous fixed-sleep launch. This
prevents the multi-line boot block from being typed into a bare shell, where
it would be interpreted line-by-line as failing commands. The same gate also
recognises a first-run trust/bypass acceptance dialog and fails the launch
loudly rather than risk a blind `Enter` accepting it. The gate guards every
pane that receives an injected prompt: bare `git paw start` (with the broker
enabled), a spec-driven launch (`--specs` / `--from-all-specs`, with or
without `--supervisor`), and `git paw add`. Session recovery and a
broker-less bare launch spawn panes without ever calling the gate — harmless,
since neither path injects anything into them.

## Non-Supervisor Layout

When supervisor mode is OFF and the broker is on, the dashboard occupies
pane 0 and the agent CLIs occupy panes 1 onwards in a single row-major
grid (no top row):

```
┌───────────────────────────────────────────────────────┐
│  pane 0: dashboard                                    │
├──┬──┬──┬──┬──┬────────────────────────────────────────┤
│ 1│ 2│ 3│ 4│ 5│  agent grid (row 1)                   │
├──┴──┴──┴──┴──┤                                       │
│ 6│..│..│..│ N│  agent grid (row 2..M)                │
└──┴──┴──┴──┴──┴───────────────────────────────────────┘
```

When the broker is disabled too, every pane (0..N) is an agent CLI and
there is no dashboard pane.

## Worktree Lifecycle

Git worktrees are the foundation of git-paw's parallel workflow.

### Creation

Where a worktree goes is set by `worktree_placement` in the config
(`git::worktree_path_for`). `git paw init` writes `worktree_placement = "child"`,
which nests worktrees inside the repo. For a project named `my-app` and
branches `feature/auth-flow` and `feat/api`:

```
my-app/                             ← main repo (current directory)
└── .git-paw/worktrees/
    ├── feature-auth-flow/          ← worktree (created by git-paw)
    └── feat-api/                   ← worktree (created by git-paw)
```

A child worktree is named by the branch slug: slashes become hyphens and
characters outside `[A-Za-z0-9._-]` are dropped.

A config with no `worktree_placement` key resolves to `"sibling"`, the earlier
layout, which creates worktrees beside the main repo directory:

```
my-app/                         ← main repo (current directory)
my-app-feature-auth-flow/       ← worktree (created by git-paw)
my-app-feat-api/                ← worktree (created by git-paw)
```

The sibling naming convention is `<project>-<sanitized-branch>`, where every
non-alphanumeric character becomes a hyphen.

### Lifecycle states

```
create_worktree()          stop              start (recover)
     │                      │                     │
     ▼                      ▼                     ▼
  [exists on disk]  →  [still on disk]  →  [reused as-is]
                                                  │
                                            purge │
                                                  ▼
                                          [removed from disk]
```

Key points:
- **Stop** preserves worktrees — uncommitted work survives
- **Recover** reuses existing worktrees — no data loss
- **Purge** removes worktrees — `git worktree remove` followed by prune
- **Start** prunes stale worktree registrations (`git worktree prune`) before creating new worktrees

## Session State

Session state is persisted as JSON under `~/.local/share/git-paw/sessions/`
(`~/Library/Application Support/git-paw/sessions/` on macOS):

```json
{
  "session_name": "paw-my-app",
  "repo_path": "/home/you/projects/my-app",
  "project_name": "my-app",
  "created_at": "2025-01-15T10:30:00Z",
  "status": "active",
  "worktrees": [
    {
      "branch": "feat/auth",
      "worktree_path": "/home/you/projects/my-app/.git-paw/worktrees/feat-auth",
      "cli": "claude",
      "branch_created": true
    },
    {
      "branch": "feat/api",
      "worktree_path": "/home/you/projects/my-app/.git-paw/worktrees/feat-api",
      "cli": "codex",
      "branch_created": false
    }
  ],
  "broker_port": 9119,
  "broker_bind": "127.0.0.1",
  "broker_log_path": "/home/you/.local/share/git-paw/sessions/broker.log",
  "mode": "bare",
  "dashboard_pane": 0
}
```

`status` is `active`, `paused`, or `stopped`. `broker_port`, `broker_bind`,
`broker_log_path`, and `dashboard_pane` are present only when the broker is
enabled; `git paw status` uses the bind and port to probe the broker, and
`git paw purge` uses `broker_log_path` to clean up `broker.log`. `mode` (`bare`
or `supervisor`) records the pane layout so recovery rebuilds the same shape.
`branch_created` records whether git-paw created the branch (rather than
reusing an existing one). A worktree entry can also carry
`pending_boot_prompt`, `runtime_slot`, and `cli_session_id`; each is omitted
when unset.

### Atomic writes

Session state is written atomically: write to a temporary file, then rename. This prevents corruption if the process is killed mid-write.

### Effective status

The on-disk status may not reflect reality (e.g., tmux was killed externally). git-paw checks the actual tmux state:

| File status | tmux alive? | Effective status |
|-------------|-------------|-----------------|
| `active` | Yes | Active (reattach) |
| `active` | No (session gone) | Stale — `start` invalidates the receipt and launches fresh |
| `paused` | Yes | Paused (restart the broker, reattach) |
| `paused` | No | Stopped (recover) |
| `stopped` | N/A | Stopped (recover) |
| No file | N/A | No session |

For an `active` receipt, "alive" also requires at least one agent pane running
something other than a bare shell once the session is past a ~30-second launch
grace period; a session whose every agent CLI has exited is treated as Stopped
and recovered.

## Tmux Builder Pattern

The tmux module uses a builder pattern that accumulates operations as data structures rather than immediately executing shell commands. This enables:

- **Testability** — generate commands without executing them
- **Dry run** — render the plan (`command_strings()`) without side effects
- **Atomicity** — validate the full plan before running anything

```rust
TmuxSessionBuilder::new("my-app")
    .session_name(resolved_name)   // e.g. "paw-my-app"
    .mouse_mode(true)
    .border_affordances(true)
    .set_environment("GIT_PAW_BROKER_URL", &broker_url)
    .add_pane(PaneSpec { branch, worktree, cli_command })
    .add_pane(PaneSpec { ... })
    .build()?
    // → TmuxSession { name, commands: Vec<TmuxCommand> }
```

The built `TmuxSession` can be inspected (`command_strings()`) or executed
(`execute()`). Supervisor-mode sessions are assembled the same way by
`tmux::build_supervisor_session`; `git paw add` splices a pane in with
`build_add_agent_commands` and `git paw remove` retiles with
`build_remove_retile_commands` — all in `src/tmux/command.rs`.

### CLI-launch robustness

Supervisor-mode sessions and panes hot-added by `git paw add` are created with
shell auto-update prompts suppressed
(`new-session`/session env set `DISABLE_AUTO_UPDATE=true` and
`DISABLE_UPDATE_PROMPT=true`) so an interactive framework prompt (e.g.
oh-my-zsh's `Would you like to update? [Y/n]`) cannot fire as the pane's
shell reads its rc and swallow the first keystroke of the CLI-launch command.
As a second layer, those builders send a `C-u` line-clear immediately before
each CLI-launch command, so any stray pending input cannot corrupt it. Every
headless `new-session` canvas is sized (480×140) to tile a supervisor session
with several agents when no client is attached; a real terminal resizes the
session on attach.

## Error Strategy

All errors flow through `PawError` (defined with `thiserror`). Each variant carries an actionable message telling the user what went wrong and how to fix it. No panics in non-test code — all `Result` propagation.

Exit codes:
- **0** — success
- **1** — operational error
- **2** — user cancelled
