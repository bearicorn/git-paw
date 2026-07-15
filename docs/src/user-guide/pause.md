# Pause and Resume

`git paw pause` is the soft-stop verb for short breaks (lunch, a
meeting, end-of-day). It freezes the running session without losing
agent state, so you can resume mid-conversation an hour later by
running `git paw start`.

## The three teardown verbs

git-paw v0.5.0 ships three verbs for taking a session down:

| Verb    | What it kills                       | What survives                          | Use when                                                    |
|---------|-------------------------------------|----------------------------------------|-------------------------------------------------------------|
| `pause` | Tmux client attach, broker process  | Tmux session, all CLI panes (in RAM)   | Short break — you'll resume in minutes/hours                |
| `stop`  | Tmux session, every CLI pane        | Worktrees and branches on disk         | Longer break — you want RAM back, OK with fresh CLIs later |
| `purge` | Everything (tmux, worktrees, state) | Nothing                                | You're done with the project, or you want to start clean   |

A future `git paw hibernate` (v1.0.0) will snapshot tmux state and
each CLI's conversation to disk, then kill the processes — combining
pause's state preservation with stop's RAM release.

## What `pause` does, mechanically

When you run `git paw pause`:

1. Every tmux client attached to the session is detached
   (`tmux detach-client -s <session>`).
2. The dashboard pane only is killed (`tmux kill-pane -t :0.<idx>`),
   which causes the `__dashboard` subprocess to exit, which drops
   the `BrokerHandle`, which gracefully shuts down the broker and
   flushes `broker.log`.
3. The session state file flips from `status: active` to
   `status: paused`.
4. Every coding-agent CLI pane keeps running. Their in-memory
   conversation, their CLI process, and the worktree they're working
   in are untouched.

`git paw status` shows the paused state with a blue indicator and a
"run `git paw start` to resume" hint.

## The RAM trade-off

Pause is fast and state-preserving, but the CLI processes stay
allocated. A typical Claude Code instance holds **~300 MB**, so a
10-pane session is roughly **3–5 GB** of RAM sitting idle while
paused.

Pick the right verb based on duration:

- **Pause** is right for short breaks where instant resume matters
  more than RAM.
- **Stop** is right for long breaks where you'd rather get the RAM
  back. Resuming via `git paw start` spawns fresh CLI processes —
  you lose conversation context but the worktrees and branches
  carry over.
- **Hibernate** (future v1.0.0) will be both: state preserved AND
  RAM released.

## Resuming a paused session

```bash
git paw start
```

When `git paw start` detects a paused session that's still alive in
tmux, it takes the restart-from-pause path:

1. Re-creates the dashboard pane at its saved index.
2. Sends the `git paw __dashboard` command to spawn the broker
   subprocess again.
3. Flips session status back to `active`.
4. Re-attaches your tmux client.

Cost: one tmux pane spawn + one broker boot. **No** worktree
creation, **no** CLI process spawn, **no** boot-prompt injection.
The agent panes are exactly as you left them — open conversations
intact, prompt buffer intact.

If the tmux server died while paused (rare — typically only happens
on machine reboot or `tmux kill-server`), `git paw status` shows
the session as stopped instead, and `git paw start` falls through to
the normal cold-recovery path (fresh CLI spawn).

## Idempotency

`git paw pause` is safe to run repeatedly:

- Pausing an already-paused session prints
  "Session 'NAME' is already paused." and exits 0 without changing
  state.
- Pausing a stopped session prints
  "Session 'NAME' is already stopped; pause has no effect." and
  exits 0.
- Pausing with no session for the current repo prints
  "No active session for this repo." and exits 0.

## Future: per-CLI cold resume (drift 61)

For CLIs that support `--continue` / `--resume` (Claude Code, etc.),
v1.0.0 will extend the cold-recovery path so `git paw start` after a
`stop` can also restore conversation context — by spawning the CLI
with the resume flag. That solves the long-break case without
holding RAM, complementing pause for the short-break case.

## See also

- Pause behavior is formally specified in the
  [`broker-lifecycle`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/broker-lifecycle/spec.md)
  capability spec (pause flow, idempotency, restart-from-pause),
  alongside the `pause` subcommand contract in
  [`cli-parsing`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/cli-parsing/spec.md)
  and the paused status variant in
  [`session-state`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/session-state/spec.md).
- [`git paw stop`](../cli-reference.md#git-paw-stop) — the
  destructive teardown verb with its new confirmation prompt.
- [`git paw purge`](../cli-reference.md#git-paw-purge) — the full
  reset (removes worktrees and branches).
- [Dashboard](dashboard.md) — what the dashboard pane shows while
  the session is active.
