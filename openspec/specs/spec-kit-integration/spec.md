# spec-kit-integration Specification

## Purpose
TBD - created by archiving change spec-kit-format. Update Purpose after archive.
## Requirements
### Requirement: SpecKitBackend implements SpecBackend trait

The system SHALL provide a `SpecKitBackend` type that implements the existing `SpecBackend` trait. The backend's `scan(&Path)` method SHALL treat the configured directory as the parent of feature directories: each immediate subdirectory of `<dir>/` represents one Spec Kit feature.

For each feature directory `<dir>/<feature>/`, the backend SHALL produce zero or more `SpecEntry` values from the feature's current phase, per the decomposition rules below.

#### Scenario: Backend scans feature subdirectories

- **GIVEN** a directory containing `.specify/specs/001-room-setup/`, `.specify/specs/002-poker-voting/`, and `.specify/specs/003-user-list/`, each with a `tasks.md`
- **WHEN** `SpecKitBackend::scan(".specify/specs/")` is called
- **THEN** the result includes `SpecEntry` values whose ids reference each feature directory
- **AND** files at the directory root (not subdirectories) are ignored

#### Scenario: Empty specs directory returns empty Vec

- **WHEN** `SpecKitBackend::scan` is called on a directory with no subdirectories
- **THEN** the result is an empty `Vec`

### Requirement: tasks.md parser

The system SHALL parse Spec Kit `tasks.md` files using line-oriented pattern matching. The parser SHALL recognise three line shapes (case-insensitive on the `[x]` checkbox marker, with leniency on punctuation):

- **Phase heading**: `## Phase <N> <separator> <Name>` where `<separator>` is `:`, `—`, or `-` (with optional surrounding whitespace).
- **Incomplete task**: `- [ ] T<NNN> [P]? <description>` where `[P]` is optional.
- **Complete task**: `- [x] T<NNN> [P]? <description>` (case-insensitive `x`).

Lines that match no pattern SHALL be ignored — the parser SHALL NOT error on free-form prose interleaved with task lines. Tasks SHALL be associated with the most recent preceding phase heading; tasks that appear before any phase heading SHALL be associated with an implicit "Phase 0" or treated as part of a single phase if the file has no headings at all.

#### Scenario: Standard task line is parsed

- **GIVEN** a `tasks.md` line `- [ ] T001 Create project structure per implementation plan`
- **WHEN** the parser runs
- **THEN** a task with id `T001`, `[P] = false`, and the description `Create project structure per implementation plan` is recorded

#### Scenario: Task with [P] marker is parsed

- **GIVEN** a `tasks.md` line `- [ ] T009 [P] Contract test POST /api/v1/auth/otp/request`
- **WHEN** the parser runs
- **THEN** a task with id `T009`, `[P] = true`, and the description `Contract test POST /api/v1/auth/otp/request` is recorded

#### Scenario: Complete task is parsed regardless of x case

- **GIVEN** `tasks.md` lines containing both `- [x] T001 ...` and `- [X] T002 ...`
- **WHEN** the parser runs
- **THEN** both tasks are recorded as complete

#### Scenario: Phase heading variants are accepted

- **GIVEN** `tasks.md` containing the headings `## Phase 1: Setup`, `## Phase 2 — Foundational`, and `## Phase 3 - User Story 1`
- **WHEN** the parser runs
- **THEN** all three phases are recognised with their respective numbers and names

#### Scenario: Tasks attach to the preceding phase heading

- **GIVEN** a `tasks.md` with `## Phase 1: Setup` followed by two task lines, then `## Phase 2: Foundational` followed by three task lines
- **WHEN** the parser runs
- **THEN** the first two tasks are associated with phase 1
- **AND** the next three tasks are associated with phase 2

#### Scenario: Unrecognised lines do not error

- **GIVEN** a `tasks.md` with a phase heading, two task lines, and three lines of free-form commentary between them
- **WHEN** the parser runs
- **THEN** the parser succeeds
- **AND** the commentary lines are not associated with any task

#### Scenario: Phase-less tasks.md treats the whole file as one implicit phase

- **GIVEN** a `tasks.md` containing only task lines (no `## Phase ...` headings)
- **WHEN** the parser runs
- **THEN** all task lines are grouped into a single implicit phase

### Requirement: Current-phase identification

The system SHALL identify the *current phase* of each feature as the first phase (lowest phase number) that contains at least one incomplete (`- [ ]`) task. Phases earlier than the current phase SHALL be assumed complete and SHALL NOT contribute `SpecEntry` values to this session. Phases later than the current phase SHALL be deferred and SHALL NOT contribute `SpecEntry` values to this session.

