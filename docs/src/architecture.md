# Architecture

This chapter covers git-paw's internal architecture: module structure, data flow, and key design decisions.

## Module Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                              main.rs                             │
│                       (entry point, dispatch)                    │
├──────────┬──────────────┬──────────────┬─────────────┬───────────┤
│  cli.rs  │ interactive  │   config.rs  │  error.rs   │  dirs.rs  │
│  (clap)  │ (dialoguer)  │    (TOML)    │ (PawError)  │  (XDG)    │
├──────────┴──────────────┴──────────────┴─────────────┴───────────┤
│                                                                   │
│  detect.rs    git.rs      tmux.rs    session.rs    logging.rs    │
│  (PATH scan)  (worktrees) (builder)  (JSON state)  (pane logs)   │
│                                                                   │
│  agents.rs    skills.rs   init.rs    replay.rs    selftest.rs    │
│  (AGENTS.md)  (skill      (project    (log        (smoke         │
│               templates)  bootstrap)  playback)    check)        │
├───────────────────────────────────────────────────────────────────┤
│  broker/                  supervisor/             specs/         │
│  ├── mod.rs               ├── mod.rs              ├── mod.rs     │
│  ├── server.rs            ├── approve.rs          ├── openspec.rs│
│  ├── messages.rs          ├── auto_approve.rs     ├── markdown.rs│
│  ├── delivery.rs          ├── curl_allowlist.rs   ├── speckit.rs │
│  ├── conflict.rs          ├── dev_allowlist.rs    └── resolve.rs │
│  ├── learnings.rs         ├── layout.rs                          │
│  ├── watcher.rs           ├── permission_prompt.rs               │
│  └── publish.rs           ├── poll.rs                            │
│                           └── stall.rs                           │
└───────────────────────────────────────────────────────────────────┘
```

### Module Responsibilities

| Module | File | Purpose |
|--------|------|---------|
| **CLI** | `src/cli.rs` | Argument parsing with clap v4 derive macros. Defines all subcommands, flags (`--from-all-specs`, `--specs`, `--specs-format`, `--supervisor`, `--no-supervisor`, `--force`, ...), and help text. |
| **Detection** | `src/detect.rs` | Scans `PATH` for known AI CLI binaries (`KNOWN_CLIS`). Resolves custom CLIs from config. Merges and deduplicates. |
| **Git** | `src/git.rs` | Validates git repos, lists branches (local + remote, deduplicated), creates/removes worktrees, derives safe directory names. |
| **Tmux** | `src/tmux.rs` | Builder pattern for tmux operations. Creates sessions, splits panes, sends commands, applies the supervisor-as-pane layout, sets pane titles. |
| **Session** | `src/session.rs` | Persists session state to JSON files under `~/.local/share/git-paw/sessions/`. Atomic writes, crash recovery. |
| **Config** | `src/config.rs` | Parses TOML from global (`~/.config/git-paw/config.toml`) and per-repo (`.git-paw/config.toml`). Merges with repo-wins semantics. |
| **Interactive** | `src/interactive.rs` | Terminal prompts. The branch and spec multi-selects are ratatui/crossterm fuzzy-filter pickers (type to filter, space to toggle); the mode and CLI single-selects use dialoguer. Skips prompts when flags are provided. |
| **Error** | `src/error.rs` | `PawError` enum with thiserror. Actionable error messages and distinct exit codes. |
| **Dirs** | `src/dirs.rs` | In-tree platform XDG path helper. Replaces the upstream `dirs` crate (removed in v0.5.0 for license reasons); see `AGENTS.md § Dependencies`. |
| **Agents** | `src/agents.rs` | Generates the gitignored `.git-paw/AGENTS.local.md` sidecar (the combined view) per worktree; manages the `<!-- git-paw:start … end -->` marker region; leaves the tracked `AGENTS.md` committable. |
| **Skills** | `src/skills.rs` | Loads standardized agent skills from `.agents/skills/` following the [agentskills.io specification](https://agentskills.io). Injects coordination + supervisor instructions into the worktree sidecar. |
| **Init** | `src/init.rs` | `git paw init` bootstrap. Creates `.git-paw/`, default config, logs directory, gitignore entries. Auto-detects `.specify/` for Spec Kit. |
| **Replay** | `src/replay.rs` | `git paw replay`. Reads pane logs from `.git-paw/logs/` and either strips ANSI or pipes through `less -R`. |
| **Selftest** | `src/selftest.rs` | `git paw selftest`. Isolated end-to-end lifecycle smoke check (start → add → remove → stop) against a throwaway repo and a dummy CLI (`cat`) — private tmux socket, ephemeral broker port, isolated `HOME`, no LLM backend. The shipped form of the dogfood isolation recipe. |
| **Logging** | `src/logging.rs` | Per-pane log capture via `tmux pipe-pane`. Files at `.git-paw/logs/<session>/<branch>.log`. |
| **Broker** | `src/broker/` | HTTP coordination server (axum) with watcher + conflict detector + learnings subsystems. Detail below. |
| **Supervisor** | `src/supervisor/` | Supervisor-mode subsystems (auto-approve, dev allowlist, stall sweeps, permission prompts, pane layout). Detail below. |
| **Specs** | `src/specs/` | Spec scanning. Three backends (`openspec`, `markdown`, `speckit`); `resolve.rs` is the dispatch entry point. |
| **MCP** | `src/mcp/` | `git paw mcp`. Read-only Model Context Protocol server over stdio — exposes coordination, governance, specs, session, learnings, skills, git, and source-browsing state to MCP-aware clients. Runs standalone (no session/broker/supervisor). Detail below. |
| **Coordination** | `src/coordination/` | User→agent coordination helpers (inventory + target validation) backing the `/agents` and `/tell` supervisor commands. Distinct from `src/broker/` peer-to-peer (agent↔agent) coordination. Detail below. |

### `src/broker/` modules

| File | Purpose |
|------|---------|
| `src/broker/mod.rs` | Public surface (start/stop entry points, shared state types). |
| `src/broker/server.rs` | `axum` HTTP server: `/publish`, `/watch`, `/messages/:agent_id`, `/status`. |
| `src/broker/messages.rs` | `BrokerMessage` enum + payload types + slug validation. Source of truth for the wire format used in user-facing examples. |
| `src/broker/publish.rs` | Validation + sequence assignment for incoming `/publish` calls. |
| `src/broker/delivery.rs` | Routing layer: which inboxes a message lands in (broadcast, supervisor inbox, targeted delivery). |
| `src/broker/watcher.rs` | Filesystem watcher that auto-publishes `agent.status` (with `modified_files`) whenever a tracked file changes in a worktree. |
| `src/broker/conflict.rs` | Forward / in-flight / ownership conflict detection. Auto-emits `[conflict-detector]`-tagged `agent.feedback` and escalates via `agent.question`. |
| `src/broker/learnings.rs` | Opt-in learnings subsystem. Aggregates the five deterministic categories and flushes to `.git-paw/session-learnings.md`. |

`src/dashboard.rs` (top-level, not inside `src/broker/`) renders the dashboard
pane — the live status table and the optional message-log panel — by reading
the shared broker state. The dashboard pane sits at pane index 1 in supervisor
mode (see the layout diagram below) and at pane 0 in non-supervisor broker
mode.

### `src/supervisor/` modules

| File | Purpose |
|------|---------|
| `src/supervisor/mod.rs` | Supervisor boot — composes the subsystems below and drives the supervisor pane. |
| `src/supervisor/approve.rs` | Generic approval/feedback decision plumbing shared by the auto-approver. |
| `src/supervisor/auto_approve.rs` | Safe-command auto-approver against stalled panes (`approval_level`, `safe_commands`, sweeps). |
| `src/supervisor/curl_allowlist.rs` | Seeds the least-privilege agent-broker helper path (`.git-paw/scripts/broker.sh`) into `.claude/settings.json::allowed_bash_prefixes` so the agent's first broker call never hits a permission prompt — a single stable path grant, not per-endpoint `curl` prefixes or a broad `curl *` rule. |
| `src/supervisor/dev_allowlist.rs` | Seeds the curated `[supervisor.common_dev_allowlist]` preset (cargo / git / just / mdBook / OpenSpec) into `.claude/settings.json`. |
| `src/supervisor/layout.rs` | Supervisor-as-pane tmux layout: pane 0 supervisor, pane 1 dashboard, agent panes 2 onwards in the bottom-row grid (row-height proportions documented below). |
| `src/supervisor/permission_prompt.rs` | Pane classification for permission-prompt detection (`tmux capture-pane` parsing). |
| `src/supervisor/poll.rs` | Stalled-pane polling loop driving the auto-approver. |
| `src/supervisor/stall.rs` | Stall heuristics (last-seen window, approval-level filter). |

### `src/specs/` modules

| File | Purpose |
|------|---------|
| `src/specs/mod.rs` | Public surface for the spec subsystem. |
| `src/specs/resolve.rs` | Dispatch entry point. Picks the backend from `[specs] type`, the `--specs-format` CLI override, or `.specify/` auto-detection. |
| `src/specs/openspec.rs` | OpenSpec backend: scans `<dir>/<change>/tasks.md` directories, skips `<dir>/archive/`. |
| `src/specs/markdown.rs` | Markdown backend: scans flat `.md` files with YAML frontmatter; only `paw_status: pending` is picked up. |
| `src/specs/speckit.rs` | Spec Kit backend: scans `.specify/specs/<feature>/`, decomposes the current phase into `[P]`-task worktrees plus one consolidated `phase/…` worktree; probes `<dir>/../memory/constitution.md` for the governance auto-wire. |

### `src/mcp/` modules

The MCP subsystem follows a strict one-way dependency direction — `query` knows
nothing about MCP, `tools` knows about MCP and `query`, and `server` only wires
`tools` onto a transport — so a future HTTP transport stays additive.

| File | Purpose |
|------|---------|
| `src/mcp/mod.rs` | Entry point: `cmd_mcp()`, `RepoContext`, and repository resolution (`--repo` wins, else nearest `.git` ancestor). |
| `src/mcp/server.rs` | stdio transport setup, tool-registry wiring, and process lifecycle (exits when stdin closes). |
| `src/mcp/logging.rs` | Tracing setup — diagnostics to stderr and, with `--log-file`, tee'd to a file; stdout stays reserved for the JSON-RPC stream. |
| `src/mcp/query/*` | Data-layer reads (no MCP types): `conflicts.rs`, `docs.rs`, `git.rs`, `governance.rs`, `intents.rs`, `learnings.rs`, `session.rs`, `source.rs`, `specs.rs`, plus `mod.rs`. Built from broker HTTP state, files on disk, and git output. |
| `src/mcp/tools/*` | MCP tool surfaces (one file per category): `coordination.rs`, `docs.rs`, `git.rs`, `governance.rs`, `project.rs`, `session.rs`, `source.rs`, plus `mod.rs`. Each maps a `query` reader onto an MCP tool definition. |

### `src/coordination/` modules

| File | Purpose |
|------|---------|
| `src/coordination/mod.rs` | Public surface for the user→agent coordination helpers. |
| `src/coordination/inventory.rs` | Agent inventory + target-validation helpers (unknown-target rejection) shared by the supervisor routing commands. |
| `src/coordination/tell.rs` | Backs the `/tell` supervisor command — routes a user message to a named agent, mediated by the supervisor. |

## Start Flow

The `start` command is the primary flow. Here's what happens step by step:

```
git paw start
     │
     ▼
┌─ Check for existing session ──────────────────────┐
│                                                     │
│  Session active + tmux alive?  ──yes──► Reattach   │
│         │ no                                        │
│  Session saved + tmux dead?   ──yes──► Recover     │
│         │ no                                        │
│  No session                   ──────► Fresh start  │
└─────────────────────────────────────────────────────┘
     │
     ▼ (fresh start)
┌─ Validate git repo ─────────────────────────────────┐
│  git.validate_repo() → repo root path               │
└──────────────────────────────────────────────────────┘
     │
     ▼
┌─ Load config ────────────────────────────────────────┐
│  config.load_config() → merged PawConfig             │
└──────────────────────────────────────────────────────┘
     │
     ▼
┌─ Detect CLIs ────────────────────────────────────────┐
│  detect.detect_clis() → Vec<CliInfo>                 │
│  (auto-detected + custom, deduplicated)              │
└──────────────────────────────────────────────────────┘
     │
     ▼
┌─ Interactive selection ──────────────────────────────┐
│  interactive.run_selection()                          │
│  → Vec<(branch, cli)> mappings                       │
│  (skipped if --cli + --branches provided)            │
└──────────────────────────────────────────────────────┘
     │
     ▼
┌─ Create worktrees ───────────────────────────────────┐
│  git.create_worktree() for each branch               │
│  → ../project-branch-name/ directories               │
└──────────────────────────────────────────────────────┘
     │
     ▼
┌─ Build tmux session ────────────────────────────────┐
│  TmuxSessionBuilder                                  │
│    .session_name("paw-project")                      │
│    .pane(branch, worktree, cli) × N                  │
│    .mouse(true)                                      │
│    .build() → TmuxSession with command sequence      │
└──────────────────────────────────────────────────────┘
     │
     ▼
┌─ Mode? ─────────────────────────────────────────────┐
│  supervisor   → Pane 0 = supervisor CLI              │
│                 Pane 1 = `git paw __dashboard`       │
│                 Pane 2..N = per-spec agent CLIs      │
│  broker-only  → Pane 0 = `git paw __dashboard`       │
│                 Pane 1..N = per-branch agent CLIs    │
│  no broker    → Pane 0..N = per-branch agent CLIs    │
│                                                       │
│  In every broker mode the dashboard pane:            │
│   ├─ Starts axum HTTP server on configured port      │
│   ├─ Injects GIT_PAW_BROKER_URL into all agent panes │
│   └─ Renders the ratatui status table                │
└──────────────────────────────────────────────────────┘
     │
     ▼
┌─ Save session state ────────────────────────────────┐
│  session.save_session() → atomic JSON write          │
└──────────────────────────────────────────────────────┘
     │
     ▼
┌─ Attach ─────────────────────────────────────────────┐
│  tmux.attach() → user enters tmux session            │
└──────────────────────────────────────────────────────┘
```

### Per-repo session discovery file

Alongside the global receipt under `~/.local/share/git-paw/sessions/`, `git paw
start` writes a **per-repo** discovery file at
`<repo>/.git-paw/sessions/<session>.json`. This is the surface the bundled
`sweep.sh` supervisor helper reads to find the active session and its agent
roster from inside the repo (without reaching into the XDG state dir). `purge`
removes it.

Its shape — stable for sweep.sh and forward-compatible (consumers ignore
unknown keys):

```json
{
  "session_name": "paw-myproject",
  "agents": [
    {
      "branch_id": "feat-add-auth",
      "worktree_path": "/abs/path/to/myproject-feat-add-auth",
      "cli": "claude",
      "pane_index": 2
    }
  ]
}
```

`branch_id` is the broker agent id (slugified branch); `pane_index` is the
agent's tmux pane within the session window. When the file is absent (e.g. a
supervisor attached to a pre-existing `paw-*` session), `sweep.sh` falls back to
resolving the session name from `$TMUX` / `tmux display-message -p '#S'`, so the
file never needs to be hand-authored.

## Broker Architecture

When `[broker] enabled = true`, the dashboard pane runs `git paw __dashboard`. This single process hosts both the HTTP broker and the dashboard TUI. The dashboard pane sits at pane 1 in supervisor mode and at pane 0 in non-supervisor broker mode.

```
Dashboard pane process (git paw __dashboard):
├── tokio runtime (background threads)
│   ├── axum HTTP server on localhost:9119
│   │   ├── POST /publish
│   │   ├── POST /watch   (register a hot-added worktree as a live watch target)
│   │   ├── GET /messages/:agent_id?since=N
│   │   └── GET /status
│   ├── Filesystem watcher (src/broker/watcher.rs)
│   │   └── Auto-publishes agent.status on file changes; prunes a vanished worktree
│   ├── Conflict detector (src/broker/conflict.rs)
│   │   └── Forward / in-flight / ownership shapes
│   └── Learnings aggregator (src/broker/learnings.rs)
│       └── Opt-in; flushes to .git-paw/session-learnings.md
├── Flush thread (std::thread, 5s interval)
│   └── Appends to broker.log
└── Main thread
    └── ratatui dashboard (1s tick)
```

### Broker state

The broker state is held in `Arc<Mutex<...>>` by `src/broker/mod.rs` and shared
between the axum server handlers, the watcher, the conflict detector, the
learnings aggregator, and the ratatui dashboard render loop. The server writes
incoming messages (validated and sequenced by `src/broker/publish.rs`, routed
by `src/broker/delivery.rs`); the dashboard reads the latest snapshot each
tick.

The flush thread periodically serializes the message log to
`.git-paw/broker.log` as a JSONL audit trail. This runs on a plain
`std::thread` to avoid contention with the tokio runtime.

### Environment injection

When the broker is enabled, git-paw sets `GIT_PAW_BROKER_URL=http://127.0.0.1:<port>` in the tmux environment for the session. Each agent pane inherits this variable and can use it to communicate with the broker.

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

The top row is fixed at 50% of the supervisor pane width and the agent rows
share the remaining vertical space. Row-height proportions for the agent
grid depend on how many bottom rows the layout produces:

| Agent rows | Bottom-row heights |
|------------|--------------------|
| 1 | 60% (top row 40%) |
| 2 | 40% / 30% / 30%  (top + 2 bottom rows) |
| 3 | 28% / 24% / 24% / 24% |
| 4 | 28% / 18% / 18% / 18% / 18% |
| 5 | 28% / 14.4% / 14.4% / 14.4% / 14.4% / 14.4% |

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
`src/tmux.rs` are the source of truth.

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
it would be interpreted line-by-line as failing commands. The same gate guards
both the `git paw start` and `git paw add` launch paths.

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

For a project named `my-app` and branch `feature/auth-flow`:

```
my-app/                         ← main repo (current directory)
my-app-feature-auth-flow/       ← worktree (created by git-paw)
my-app-feat-api/                ← worktree (created by git-paw)
```

Worktrees are created as siblings of the main repo directory. The naming convention is `<project>-<sanitized-branch>` where slashes become hyphens.

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

## Session State

Session state is persisted as JSON under `~/.local/share/git-paw/sessions/`:

```json
{
  "session_name": "paw-my-app",
  "repo_path": "/Users/you/projects/my-app",
  "project_name": "my-app",
  "created_at": "2025-01-15T10:30:00Z",
  "status": "active",
  "broker_port": 9119,
  "broker_enabled": true,
  "worktrees": [
    {
      "branch": "feat/auth",
      "worktree_path": "/Users/you/projects/my-app-feat-auth",
      "cli": "claude"
    },
    {
      "branch": "feat/api",
      "worktree_path": "/Users/you/projects/my-app-feat-api",
      "cli": "gemini"
    }
  ]
}
```

The `broker_port` and `broker_enabled` fields are present when the broker is configured. They allow `git paw status` to display broker information and `git paw purge` to clean up `broker.log`.

### Atomic writes

Session state is written atomically: write to a temporary file, then rename. This prevents corruption if the process is killed mid-write.

### Effective status

The on-disk status may not reflect reality (e.g., tmux was killed externally). git-paw checks the actual tmux state:

| File status | tmux alive? | Effective status |
|-------------|-------------|-----------------|
| `active` | Yes | Active (reattach) |
| `active` | No | Stopped (recover) |
| `stopped` | N/A | Stopped (recover) |
| No file | N/A | No session |

## Tmux Builder Pattern

The tmux module uses a builder pattern that accumulates operations as data structures rather than immediately executing shell commands. This enables:

- **Testability** — generate commands without executing them
- **Dry run** — print the plan without side effects
- **Atomicity** — validate the full plan before running anything

```rust
TmuxSessionBuilder::new()
    .session_name("paw-my-app")
    .pane(PaneSpec { branch, worktree_path, cli_command })
    .pane(PaneSpec { ... })
    .mouse(true)
    .build()
    // → TmuxSession { name, commands: Vec<TmuxCommand> }
```

The built `TmuxSession` can be inspected, printed (dry run), or executed.

### CLI-launch robustness

Panes are created with shell auto-update prompts suppressed
(`new-session`/session env set `DISABLE_AUTO_UPDATE=true` and
`DISABLE_UPDATE_PROMPT=true`) so an interactive framework prompt (e.g.
oh-my-zsh's `Would you like to update? [Y/n]`) cannot fire as the pane's
shell reads its rc and swallow the first keystroke of the CLI-launch command.
As a second layer, the builder sends a `C-u` line-clear immediately before
each CLI-launch command, so any stray pending input cannot corrupt it. The
headless `new-session` canvas is sized (480×140) to tile a supervisor session
with several agents when no client is attached; a real terminal resizes the
session on attach.

## Error Strategy

All errors flow through `PawError` (defined with `thiserror`). Each variant carries an actionable message telling the user what went wrong and how to fix it. No panics in non-test code — all `Result` propagation.

Exit codes:
- **0** — success
- **1** — operational error
- **2** — user cancelled
