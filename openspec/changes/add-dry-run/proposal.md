## Why

`git paw start --dry-run` prints the session plan — the session name, each branch with its worker and
worktree path, and each worker's exact pane command on a `runs:` line — and creates nothing. `git paw
add` has no equivalent. To see what one hot-attached pane would run (which CLI, which approval flags,
which profile arguments, which worktree path) an orchestrator has to run the real `add`, which creates
a worktree, rebases the branch, splices a pane into the live grid, rewrites the session file and boots
an agent. External feedback on v0.20.0 (#31) asked for the same preview `start` has. It matters most
to an orchestrator agent, which wants to check a launch before it spends a worker on it.

## What Changes

- `git paw add` accepts `--dry-run`. It resolves the add exactly as the real command does (the session
  and its effective status, the branch or `--from-spec` spec, the worker profile or CLI, CLI
  validation, the broker agent-id clash, the agent cap) and refuses in the same cases with the same
  errors, so a previewed add is one that could actually run.
- On success it prints the plan in the shape `start --dry-run` uses: a `Dry run — add plan:` header,
  the `Session:` line, a paused-session note when the agent would start on the next `git paw start`,
  one `<branch> → <worker> (<worktree path>)` line and the worker's `runs:` line, built by the same
  function `start --dry-run` uses.
- A dry run changes nothing: no worktree, rebase, provisioning, pane, session-file write, discovery
  file write, broker registration or prompt. It takes no add/remove lock, so a preview never fails
  with "operation in progress".
- With `uniform-json-output`'s `add --json`, `add --dry-run --json` prints the same `add` document with
  `dry_run: true` and the `launch_command`; a real `add --json` gains `dry_run: false` and its
  `launch_command` (additive, schema version 1).

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `git-add-remove-branch`: ADDED `add --dry-run` resolves and refuses like a real add, prints the plan
  in `start --dry-run`'s shape, changes nothing, and reports the plan as JSON with `--json`.
- `cli-parsing`: ADDED `add` accepts `--dry-run`.

## Impact

- **Code:** `src/cli.rs` (`Command::Add` gains `dry_run`; `main.rs` dispatch passes it);
  `src/commands/add.rs` (`AddOptions.dry_run`; the resolution steps up to the agent cap stay as they
  are, then a dry run returns before `SessionLock::acquire`; `AddOutcome` gains the plan, or a new
  `AddOutcome::Planned` shape); the plan renderer reuses `commands/launch.rs::dry_run_launch_line`
  and `worker_label`, and `git::worktree_display_path`. With `uniform-json-output`: `src/output.rs`
  `AddOutput` gains `dry_run` and `launch_command`.
- **MCP:** none in this change. The MCP surface has no `add` tool today; if one is added it takes the
  same `dry_run` flag, as `start_session` does.
- **Tests:** CLI parse test; e2e against a real repo and tmux session (`tests/add_remove_e2e.rs`
  style, isolated `HOME`/`XDG_DATA_HOME`): plan lines, paused note, each refusal, and nothing created
  (worktree list, session file bytes, pane count unchanged); JSON contract test.
- **Docs:** `docs/src/cli-reference.md` (`add --dry-run`, and the JSON keys), the `add` long help
  example list.
- **Enum-variant ripple:** none (a field on a struct literal: `AddOptions` is built in `main.rs` and
  tests only).
- **Cross-change sequencing:**
  - `uniform-json-output` defines `add --json`; the JSON requirement here is written on that document
    and lands after it. Without it, only the human plan ships.
  - `session-state-locking` moves `add`'s lock before its session read; a dry run still takes no lock
    (it is a reader).
  - `supervisor-vocabulary-split` rewords `add` help; this change's help text uses "orchestrator".
  - `worker-resume-recovery` (this release) changes how recovery chooses session arguments, not the
    fresh-launch path a dry run previews; no overlap.
