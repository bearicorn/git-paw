# git-paw

**Parallel AI Worktrees** — orchestrate multiple AI coding CLI sessions across git worktrees from a single terminal using tmux.

[![CI](https://github.com/bearicorn/git-paw/actions/workflows/ci.yml/badge.svg)](https://github.com/bearicorn/git-paw/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/git-paw.svg)](https://crates.io/crates/git-paw)
[![Downloads](https://img.shields.io/crates/d/git-paw.svg)](https://crates.io/crates/git-paw)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![MSRV: stable](https://img.shields.io/badge/MSRV-stable-brightgreen.svg)](rust-toolchain.toml)

## Demo

```
$ git paw

  🐾 git-paw — Parallel AI Worktrees

  ? Select mode:
  > Same CLI for all branches
    Different CLI per branch

  ? Select branches (space to toggle, enter to confirm):
  > [x] feat/auth
    [x] feat/api
    [ ] fix/typo
    [ ] main

  ? Select AI CLI:
  > claude
    codex
    gemini

  ✔ Creating worktrees...
    ../myproject-feat-auth/
    ../myproject-feat-api/

  ✔ Launching tmux session: paw-myproject
    Pane 1: feat/auth → claude
    Pane 2: feat/api → claude

  Attaching to tmux session...
```

## What It Does

git-paw lets you run multiple AI coding assistants in parallel, each in its own git worktree and tmux pane. Pick your branches, pick your AI CLI(s), and git-paw handles the rest — creating worktrees, launching a tmux session, and wiring everything together. Stop and resume sessions at will; your worktrees and uncommitted work are preserved.

## Features

- **Parallel AI sessions** — run Claude, Codex, Gemini, or any AI CLI across multiple branches simultaneously
- **Git worktree isolation** — each branch gets its own working directory, no stashing or switching needed
- **Smart session management** — reattach to active sessions, auto-recover after crashes or reboots, and rebase existing agent branches onto the repository's default branch on every `git paw start` so agents never drift behind supervisor commits on `main` (pass `--no-rebase` to opt out)
- **Interactive or scripted** — fuzzy branch picker and CLI selector, or pass `--cli` and `--branches` flags
- **Per-branch CLI assignment** — use Claude on one branch and Gemini on another in the same session
- **Presets** — save branch + CLI combos in config for one-command launches
- **Custom CLI support** — register any AI CLI with `git paw add-cli`
- **Session persistence** — state saved to disk, survives tmux crashes and system reboots
- **Dry run** — preview the session plan before executing with `--dry-run`
- **Mouse-friendly tmux** — click to switch panes, drag borders to resize, scroll with mouse wheel
- **Spec-driven launch** — auto-discover specs and launch sessions with `--from-all-specs` (or narrow to a subset via `--specs NAME[,NAME...]`)
- **AGENTS.md integration** — auto-inject session context into worktree AGENTS.md files
- **Session logging** — capture raw terminal output per pane for later review
- **Replay** — view session logs with ANSI stripping or colored output via `less -R`
- **Project init** — `git paw init` bootstraps `.git-paw/`, config, and gitignore
- **Standards-based** — uses `AGENTS.md` following the Linux Foundation standard for AI agent instructions
- **Agent coordination** — built-in HTTP broker lets agents share status, artifacts, and blocked requests
- **Dashboard TUI** — live status table in pane 0 shows agent progress at a glance
- **Broker messages panel** — optional dashboard section showing real-time agent communication (configurable via `[dashboard] show_message_log = true`)
- **Skill templates** — coordination instructions auto-injected into each agent's AGENTS.md
- **Boot-prompt injection** — standardized boot instructions automatically prepended to all agent prompts, ensuring reliable self-reporting (register, done, blocked, question operations) — always enabled for broker sessions
- **Cursor-based messaging** — lossless message polling with sequence tracking
- **Spec Kit backend** — first-class support for [GitHub Spec Kit](https://github.com/github/spec-kit) projects via `[specs] type = "speckit"`; `.specify/specs/` is auto-detected at the repo root and the `[P]`/non-`[P]` task split decomposes into per-task and consolidated worktrees
- **`--specs-format` override** — force-select the spec backend (`openspec`, `markdown`, `speckit`) on the command line, overriding both `[specs] type` in config and the `.specify/` auto-detection
- **`--no-supervisor`** — single-session override of `[supervisor] enabled = true` for plain (non-supervisor) operation without editing config
- **`start --force`** — bypass the uncommitted-spec validation warning when launching with `--from-all-specs` or `--specs`
- **Forward coordination** — agents publish `agent.intent` before they begin editing so peers (and the broker conflict detector) see the planned file set ahead of the first commit
- **Automatic conflict detection** — the broker auto-emits `[conflict-detector]`-tagged `agent.feedback` for forward (overlapping intents), in-flight (overlapping `modified_files`), and ownership-violation conflicts; unresolved in-flight overlaps escalate to the supervisor inbox via `agent.question`
- **Learnings mode** — opt-in `[supervisor] learnings = true` collects deterministic friction signals (stuck duration, recovery cycles, forward conflicts, in-flight conflicts, ownership violations) into `.git-paw/session-learnings.md` for post-session review
- **Governance pointers** — point the supervisor at your existing ADRs, test strategy, security checklist, DoD, and constitution via the `[governance]` config table; Spec Kit projects auto-wire `.specify/memory/constitution.md` when present
- **Auto-approval policy** — `[supervisor.auto_approve]` controls safe-command prefixes and approval level for stalled-pane sweeps; `[supervisor.common_dev_allowlist]` seeds a curated dev-loop preset into `.claude/settings.json` so common build/test/git commands bypass per-prompt approval
- **Conflict-detector tuning** — `[supervisor.conflict]` exposes the in-flight escalation window (`window_seconds`), the intent-overlap warning toggle (`warn_on_intent_overlap`), and the ownership-violation escalation toggle (`escalate_on_violation`)
- **Learnings flush cadence** — `[supervisor.learnings_config] flush_interval_seconds` (default 60) controls how often learnings entries are flushed from memory to `.git-paw/session-learnings.md`
- **Routing through the supervisor** — type `/agents` in the supervisor pane to see the live agent inventory (status, mode, pane) and `/tell <agent> <prompt>` to route a prompt to one agent without tab-switching into its pane; `[supervisor.tell] mode` picks the delivery channel (`feedback` queue by default, `send-keys` for accept-edits agents) and every route is recorded in the session learnings

> **Tip:** git-paw uses `AGENTS.md` as the standard agent instruction file. If your AI CLI reads a different file (e.g., `CLAUDE.md`, `GEMINI.md`), you can symlink it:
> ```bash
> ln -s AGENTS.md CLAUDE.md   # Claude Code reads CLAUDE.md
> ln -s AGENTS.md GEMINI.md   # Gemini reads GEMINI.md
> ```
> Add these symlinks to `.gitignore` so they stay local to each developer.

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| macOS (ARM) | Supported | Primary development platform |
| macOS (x86) | Supported | |
| Linux (x86_64) | Supported | |
| Linux (ARM64) | Supported | |
| Windows | WSL only | tmux is a Unix tool — use Windows Subsystem for Linux |

> **Why no native Windows?** git-paw relies on tmux for terminal multiplexing, which is not available natively on Windows. WSL provides a full Linux environment where git-paw works perfectly.

## Quick Start: Same CLI

Use the same AI CLI across all branches:

```bash
# Interactive — pick branches and CLI from prompts
git paw

# Non-interactive — specify everything upfront
git paw start --cli claude --branches feat/auth,feat/api
```

This creates:
- A worktree for each branch (`../yourproject-feat-auth/`, `../yourproject-feat-api/`)
- A tmux session with one pane per branch, each running `claude`

## Quick Start: Per-Branch CLI

Use different AI CLIs on different branches:

```bash
# Interactive mode — select "Different CLI per branch"
git paw
# → Pick branches: feat/auth, feat/api
# → Pick CLI for feat/auth: claude
# → Pick CLI for feat/api: gemini
```

Result: a tmux session where `feat/auth` runs Claude and `feat/api` runs Gemini, side by side.

## Quick Start: Supervisor Mode

Run an unattended supervisor agent that orchestrates the worker agents on your behalf:

```bash
git paw start --supervisor

# Skip supervisor for a single run even when [supervisor] enabled = true is set
git paw start --no-supervisor

# Bypass the uncommitted-spec validation warning when launching from specs
git paw start --from-all-specs --force
git paw start --supervisor --force
```

The supervisor agent runs in its own pane, polls each worker agent for progress and artifacts via the broker, runs the configured test command between merges, and writes a session summary when work completes. Use this mode when you want to leave a multi-branch session running without continually steering each agent yourself.

In v0.5.0 supervisor mode also seeds a curated dev-command allowlist into `.claude/settings.json` on session start so common dev-loop commands (`cargo build`, `git commit`, `just`, `mdbook build`, `openspec validate`, ...) bypass per-prompt approval. Opt out with `[supervisor.common_dev_allowlist] enabled = false`; extend with `extra = [...]`.

`--no-supervisor` is the highest-precedence step in the supervisor-mode resolution chain — it wins over both `[supervisor] enabled = true` in config and any interactive prompt. It is mutually exclusive with `--supervisor`; passing both fails at parse time. `--force` only matters for spec-mode launches (`--from-all-specs` / `--specs`) and bypasses the warning when uncommitted spec changes are detected on disk.

## Installation

### From crates.io

```bash
cargo install git-paw
```

### Homebrew

```bash
brew install bearicorn/tap/git-paw
```

### Shell installer

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/bearicorn/git-paw/releases/latest/download/git-paw-installer.sh | sh
```

### Windows (WSL)

Install [WSL](https://learn.microsoft.com/en-us/windows/wsl/install), then use any of the Linux installation methods above inside your WSL environment:

```bash
# Inside WSL
sudo apt install tmux
cargo install git-paw
```

### Prerequisites

- [tmux](https://github.com/tmux/tmux) — terminal multiplexer
- [Git](https://git-scm.com/) — with worktree support (2.5+)

### `git paw` vs `git-paw`

Once installed to your PATH, git-paw works as a git subcommand:

```bash
git paw start    # git finds `git-paw` on PATH automatically
```

You can also call the binary directly — useful during development or if it's not on PATH:

```bash
git-paw start    # equivalent
cargo run -- start  # during development
```

All examples below use `git paw`, but `git-paw` works identically.

## Usage

### `init` — Initialize project

```bash
git paw init
```

Creates `.git-paw/` directory with default config and sets up `.gitignore` for logs.

### `start` — Launch or reattach

```bash
# Interactive launch
git paw start

# Specify CLI and branches
git paw start --cli claude --branches feat/auth,feat/api

# Launch every discovered spec (OpenSpec, Markdown, or Spec Kit)
git paw start --from-all-specs
git paw start --from-all-specs --cli claude

# Force-select the spec backend, overriding [specs] type and .specify/ auto-detection
git paw start --from-all-specs --specs-format speckit

# Narrow to specific specs, or open a multi-select picker
git paw start --specs add-auth,fix-session
git paw start --specs   # interactive picker (requires a TTY)

# Use a preset from config
git paw start --preset backend

# Preview without executing
git paw start --dry-run

# Bypass the uncommitted-spec validation warning
git paw start --from-all-specs --force
```

Smart behavior:
- **Active session exists** → reattaches
- **Stopped/crashed session** → auto-recovers (reuses worktrees, relaunches CLIs)
- **No session** → full interactive launch

### `add` — Attach a branch mid-session (v0.6.0+)

```bash
git paw add feat/new-thing            # attach a worktree + agent pane (session's default CLI)
git paw add feat/api --cli codex      # choose the CLI for the new pane
git paw add --from-spec add-export    # derive branch + CLI from a discovered spec
```

Hot-attaches a worktree and agent pane to a running supervisor session — no stop/purge/restart, the other agents keep working. The grid re-tiles to the layout a `start` of that many agents would produce, the new agent boots with the same broker boot block + prompt a start-time agent gets, and the supervisor discovers it on its next sweep. Adding past the 25-agent cap is rejected; adding to a paused session leaves the new pane paused until `git paw resume`. See [Session Lifecycle](docs/src/user-guide/session-lifecycle.md#adding-and-removing-branches-mid-session).

### `remove` — Detach a single agent (v0.6.0+)

```bash
git paw remove feat/done-thing            # close pane, remove worktree, drop from session
git paw remove feat/wip --force           # remove even with uncommitted changes
git paw remove feat/keep --keep-worktree  # detach pane only; leave worktree + branch on disk
```

Detaches one agent: closes its pane, re-tiles the grid for the smaller agent count, removes its worktree (reusing `purge`'s teardown), and drops it from the session. Safe by default — refuses a worktree with uncommitted changes (listing what would be lost) unless `--force`; `--keep-worktree` detaches the pane but leaves the worktree on disk. `git paw remove supervisor` is refused — use `git paw stop` to end the whole session.

### `pause` — Soft-stop the session (v0.5.0+)

```bash
git paw pause
```

Detaches the tmux client, stops the broker, and leaves every CLI pane running. Preserves agent conversation state for instant resume via `git paw start`. Holds RAM (~300 MB per Claude pane), so use it for short breaks (lunch, meetings, end-of-day). See [Pause and Resume](docs/src/user-guide/pause.md) for the full trade-off.

### `stop` — Kill the CLIs, keep the worktrees

```bash
git paw stop          # prompts for confirmation in a TTY
git paw stop --force  # skip the prompt (scripts)
```

Kills the tmux session and every CLI pane process but preserves worktrees and state on disk. CLI conversation context is lost. Run `git paw start` later to recover with fresh CLI processes.

### `purge` — Remove everything

```bash
# With confirmation prompt
git paw purge

# Skip confirmation
git paw purge --force
```

Removes the tmux session, all worktrees, and session state.

### `status` — Check session state

```bash
git paw status
```

Shows session name, branches, CLIs, and status (active/stopped/no session).

### `list-clis` — Show available CLIs

```bash
git paw list-clis
```

Lists auto-detected and custom AI CLIs with their source.

### `add-cli` — Register a custom CLI

```bash
# With absolute path
git paw add-cli my-agent /usr/local/bin/my-agent

# With display name
git paw add-cli my-agent my-agent --display-name "My Agent"
```

### `remove-cli` — Unregister a custom CLI

```bash
git paw remove-cli my-agent
```

Only custom CLIs can be removed — auto-detected CLIs cannot.

### `replay` — View session logs

```bash
# List available log sessions
git paw replay --list

# View a branch's log (ANSI stripped)
git paw replay feat/auth

# View with colors via less -R
git paw replay feat/auth --color

# Replay from a specific session
git paw replay feat/auth --session paw-myproject
```

Requires session logging to be enabled in config.

## Configuration

### Per-repo config (`.git-paw/config.toml`)

```toml
# Pre-select a CLI in the interactive picker
default_cli = "my-cli"
mouse = true

# Bypass picker entirely for spec-mode launches (--from-all-specs, --specs)
default_spec_cli = "my-cli"

# Prefix for spec-derived branches (default: "spec/")
branch_prefix = "spec/"

# Spec scanning
[specs]
dir = "specs"
type = "openspec"  # "openspec", "markdown", or "speckit"

# Session logging
[logging]
enabled = true

# Dashboard configuration
[dashboard]
# Show broker messages panel for real-time agent communication
show_message_log = true

[presets.backend]
branches = ["feat/api", "fix/db"]
cli = "my-cli"
```

### Global config (`~/.config/git-paw/config.toml`)

```toml
default_cli = "my-cli"
mouse = true

[clis.my-agent]
command = "/usr/local/bin/my-agent"
display_name = "My Agent"

[clis.local-llm]
command = "ollama-code"
display_name = "Local LLM"

[presets.backend]
branches = ["feat/api", "fix/db"]
cli = "my-cli"
```

Per-repo config overrides global config for overlapping fields.

## Supported AI CLIs

git-paw auto-detects the following AI coding CLIs when they are on `PATH`. The list reflects `src/detect.rs::KNOWN_CLIS` at the time of this release; the table grows as binaries land in upstream releases.

| CLI | Binary |
|-----|--------|
| Claude Code | `claude` |
| OpenAI Codex | `codex` |
| Google Gemini CLI | `gemini` |
| Aider | `aider` |
| Vibe | `vibe` |
| Qwen | `qwen` |
| Amp | `amp` |
| opencode | `opencode` |
| Cline | `cline` |
| Droid | `droid` |
| Pi | `pi` |
| Junie | `junie` |
| Cursor Agent | `cursor` |
| GitHub Copilot CLI | `copilot` |
| cn | `cn` |
| Kilo Code | `kilo` |
| Kimi | `kimi` |

Don't see your CLI? Register it:

```bash
git paw add-cli my-cli /path/to/my-cli --display-name "My CLI"
```

## How It Works

```
git paw start
    │
    ├─ 1. Validate git repo
    ├─ 2. Load config (global + repo)
    ├─ 3. Detect AI CLIs on PATH + custom
    ├─ 4. Interactive selection (or use flags)
    ├─ 5. Create git worktrees
    │      ../project-feat-auth/
    │      ../project-feat-api/
    ├─ 6. Create tmux session (paw-project)
    │      ┌─────────────────┬─────────────────┐
    │      │ feat/auth        │ feat/api         │
    │      │ → claude         │ → claude         │
    │      │                  │                  │
    │      └─────────────────┴─────────────────┘
    ├─ 7. Save session state to disk
    └─ 8. Attach to tmux session

git paw pause  → soft stop (detach + broker stop; CLIs keep running)
git paw stop   → kills CLIs, keeps worktrees + state
git paw start  → auto-recovers (or restarts a paused session)
git paw purge  → removes everything
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Releases

Releases follow a single `chore: prepare vX.Y.Z release` commit on `main` that
bumps `Cargo.toml`, regenerates `CHANGELOG.md` via `git cliff`, and archives
completed OpenSpec changes (moving them under
`openspec/changes/archive/<date>-<change>/` and syncing their delta specs into
`openspec/specs/`). Pushing the `vX.Y.Z` tag triggers
[`cargo-dist`](https://github.com/axodotdev/cargo-dist) on GitHub Actions to
build cross-platform binaries and update the Homebrew tap.

After the tag, the maintainer publishes to crates.io **manually** (it is not
wired into cargo-dist):

```bash
cargo publish --dry-run   # verify
cargo publish             # upload vX.Y.Z
```

The full procedure (archive ordering, changelog regeneration, tag rules,
crates.io publish step, recovery from a botched prep commit) lives in
[`AGENTS.md` § Release & Distribution](AGENTS.md#release--distribution).

## License

[MIT](LICENSE) — Copyright 2026 bearicorn
