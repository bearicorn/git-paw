## Why

Two v0.17 dogfood findings (`.git-paw/v0.17.0-dogfood-findings.md`) trace to the shape of the
prompt git-paw types into an agent pane at launch:

- **GP-54 — the entry point was narrated, not run.** A Haiku worker printed `/opsx:apply <id>` as
  text and went idle; it ran only after the orchestrator typed the command into the composer. The
  launch submits one message: the boot block and the task prompt joined by a blank line
  (`combine_boot_and_task_prompt`, `src/main.rs:434-436`), and the OpenSpec task prompt embeds the
  command in prose (`assets/boot-task-prompts/openspec.md`: "Read … first — … — then run
  /opsx:apply {{SPEC_ID}}.", selected by `build_task_prompt`, `src/main.rs:404-427`). A weaker model
  may describe a command it should invoke.
- **GP-55 — the worktree was lost.** The same worker tried to edit the main checkout through
  `../../../assets/…`. With child placement the main checkout is an ancestor of every worktree. The
  worker skill says to use relative paths only (`assets/agent-skills/coordination.md:182-188`), and
  nothing in the prompt names the worktree's absolute root.

Every launch path that injects a prompt goes through the same few pieces: the bare pre-fill (boot
block only, no Enter, `src/commands/start.rs:400-423`), spec-driven start
(`start.rs:818-842`), supervised start (`src/commands/supervisor.rs:897-911`), `add`
(`src/commands/add.rs:285-295`, or held in `pending_boot_prompt` when paused, `:236-239`) and the
held-prompt submit on restart from pause (`start.rs:962-975`). All submit through
`submit_prompt_to_pane` (`src/main.rs:755-767`: literal inject, settle delay, separate Enter).

## What Changes

- **The entry point is its own composer line.** `build_task_prompt` returns the prose and, when the
  backend's template set declares one, a workflow entry point. The OpenSpec prose stops embedding
  `/opsx:apply` and says the workflow command follows as the next message; its template set declares
  `/opsx:apply {{SPEC_ID}}`. Rust names no command: the text comes only from the template.
  Markdown, Spec Kit, Superpowers and the no-spec fallback declare none and are unchanged.
- **Submitted only when the pane is idle.** After the boot + task prompt is submitted, the launch
  waits — bounded, all panes at once — until the pane is idle by the CLI prompt profile (no live
  permission prompt, not mid-response, an empty input box), re-checks right before typing, then types
  the entry point and submits it with the same split-send. It never types into a live prompt or a
  busy or non-empty composer: a pane that does not go idle within the budget gets nothing typed, and
  stderr names the pane and the exact command to submit. An agent added while paused holds its entry
  point with its held prompt; restart submits both, in order, under the same rule.
- **The prompt names the worktree root.** Every prompt git-paw injects into a coding agent at launch
  — the bare pre-fill included — ends with a bundled worktree rule naming the absolute worktree root
  and stating that every edit stays under it, never outside, by absolute path or by `../`.
- Reconciles stale text in *Initial prompt injection via tmux send-keys*: it still requires a single
  `send-keys` with a trailing `Enter` after a fixed ~2 s wait, while the code submits with the
  split-send (`cli-resolution`) after the launch-readiness gate (`tmux-orchestration`).

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `supervisor-launch`: *Initial prompt injection via tmux send-keys* — the task prompt carries an
  optional entry point; the OpenSpec prose no longer embeds it; the composed prompt ends with the
  worktree rule; submission text matches the split-send and the readiness gate.
- `boot-block`: *Boot-block injection in cmd_start_from_specs* and *Manual mode boot block
  pre-fill* updated; new requirements for the worktree rule and the entry-point submit.

## Impact

- **Predecessors (implementation order):** archive position 11.
  - After `pane-less-session-parity` (9): it gates every injecting path (bare, spec-driven,
    supervised, `add`, restart from pause) and keeps a blocked pane's held prompt for the next
    restart. It modifies none of the requirements changed here (its `tmux-orchestration`
    *Launch-readiness gate before boot-block injection* stays as it wrote it); this change relies on
    that gate and holds the entry point wherever it holds the prompt.
  - After `supervisor-pane-removal` (6): supervised start launches no supervisor pane; it modifies
    other `supervisor-launch` and `boot-block` requirements, not the four touched here. Every
    MODIFIED delta is written on the released v0.17.0 text.
  - After `doctor-spec-workflow-check` (4): its probe needs "the workflow command the format's task
    prompt names"; this change makes that the declared entry point and points the probe at it
    instead of a second per-backend match.
- **Successor:** `terminal-backend-seam` (12) moves pane capture and keystroke submission behind the
  terminal port; the idle wait and the entry-point submit move with them.
- **Code:** `src/main.rs` (`build_task_prompt` → `TaskPrompt { prose, entry_point }`; a
  `compose_launch_prompt(boot, prose, worktree_root)` replacing `combine_boot_and_task_prompt`; an
  idle-wait + entry-point submit beside `submit_prompt_to_pane`; `AttachedAgent` carries the entry
  point), `src/commands/{start,supervisor,add}.rs`, `src/session.rs` (additive
  `WorktreeEntry.pending_entry_point`, `serde(default, skip_serializing_if)`), the idle predicate
  shared with the drive loop (`src/supervisor/drive.rs:2407-2410`, `:2487-2493`), `src/doctor.rs`
  (DSWC probe reads the declared entry point), `assets/boot-task-prompts/openspec.md` + a new
  entry-point template, a new `assets/worktree-root-rule.md`.
- **Persisted state:** additive optional field; sessions saved earlier load unchanged; a paused
  session saved by an earlier binary has no held entry point (its held prompt still embeds the
  command) and restarts as before.
- **Tests:** `build_task_prompt` table (prose, entry point per backend; no-spec verbatim); idle-wait
  unit tests over captured fixtures (idle, mid-response, live prompt, buffered input, unknown CLI)
  with a fake capturer and clock; spec-driven e2e (`tests/start_spec_launch_parity_integration.rs`):
  the entry point arrives as a second submitted line after the first; paused `add` + restart
  (`tests/pause_e2e.rs`); worktree rule present on every path including the bare pre-fill;
  `lang_agnostic_skill_audit` covers the new assets.
- **Docs:** `docs/src/user-guide/spec-driven-launch.md` (two messages, the idle wait, the stderr
  hint), `docs/src/user-guide/coordination.md` (worktree rule), AGENTS.md §7 (`SpecBackendKind`
  sites: the entry-point selector).
- **Enum-variant ripple:** `SpecBackendKind` — the entry-point template selector is an exhaustive
  match co-located with `build_task_prompt`; DSWC reuses it instead of its own match.
- **Security / safety:** the only new keystrokes are the declared entry point, typed only into an
  idle composer (re-checked immediately before) and never into a permission prompt; the text is a
  bundled template with the spec id substituted, as today. No CLI name is hard-coded; an
  unrecognised CLI is treated as idle (its markers match nothing), which at worst submits the entry
  point right after the first message, as the CLI's own input queue would.
- **Dependencies:** none.
