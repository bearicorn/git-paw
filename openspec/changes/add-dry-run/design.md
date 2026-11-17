## Context

`start --dry-run` exists on both launch paths and prints a plan without creating anything:

- Bare `start` (`src/commands/start.rs:236-264`): a `Dry run — session plan:` header, `Session:` and
  `Mouse:` lines, then per worker `  <branch> → <worker label> (<worktree>)` and the `runs:` line. It
  resolves every `runs:` line before printing any worker header (`start.rs:247-253`), so a refusal
  (an ambiguous PATH shim) is reported before a partial plan.
- Coordinated `start` (`src/commands/supervisor.rs:628-681`): the same per-worker lines under a
  header with `Session:`, `Agent CLI:`, `Approval:`, `Mouse:` and `Broker URL:`.
- Both build the `runs:` line with `commands/launch.rs:378-389` `dry_run_launch_line` and the label
  with `launch.rs:393-398` `worker_label`; the worktree path is `git::worktree_display_path`
  (`src/git.rs:799`), a pure derivation that creates nothing (`git.rs:768-793`).
- The dry run skips everything that touches the world: existing-session handling
  (`start.rs:97`, `supervisor.rs:397`), the broker port probe (`start.rs:119`), worktree creation.
  It still runs the refusals that make a plan launchable — the agent-id clash (`start.rs:218-227`).
- `--dry-run` is a `start` flag only (`src/cli.rs:200-205`). `start --dry-run` has no JSON form;
  `uniform-json-output` (this release) adds `add --json` and does not add a dry-run document.

`git paw add` (`src/commands/add.rs:64-495`) runs, in order: profile-flag check (`add.rs:84-86`);
session lookup and "no active session" error (`add.rs:89-93`); effective status, refusing a stopped
session (`add.rs:95-105`); legacy tiled-layout refusal (`add.rs:117-122`); tmux check and config
(`add.rs:124-125`); branch or `--from-spec` resolution (`add.rs:129-151`); "already an agent"
(`add.rs:153-158`); worker resolution (`add.rs:160-184`); CLI validation (`add.rs:186-215`); agent-id
clash (`add.rs:223-234`); agent cap (`add.rs:241-242`). Only then does it mutate: the add/remove lock
(`add.rs:245`), the rebase-skip probe and notice (`add.rs:251-261`), `attach_agent` (worktree create,
provisioning, `AGENTS.md`, hooks; `add.rs:318-335`, `launch.rs:254-371`), pane splice and re-tile
(`add.rs:344-375`), session save and discovery file (`add.rs:386-395`), broker watch registration
(`add.rs:405-418`), and the readiness gate plus prompt (`add.rs:420-467`).

## Goals / Non-Goals

**Goals:** preview exactly one added pane — its worker, worktree path and pane command — with the
same refusals the real `add` makes, and nothing changed.

**Non-Goals:**
- Previewing the boot prompt, `AGENTS.md` content or provisioning hooks (start's dry run does not
  either).
- A JSON form for `start --dry-run` (an open question, below).
- Predicting the rebase outcome (a rebase conflict is only known by running it).

## Decisions

### D1 — One resolution, a fork before the first mutation

`add` keeps its resolution steps unchanged and returns the plan between the agent-cap check
(`add.rs:241-242`) and `SessionLock::acquire` (`add.rs:245`). Every refusal a real add makes before
mutating is therefore made by the dry run too, with the same error text and exit code. Alternative —
a separate `plan_add` that re-implements the resolution — rejected: two copies of the precedence
chain (`--profile`/`--cli` > spec > session's first worker > defaults, `add.rs:160-179`) drift, and
the point of a preview is that it cannot disagree with the real run. Extracting the resolution into a
pure `AddPlan` that both paths consume is the natural refactor and is task 1.1.

### D2 — Same output shape as `start --dry-run`, same line builders

```
Dry run — add plan:

  Session:  paw-proj
  Status:   paused — the agent starts on the next `git paw start`

  feat/x → routine [claude] (.git-paw/worktrees/feat-x)
      runs: env CLAUDE_CODE_… claude --model small --setting-sources project,local --permission-mode acceptEdits
```

`Status:` is printed only for a paused session (an active one is the default, as `start`'s plan
omits it). The worker line and the `runs:` line come from `worker_label`, `dry_run_launch_line` and
`worktree_display_path`, so an `add` preview and a `start` preview of the same worker are
byte-identical on those lines. The plan goes to stdout through `Output::line`; launch warnings
(unsandboxed full-auto, `LaunchWarnings::admit`, `launch.rs:134-144`) go to stderr as on start.

### D3 — A dry run takes no lock and probes nothing live

No `SessionLock` (it is a reader; `session-state-locking` keeps readers lock-free), no
`git::rebase_skip_reason`, no `create_worktree`, no `spawn_surface`, no `save_session`, no discovery
file, no broker HTTP. `tmux::ensure_tmux_installed` still runs: the real add would refuse without
tmux (`add.rs:124`), and start's dry run checks it too (`start.rs:111`).

### D4 — JSON: the `add` document, flagged

With `uniform-json-output`, `add --dry-run --json` prints the `add` document (`schema_version`,
`session`, `branch`, `agent_id`, `cli`, `worktree_path`, `paused`) with `dry_run: true`, the absolute
path the worktree would be created at (`git::worktree_path_for`, not the display form), and
`launch_command` (the `runs:` command without its prefix). A real `add --json` gains `dry_run: false`
and the `launch_command` it ran: an additive change inside schema version 1. Alternative — a separate
`add_plan` document — rejected: a consumer would parse two shapes for one command, and the MCP
`start_session` precedent reports a dry run as a status of the same response (`src/mcp/tools/session.rs`,
`dry_run` status).

## Risks / Trade-offs

- [The `runs:` line omits the session-id arguments the real launch appends: `dry_run_launch_line`
  renders `LaunchSession::None` (`launch.rs:388`) while `attach_agent` renders
  `LaunchSession::Assign(<uuid>)` (`launch.rs:336-344`), e.g. `--session-id abc`
  (`src/agent_cli/tests.rs:522`)] → inherited from `start --dry-run` and kept identical here; listed
  as an open question rather than fixed in one of the two previews only.
- [A dry run on a session another process is mutating may preview a plan the real add then refuses]
  → inherent to an unlocked preview; the real add re-resolves under its lock.

## Resolved Questions (2026-11-17)

- **Session-id placeholder in previews:** not now. Both previews keep their current output; one
  later change can add it to `add` and `start` together.
- **`start --dry-run --json`:** not now. `uniform-json-output` does not roster it; it can be added
  later as an additive document.
