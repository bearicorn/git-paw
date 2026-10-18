## Context

`build_task_prompt` (`src/main.rs:410-427`) picks one bundled template per `SpecBackendKind` and
substitutes `{{SIDECAR_REL_PATH}}` / `{{SPEC_ID}}`; only `openspec.md` names a command
(`/opsx:apply {{SPEC_ID}}`), at the end of a prose sentence. `combine_boot_and_task_prompt`
(`:434-436`) joins boot block and task prompt; `submit_prompt_to_pane` (`:755-767`) types it
literally, sleeps the per-CLI settle delay, sends `Enter`. The drive loop already has the idle
notion this change needs: `pane_has_buffered_input` (`src/supervisor/drive.rs:2407-2410`: no live
prompt via `approval_gate::live_prompt_in_tail`, not mid-response via the prompt profile's
`mid_response_markers`, `:2487-2493`) and `input_box_text` for the composer.

## Goals / Non-Goals

**Goals:**
- A declared entry point reaches the CLI as its own submitted line, never as prose.
- Nothing is ever typed into a permission prompt, a busy pane or a non-empty composer.
- Every launch prompt names the absolute worktree root.

**Non-Goals:**
- Submitting a held entry point from the drive loop (see Open Questions).
- Changing the boot block template, the readiness gate, or which backends have an entry point.
- Enforcing confinement (the CLI's own outside-cwd prompt and the sandbox do that); this is
  instruction, as GP-55 asked.

## Decisions

### D1 — The template declares the entry point

Each backend's task prompt is a pair of bundled assets: the prose template (as today) and an
optional entry-point template (`assets/boot-task-prompts/openspec.entry.md` holding
`/opsx:apply {{SPEC_ID}}`). One exhaustive `SpecBackendKind` match selects both; `None` for every
other backend. Rust contains no command name, and `doctor-spec-workflow-check`'s probe reads the
same selector instead of keeping its own match (one source for "the command the prompt names").
The new asset joins the `lang_agnostic_skill_audit` list.

### D2 — Submit when idle, not when the first message is merely sent

Typing while the CLI works depends on per-CLI queueing: a slash command typed into a busy composer
may be queued as text, run later, or dropped, and a keystroke landing on a permission prompt answers
it. Waiting for idle is the only CLI-agnostic rule. Idle = the drive loop's predicate (no live
prompt, not mid-response) plus an empty or unrecognised input box, on two consecutive captures one
settle delay apart, the first no sooner than one settle delay after the prompt's `Enter` (so the
instant before the CLI starts its turn does not count). Re-captured immediately before typing
(the TOCTOU discipline of `submit_buffered_input`). The OpenSpec prose tells the agent its workflow
command follows, so its first turn is short (register, read the sidecar, wait).

### D3 — One bounded wait for all panes

After every pane's prompt is submitted, one loop polls all panes that have an entry point until
each is submitted or the budget elapses (a constant, ~120 s; a `GIT_PAW_*` override for tests, like
the drive loop's). Launch time grows by the slowest first turn, not by the sum. On expiry: nothing
typed, one stderr line per pane with the exact command. A human attached to the session types it;
an orchestrator reads it.

### D4 — Paused `add` holds the entry point beside the prompt

`WorktreeEntry.pending_entry_point: Option<String>` (`serde(default, skip_serializing_if)`), set
with `pending_boot_prompt` (`add.rs:236-239`) and taken with it on restart (`start.rs:962-975`);
the restart path waits for idle per D2. If `pane-less-session-parity` keeps a blocked pane's held
prompt, the entry point stays with it.

### D5 — The worktree rule is a separate fragment composed last

`assets/worktree-root-rule.md` with `{{WORKTREE_ROOT}}`, rendered from the recorded worktree path.
`compose_launch_prompt(boot: Option<&str>, prose: Option<&str>, worktree_root: &Path)` joins the
present parts with blank lines and appends the rule; the bare pre-fill calls it with no prose. The
task-prompt templates stay unchanged except the OpenSpec one, so the pinned no-spec fallback is
byte-identical. Placed last for recency; the coordination skill's "Stay inside your worktree"
section stays as is.

## Risks / Trade-offs

- [Unattended first turn stalls on a permission prompt past the budget] → the entry point is not
  typed; the drive loop escalates the prompt; once answered, the agent idles without its workflow
  command and the stderr line is the record. See Open Questions.
- [Launch waits up to the budget] → only for panes with an entry point; all panes concurrently;
  progress printed.
- [Unrecognised CLI] → treated as idle, so the entry point follows the first message almost at
  once — what the CLI's own input queue would see if a human typed two messages.
- [Profile-specific idle markers] → the same profile the drive loop and readiness gate use; a CLI
  with a custom profile gets its own markers.

## Migration Plan

Additive session field; older paused sessions restart as before (their held prompt still embeds
the command). Rollback: revert; the field is ignored by older binaries.

## Open Questions

- Should the drive loop (or `drive-loop-reattach`) submit an entry point the launch could not,
  once the pane goes idle? That needs the held entry point in the record for every launch, not only
  paused adds, and a drive-loop step `drive-loop-functional-core` would have to carry. Deferred
  until the dogfood shows the budget expiring in unattended runs.
