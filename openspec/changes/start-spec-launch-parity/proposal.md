## Why

`git paw start --specs <names>` without supervisor mode produces a session in which no
agent ever starts work. Observed directly: three agents came up with the full boot block
sitting unsubmitted in the composer (`❯ ## BOOT INSTRUCTIONS …`) and would have sat there
indefinitely. Three separate defects stack on that one path.

This is **not** a regression and not merely "the path missed v0.16.0's hardening". The
spec set already contains the contradiction:

- `cli-resolution`'s *Boot prompt submitted via split-send + settle delay* requires the
  boot-injection path to inject literally and then submit with a separate `Enter` after a
  settle delay.
- `boot-block`'s *Manual mode boot block pre-fill* requires manual broker mode to pre-fill
  the boot block **without** sending `Enter`, explicitly so "users can paste their actual
  task after the boot instructions".

Both are right for the case they were written for. Bare `git paw start` is human-driven —
the human supplies the task, so not submitting is correct. A `--specs` launch is *not*
human-driven: the task comes from the spec, nobody is going to type it, and pre-filling
without submitting strands the agent. `cmd_start_with_specs` inherits the manual-mode
contract and calls `build_boot_inject_args`, which is `send-keys -l` and nothing else.

The second defect was anticipated by the spec itself. *Boot-block injection in
cmd_start_from_specs* ends: "This requirement does NOT cover spec-content / task-prompt
injection … and, in a future change, may be augmented by a format-native apply skill
invocation." This is that future change. Without the task prompt the agent never runs
`/opsx:apply`, so `tasks.md` checkboxes are never marked — during the dogfood one agent
completed its work correctly with all six boxes still unchecked and miscounted its own
task list in its final report. `openspec status` reports such a change 0/N forever and an
orchestrator has no machine-readable progress to read. The spec workflow is silently
replaced by an ad-hoc one.

The third: `config::resolve_approval_flags` is called only from `commands/supervisor.rs`
and `commands/add.rs`. `cmd_start` and `cmd_start_with_specs` build the pane with the bare
CLI name, so `agent_approval` and `[clis.<name>].approval_args` are silently ignored
outside supervisor mode and workers launch in whatever permission mode their CLI defaults
to.

## What Changes

- A **spec-driven** launch SHALL submit the boot prompt, using the existing split-send +
  settle-delay mechanism rather than pre-fill-only.
- *Manual mode boot block pre-fill* SHALL be scoped explicitly to the bare, human-driven
  launch (no spec selected), so the two requirements no longer contradict each other.
  Bare `git paw start` behaviour is unchanged.
- A spec-driven launch without supervisor mode SHALL inject the task prompt that names the
  agent's assignment and its format-native apply invocation — the same prompt the
  supervisor path builds — so the agent drives the spec workflow and marks tasks off.
- Resolved approval flags SHALL be applied on **every** launch path that spawns a worker
  CLI, not only the supervisor and `add` paths.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `boot-block`: *Manual mode boot block pre-fill* is scoped to the bare human-driven path;
  *Boot-block injection in cmd_start_from_specs* additionally requires submission and
  task-prompt injection.
- `session-state`: *start writes a per-repo session JSON* is widened — "any flags" now
  explicitly includes a spec-driven launch without supervisor mode. `cmd_start_with_specs`
  was the only launch path that never called `write_repo_discovery_file`, so
  `.git-paw/sessions/paw-<project>.json` never existed for that path and `sweep.sh` could
  not discover the session from the repo root (folded in during review).

### Added Capabilities

- `supervisor-config`: a new requirement that resolved approval flags apply on every
  worker-launch path.

## Impact

- `src/commands/start.rs` — `cmd_start_with_specs`: submit the boot prompt via
  `submit_prompt_to_pane` (the existing seam, already used by the supervisor path) instead
  of a bare `build_boot_inject_args`; call `build_task_prompt`; call
  `resolve_approval_flags` when building `PaneSpec.cli_command`.
- `src/main.rs` — `build_task_prompt` becomes reachable from `commands/start.rs`
  (currently `pub(crate)` in `main.rs`; may need to move or be re-exported).
- `src/commands/start.rs` — bare `cmd_start` also gains approval-flag resolution.
- **Behaviour change, intentional and narrow**: a `--specs` launch now submits where it
  previously pre-filled. Bare `git paw start` is untouched, so the documented
  paste-your-task flow still works.
- **Delta coordination**: `boot-block` and `supervisor-config` are not touched by any
  other in-flight v0.17.0 change.
