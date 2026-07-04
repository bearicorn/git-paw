<div align="center">

<img src="https://raw.githubusercontent.com/bearicorn/git-paw/main/.github/assets/logo.jpg" alt="git-paw logo" width="152">

# git-paw

**Parallel AI Worktrees** — orchestrate multiple AI coding CLI sessions across git worktrees from a single terminal using tmux.

[![CI](https://github.com/bearicorn/git-paw/actions/workflows/ci.yml/badge.svg)](https://github.com/bearicorn/git-paw/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/git-paw.svg)](https://crates.io/crates/git-paw)
[![Downloads](https://img.shields.io/crates/d/git-paw.svg)](https://crates.io/crates/git-paw)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![MSRV: stable](https://img.shields.io/badge/MSRV-stable-brightgreen.svg)](rust-toolchain.toml)

</div>

## Demo

```
$ git paw

  🐾 git-paw — Parallel AI Worktrees

  ? Select mode:
  > Same CLI for all branches
    Different CLI per branch

  ? Select branches (type to filter, space to toggle, enter to confirm):
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

- **Contained worktree layout (v0.8.0)** — new repos place agent worktrees *inside* the project at `.git-paw/worktrees/<branch-slug>/` instead of scattering them as siblings of the repo, enabling a single project-scoped permission grant for every agent. Configurable via `worktree_placement` (`"child"` | `"sibling"`); existing repos default to the v0.7.0 sibling layout and stay there until they opt in. See [Worktree Placement](docs/src/user-guide/worktree-placement.md)
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
- **`start --unattended`** — run a supervisor wave to completion with no human in the seat: engages supervisor mode and drives an in-process loop that auto-approves classifier-safe prompts, escalates the rest for later review without blocking the wave, detects completion, and exits with a summary (designed for detached operation; mutually exclusive with `--no-supervisor`). See [Supervisor → Unattended mode](docs/src/user-guide/supervisor.md#unattended-mode---unattended)
- **`start --force`** — bypass the uncommitted-spec validation warning when launching with `--from-all-specs` or `--specs`
- **Forward coordination** — agents publish `agent.intent` before they begin editing so peers (and the broker conflict detector) see the planned file set ahead of the first commit
- **Automatic conflict detection** — the broker auto-emits `[conflict-detector]`-tagged `agent.feedback` for forward (overlapping intents), in-flight (overlapping `modified_files`), and ownership-violation conflicts; an unresolved in-flight overlap is classified from the agents' declared regions — a true collision escalates to the supervisor inbox via `agent.question`, while an additive overlap (disjoint declared regions) is downgraded to an informational `agent.feedback` and never blocks on human input
- **Learnings mode** — opt-in `[supervisor] learnings = true` collects deterministic friction signals (stuck duration, recovery cycles, forward conflicts, in-flight conflicts, ownership violations) into `.git-paw/session-learnings.md` for post-session review. **No telemetry** — the file is purely local and nothing is sent anywhere; you can optionally share it via a [GitHub issue](https://github.com/bearicorn/git-paw/issues) (after reviewing/anonymising it) to help improve the tool. See [Learnings → Privacy & Sharing](docs/src/user-guide/learnings.md#privacy--sharing)
- **Governance pointers** — point the supervisor at your existing ADRs, test strategy, security checklist, DoD, and constitution via the `[governance]` config table; Spec Kit projects auto-wire `.specify/memory/constitution.md` when present
- **Auto-approval policy** — `[supervisor.auto_approve]` controls safe-command prefixes and approval level for stalled-pane sweeps; `[supervisor.common_dev_allowlist]` seeds a curated dev-loop preset into `.claude/settings.json` so common build/test/git commands bypass per-prompt approval
- **Conflict-detector tuning** — `[supervisor.conflict]` exposes the in-flight escalation window (`window_seconds`), the intent-overlap warning toggle (`warn_on_intent_overlap`), and the ownership-violation escalation toggle (`escalate_on_violation`)
- **Learnings flush cadence** — `[supervisor.learnings_config] flush_interval_seconds` (default 60) controls how often learnings entries are flushed from memory to `.git-paw/session-learnings.md`
- **Routing through the supervisor** — type `/agents` in the supervisor pane to see the live agent inventory (status, mode, pane) and `/tell <agent> <prompt>` to route a prompt to one agent without tab-switching into its pane; `[supervisor.tell] mode` picks the delivery channel (`feedback` queue by default, `send-keys` for accept-edits agents) and every route is recorded in the session learnings
- **MCP server (v0.7.0+)** — `git paw mcp` exposes this repo's read-only state (coordination intents/conflicts, governance docs, specs/tasks, session status/learnings, agent skills, git context) over the [Model Context Protocol](https://modelcontextprotocol.io) so any MCP-aware client can query it; runs standalone over stdio, degrades gracefully when no session is active, and never invokes an agent CLI as a backend

> **Tip:** git-paw uses `AGENTS.md` as the standard agent instruction file. Point your CLI at it without duplicating content:
> - **Claude Code** reads only `CLAUDE.md` and supports imports, so create a `CLAUDE.md` whose first line imports `AGENTS.md`, then add any machine-local notes below it:
>   ```markdown
>   @AGENTS.md
>
>   # personal, machine-local notes (optional)
>   ```
>   The import stays in sync with `AGENTS.md` and survives tools that rewrite `CLAUDE.md` (e.g. `rtk init`) — a symlink gets clobbered by those and can't carry CLI-specific additions.
> - **Other CLIs** that read a fixed file and have no import syntax (e.g. Gemini → `GEMINI.md`) can symlink instead: `ln -s AGENTS.md GEMINI.md`.
>
> Add the local `CLAUDE.md` / symlinks to `.gitignore` so they stay per-developer.

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

# Drive the whole wave to completion with no human in the seat (detached)
git paw start --unattended --from-all-specs
git paw start --unattended --branches feat/auth,feat/api
```

The supervisor agent runs in its own pane, polls each worker agent for progress and artifacts via the broker, runs the configured test command between merges, and writes a session summary when work completes. Use this mode when you want to leave a multi-branch session running without continually steering each agent yourself.

By default `git paw start --supervisor` builds the session and returns, expecting you to watch the panes and clear safe permission prompts. Add **`--unattended`** to fold that operator loop into the tool: it engages supervisor mode and drives an in-process loop that sweeps every pane (including the supervisor's own) on a ~15-second cadence, auto-approves classifier-safe prompts, escalates risky/unknown prompts for later review without blocking the wave, and exits with a summary on completion or a ~25-minute heartbeat. It needs no attached terminal. See [Supervisor → Unattended mode](docs/src/user-guide/supervisor.md#unattended-mode---unattended).

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

`git paw <command>` (equivalently `git-paw <command>`). One-line summary below — the **full reference, with every flag and example, lives in the [CLI Reference](https://bearicorn.github.io/git-paw/cli-reference.html)**.

| Command | What it does |
|---------|--------------|
| `init` | Scaffold `.git-paw/` (default config + `.gitignore`). |
| `start` | Launch or reattach a session — interactive, `--cli`/`--branches`, or spec-driven (`--from-all-specs`, `--specs`, `--preset`, `--dry-run`, `--unattended`). Auto-recovers a stopped session. |
| `add` | Attach a branch + agent pane mid-session (v0.6.0+). |
| `remove` | Detach one agent — pane + worktree — with `--force` / `--keep-worktree` (v0.6.0+). |
| `pause` | Soft-stop: detach and stop the broker, keep CLI panes for instant resume (v0.5.0+). |
| `stop` | Kill the tmux session and CLIs; keep worktrees + state. |
| `purge` | Remove the session, all worktrees, and state (`--force` to skip the prompt). |
| `status` | Show session name, branches, CLIs, and state. |
| `list-clis` / `add-cli` / `remove-cli` | Manage auto-detected and custom AI CLIs. |
| `replay` | View session logs (`--list`, `--color`, `--session`). |
| `mcp` | Read-only [MCP](https://modelcontextprotocol.io) server over stdio — no session or broker required (v0.7.0+). |
| `selftest` | Run a full lifecycle against a throwaway repo + dummy CLI to verify the plumbing (v0.9.0+). |

Full flags, examples, and per-client MCP setup: **[CLI Reference](https://bearicorn.github.io/git-paw/cli-reference.html)**.

## Configuration

git-paw reads a per-repo `.git-paw/config.toml` and a global `~/.config/git-paw/config.toml` (per-repo overrides global for overlapping fields). The common fields:

```toml
default_cli = "claude"        # pre-select in the interactive picker
default_spec_cli = "claude"   # skip the picker for spec-mode launches
branch_prefix = "spec/"       # prefix for spec-derived branches

[specs]
type = "openspec"             # "openspec" | "markdown" | "speckit"

[presets.backend]
branches = ["feat/api", "fix/db"]
cli = "claude"
```

Custom CLIs, dashboard, logging, broker, supervisor, and worktree placement each have their own sections — see the **[Configuration reference](https://bearicorn.github.io/git-paw/configuration/index.html)** for the full schema and defaults.

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

OpenSpec changes are archived **per-change on the feature branch** as they land
(syncing their delta specs into `openspec/specs/`); the archive directories are
gitignored, so the canonical post-archive state lives in `openspec/specs/`. The
`chore: prepare vX.Y.Z release` commit therefore touches **only** the version
bump in `Cargo.toml` and the `CHANGELOG.md` regenerated via `git cliff` — no
archive moves. Pushing the `vX.Y.Z` tag triggers
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
