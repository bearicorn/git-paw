# Quick Start: Per-Branch CLI Mode

This walkthrough shows how to assign different AI CLIs to different branches — useful when you want to compare AI assistants or use specialized tools for specific tasks.

## Scenario

You have three branches and want to use:
- **Claude** for the auth feature (complex logic)
- **Gemini** for the API work (lots of boilerplate)
- **Aider** for the database refactor (incremental edits)

## Step 1: Launch git-paw

```bash
cd ~/projects/my-app
git paw
```

## Step 2: Select per-branch mode

```
? How would you like to assign CLIs to branches?
  Same CLI for all branches
> Different CLI per branch
```

## Step 3: Select branches

```
? Select branches (space to toggle, enter to confirm):
  [ ] main
  [x] feat/auth
  [x] feat/api
  [x] refactor/db
```

## Step 4: Assign a CLI to each branch

git-paw prompts you for each branch individually:

```
? Select CLI for feat/auth:
> claude
  codex
  gemini
  aider
```

```
? Select CLI for feat/api:
  claude
  codex
> gemini
  aider
```

```
? Select CLI for refactor/db:
  claude
  codex
  gemini
> aider
```

## Step 5: Watch it launch

```
Creating worktrees...
  ✓ my-app-feat-auth (feat/auth)
  ✓ my-app-feat-api (feat/api)
  ✓ my-app-refactor-db (refactor/db)

Launching tmux session: paw-my-app
  Pane 1: feat/auth → claude
  Pane 2: feat/api → gemini
  Pane 3: refactor/db → aider

Attaching to session...
```

The tmux session shows each pane with its branch and CLI clearly labeled in the pane border:

```
┌─── feat/auth → claude ────────┬─── feat/api → gemini ─────────┐
│                                │                                │
│  Claude is ready to help...    │  Gemini is ready...            │
│                                │                                │
├─── refactor/db → aider ───────┴────────────────────────────────┤
│                                                                 │
│  Aider v0.x loaded...                                           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Comparing approaches

Per-branch mode is great for:

- **A/B testing AI assistants** — give the same task to Claude and Gemini, compare results
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
