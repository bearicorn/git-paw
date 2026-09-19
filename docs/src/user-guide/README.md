# User Guide

This guide covers the full range of git-paw features beyond the quick starts.

## Starting Sessions

### Interactive start

Running `git paw` (or `git paw start`) with no flags launches the interactive flow:

1. Branch selection — multi-select with type-to-filter search (type to narrow, ctrl-u to clear, space to toggle, enter to confirm)
2. Mode selection — same CLI for all branches, or different CLI per branch
3. CLI selection — single pick (uniform) or per-branch assignment

### Smart start behavior

`git paw start` inspects the current repo and decides what to do:

| State | Behavior |
|-------|----------|
| Active tmux session exists | Reattaches immediately (with no TTY — a script — it refuses with an error naming the session instead) |
| Paused session | Restarts the dashboard/broker and reattaches; the CLI panes are untouched |
| Stopped session (`git paw stop`, or a paused session whose tmux died), or a live tmux session whose agent panes have all dropped back to a shell | Recovers: reuses worktrees, recreates tmux, relaunches CLIs (resuming conversations where the CLI supports it) |
| Stale receipt (says active, but its tmux session is gone — a crash or reboot) | Clears the receipt the way `git paw purge --stale` does, then launches fresh |
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
Tmux:    running

  feat/auth → claude (/path/to/my-app/.git-paw/worktrees/feat-auth)
  feat/api → claude (/path/to/my-app/.git-paw/worktrees/feat-api)
  refactor/db → aider (/path/to/my-app/.git-paw/worktrees/refactor-db)
```

With the broker enabled, a `Broker:` line shows its URL and whether it is
responding. `git paw status --json` emits the same information
machine-readably, including each worker's `cli_session_id`.

Status indicators:
- 🟢 **active** — tmux session is running
- 🔵 **paused** — `git paw pause` detached it; CLI panes are still running
- 🟡 **stopped** — session state saved, tmux not running (recoverable)
- 🔴 **stale** — the receipt claims active, but the tmux session is gone (or,
  past a short launch grace period, every agent pane has dropped back to a
  bare shell); `git paw start` self-heals it, `git paw purge --stale` clears it
- No session — nothing saved for this repo

### Reattaching

```bash
git paw attach
```

Reattaches your terminal to the session running for this repository — the same
as `tmux attach -t paw-<project>`, without needing the exact session name. It
errors with a pointer to `git paw start` when no session is running.

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

## Shell Completions

`git paw completions <shell>` prints a completion script to stdout for your
shell's standard completion mechanism:

```bash
git paw completions bash > /etc/bash_completion.d/git-paw
git paw completions zsh > "${fpath[1]}/_git-paw"
git paw completions fish > ~/.config/fish/completions/git-paw.fish
```

See the [CLI reference](../cli-reference.md#git-paw-completions) for the
supported shells.
