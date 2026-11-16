## Why

The peer-4-poker run on v0.19.0 (a Spec Kit project) showed that git-paw plans a Spec Kit wave
from checkboxes alone and ignores everything else the plan says:

- **PK-03.** `--from-all-specs` launched the current phase of every feature at once: 14 workers.
  The plan said Phase 1 had to merge first, and some tasks were for a human. git-paw read neither.
- **PK-05.** A `[P]` task gets its own worktree even when it comes after a sequential task of
  the same phase that it needs, or names another open task it depends on.
- **PK-17.** `[P]` siblings run on separate unmerged branches, so none can import another's
  exports. Their prompts don't say so, and each worker re-declared the shared types.
- **PK-04.** A `[P]` branch is the slug of the whole task line, often more than 80 characters.
  The worktree directory, the agent id and the log names all inherit that length.
- **PK-08.** Every worker must tick its own checkbox in `specs/<feature>/tasks.md`, so every split
  phase opens with a false forward conflict on that file (and an in-flight one once two workers
  have ticked). Today only ownership violations are exempt for that file.
- **PK-09.** In a coordinated launch, every worktree's `on_create` hook runs (about a minute of
  `npm ci` each) before the broker host starts. `--unattended` blocks the launching command by
  design, so an orchestrator that backgrounds it and publishes its bootstrap message gets a refused
  connection and no hint to wait.

## What Changes

- **One feature per wave.** Under Spec Kit, `--from-all-specs` (and MCP `from_all_specs`)
  launches only the first open feature, in feature-directory order. Every held-back feature is
  listed. `[specs] wave = "all-features"` restores today's behaviour. Selections made with
  `--specs` or the picker are unchanged.
- **Human-only tasks.** A task line marked `[HUMAN]` is never given to a worker. A current phase
  whose only open tasks are human-only launches nothing and is reported as waiting on a human.
- **Parallel-ready `[P]` only.** A `[P]` task gets its own worktree only if no open sequential task
  comes before it in the phase and its text names no other open task of the phase. Otherwise it
  joins the consolidated entry, in `tasks.md` order.
- **Sibling list in `[P]` prompts.** Each `[P]` worker's prompt lists its parallel siblings and
  says their exports are not available to it.
- **Branch length cap.** The part of a Spec Kit branch after `task/` or `phase/` is capped at 48
  characters, cut at a word boundary. A collision refuses the launch.
- **Task-file overlap exemption.** The shared task-tracking file is left out of forward and
  in-flight overlap, as it already is for ownership violations.
- **Broker before `on_create`.** In a coordinated launch the broker answers before the first
  worktree's `on_create` runs, and `--unattended` prints a `broker ready` line, so an orchestrator
  that backgrounds the launch knows when it can publish.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `spec-providers`: MODIFIED *Current-phase decomposition into SpecEntry values*. ADDED
  *Spec Kit all-specs launch takes one feature per wave*, *Human-only Spec Kit tasks are never
  assigned to a worker*, *A [P] task after an open sequential task joins the consolidated entry*,
  *A [P] task naming another open task of its phase joins the consolidated entry*, *A [P]
  worker's prompt names its parallel siblings* and *Spec Kit branch segments are length-capped*.
- `broker-conflict-detection`: ADDED *Shared spec-task-tracking artifact is exempt from forward
  and in-flight overlap*.
- `session-runtime-isolation`: ADDED *A coordinated launch's broker answers before on_create
  runs*.
- `supervisor-unattended-operation`: ADDED *Unattended launch announces broker readiness before
  provisioning*.

## Impact

- **Code:**
  - `src/specs/speckit.rs`: the parser gains `[HUMAN]`, decomposition gains the
    parallel-ready rule, the sibling list and the cap, and a new pure `plan_wave` handles feature
    selection.
  - `src/commands/spec_mode.rs`: `SpecMode::All` applies the wave rule for Spec Kit.
  - `src/config/`: `[specs] wave`.
  - `src/broker/conflict.rs`: the task-file exemption is extended to forward and in-flight overlap.
  - `src/commands/supervisor.rs`: the broker host starts before the attach loop.
  - `assets/task-prompts/speckit/`: the sibling heading.
- **Behaviour change (pre-1.0):** the default `--from-all-specs` wave for Spec Kit shrinks to one
  feature, and long `[P]` branch names get shorter. Branches already created keep their names
  (design D6).
- **Docs:**
  - `docs/src/user-guide/` Spec Kit chapter: the wave rule, `[HUMAN]`, parallel-ready `[P]`, the
    branch cap;
  - the configuration reference (`[specs] wave`) and the commented init default;
  - the conflict-detection chapter (the task-file exemption);
  - the orchestrator skill: background `--unattended` and wait for `broker ready`.
- **Enum-variant ripple:** none. `SpecBackendKind` and `BrokerMessage` are unchanged.
- **Cross-version dependencies:**
  - **`domain-value-objects` (this release).** That change moves every branch-derived name into
    `BranchName` and changes no bytes. The cap here changes the *branch* itself, before any
    derivation, in `speckit.rs`. It is deliberately not part of `BranchName`: that change promises
    byte-identical derivations and must not take on a behaviour change. Either order works. If
    `domain-value-objects` lands first, the capped branch is the `BranchName` input, and the agent
    id, worktree slug and log stem derive from it unchanged.
  - **`supervisor-vocabulary-split` (this release).** `[specs] wave` is outside the split tables.
    New text here already says "orchestrator". The broker wire field `from = "supervisor"` in
    conflict feedback is unchanged.
  - **`worktree-shared-dep-cache` (this release)** makes `on_create` installs cheaper. It does not
    remove the ordering problem PK-09 describes.