If a feature has no incomplete tasks across any phase, the backend SHALL skip the feature (no `SpecEntry` produced). If a feature has no phase headings, the entire file is treated as a single implicit phase, and that phase is current iff it contains any incomplete task.

#### Scenario: Current phase is the first phase with incomplete tasks

- **GIVEN** a feature whose phase 1 has all `- [x]` tasks, phase 2 has a mix of `- [ ]` and `- [x]`, and phase 3 has all `- [ ]` tasks
- **WHEN** the backend scans
- **THEN** the current phase is phase 2

#### Scenario: Fully completed feature is skipped

- **GIVEN** a feature whose `tasks.md` is entirely `- [x]`
- **WHEN** the backend scans
- **THEN** no `SpecEntry` is produced for this feature
- **AND** a warning is written to stderr identifying the feature as complete

#### Scenario: Feature with no tasks.md is skipped

- **GIVEN** a feature directory containing `spec.md` and `plan.md` but no `tasks.md`
- **WHEN** the backend scans
- **THEN** no `SpecEntry` is produced for this feature
- **AND** a warning is written to stderr identifying the missing `tasks.md`

### Requirement: Current-phase decomposition into SpecEntry values

For the current phase of each feature, the system SHALL decompose tasks into `SpecEntry` values according to:

- **Each incomplete `[P]` task** SHALL produce one `SpecEntry`:
  - `id = "<feature-dir>-<task-id>"` (e.g. `003-user-list-T009`)
  - `branch = "task/<task-id>-<slugified-description>"` (e.g. `task/T009-add-login-form`)
  - `prompt` = boot context (per the boot-prompt requirement) followed by the single task description
  - `owned_files = None`
- **All incomplete non-`[P]` tasks in the current phase** SHALL produce *one* consolidated `SpecEntry`:
  - `id = "<feature-dir>-phase-<N>"` (e.g. `003-user-list-phase-2`)
  - `branch = "phase/<feature-dir>-<phase-name-slug>"` (e.g. `phase/003-user-list-foundational`)
  - `prompt` = boot context followed by all non-`[P]` task descriptions in `tasks.md` order, with task IDs prefixed (e.g. `T004 — Setup database schema`), plus a sequential-execution instruction
  - `owned_files = None`

A phase containing only `[P]` tasks SHALL produce N `SpecEntry` values and no consolidated entry. A phase containing only non-`[P]` tasks (including a single non-`[P]` task) SHALL produce exactly one consolidated `SpecEntry`. A phase with no incomplete tasks SHALL produce zero entries (this implies that phase is not the current phase per the previous requirement).

#### Scenario: Phase with mixed [P] and non-[P] tasks produces N+1 entries

- **GIVEN** a feature whose current phase has 2 incomplete `[P]` tasks and 3 incomplete non-`[P]` tasks
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

### Requirement: Boot-prompt assembly

The system SHALL assemble each `SpecEntry.prompt` from the following sections in this order, separated by `\n\n---\n\n`:

1. **Feature Context** — full content of `<feature>/spec.md` (verbatim).
2. **Implementation Plan** — full content of `<feature>/plan.md` (verbatim). Section omitted if `plan.md` is missing.
3. **Validation Criteria** — for each file in `<feature>/checklists/`, the file content is included under a heading naming the file. The section preamble SHALL state that checklists are advisory in this release. Section omitted if the directory is missing or empty.
4. **Your Task** — for `[P]` entries, the single task ID and description. For consolidated entries, an ordered list of `<task-id> — <description>` lines plus a sequential-execution instruction telling the agent to flip `- [x]` in `tasks.md` per task and to publish `agent.done` only when all listed tasks show `- [x]`.

#### Scenario: Boot prompt includes spec.md and plan.md

- **GIVEN** a feature directory with `spec.md` and `plan.md` populated
- **WHEN** the backend assembles a `SpecEntry.prompt`
- **THEN** the prompt contains the full content of `spec.md` under a "Feature Context" section
- **AND** the prompt contains the full content of `plan.md` under an "Implementation Plan" section

#### Scenario: Boot prompt omits Implementation Plan when plan.md is missing

- **GIVEN** a feature directory with `spec.md` but no `plan.md`
- **WHEN** the backend assembles a `SpecEntry.prompt`
- **THEN** the prompt contains the "Feature Context" section
- **AND** the prompt does NOT contain an "Implementation Plan" section

#### Scenario: Boot prompt includes checklists when present

