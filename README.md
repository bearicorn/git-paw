# git-paw

**Parallel AI Worktrees** — orchestrate multiple AI coding CLI sessions across git worktrees from a single terminal using tmux.

[![CI](https://github.com/bearicorn/git-paw/actions/workflows/ci.yml/badge.svg)](https://github.com/bearicorn/git-paw/actions/workflows/ci.yml)
[![Tests](https://github.com/bearicorn/git-paw/actions/workflows/ci.yml/badge.svg?event=push)](https://github.com/bearicorn/git-paw/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/bearicorn/git-paw/branch/main/graph/badge.svg)](https://codecov.io/gh/bearicorn/git-paw)
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
- **Smart session management** — reattach to active sessions, auto-recover after crashes or reboots
- **Interactive or scripted** — fuzzy branch picker and CLI selector, or pass `--cli` and `--branches` flags
- **Per-branch CLI assignment** — use Claude on one branch and Gemini on another in the same session
- **Presets** — save branch + CLI combos in config for one-command launches
- **Custom CLI support** — register any AI CLI with `git paw add-cli`
- **Session persistence** — state saved to disk, survives tmux crashes and system reboots
- **Dry run** — preview the session plan before executing with `--dry-run`
- **Mouse-friendly tmux** — click to switch panes, drag borders to resize, scroll with mouse wheel

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

## Usage

### `start` — Launch or reattach

```bash
# Interactive launch
git paw start

# Specify CLI and branches
git paw start --cli claude --branches feat/auth,feat/api

# Use a preset from config
git paw start --preset backend

# Preview without executing
git paw start --dry-run
```

Smart behavior:
- **Active session exists** → reattaches
- **Stopped/crashed session** → auto-recovers (reuses worktrees, relaunches CLIs)
- **No session** → full interactive launch

### `stop` — Pause session

```bash
git paw stop
```

Kills the tmux session but preserves worktrees and state. Run `git paw start` later to pick up where you left off.

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

## Configuration

### Per-repo config (`.git-paw.toml`)

```toml
default_cli = "claude"
mouse = true

[presets.backend]
branches = ["feat/api", "fix/db"]
cli = "claude"
```

### Global config (`~/.config/git-paw/config.toml`)

```toml
default_cli = "claude"
mouse = true

[clis.my-agent]
command = "/usr/local/bin/my-agent"
display_name = "My Agent"

[clis.local-llm]
command = "ollama-code"
display_name = "Local LLM"

[presets.backend]
branches = ["feat/api", "fix/db"]
cli = "claude"
```

Per-repo config overrides global config for overlapping fields.

## Supported AI CLIs

| CLI | Binary | Link |
|-----|--------|------|
| Claude Code | `claude` | [claude.ai](https://claude.ai/download) |
| OpenAI Codex | `codex` | [github.com/openai/codex](https://github.com/openai/codex) |
| Google Gemini CLI | `gemini` | [github.com/google-gemini/gemini-cli](https://github.com/google-gemini/gemini-cli) |
| Aider | `aider` | [aider.chat](https://aider.chat) |
| Mistral | `mistral` | [mistral.ai](https://mistral.ai) |
| Qwen | `qwen` | [github.com/QwenLM/qwen-agent](https://github.com/QwenLM/qwen-agent) |
| Amp | `amp` | [amp.dev](https://amp.dev) |
| GitHub Copilot | `copilot` | [github.com/features/copilot](https://github.com/features/copilot) |

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

git paw stop   → kills tmux, keeps worktrees + state
git paw start  → auto-recovers from saved state
git paw purge  → removes everything
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

[MIT](LICENSE) — Copyright 2026 bearicorn
