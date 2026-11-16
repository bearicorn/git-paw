## MODIFIED Requirements

### Requirement: Current-phase decomposition into SpecEntry values

For the current phase of each feature, the system SHALL decompose tasks into `SpecEntry` values according to:

- **Each incomplete, parallel-ready `[P]` task that is not human-only** SHALL produce one `SpecEntry`:
  - `id = "<feature-dir>-<task-id>"` (e.g. `003-user-list-T009`)
  - `branch = "task/<task-id>-<slugified-description>"` (e.g. `task/T009-add-login-form`)
  - `prompt` = boot context (per the boot-prompt requirement) followed by the single task description
  - `owned_files = None`
- **All other incomplete tasks in the current phase that are not human-only** — the non-`[P]` tasks and every `[P]` task that is not parallel-ready — SHALL produce *one* consolidated `SpecEntry`:
  - `id = "<feature-dir>-phase-<N>"` (e.g. `003-user-list-phase-2`)
  - `branch = "phase/<feature-dir>-<phase-name-slug>"` (e.g. `phase/003-user-list-foundational`)
  - `prompt` = boot context followed by those task descriptions in `tasks.md` order, with task IDs prefixed (e.g. `T004 — Setup database schema`), plus a sequential-execution instruction
  - `owned_files = None`

A phase containing only parallel-ready `[P]` tasks SHALL produce N `SpecEntry` values and no consolidated entry. A phase containing only non-`[P]` tasks (including a single non-`[P]` task) SHALL produce exactly one consolidated `SpecEntry`. A phase with no incomplete tasks SHALL produce zero entries (this implies that phase is not the current phase per the previous requirement).

#### Scenario: Phase with mixed [P] and non-[P] tasks produces N+1 entries

- **GIVEN** a feature whose current phase lists 2 incomplete `[P]` tasks before 3 incomplete non-`[P]` tasks, and no task names another
- **WHEN** the backend scans
- **THEN** 3 `SpecEntry` values are produced — 2 single-task entries (one per `[P]`) and 1 consolidated entry containing all 3 non-`[P]` tasks

#### Scenario: Phase with only [P] tasks produces N entries

- **GIVEN** a feature whose current phase has 4 incomplete `[P]` tasks and no non-`[P]` tasks
- **WHEN** the backend scans
- **THEN** 4 `SpecEntry` values are produced
- **AND** no consolidated `phase/...` entry is produced

#### Scenario: Phase with only non-[P] tasks produces one consolidated entry

- **GIVEN** a feature whose current phase has 3 incomplete non-`[P]` tasks and no `[P]` tasks
- **WHEN** the backend scans
- **THEN** exactly 1 `SpecEntry` is produced
- **AND** the entry's branch begins with `phase/`
- **AND** the entry's prompt lists all 3 task descriptions in order

#### Scenario: Single non-[P] task in a phase still uses phase/ branch

- **GIVEN** a feature whose current phase has 1 incomplete non-`[P]` task and no `[P]` tasks
- **WHEN** the backend scans
- **THEN** the resulting `SpecEntry` has a branch beginning with `phase/`

#### Scenario: SpecEntry id encodes feature and task or phase

- **WHEN** a `[P]` task `T009` from feature `003-user-list` is decomposed
- **THEN** the `SpecEntry.id` is `003-user-list-T009`

- **WHEN** the consolidated entry for phase 2 of feature `003-user-list` is decomposed
- **THEN** the `SpecEntry.id` is `003-user-list-phase-2`

#### Scenario: SpecEntry owned_files is None for all SpecKit entries

- **WHEN** any `SpecEntry` is produced by the SpecKit backend
- **THEN** `owned_files` is `None`

## ADDED Requirements

### Requirement: Spec Kit all-specs launch takes one feature per wave

When `--from-all-specs`, or MCP `from_all_specs`, launches a Spec Kit project, git-paw SHALL launch only the entries of the first feature, in feature-directory order, that has an incomplete task. It SHALL name each held-back feature and its current phase on stderr. `[specs] wave = "all-features"` SHALL restore launching every feature's current phase. Selections made with `--specs` or the picker SHALL be unaffected.

#### Scenario: Only the first open feature launches

- **GIVEN** a Spec Kit project whose features `001-setup`, `002-voting` and `003-users` each have an open phase 1, and no `[specs] wave` set
- **WHEN** `git paw start --from-all-specs` runs
- **THEN** worktrees SHALL be created only for `001-setup`'s entries
- **AND** stderr SHALL name `002-voting` and `003-users` as held, each with its phase

#### Scenario: all-features restores the full wave

- **GIVEN** the same project with `[specs] wave = "all-features"`
- **WHEN** `git paw start --from-all-specs` runs
- **THEN** entries SHALL be created for the current phase of all three features

#### Scenario: A named selection is not narrowed

- **GIVEN** the same project
- **WHEN** `git paw start --specs 002-voting` runs
- **THEN** `002-voting`'s entries SHALL launch and nothing SHALL be reported as held

