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

## What `pause` does, mechanically

When you run `git paw pause`:

1. Every tmux client attached to the session is detached
   (`tmux detach-client -s <session>`).
2. When the broker is enabled, the dashboard pane only is killed
   (`tmux kill-pane -t :0.<idx>`),
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
creation, **no** CLI process spawn, **no** boot-prompt injection
(beyond submitting the held boot prompt of any agent added with
`git paw add` while paused). The agent panes are exactly as you left
them — open conversations intact, prompt buffer intact.

If the tmux server died while paused (rare — typically only happens
on machine reboot or `tmux kill-server`), `git paw status` shows
the session as stopped instead, and `git paw start` falls through to
the normal cold-recovery path (fresh CLI processes, resuming each
conversation where the CLI supports it — see
[Per-CLI cold resume](#per-cli-cold-resume)).

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

## Per-CLI cold resume

Stop's cold-recovery path (and the same recovery `git paw start` runs
when every agent pane has dropped back to a bare shell) restores
conversation context too, for CLIs that support it. At launch git-paw
assigns each worker a fresh session
identifier and passes it on the CLI's own launch command (Claude
Code's `--session-id <uuid>`), then records it in the session state.
When that worktree is later recovered, git-paw relaunches its CLI
with the matching resume flag (`--resume <uuid>` for Claude Code)
instead of a bare launch — the worker picks up its prior conversation
instead of starting cold.

This solves the long-break case without holding RAM, complementing
pause for the short-break case: `stop` releases the CLI processes,
and the next `start` brings their conversations back rather than
only their worktrees.

A worktree whose CLI has no known resume flag (no built-in row and no
`[clis.<name>].resume_args` override) launches fresh exactly as
before — nothing about cold recovery changes for it. See
[`session_id_args` / `resume_args`](../configuration/README.md#custom-clis)
to add resume support for a CLI git-paw doesn't know about, and
[`git paw status --json`](../cli-reference.md) to read a worker's
assigned id (e.g. to resume it by hand outside git-paw).

## See also

- Pause behavior is formally specified in the
  [`broker-runtime`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/broker-runtime/spec.md)
  capability spec (pause flow, idempotency, restart-from-pause),
  alongside the `pause` subcommand contract in
  [`cli-parsing`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/cli-parsing/spec.md)
  and the paused status variant in
  [`session-state`](https://github.com/bearicorn/git-paw/blob/main/openspec/specs/session-state/spec.md).
- [`git paw stop`](../cli-reference.md#git-paw-stop) — the
  destructive teardown verb.
- [`git paw purge`](../cli-reference.md#git-paw-purge) — the full
  reset (removes worktrees and branches).
- [Dashboard](dashboard.md) — what the dashboard pane shows while
  the session is active.
