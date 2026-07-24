# superpowers-integration Specification

## Purpose
Integrate the [obra/superpowers](https://github.com/obra/superpowers) methodology's implementation plans as a first-class git-paw spec source. This capability defines how the `superpowers` spec backend scans `docs/superpowers/plans/*.md` plan documents, decomposes each incomplete plan into a launchable `SpecEntry`, derives its branch name, and builds the agent task prompt — so a superpowers-planned project drives spec-driven git-paw sessions the same way OpenSpec, Markdown, and Spec Kit projects do.
## Requirements
### Requirement: SuperpowersBackend implements SpecBackend trait

The system SHALL provide a `SuperpowersBackend` type that implements the existing `SpecBackend` trait. Unlike the Spec Kit backend (which treats the configured directory as a parent of feature *directories*), the `scan(&Path)` method SHALL treat the configured directory as a flat directory of plan **files**: each immediate `*.md` file in `<dir>/` is one obra/superpowers implementation plan (as produced by the `writing-plans` skill). The default directory SHALL be `docs/superpowers/plans/`.

Subdirectories and non-`.md` files SHALL be ignored. For each plan file, the backend SHALL produce zero or one `SpecEntry` per the decomposition rule below.

#### Scenario: Backend scans plan files, not subdirectories

- **GIVEN** a directory containing `docs/superpowers/plans/2026-07-20-add-auth.md` and `docs/superpowers/plans/2026-07-21-export-csv.md`, plus an unrelated `notes.txt` and a `drafts/` subdirectory
- **WHEN** `SuperpowersBackend::scan("docs/superpowers/plans/")` is called
- **THEN** the result includes `SpecEntry` values referencing the two `.md` plan files
- **AND** `notes.txt` and the `drafts/` subdirectory are ignored

#### Scenario: Empty plans directory returns empty Vec

- **WHEN** `SuperpowersBackend::scan` is called on a directory with no `.md` files
- **THEN** the result is an empty `Vec`

### Requirement: Plan-document parser

The system SHALL parse a superpowers plan file using line-oriented pattern matching. The parser SHALL recognise:

- **Plan header marker**: the `writing-plans` header line `REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development` (case-insensitive, leniency on surrounding markdown), used to confirm the file is a plan.
- **Plan metadata**: the `**Goal:**`, `**Architecture:**`, and `**Tech Stack:**` header fields (each optional).
- **Task heading**: `### Task <N>: <name>` (with leniency on the separator and surrounding whitespace).
- **Incomplete step**: a checkbox line `- [ ] <text>` (the plan's bite-sized steps, optionally bolded like `- [ ] **Step 1: …**`).
- **Complete step**: `- [x] <text>` (case-insensitive `x`).
- **Task file list**: a `**Files:**` block whose `Create:`/`Modify:`/`Test:` lines name paths.

Lines that match no pattern SHALL be ignored — the parser SHALL NOT error on the prose, code blocks, or `Run:` commands interleaved between steps. Steps SHALL be associated with the most recent preceding `### Task N` heading.

#### Scenario: Task heading and its steps are parsed

- **GIVEN** a plan containing `### Task 1: Add validation` followed by `- [ ] **Step 1: Write the failing test**` and `- [x] **Step 2: Run it**`
- **WHEN** the parser runs
- **THEN** a task `1` named `Add validation` is recorded with one incomplete and one complete step

#### Scenario: Complete step is parsed regardless of x case

- **GIVEN** a plan containing both `- [x] ...` and `- [X] ...` step lines
- **WHEN** the parser runs
- **THEN** both steps are recorded as complete

#### Scenario: Files block and interleaved prose do not error

- **GIVEN** a task whose body contains a `**Files:**` block with `Create:`/`Test:` lines, a fenced code block, and a `Run:` command line
- **WHEN** the parser runs
- **THEN** the parser succeeds
- **AND** the code block and `Run:` line are not mistaken for steps

### Requirement: Incomplete-plan identification

The system SHALL treat a plan as in-scope for the session iff it contains at least one incomplete (`- [ ]`) step across all its tasks. A plan whose every step is `- [x]` SHALL be skipped (no `SpecEntry`) with a stderr warning identifying it as complete. A file lacking the plan-header marker and containing no recognised `### Task`/step lines SHALL be skipped silently (it may be a design doc, not a plan).

#### Scenario: Plan with remaining steps is in scope

- **GIVEN** a plan whose Task 1 is fully `- [x]` and Task 2 has a `- [ ]` step
- **WHEN** the backend scans
- **THEN** one `SpecEntry` is produced for the plan

#### Scenario: Fully complete plan is skipped with a warning

- **GIVEN** a plan whose every step across every task is `- [x]`
- **WHEN** the backend scans
- **THEN** no `SpecEntry` is produced for that plan
- **AND** a warning is written to stderr identifying the plan as complete

### Requirement: Plan decomposition into one SpecEntry per plan

For each in-scope plan the system SHALL produce exactly **one** `SpecEntry`. A superpowers plan is a sequential TDD chain intended for a single `subagent-driven-development` worktree; the backend SHALL NOT fan a plan out into per-task or per-step entries (there is no Spec Kit `[P]` equivalent). The entry SHALL have:

- `id = "<plan-file-stem>"` (e.g. `2026-07-20-add-auth`)
- `branch` per the branch-name requirement below
- `prompt` = the assembled boot context (per the boot-prompt requirement)
- `owned_files = None`

#### Scenario: Each in-scope plan yields exactly one entry

- **GIVEN** two in-scope plan files, each with several incomplete tasks
- **WHEN** the backend scans
- **THEN** exactly two `SpecEntry` values are produced — one per plan
- **AND** no per-task or per-step entries are produced

#### Scenario: SpecEntry id is the plan file stem, owned_files is None

- **WHEN** an entry is produced for `docs/superpowers/plans/2026-07-20-add-auth.md`
- **THEN** the `SpecEntry.id` is `2026-07-20-add-auth`
- **AND** `owned_files` is `None`

### Requirement: Boot-prompt assembly

The system SHALL assemble the `SpecEntry.prompt` from the following sections, separated by `\n\n---\n\n`:

1. **Plan Context** — the plan's `Goal` / `Architecture` / `Tech Stack` header fields (those present), verbatim.
2. **Your Tasks** — the plan's tasks in file order, each rendered with its `### Task N: <name>` heading and its steps. Completed (`- [x]`) steps MAY be included for context but SHALL be clearly marked done; at least all incomplete steps and their `Files:`/`Run:` metadata SHALL be included verbatim so the agent has the exact paths and commands.
3. **Execution instruction** — text telling the agent to work the steps in order, flip each `- [ ]` to `- [x]` in the plan file as it completes the step (mid-flight writeback), and publish `agent.done` only when every step in the plan shows `- [x]`.

#### Scenario: Boot prompt carries plan header and remaining tasks

- **GIVEN** a plan with a `**Goal:**` line and two incomplete tasks with `Files:` and `Run:` metadata
- **WHEN** the backend assembles the prompt
- **THEN** the prompt contains the Goal under a "Plan Context" section
- **AND** the prompt contains both tasks' descriptions, `Files:` paths, and `Run:` commands under a "Your Tasks" section

#### Scenario: Boot prompt instructs checkbox writeback and completion signal

- **WHEN** a superpowers `SpecEntry.prompt` is inspected
- **THEN** it instructs the agent to flip `- [ ]` → `- [x]` in the plan file per completed step
- **AND** it instructs the agent to publish `agent.done` only when all steps show `- [x]`

### Requirement: Branch-name shape for Superpowers entries

The system SHALL derive each entry's branch by applying the existing `slugify_branch` helper to the plan file stem, prefixed with `plan/`: `plan/<slugified-stem>`. Branch names SHALL contain only characters from the slug set `[a-z0-9/_-]`.

#### Scenario: Plan file produces a plan/ branch

- **WHEN** an entry is produced for `2026-07-20-Add-Auth.md`
- **THEN** the branch is `plan/2026-07-20-add-auth`

#### Scenario: Branch slug contains only safe characters

- **WHEN** any Superpowers `SpecEntry` branch is produced
- **THEN** it contains only characters from the slug set `[a-z0-9/_-]`

### Requirement: SuperpowersBackend skips invalid plans without failing the scan

The system SHALL skip plan files that cannot produce a `SpecEntry` — a fully-`- [x]` plan (with a warning) and a file with no recognised tasks/steps (silently) — and SHALL continue processing the remaining plan files. Skipping one plan SHALL NOT cause the overall scan to fail.

#### Scenario: An invalid plan is skipped and others still scan

- **GIVEN** three plan files, one of which contains no recognised `### Task`/step lines
- **WHEN** the backend scans
- **THEN** the no-task file produces no `SpecEntry`
- **AND** the other two plan files still produce their entries
- **AND** the scan does not return an error

