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

If neither your repo nor your global config has a `[supervisor]` section,
git-paw first asks `Start in supervisor mode? [y/N]` — answer `n` (the default)
for this walkthrough.

## Step 3: Select branches

A multi-select list of all your branches (sorted by name) appears with
type-to-filter search:

```
Select branches (type to filter, ctrl-u to clear, space to toggle, enter to confirm)
filter:
  [x] feat/api
  [x] feat/auth
  [ ] fix/typo
  [ ] main
> [x] refactor/db
```

Type to narrow the list (case-insensitive substring match), use arrow keys to
navigate, Space to toggle, Ctrl-U to clear the filter, and Enter to confirm.
Branches you toggle stay selected even if a later filter hides them.

## Step 4: Select your mode

git-paw presents a mode picker:

```
CLI assignment mode:
> Same CLI for all branches
  Different CLI per branch
```

Select **Same CLI for all branches** and press Enter.

## Step 5: Select your CLI

Pick which AI CLI to use on all selected branches. Detected CLIs are listed by
display name, with the binary in parentheses:

```
Select AI CLI for all branches:
  Antigravity (agy)
> Claude (claude)
  Codex (codex)
```

## Step 6: git-paw does the rest

git-paw now:

1. Creates a git worktree for each selected branch
2. Creates a tmux session named `paw-my-app`
3. Opens one pane per branch
4. Launches your chosen CLI in each pane
5. Saves the session state for later recovery

The worktrees land under `.git-paw/worktrees/` (for example
`.git-paw/worktrees/feat-auth/`) in a repo set up with `git paw init`, or beside
the repo (`../my-app-feat-auth/`) when no `worktree_placement` is configured —
see [Worktree Placement](user-guide/worktree-placement.md). Nothing else is
printed on success: git-paw attaches you straight to the session. (From a
non-interactive terminal it prints `Session 'paw-my-app' started in detached
mode.` and a `tmux attach -t paw-my-app` hint instead.)

You're now inside a tmux session with three panes, each running Claude in its own worktree. Each pane's border label shows its pane index and branch (with `[broker] enabled = true`, the dashboard takes pane 0 and the agents start at pane 1):

```
┌─── 0: feat/api ───────────────┬─── 1: feat/auth ──────────────┐
│                                │                                │
│  Claude is ready to help...    │  Claude is ready to help...    │
│                                │                                │
├─── 2: refactor/db ────────────┴────────────────────────────────┤
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
- **Reattach:** Run `git paw attach` (or `git paw` again — it detects the active session and reattaches)
- **Stop:** Run `git paw stop` to kill tmux but keep worktrees
- **Purge:** Run `git paw purge` to remove everything

See the [User Guide](user-guide/README.md) for the full details.
