## Why

GitHub's Spec Kit is the third major spec workflow in the wild (alongside OpenSpec and plain Markdown), and projects using it produce a richer artefact set than the other two: per-feature `spec.md` + `plan.md` + `tasks.md` (with `[P]`/non-`[P]` parallelism markers and phase ordering) plus `constitution.md` and `checklists/`. The v0.4 backends can't represent this shape — `spec-scanning` assumes one directory ↔ one `SpecEntry`, but a single Spec Kit feature decomposes into multiple work units depending on `[P]` markers and the current phase.

This change adds a Spec Kit backend that knows how to read those artefacts, decompose a feature into the right set of worktrees, and feed `spec.md` / `plan.md` / `checklists/` into agent boot prompts. It also auto-wires `constitution.md` into the governance config (the slot for which is added by the parallel `governance-config` change).

## What Changes

- **New backend `SpecKitBackend`** (`src/specs/speckit.rs` or equivalent) implementing the existing `SpecBackend` trait. The backend's `scan()` method walks `<dir>/<feature>/tasks.md` for every immediate subdirectory of the configured `specs.dir`, identifies each feature's current phase (the first phase containing any `- [ ] TNNN` line), and decomposes that phase into:
  - One `SpecEntry` per `[P]`-marked incomplete task (single-task worktree).
  - One consolidated `SpecEntry` carrying all non-`[P]` incomplete tasks of that phase as a sequential to-do list (single worktree, one agent that works through them in order).
- **`tasks.md` parser** that recognises:
  - Phase headings of the form `## Phase N: <Name>` (or `## Phase N — <Name>`, etc.) — the phase name and number are extracted but the parser is lenient on punctuation.
  - Task lines `- [ ] TNNN [P]? <description>` and `- [x] TNNN [P]? <description>`.
  - The first phase containing any `- [ ]` task is the *current phase*; later phases are deferred until the current one fully clears across all tasks (i.e. all become `- [x]` and are merged).
- **Auto-detection.** When `.specify/` exists at the repository root, `git paw start --from-specs` (and `git paw init --from-specs`) defaults to `specs.type = "speckit"` and `specs.dir = ".specify/specs"`. Users can override with `--specs-format openspec|markdown|speckit` (already a flag) or by setting `[specs] type = "..."` explicitly in `.git-paw/config.toml`.
- **Boot prompt content.** Every Spec Kit-backed `SpecEntry` ships with:
  - The feature's full `spec.md` content as read-only context.
  - The feature's full `plan.md` content as read-only context.
  - For each file in `<feature>/checklists/`, the file content tagged as advisory validation criteria. (Not enforced — full checklist enforcement is v1.0.0.)
  - For consolidated entries: the tasks list with full descriptions, in `tasks.md` order, plus an instruction to mark each `- [x]` as it completes and to publish `agent.done` only when the whole list is done.
  - For single-`[P]` entries: the one task description plus the same `- [x]` writeback instruction.
- **Branch naming.**
  - `[P]` task entries: branch `task/<task-id>-<slug-of-description>` (e.g. `task/T009-add-login-form`).
  - Consolidated entries: branch `phase/<feature-dir>-<phase-name>` (e.g. `phase/002-foundational`).
  - The branch-name slug logic SHALL reuse the existing `slugify_branch` rules where applicable.
- **`SpecEntry.id` shape.** For Spec Kit entries:
  - `[P]` entries: `id = "<feature-dir>-<task-id>"` (e.g. `003-user-list-T009`).
  - Consolidated entries: `id = "<feature-dir>-phase-<N>"` (e.g. `003-user-list-phase-2`).
