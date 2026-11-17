## Context

Today a Spec Kit wave is planned from checkboxes and `[P]` markers alone. All paths below are
under `src/` unless noted.

- **Parser.** `specs/speckit.rs:277-368` (`parse_tasks_md`) recognises `## Phase N` headings
  (`:280`) and task lines `- [ ]/[x] T### [P]? …` (`:285`, `:290`). Every other line is ignored
  (`:364`). Nothing reads "depends on", a Dependencies section, "must merge first" or a human
  marker. `plan.md` and `spec.md` go into the prompt verbatim (`:496-524`).
- **Current phase.** `current_phase` (`speckit.rs:372-377`) is the lowest phase with an open task,
  per feature.
- **Decomposition.** `decompose_feature` (`speckit.rs:398-464`) gives every open `[P]` task its own
  entry (`:420`), whatever comes before it. It puts the open non-`[P]` tasks into one consolidated
  entry (`:437-461`).
  - `[P]` branch: `task/` + `branch_segment_slug("<id>-<full description>")` (`:422-423`).
  - Consolidated branch: `phase/` + the slug of `<feature-dir>-<phase name>` (`:444-445`).
- **Slugs.** `branch_segment_slug` (`specs/mod.rs:374`) has no length cap. Neither do the names
  derived from the branch:
  - `slugify_branch` (`broker/messages.rs:930`, the agent id);
  - `git::worktree_dir_name` (`git.rs:119`);
  - `git::branch_slug` → `BranchSlug::for_branch` (`git.rs:140`, `domain.rs:171`).

  `ensure_distinct_agent_ids` (`broker/messages.rs:1018`) refuses a launch whose branches collide
  on an agent id.
- **Prompts.** A single `[P]` prompt's task section is only `"{id} — {desc}"`
  (`speckit.rs:529-532`). The consolidated footer tells the worker to tick each checkbox in
  `tasks.md` (`assets/task-prompts/speckit/consolidated-footer.md:1`), and the path doctrine tells
  every Spec Kit worker the same (`assets/spec-path-doctrine/speckit.md:1`). So every worker edits
  `tasks.md`.
- **Selection.** `SpecMode::All` returns every scanned entry unchanged (`commands/spec_mode.rs:56`).
  MCP `start_session` reaches the same mode through `from_all_specs`
  (`mcp/tools/session.rs:139`, `:320-337`).
  - Bare launch: `start_with_specs` (`commands/start.rs:624`) → `scan_specs_with_override` (`:670`)
    → `launch_spec_session` (`:776`), which creates and provisions every entry (`:868-890`).
  - Coordinated launch: scan at `commands/supervisor.rs:518-560`, attach loop at `:771-807`.
  - The only limit is the 25-agent layout cap (`supervisor.rs:683-687`; `start.rs:280`, `:811`).
- **Conflicts.** In `broker/conflict.rs`:
  - forward overlap `forward_overlaps` (`:678`), published at `:1232-1249`;
  - in-flight overlap `in_flight_overlaps` (`:737`), first warning at `:1256-1266`;
  - both drop `.git-paw/` through `is_managed_bookkeeping_path` (`:490`, applied at `:691` and
    `:755`).

  `is_shared_task_tracking_path` (`:504-506`, any file named `tasks.md`) is applied only in
  ownership detection (`:787`).
- **Launch order.** `--unattended` implies coordinated mode (`main.rs:91-92`). It runs the drive
  loop in the foreground: `finish_launch` (`main.rs:548`) → `drive_fresh` (`:580`) →
  `run_drive_loop` (`:640`), as `supervisor.rs:991-1006` says and as *Unattended drive loop runs a
  supervisor wave to completion* requires.
  - Each `attach_agent` in the loop (`supervisor.rs:799`) provisions its worktree. That includes
    `on_create`, a synchronous `/bin/sh -c` with no timeout (`worktree_provision.rs:125`); a
    non-zero exit fails the launch (`:322-329`).
  - Only after the loop does `launch_session` open the session (`supervisor.rs:845-851`), and only
    then does `start_broker_host` start the broker and register the agents (`:853-873`).

  With N worktrees and a one-minute `npm ci`, the broker is down for N minutes.

