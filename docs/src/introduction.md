# Introduction

**git-paw** (Parallel AI Worktrees) orchestrates multiple AI coding CLI sessions across git worktrees from a single terminal using tmux.

Working with AI coding assistants like Claude, Codex, or Gemini is powerful — but what if you could run them in parallel across multiple branches at once? That's exactly what git-paw does.

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

- **Detect** which AI CLIs you have installed (Claude, Codex, Gemini, Aider, etc.)
- **Prompt** you to pick branches and a CLI (or different CLIs per branch)
- **Create** git worktrees for each selected branch
- **Launch** a tmux session with one pane per branch, each running your chosen AI CLI
- **Persist** the session state so you can stop, resume, or recover after crashes

## Key Features

- **One command** to go from zero to parallel AI sessions
- **Smart start** — reattaches to active sessions, recovers crashed ones, or launches fresh
- **Live branch-set editing** (v0.6.0) — `git paw add` and `git paw remove` attach or detach a single agent mid-session, no stop/purge/restart, the other agents untouched
- **Per-branch CLI selection** — use Claude on one branch and Gemini on another
- **Session persistence** — stop and resume without losing your place
- **Custom CLI support** — register any AI CLI binary, not just the built-in ones
- **Presets** — save branch + CLI combos in config for one-command launch
- **Non-interactive mode** — pass `--cli` and `--branches` flags for scripting
- **Dry run** — preview what git-paw will do before it does it

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
│  │  fix/bug → gemini   │  │  refactor/db → aider   │  │
│  │                     │  │                        │  │
│  │  (git worktree)     │  │  (git worktree)        │  │
│  │                     │  │                        │  │
│  └────────────────────┘  └────────────────────────┘  │
└──────────────────────────────────────────────────────┘
```

Each pane runs in its own git worktree, so there are no branch conflicts. Your AI assistants work independently and in parallel.

## Requirements

- **Git** (2.20+ recommended for worktree improvements)
- **tmux** (any recent version)
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