- **`SpecEntry.owned_files`.** Spec Kit doesn't declare per-task file ownership, so `owned_files = None` for all entries. Conflict detection (from `conflict-detection`) handles overlap dynamically via `agent.intent`.
- **Constitution auto-wiring.** When the SpecKit backend is active and `<dir>/../memory/constitution.md` exists, the backend SHALL signal this path so that `governance-config` (parallel change) can populate `governance.constitution` with it when the user has not explicitly configured a path. The wiring contract is: SpecKit backend exposes a method (or returns context) advertising the detected constitution path; the governance-config consumer decides whether to use it.
- **Agent skill update.** The embedded coordination skill SHALL gain a "When working in a Spec Kit consolidated worktree" sub-section instructing the agent to: (a) work through the listed tasks sequentially in `tasks.md` order; (b) flip `- [x]` for each task as it completes (committing the writeback alongside the task's code change is acceptable but not required); (c) publish `agent.done` only when all listed tasks show `- [x]`.
- **No supervisor-side checkbox audit.** Earlier drafts had the supervisor skill validate that every `- [x]` flip in `tasks.md` had matching work in the diff. Dropped after scope review: the supervisor's existing Spec Audit Procedure already verifies code against the spec; layering a separate "checkbox bookkeeping audit" on top is supervisor-mode-specific over-reach. The agent-side `- [x]` writeback (per the coordination skill update above) gives Spec Kit projects working `tasks.md` progress tracking even in pure-manual sessions; supervisor-side validation can be a follow-up if dogfood demands it.

Not in scope:
- The cross-format `--spec NAME` narrowing flag (its own change `cross-format-spec-flag`). This change's scan returns *every* incomplete feature; narrowing happens at a layer above.
- Multi-feature default behavior change for OpenSpec / Markdown (also `cross-format-spec-flag`).
- Spec Kit slash commands (`/speckit.specify`, `/speckit.plan`, `/speckit.tasks`). git-paw consumes Spec Kit *output* only.
- Full checklist-driven verification gating. Checklists are advisory in v0.5.0; full enforcement is v1.0.0.
- The `[governance.constitution]` config slot itself — owned by `governance-config`. This change provides the path; that change provides the slot.
- Cross-feature dependency resolution. Each feature's phase clock advances independently.

## Capabilities

### New Capabilities
- `spec-kit-integration`: the Spec Kit backend, `tasks.md` parser, phase-and-`[P]` decomposition, branch-naming rules, boot-prompt assembly, and the constitution-path probe.

### Modified Capabilities
- `spec-scanning`: add the `Type "speckit" selects SpecKit backend` dispatch requirement; add the `.specify/` auto-detection requirement (when present, the system SHALL default `specs.type` to `"speckit"` and `specs.dir` to `.specify/specs` unless the user has set them explicitly).
- `agent-skills`: extend the embedded coordination skill with the Spec Kit consolidated-worktree behaviour (sequential task execution, `- [x]` writeback, `agent.done` timing). The supervisor skill is NOT modified by this change; supervisor-side checkbox auditing is out of scope per the design decision above.

## Impact

**Code**:
- `src/specs/speckit.rs` (or under `src/specs/backends/speckit.rs` if the codebase has been re-organized): new backend with `tasks.md` parser, phase identification, decomposition logic, boot-prompt builder.
- `src/specs/mod.rs` (or `src/specs/scanning.rs`): backend dispatch table gains a `"speckit"` arm.
- `src/cli.rs` (or wherever `--specs-format` lives): accept `speckit` as a value; auto-detect `.specify/` when no flag and no config.
- `src/init.rs`: `git paw init` detects `.specify/` and writes `specs.type = "speckit"` plus `specs.dir = ".specify/specs"` into the generated config.
- `assets/agent-skills/coordination.md`: append the Spec Kit consolidated-worktree section.
- `docs/src/user-guide/spec-formats.md` (or equivalent): new chapter or section for Spec Kit.

**Tests**:
- `tasks.md` parser unit tests: phase heading variants, `[P]` markers, completion state, malformed lines, duplicate task IDs.
- Decomposition tests: phase with all `[P]`, all non-`[P]`, mix, only-non-`[P]` single task, fully-completed phase advances to next phase, fully-completed feature is skipped.
- Auto-detection: `.specify/` present → defaults applied; explicit `[specs]` config wins; `--specs-format` CLI flag wins over both.
- Boot-prompt assembly: spec.md/plan.md included; checklists included when present; consolidated entries list tasks in order; single-`[P]` entries carry one task.
- Branch-name shape tests for both `[P]` and consolidated entries.
- Skill content tests: coordination skill mentions consolidated-worktree behaviour; supervisor skill Spec Audit covers `- [x]` validation.
- Round-trip: build a fixture `.specify/` tree with multiple features, scan, verify the entries match expected count and shape.

**Backward compatibility**:
- New optional `[specs] type = "speckit"` value. Existing configs untouched. Auto-detection only fires when no explicit `[specs]` configuration is present (matches the behaviour of other zero-config defaults).
- The `SpecEntry` type itself is unchanged on the wire; the backend just produces more entries per feature directory than the OpenSpec backend produces per change directory.
- Agents in v0.4-shaped sessions (OpenSpec or Markdown) behave identically — no new code path runs unless a Spec Kit project is present and chosen.

**Mismatches surfaced (for future tracking)**:
- The existing `[specs] dir` field is interpreted differently by each backend (OpenSpec: directory of changes; Markdown: directory of `.md` files; SpecKit: directory of feature subdirectories). This is consistent at the abstract level (the *unit container*) but worth documenting on the configuration reference page so users picking a backend understand what `dir` should point at. Folded into this change's docs work.
- Constitution auto-wiring is a cross-change handshake between `spec-kit-format` and `governance-config`. Without `governance-config` shipped, the SpecKit backend's "advertise the path" step is a no-op consumer. This is fine — the parallel change ships in the same release; if for some reason `governance-config` slips, this change's behaviour gracefully degrades.