## Goals / Non-Goals

**Goals:**
- A default Spec Kit wave that can't run ahead of a gate the plan states.
- No worker given a task it can't start yet, or a task meant for a human.
- Short, stable branch names.
- No false conflict from the mandatory checkbox writeback.
- A broker that answers as soon as the launch is visibly under way.

**Non-Goals:**
- Reading free prose in `plan.md`. git-paw doesn't interpret natural language: the gate is the
  feature boundary, and the markers are explicit tokens.
- Cross-feature dependency graphs.
- Changing OpenSpec, Markdown or Superpowers planning.
- Making `--unattended` return early. The foreground drive loop is specified behaviour.
- Merge-time conflicts on adjacent `tasks.md` lines. Those are git's business, at `finish`.

## Decisions

### D1 — The wave is one feature, chosen by a pure planner
`speckit::plan_wave(features, WaveMode) -> WavePlan { launch: Vec<SpecEntry>, held: Vec<HeldFeature> }`.

- In `Feature` mode (the default), `launch` holds the entries of the first feature in directory
  order that has an open task. Every later open feature goes into `held`, with its current phase
  number and name.
- `AllFeatures` reproduces today's `scan` output.

`SpecMode::All` calls it only when the backend is Spec Kit, and the launch prints each held
feature on stderr. Inside a feature, gating already holds. `current_phase` reads the main
checkout's `tasks.md`, so phase N+1 is never launched until phase N's checkbox writebacks have
merged. That is exactly the "Phase 1 must merge first" rule, provided the wave stops at one
feature.

*Alternatives:*
- Parse "must merge first" / "BLOCKS" from `plan.md`: brittle, and it varies by language and
  template.
- An explicit per-feature `depends:` header: Spec Kit doesn't define one, and `/speckit.tasks`
  would overwrite it.
- Cap the worker count: still ignores the gate.

### D2 — `[HUMAN]` is a task token beside `[P]`
The task regexes gain an optional `[HUMAN]` token (case-insensitive), accepted before or after
`[P]`. `Task` gains `human: bool`. Human tasks never produce or join an entry. A current phase
whose only open tasks are human-only produces no entry, and the feature counts as *waiting on a
human*. In `Feature` mode it still holds the wave: git-paw launches nothing and says why, rather
than skipping to the next feature past a gate.

### D3 — Parallel-ready `[P]`
In Spec Kit, order inside a phase is meaningful: a `[P]` task may run beside other `[P]` tasks,
but not ahead of an earlier sequential task. A `[P]` task is *parallel-ready* when both hold:

- no open non-`[P]`, non-human task comes before it in the current phase;
- its description names no other open task id of the phase (`\bT\d{3,}\b`).

A task that isn't ready joins the consolidated entry at its `tasks.md` position, so the
consolidated worker runs it in order. That keeps the phase's work on one branch, not split
across branches that depend on each other. Folding is safe: it only moves work onto the
sequential worker.

### D4 — Siblings are named, not shared
The `[P]` prompt's "Your Task" section gains a "Running in parallel" list: the id, description and
branch of every other `[P]` entry of the phase. It also carries one fixed sentence: siblings are
on unmerged branches, their exports are not available, and if the task needs a sibling's type or
function the worker asks the orchestrator instead of re-declaring it. The sentence lives in
`assets/task-prompts/speckit/parallel-siblings.md` (prose in assets, assembly in code, as
today). A phase with one `[P]` entry gets no list.

### D5 — Cap at 48, on a word boundary, in the backend
`speckit::cap_segment(slug, 48)` runs after `branch_segment_slug`:

- it cuts at the last `-` at or before character 48, or hard-cuts when there is none;
- it then trims trailing `-` / `_`.

