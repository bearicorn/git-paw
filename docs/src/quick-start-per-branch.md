# Quick Start: Per-Branch CLI Mode

This walkthrough shows how to assign different AI CLIs to different branches — useful when you want to compare AI assistants or use specialized tools for specific tasks.

## Scenario

You have three branches and want to use:
- **Claude** for the auth feature (complex logic)
- **Codex** for the API work (lots of boilerplate)
- **Aider** for the database refactor (incremental edits)

## Step 1: Launch git-paw

```bash
cd ~/projects/my-app
git paw
```

(If neither your repo nor your global config has a `[supervisor]` section,
git-paw first asks `Start in supervisor mode? [y/N]` — answer `n`.)

## Step 2: Select branches

```
Select branches (type to filter, ctrl-u to clear, space to toggle, enter to confirm)
filter:
  [x] feat/api
  [x] feat/auth
  [ ] main
> [x] refactor/db
```

Type to filter the list (case-insensitive substring match); selections persist
across filter changes, and Ctrl-U clears the filter.

## Step 3: Select per-branch mode

```
CLI assignment mode:
  Same CLI for all branches
> Different CLI per branch
```

## Step 4: Assign a CLI to each branch

git-paw prompts you for each selected branch in turn (in the order they are
listed above):

```
Select CLI for feat/api:
  Aider (aider)
  Claude (claude)
> Codex (codex)
```

```
Select CLI for feat/auth:
  Aider (aider)
> Claude (claude)
  Codex (codex)
```

```
Select CLI for refactor/db:
> Aider (aider)
  Claude (claude)
  Codex (codex)
```

## Step 5: Watch it launch

git-paw creates a worktree per branch, opens the `paw-my-app` tmux session with
each branch's chosen CLI in its own pane, and attaches you to it — nothing else
is printed on success.

Each pane's border label shows its pane index and branch (with `[broker] enabled = true`, the dashboard takes pane 0 and the agents start at pane 1):

```
┌─── 0: feat/api ───────────────┬─── 1: feat/auth ──────────────┐
│                                │                                │
│  Codex is ready...             │  Claude is ready to help...    │
│                                │                                │
├─── 2: refactor/db ────────────┴────────────────────────────────┤
│                                                                 │
│  Aider v0.x loaded...                                           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Comparing approaches

Per-branch mode is great for:

- **A/B testing AI assistants** — give the same task to Claude and Codex, compare results
- **Specialization** — use a code-generation-focused tool for boilerplate and a reasoning-focused tool for complex logic
- **Trying new tools** — test a new AI CLI on one branch while using your trusted tool on others

## Recovery

Session state captures the per-branch CLI assignments. If your terminal closes or tmux crashes:

```bash
git paw
```

git-paw detects the saved session, recreates tmux, and relaunches each branch with its assigned CLI — no re-selection needed.

## Next Steps

- [User Guide](user-guide/README.md) — session management, presets, dry-run, and more
- [Configuration](configuration/README.md) — save per-branch presets in config
