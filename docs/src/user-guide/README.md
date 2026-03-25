# User Guide

This guide covers the full range of git-paw features beyond the quick starts.

## Starting Sessions

### Interactive start

Running `git paw` (or `git paw start`) with no flags launches the interactive flow:

1. Mode selection — same CLI for all branches, or different CLI per branch
2. Branch selection — multi-select with fuzzy search
3. CLI selection — single pick (uniform) or per-branch assignment

### Smart start behavior

`git paw start` inspects the current repo and decides what to do:

| State | Behavior |
|-------|----------|
| Active tmux session exists | Reattaches immediately |
| Saved session, tmux dead (crash/reboot) | Auto-recovers: reuses worktrees, recreates tmux, relaunches CLIs |
| No session | Full interactive launch |

You never need to think about whether to "start" or "resume" — just run `git paw`.

### Non-interactive start

Skip prompts with flags:

```bash
# Specify both CLI and branches — no prompts at all
git paw start --cli claude --branches feat/auth,feat/api

# Specify just CLI — still prompted for branches
git paw start --cli claude

# Specify just branches — still prompted for CLI
git paw start --branches feat/auth,feat/api
```

### Using presets

Define named presets in your config (see [Configuration](../configuration/README.md)):

```bash
git paw start --preset backend
```

This uses the branches and CLI defined in the `[presets.backend]` section of your config.

## CLI Modes

### Same CLI for all branches

The default mode. Every branch gets the same AI CLI. Best for:
- Working on related features with your preferred tool
- Batch processing branches with a single assistant

### Different CLI per branch

Assign a different CLI to each branch. Best for:
- Comparing AI assistants side by side
- Using specialized tools for specific tasks
- Trying a new CLI on one branch while keeping your usual tool on others

## Session Management

### Checking status

```bash
git paw status
```

Displays the current session state:

```
Session: paw-my-app
Status:  🟢 active
Created: 2025-01-15T10:30:00Z

Worktrees:
  feat/auth    → claude  (../my-app-feat-auth)
  feat/api     → claude  (../my-app-feat-api)
  refactor/db  → aider   (../my-app-refactor-db)
```

Status indicators:
- 🟢 **active** — tmux session is running
- 🟡 **stopped** — session state saved, tmux not running (recoverable)
- No session — nothing saved for this repo

### Stopping a session

```bash
git paw stop
```

This kills the tmux session but **preserves**:
- Git worktrees (with any uncommitted work)
- Session state file (branch/CLI assignments)

Run `git paw` later to recover the session with the same setup.

### Purging a session

```bash
git paw purge
```

The nuclear option. Removes:
- Tmux session
- All git worktrees created by git-paw
- Session state file

Requires confirmation. Use `--force` to skip:

```bash
git paw purge --force
```

## Dry Run

Preview what git-paw will do without executing:

```bash
git paw start --dry-run
```

Or with flags:

```bash
git paw start --cli claude --branches feat/auth,feat/api --dry-run
```

This runs the detection, selection, and planning steps, then prints the session plan and exits without creating worktrees or tmux sessions.

## Tmux Navigation

Once inside a git-paw tmux session:

| Action | Keys |
|--------|------|
| Switch pane | Click with mouse, or `Ctrl-b` + arrow key |
| Resize pane | Drag border with mouse, or `Ctrl-b Ctrl-arrow` |
| Detach (keep running) | `Ctrl-b d` |
| Scroll up | `Ctrl-b [` then arrow keys, `q` to exit |
| Zoom pane (fullscreen toggle) | `Ctrl-b z` |

Mouse mode is enabled by default, so clicking and dragging just works. You can disable it in your [config](../configuration/README.md).

## One Session Per Repo

git-paw manages one session per repository. If you run `git paw` in a repo that already has a session, it reattaches rather than creating a second session.

To work with multiple repos simultaneously, open separate terminals and run `git paw` in each repo directory.