- **GIVEN** a feature directory with `checklists/auth-checklist.md` and `checklists/data-checklist.md`
- **WHEN** the backend assembles a `SpecEntry.prompt`
- **THEN** the prompt contains a "Validation Criteria" section
- **AND** the section includes the content of both checklist files under headings naming each file
- **AND** the section preamble indicates the checklists are advisory

#### Scenario: Consolidated boot prompt lists tasks with IDs

- **GIVEN** a consolidated `SpecEntry` for a phase with 3 non-`[P]` tasks T004, T005, T006
- **WHEN** the prompt is inspected
- **THEN** the prompt lists all 3 tasks in `tasks.md` order
- **AND** each task entry includes its task ID prefix (e.g. `T004 — ...`)
- **AND** the prompt instructs the agent to flip `- [x]` in `tasks.md` per task as it completes
- **AND** the prompt instructs the agent to publish `agent.done` only when all listed tasks show `- [x]`

#### Scenario: Single-[P] boot prompt contains one task description

- **GIVEN** a `[P]` `SpecEntry` for task T009
- **WHEN** the prompt is inspected
- **THEN** the prompt contains the T009 description
- **AND** the prompt does NOT contain a sequential-execution instruction

### Requirement: Branch-name shape for SpecKit entries

The system SHALL produce branch names using the existing `slugify_branch` helper applied to the appropriate input string:

- For `[P]` entries: input is `<task-id>-<description>` (e.g. `T009-add-login-form`); branch becomes `task/<slugified-input>` → `task/T009-add-login-form`.
- For consolidated entries: input is `<feature-dir>-<phase-name>` (e.g. `003-user-list-Foundational`); branch becomes `phase/<slugified-input>` → `phase/003-user-list-foundational`.

Branch names SHALL contain only characters from the slug character set per the existing `slugify_branch` rules.

#### Scenario: [P] entry produces task/ branch

- **WHEN** a `[P]` `SpecEntry` is produced for task `T009` with description `"Add login form component"` in feature `003-user-list`
- **THEN** the branch is `task/T009-add-login-form-component`

#### Scenario: Consolidated entry produces phase/ branch

- **WHEN** a consolidated `SpecEntry` is produced for phase 2 (`Foundational`) of feature `003-user-list`
- **THEN** the branch is `phase/003-user-list-foundational`

#### Scenario: Branch slug contains only safe characters

- **WHEN** any SpecKit `SpecEntry` branch is produced
- **THEN** the branch contains only characters from the slug set `[a-z0-9/_-]`

### Requirement: Constitution path probe

The system SHALL provide a way for downstream consumers (e.g. governance configuration) to discover the path to a Spec Kit project's `constitution.md`. The probe SHALL examine `<dir>/../memory/constitution.md` (where `<dir>` is the configured `specs.dir`) and return `Some(path)` if the file exists, `None` otherwise.

The probe SHALL NOT modify any state or write to any configuration; consumers decide whether and how to use the path.

#### Scenario: Constitution detected when file exists

- **GIVEN** a project layout with `.specify/memory/constitution.md` and `specs.dir = ".specify/specs"`
- **WHEN** the constitution probe is called
- **THEN** the result is `Some(".specify/memory/constitution.md")`

#### Scenario: Constitution not detected when file is absent

- **GIVEN** a project layout with `.specify/specs/` but no `.specify/memory/constitution.md`
- **WHEN** the constitution probe is called
- **THEN** the result is `None`

### Requirement: SpecKitBackend skips invalid features

The system SHALL skip feature directories that cannot produce any `SpecEntry`:

- A feature directory missing `tasks.md` SHALL be skipped with a stderr warning.
- A feature directory whose `tasks.md` is entirely `- [x]` SHALL be skipped with a stderr warning.
- A feature directory whose `tasks.md` parses cleanly but contains zero recognised task lines SHALL be skipped (no warning required — the file may legitimately be a placeholder).

Skipping a feature SHALL NOT cause the overall scan to fail; other features in the same scan continue to be processed.

#### Scenario: Missing tasks.md produces a warning and is skipped

- **GIVEN** feature `003-user-list/` containing `spec.md` but no `tasks.md`
- **WHEN** the backend scans
- **THEN** no `SpecEntry` is produced for `003-user-list`
- **AND** a warning is written to stderr identifying the feature directory and the missing file

#### Scenario: Fully complete feature is skipped silently in the entries list

- **GIVEN** feature `001-room-setup/` whose `tasks.md` is entirely `- [x]`
- **WHEN** the backend scans
- **THEN** no `SpecEntry` is produced for `001-room-setup`
- **AND** other features in the same scan still produce entries