### Requirement: Human-only Spec Kit tasks are never assigned to a worker

The `tasks.md` parser SHALL recognise a `[HUMAN]` token (case-insensitive) after the task id, before or after `[P]`. An incomplete human-only task SHALL produce no entry and SHALL NOT appear in any worker's task list. When a current phase's only incomplete tasks are human-only, the feature SHALL produce no entry and the launch SHALL report it as waiting on a human.

#### Scenario: A human task is left out of the consolidated list

- **GIVEN** a current phase with `- [ ] T004 Create schema` and `- [ ] T005 [HUMAN] Register the OAuth app`
- **WHEN** the backend scans
- **THEN** the consolidated entry's prompt SHALL list `T004` and SHALL NOT list `T005`

#### Scenario: A phase of only human tasks launches nothing

- **GIVEN** a feature whose current phase has only `- [ ] T010 [HUMAN] [P] Approve copy`
- **WHEN** `git paw start --from-all-specs` runs
- **THEN** no entry SHALL be produced for that feature
- **AND** stderr SHALL report the feature as waiting on a human, naming `T010`

### Requirement: A [P] task after an open sequential task joins the consolidated entry

A `[P]` task SHALL be given its own entry only when no incomplete, non-human, non-`[P]` task comes before it in the current phase. Any other `[P]` task SHALL join the consolidated entry at its `tasks.md` position.

#### Scenario: A [P] task behind a sequential task is folded

- **GIVEN** a current phase listing `T004 Create models`, then `T005 [P] Add login form`, then `T006 [P] Add list view`
- **WHEN** the backend scans
- **THEN** exactly one entry SHALL be produced, the consolidated `phase/` entry
- **AND** its prompt SHALL list `T004`, `T005`, `T006` in that order

#### Scenario: Leading [P] tasks stay parallel

- **GIVEN** a current phase listing `T001 [P] Add lint config`, then `T002 Create models`
- **WHEN** the backend scans
- **THEN** `T001` SHALL have its own `task/` entry and `T002` SHALL be in the consolidated entry

### Requirement: A [P] task naming another open task of its phase joins the consolidated entry

A `[P]` task whose description names the id of another incomplete task in the same phase SHALL join the consolidated entry at its `tasks.md` position, instead of producing its own entry.

#### Scenario: An explicit dependency folds the task

- **GIVEN** a current phase listing `T009 [P] Add store` and `T010 [P] Add view using the store from T009`, both open
- **WHEN** the backend scans
- **THEN** `T009` SHALL have its own entry
- **AND** `T010` SHALL appear in the consolidated entry, not in a `task/` entry

#### Scenario: Naming a completed task does not fold

- **GIVEN** `- [x] T009 [P] Add store` and `- [ ] T010 [P] Add view using T009`
- **WHEN** the backend scans
- **THEN** `T010` SHALL have its own `task/` entry

### Requirement: A [P] worker's prompt names its parallel siblings

The prompt of a `[P]` entry SHALL list every other `[P]` entry of the same phase and wave, by task id, description and branch. It SHALL state that those siblings are on unmerged branches whose exports this worker cannot import, and that it must ask the orchestrator rather than re-declare a sibling's type or function. A phase with a single `[P]` entry SHALL carry no sibling list.

#### Scenario: Siblings are listed with the no-import rule

- **GIVEN** a current phase with parallel-ready `T009 [P] Add store` and `T010 [P] Add form`
- **WHEN** the `T009` prompt is assembled
- **THEN** it SHALL list `T010`, its description and its branch
- **AND** it SHALL state that `T010`'s exports are not available and that the worker asks the orchestrator instead of re-declaring them

#### Scenario: A lone [P] task has no sibling list

- **GIVEN** a current phase with exactly one parallel-ready `[P]` task
- **WHEN** its prompt is assembled
- **THEN** the prompt SHALL contain no sibling list

### Requirement: Spec Kit branch segments are length-capped

The segment of a Spec Kit branch after `task/` or `phase/` SHALL be at most 48 characters. It SHALL be cut at the last `-` at or before character 48 (or at 48 when there is none), with trailing `-` and `_` trimmed. When two entries of one scan would get the same capped branch, the launch SHALL be refused, naming both task or phase ids.

#### Scenario: A long task description yields a short branch

- **GIVEN** `- [ ] T012 [P] Implement the websocket reconnection handler with exponential backoff and jitter for the voting room`
- **WHEN** the backend scans
- **THEN** the branch SHALL be `task/t012-implement-the-websocket-reconnection`
- **AND** the segment after `task/` SHALL be at most 48 characters

#### Scenario: A short description is unchanged

- **WHEN** a `[P]` entry is produced for `T009` `"Add login form component"`
- **THEN** the branch SHALL be `task/t009-add-login-form-component`

#### Scenario: A capped collision refuses the launch

- **GIVEN** two entries in one scan whose capped branches are identical
- **WHEN** the launch is planned
- **THEN** it SHALL fail before any worktree is created, naming both ids
