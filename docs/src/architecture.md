# Architecture

This chapter covers git-paw's internal architecture: module structure, data flow, and key design decisions.

## Module Diagram

```
┌─────────────────────────────────────────────────┐
│                    main.rs                       │
│              (entry point, dispatch)             │
├────────┬──────────┬───────────┬─────────────────┤
│        │          │           │                  │
│   cli.rs    interactive.rs  config.rs   error.rs │
│  (clap)    (dialoguer UI)   (TOML)    (PawError) │
│        │          │           │                  │
├────────┴──────────┴───────────┴─────────────────┤
│                                                   │
│   detect.rs      git.rs      tmux.rs  session.rs │
│  (PATH scan)   (worktrees)  (builder)  (JSON)    │
│                                                   │
├─────────────────────────────────────────────────┤
│                                                   │
│   broker/         skills.rs   dashboard.rs       │
│   ├── mod.rs       (template   (ratatui TUI,     │
│   ├── server.rs     injection)  pane 0 status)   │
│   ├── state.rs                                   │
│   └── flush.rs                                   │
│                                                   │
└───────────────────────────────────────────────────┘
```

### Module Responsibilities

| Module | File | Purpose |
|--------|------|---------|
| **CLI** | `src/cli.rs` | Argument parsing with clap v4 derive macros. Defines all subcommands, flags, and help text. |
| **Detection** | `src/detect.rs` | Scans PATH for 8 known AI CLI binaries. Resolves custom CLIs from config. Merges and deduplicates. |
| **Git** | `src/git.rs` | Validates git repos, lists branches (local + remote, deduplicated), creates/removes worktrees, derives safe directory names. |
| **Tmux** | `src/tmux.rs` | Builder pattern for tmux operations. Creates sessions, splits panes, sends commands, applies tiled layout, sets pane titles. |
| **Session** | `src/session.rs` | Persists session state to JSON files under `~/.local/share/git-paw/sessions/`. Atomic writes, crash recovery. |
| **Config** | `src/config.rs` | Parses TOML from global (`~/.config/git-paw/config.toml`) and per-repo (`.git-paw/config.toml`). Merges with repo-wins semantics. |
| **Interactive** | `src/interactive.rs` | Terminal prompts via dialoguer. Mode picker, branch multi-select, CLI picker. Skips prompts when flags are provided. |
| **Error** | `src/error.rs` | `PawError` enum with thiserror. Actionable error messages and distinct exit codes. |
| **Broker** | `src/broker/` | HTTP coordination server (axum). Receives status, artifact, and blocked messages from agents. Provides cursor-based polling. |
| **Dashboard** | `src/dashboard.rs` | Ratatui TUI running in pane 0. Renders live agent status table. Embeds the broker via shared state. |
| **Skills** | `src/skills.rs` | Loads standardized agent skills from `.agents/skills/` following the [agentskills.io specification](https://agentskills.io). Injects coordination instructions into worktree AGENTS.md files. |

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
┌─ Broker enabled? ───────────────────────────────────┐
│  yes → Pane 0 runs `git paw __dashboard`             │
│         ├─ Starts axum HTTP server on configured port│
│         ├─ Injects GIT_PAW_BROKER_URL into all panes │
│         └─ Renders ratatui status table              │
│  no  → Pane 0 runs the first agent CLI (v0.2 path)  │
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

## Broker Architecture

When `[broker] enabled = true`, pane 0 runs `git paw __dashboard` instead of an agent CLI. This single process hosts both the HTTP broker and the dashboard TUI.

```
Pane 0 process (git paw __dashboard):
├── tokio runtime (background threads)
│   └── axum HTTP server on localhost:9119
│       ├── POST /publish
│       ├── GET /messages/:agent_id?since=N
│       └── GET /status
├── Flush thread (std::thread, 5s interval)
│   └── Appends to broker.log
└── Main thread
    └── ratatui dashboard (1s tick)
```

### BrokerState

The `BrokerState` struct (in `src/broker/state.rs`) is wrapped in `Arc<Mutex<...>>` and shared between the axum server handlers and the ratatui dashboard render loop. The server writes incoming messages; the dashboard reads the latest state on each tick.

The flush thread periodically serializes the message log to `.git-paw/broker.log` as a JSONL audit trail. This runs on a plain `std::thread` to avoid contention with the tokio runtime.

### Environment injection

When the broker is enabled, git-paw sets `GIT_PAW_BROKER_URL=http://127.0.0.1:<port>` in the tmux environment for the session. Each agent pane inherits this variable and can use it to communicate with the broker.

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

## Error Strategy

All errors flow through `PawError` (defined with `thiserror`). Each variant carries an actionable message telling the user what went wrong and how to fix it. No panics in non-test code — all `Result` propagation.

Exit codes:
- **0** — success
- **1** — operational error
- **2** — user cancelled