The task id (`t009-…`) or the feature directory (`003-user-list-…`) leads the input, so the
distinguishing part survives the cut. With `task/` the whole branch stays under 54 characters,
and the derived worktree directory, agent id and log stem shrink with it.

The cap lives in `speckit.rs`, not in `branch_segment_slug`. The Superpowers backend shares that
function, and its branches aren't part of this finding. It also stays out of `BranchName`, because
`domain-value-objects` promises byte-identical derivations. A capped collision inside one scan is
refused with both task ids, before `ensure_distinct_agent_ids` would refuse it less legibly.

*Alternative:* hash suffixes. They are unique, but unreadable in `tmux` titles and `git branch`.

### D6 — Recorded branches keep their names
Resume and recovery use the branch recorded in the session, not a re-derived one. So a session
started before the cap keeps its long branches. A fresh `--from-all-specs` after the upgrade
derives the short name. If an unfinished worktree still sits on the old long branch for the same
task, the operator sees two branches; the release note says to `finish` or `remove` it first. No
rename is attempted: renaming a branch with a live worktree is not safe.

### D7 — The task-file exemption covers overlap too
`forward_overlaps` and `in_flight_overlaps` drop paths for which `is_shared_task_tracking_path` is
true, at the same points where `.git-paw/` paths are dropped (`conflict.rs:691`, `:755`). The
matcher is unchanged: any file named `tasks.md`, the same rule ownership detection uses.

*Trade-off:* a real collision inside a `tasks.md` is no longer flagged in flight. It still shows
up at merge, and the file's only contents are checkboxes git-paw tells workers to tick.

### D8 — Broker host first, then attach
`cmd_supervisor` opens the session with its service surface, starts the broker host and waits for
it to answer (the existing `start_broker_host` probe). Only then does it run the attach loop and
register each agent as it attaches. A failed `on_create` still tears down the session, broker
included, as the launch-failure path does now (`supervisor.rs:859-870`). Once the broker answers,
the launch prints `broker ready: <url>` on stderr. An orchestrator that ran `--unattended` in the
background waits for that line before it publishes.

*Alternatives:*
- Run `on_create` in parallel. Faster, but N parallel `npm ci` runs contend, and the broker is
  still down for the first.
- A `broker.sh` hint on a refused connection while a session is starting. Useful, but it needs a
  "starting" marker in the discovery file. Left as an open question.

## Risks / Trade-offs

- [The default wave shrinks] → pre-1.0, documented. `[specs] wave = "all-features"` is one line.
  Held features are printed, so nothing disappears silently.
- [`[HUMAN]` isn't a Spec Kit convention] → it is opt-in. Unmarked tasks behave as before.
- [Folding `[P]` tasks reduces parallelism] → only for tasks that would have run ahead of their
  dependency.
- [Broker-first changes launch timing] → the broker runs during provisioning. Agents register as
  they attach. A probe for an agent not yet attached gets the broker's normal "unknown agent".
- [Old long branches alongside new short ones] → D6.

## Migration Plan

1. Parser token and the parallel-ready rule, with unit tables. The existing tests at
   `speckit.rs:818-839` and `:1255-1292` either keep their inputs or gain a source-order note.
2. The branch cap, with tests for the cap and collisions.
3. `plan_wave` and `[specs] wave`, wired into `SpecMode::All`.
4. Sibling prompt section.
5. Conflict exemption.
6. Broker-first launch order and the `broker ready` line.
7. Docs and the orchestrator skill.

Rollback: revert. No persisted format changes. `[specs] wave` is ignored by older binaries, as
unknown keys are today.

## Resolved Questions (2026-11-17)

- **"Session still starting; retry" in `broker.sh`:** not now.
- **Branch-name cap for Superpowers:** no. Spec Kit only, until seen in the field.
- **A feature waiting on a human:** the stated gate wins. `Feature` mode does not move on.
