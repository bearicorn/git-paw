# Introduction

**git-paw** (Parallel AI Worktrees) orchestrates multiple AI coding CLI sessions across git worktrees from a single terminal using tmux.

Working with AI coding assistants like Claude, Codex, or Antigravity is powerful — but what if you could run them in parallel across multiple branches at once? That's exactly what git-paw does.

## The Problem

You have a feature branch, a bugfix branch, and a refactoring branch. You want an AI assistant working on each one simultaneously. Normally you'd need to:

1. Open multiple terminals
2. Create git worktrees manually
3. Navigate to each worktree
4. Launch your AI CLI in each one
5. Juggle between them

## The Solution

With git-paw, you run a single command:

```bash
git paw
```

git-paw will:

- **Detect** which AI CLIs you have installed (Claude, Codex, Antigravity, Aider, etc.)
- **Prompt** you to pick branches and a CLI (or different CLIs per branch)
- **Create** git worktrees for each selected branch
- **Launch** a tmux session with one pane per branch, each running your chosen AI CLI
- **Persist** the session state so you can stop, resume, or recover after crashes

## Key Features

- **One command** to go from zero to parallel AI sessions. See [Quick Start: Same CLI](quick-start-same-cli.md).
- **Smart start** — reattaches to a live session, recovers a crashed one, or launches fresh, and rebases agent branches onto the default branch (`--no-rebase` to skip). See [Session lifecycle](user-guide/session-lifecycle.md).
- **Live branch-set editing** — `git paw add` and `git paw remove` attach or detach one agent mid-session. See [Adding and removing branches](user-guide/session-lifecycle.md#adding-and-removing-branches-mid-session).
- **Pause and stop** — `pause` keeps CLI panes for a fast resume; `stop` keeps worktrees and state. See [Pause](user-guide/pause.md).
- **Per-branch CLIs and provider families** — Claude on one branch, Codex on another; a `[clis.<name>]` entry with `family` inherits a known CLI's flags, resume and MCP form. See [Supported CLIs](supported-clis.md#provider-families).
- **Custom CLIs** — register any AI CLI with `git paw add-cli`. See [Supported CLIs](supported-clis.md).
- **Presets, flags and dry run** — save branch and CLI combos, script with `--cli` / `--branches`, preview with `--dry-run`. See [CLI reference](cli-reference.md).
- **Worktree placement** — agent worktrees inside the project at `.git-paw/worktrees/` or beside it. See [Worktree placement](user-guide/worktree-placement.md).
- **Runtime isolation** — per-worktree env files, port blocks and `on_create` / `on_remove` hooks. See [Runtime isolation](user-guide/runtime-isolation.md).
- **Spec-driven launch** — one worktree per pending spec from OpenSpec, Spec Kit, Superpowers or Markdown. See [Spec-driven launch](user-guide/spec-driven-launch.md).
- **AGENTS.md injection** — each worktree gets its assignment and coordination rules in a gitignored sidecar. See [AGENTS.md](user-guide/agents-md.md).
- **Broker and dashboard** — agents publish status, intents and artifacts; the dashboard shows them in pane 0. See [Coordination](user-guide/coordination.md) and [Dashboard](user-guide/dashboard.md).
- **Conflict detection** — overlapping intents, edits and ownership violations are flagged before they become merge conflicts. See [Conflict detection](user-guide/conflict-detection.md).
- **Orchestrator mode** — your own agent drives the session from outside it through the exported skill, `sweep.sh` (including verified sends with `sweep.sh tell`) or the MCP server. See [Supervisor](user-guide/supervisor.md).
- **Unattended runs** — `start --unattended` drives a wave to completion and escalates every permission prompt; it approves none. See [Unattended mode](user-guide/supervisor.md#unattended-mode---unattended).
- **Usage budget** — `git paw status --json` reports per-worker usage, and `[supervisor] budget_max_tokens` / `budget_max_cost` stop dispatching at a cap. See [Usage and the budget guardrail](user-guide/supervisor.md#usage-and-the-budget-guardrail).
- **Learnings** — opt-in friction signals written to `.git-paw/session-learnings.md`, local only. See [Learnings](user-guide/learnings.md).
- **Governance pointers** — point the orchestrator at your ADRs, test strategy and definition of done. See [Governance](user-guide/governance.md).
- **MCP server** — `git paw mcp` serves repository and session state to any MCP client. See [MCP server](user-guide/mcp.md).
- **Sandboxed workers** — run a CLI under your own sandbox wrapper or its native sandbox; declare it so `doctor` reports it. See [Sandbox](user-guide/sandbox.md).
- **Doctor** — read-only preflight checks with a remedy for each finding. See [Doctor](user-guide/doctor.md).
- **Session logging and replay** — capture each pane's output and replay it later. See [Session logging](user-guide/session-logging.md).
- **Remote control** — drive a session from your phone. See [Remote control](user-guide/remote-control.md).

## How It Works

```
┌──────────────────────────────────────────────────────┐
│                    tmux session                       │
│  ┌────────────────────┐  ┌────────────────────────┐  │
│  │  feat/auth → claude │  │  feat/api → claude     │  │
│  │                     │  │                        │  │
│  │  (git worktree)     │  │  (git worktree)        │  │
│  │                     │  │                        │  │
│  ├────────────────────┤  ├────────────────────────┤  │
│  │  fix/bug → codex    │  │  refactor/db → aider   │  │
│  │                     │  │                        │  │
│  │  (git worktree)     │  │  (git worktree)        │  │
│  │                     │  │                        │  │
│  └────────────────────┘  └────────────────────────┘  │
└──────────────────────────────────────────────────────┘
```

Each pane runs in its own git worktree, so there are no branch conflicts. Your AI assistants work independently and in parallel.

## Requirements

- **Git** 2.5+ (worktree support)
- **tmux** 1.8+
- At least one AI coding CLI installed (see [Supported AI CLIs](supported-clis.md))
- macOS or Linux (Windows via WSL only)

## Next Steps

- [Install git-paw](installation.md)
- [Quick Start: Same CLI Mode](quick-start-same-cli.md) — get running in 2 minutes
- [Quick Start: Per-Branch CLI Mode](quick-start-per-branch.md) — mix different AI CLIs

## Machine-Readable Docs (for Agents)

This site — <https://bearicorn.github.io/git-paw/> — also publishes an
agent-friendly discovery surface so an LLM can find and target the right page
without crawling every chapter:

- **[`llms.txt`](https://bearicorn.github.io/git-paw/llms.txt)** — an
  [llmstxt.org](https://llmstxt.org)-style index listing every page with a
  one-line summary and its absolute URL.
- **[`sitemap.xml`](https://bearicorn.github.io/git-paw/sitemap.xml)** and
  **[`robots.txt`](https://bearicorn.github.io/git-paw/robots.txt)** — standard
  crawl and discovery files.
- **Per-page metadata** — each page embeds a `<meta name="description">` and a
  JSON block (title, canonical URL, section anchor ids) in its `<head>`, so an
  agent can identify a page and jump to a section from a single fetch.

These artifacts are generated deterministically from the documentation sources
on every build, so they never drift from the published content.
