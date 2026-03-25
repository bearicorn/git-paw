# Quick Start: Same CLI Mode

This walkthrough shows how to launch git-paw with the same AI CLI on all branches — the most common workflow.

## Prerequisites

- git-paw [installed](installation.md)
- tmux installed
- At least one AI CLI installed (e.g., `claude`)
- A git repository with multiple branches

## Step 1: Navigate to your repo

```bash
cd ~/projects/my-app
```

## Step 2: Launch git-paw

```bash
git paw
```

## Step 3: Select your mode

git-paw presents a mode picker:

```
? How would you like to assign CLIs to branches?
> Same CLI for all branches
  Different CLI per branch
```

Select **Same CLI for all branches** and press Enter.

## Step 4: Select branches

A multi-select list of all your branches appears with fuzzy search:

```
? Select branches (space to toggle, enter to confirm):
  [ ] main
  [x] feat/auth
  [x] feat/api
  [ ] fix/typo
  [x] refactor/db
```

Use arrow keys to navigate, Space to toggle, and Enter to confirm.

## Step 5: Select your CLI

Pick which AI CLI to use on all selected branches:

```
? Select AI CLI:
> claude
  codex
  gemini
```

## Step 6: git-paw does the rest

git-paw now:

1. Creates a git worktree for each selected branch
2. Creates a tmux session named `paw-my-app`
3. Opens one pane per branch
4. Launches your chosen CLI in each pane
5. Saves the session state for later recovery

```
Creating worktrees...
  ✓ my-app-feat-auth (feat/auth)
  ✓ my-app-feat-api (feat/api)
  ✓ my-app-refactor-db (refactor/db)

Launching tmux session: paw-my-app
  Pane 1: feat/auth → claude
  Pane 2: feat/api → claude
  Pane 3: refactor/db → claude

Attaching to session...
```

You're now inside a tmux session with three panes, each running Claude in its own worktree:

```
┌─── feat/auth → claude ────────┬─── feat/api → claude ─────────┐
│                                │                                │
│  Claude is ready to help...    │  Claude is ready to help...    │
│                                │                                │
├─── refactor/db → claude ──────┴────────────────────────────────┤
│                                                                 │
│  Claude is ready to help...                                     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

Mouse mode is enabled by default — click a pane to switch to it, or drag borders to resize.

## Non-interactive shortcut

Skip all prompts by passing flags:

```bash
git paw start --cli claude --branches feat/auth,feat/api,refactor/db
```

## What's next

- **Switch panes:** Click with mouse, or use `Ctrl-b` then arrow keys
- **Detach:** Press `Ctrl-b d` to detach from tmux (session keeps running)
- **Reattach:** Run `git paw` again — it detects the active session and reattaches
- **Stop:** Run `git paw stop` to kill tmux but keep worktrees
- **Purge:** Run `git paw purge` to remove everything

See the [User Guide](user-guide/README.md) for the full details.
