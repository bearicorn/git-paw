# spec-backends Specification

## Purpose

This capability defines how git-paw discovers pending specs and represents them as launchable `SpecEntry` values through a pluggable backend system. It covers the shared scanning framework — the unified `SpecEntry` type, the `SpecBackend` trait, config/CLI-driven backend dispatch, branch-name derivation, explicit (config-or-CLI-only) spec-system selection, and actionable `SpecError` reporting — together with each concrete backend: OpenSpec change directories, Markdown frontmatter files, Spec Kit feature/phase decomposition, and obra/superpowers implementation plans.

## Requirements

### Requirement: SpecEntry represents a discovered spec

The system SHALL represent each discovered spec as a `SpecEntry` with an id, derived branch name, optional CLI override, prompt content, and optional file ownership list.

#### Scenario: SpecEntry has all fields populated
- **WHEN** a `SpecEntry` is constructed with id, branch, cli, prompt, and owned_files
- **THEN** all fields SHALL be accessible

#### Scenario: SpecEntry with optional fields absent
- **WHEN** a `SpecEntry` is constructed without cli or owned_files
- **THEN** `cli` SHALL be `None` and `owned_files` SHALL be `None`

### Requirement: SpecBackend trait for format-specific scanning

The system SHALL define a `SpecBackend` trait with a `scan` method that takes a directory path and returns a list of `SpecEntry` results.

#### Scenario: Backend returns discovered specs
- **WHEN** a `SpecBackend` implementation scans a directory with pending specs
- **THEN** it SHALL return a `Vec<SpecEntry>` with one entry per pending spec

#### Scenario: Backend returns empty list when no pending specs
- **WHEN** a `SpecBackend` implementation scans a directory with no pending specs
- **THEN** it SHALL return an empty `Vec`

### Requirement: Scan specs from config

The system SHALL provide a `scan_specs` function that reads the `[specs]` config section, selects the correct backend, and returns discovered specs.

#### Scenario: Scan with valid config and pending specs
- **WHEN** `scan_specs()` is called with a config that has `specs.dir` and `specs.type` set, and the directory contains pending specs
- **THEN** it SHALL return the specs discovered by the matching backend

#### Scenario: Scan with no specs config
- **WHEN** `scan_specs()` is called with a config that has no `[specs]` section
- **THEN** it SHALL return `PawError::SpecError` indicating specs are not configured

#### Scenario: Scan with nonexistent specs directory
- **WHEN** `scan_specs()` is called and the configured `specs.dir` does not exist
- **THEN** it SHALL return `PawError::SpecError` mentioning the missing directory

#### Scenario: Scan with specs directory that is a file
- **WHEN** `scan_specs()` is called and `specs.dir` points to a file, not a directory
- **THEN** it SHALL return `PawError::SpecError`

#### Scenario: Scan with unknown spec type
- **WHEN** `scan_specs()` is called with `specs.type = "unknown"`
- **THEN** it SHALL return `PawError::SpecError` mentioning the unknown type

### Requirement: Branch name derivation

The system SHALL derive branch names by concatenating the configured `branch_prefix` with the spec's `id`.

#### Scenario: Default branch prefix
- **WHEN** `branch_prefix` is not set in config and a spec has `id = "add-auth"`
- **THEN** the derived branch SHALL be `"spec/add-auth"`

#### Scenario: Custom branch prefix
- **WHEN** `branch_prefix = "feat/"` and a spec has `id = "add-auth"`
- **THEN** the derived branch SHALL be `"feat/add-auth"`

#### Scenario: Branch prefix with no trailing slash
- **WHEN** `branch_prefix = "spec"` (no trailing slash) and a spec has `id = "add-auth"`
- **THEN** the derived branch SHALL be `"spec/add-auth"` (slash inserted automatically)

### Requirement: Backend dispatch by spec type

The system SHALL select the correct `SpecBackend` implementation based on the `specs.type` config field.

#### Scenario: Type "openspec" selects OpenSpec backend
- **WHEN** `specs.type = "openspec"`
- **THEN** the OpenSpec backend SHALL be used for scanning

#### Scenario: Type "markdown" selects Markdown backend
- **WHEN** `specs.type = "markdown"`
- **THEN** the Markdown backend SHALL be used for scanning

### Requirement: SpecError for scanning failures

The system SHALL use `PawError::SpecError` for all spec scanning failures with actionable messages.

#### Scenario: SpecError includes directory path
- **WHEN** a spec directory is missing
- **THEN** the error message SHALL include the path that was not found

#### Scenario: SpecError includes spec type
- **WHEN** an unknown spec type is configured
- **THEN** the error message SHALL include the unknown type name

### Requirement: Backend dispatch for Spec Kit type

The system SHALL select the `SpecKitBackend` implementation when `specs.type = "speckit"` is configured. The dispatch SHALL be additive to the existing dispatch table — `"openspec"` and `"markdown"` dispatch SHALL continue to work unchanged.

#### Scenario: Type "speckit" selects SpecKit backend

- **WHEN** `specs.type = "speckit"` is configured
- **THEN** the SpecKit backend SHALL be used for scanning

#### Scenario: Existing types continue to dispatch correctly

- **WHEN** `specs.type = "openspec"` or `"markdown"` is configured
- **THEN** the corresponding existing backend SHALL be used for scanning
- **AND** the SpecKit backend SHALL NOT be invoked

#### Scenario: Unknown type still produces a SpecError

- **WHEN** `specs.type = "unrecognised"` is configured
- **THEN** the system SHALL return a `PawError::SpecError` mentioning the unknown type
- **AND** the error message SHALL list the known types including `"speckit"`

### Requirement: --specs-format accepts speckit value

The system SHALL accept `speckit` as a valid value for the `--specs-format` CLI flag, alongside `openspec` and `markdown`. The flag's value SHALL override the `[specs]` config (there is no filesystem auto-detection to override).

#### Scenario: --specs-format speckit selects SpecKit backend

- **WHEN** `--specs-format speckit` is passed
- **THEN** the SpecKit backend SHALL be used regardless of any `[specs] type` set in config

#### Scenario: --specs-format with unknown value is rejected

- **WHEN** `--specs-format unknown-value` is passed
- **THEN** the CLI SHALL reject the invocation with an error listing valid values: `openspec`, `markdown`, `speckit`

### Requirement: Backend dispatch for Superpowers type

The system SHALL select the `SuperpowersBackend` implementation when `specs.type = "superpowers"` is configured. The dispatch SHALL be additive to the existing dispatch table — `"openspec"`, `"markdown"`, and `"speckit"` dispatch SHALL continue to work unchanged.

#### Scenario: Type "superpowers" selects Superpowers backend

- **WHEN** `specs.type = "superpowers"` is configured
- **THEN** the Superpowers backend SHALL be used for scanning

#### Scenario: Existing types continue to dispatch correctly

- **WHEN** `specs.type = "openspec"`, `"markdown"`, or `"speckit"` is configured
- **THEN** the corresponding existing backend SHALL be used for scanning
- **AND** the Superpowers backend SHALL NOT be invoked

#### Scenario: Unknown type error lists superpowers among known types

- **WHEN** `specs.type = "unrecognised"` is configured
- **THEN** the system SHALL return a `PawError::SpecError` mentioning the unknown type
- **AND** the error message SHALL list the known types including `"superpowers"`

### Requirement: --specs-format accepts superpowers value

The system SHALL accept `superpowers` as a valid value for the `--specs-format` CLI flag, alongside `openspec`, `markdown`, and `speckit`. The flag's value SHALL override the `[specs]` config.

#### Scenario: --specs-format superpowers selects Superpowers backend

- **WHEN** `--specs-format superpowers` is passed
- **THEN** the Superpowers backend SHALL be used regardless of any `[specs] type` set in config

#### Scenario: --specs-format value list includes superpowers

- **WHEN** `--specs-format unknown-value` is passed
- **THEN** the CLI SHALL reject the invocation with an error listing valid values: `openspec`, `markdown`, `speckit`, `superpowers`

### Requirement: Spec-system selection is explicit (config or CLI only)

The spec system SHALL be resolved from EXPLICIT sources only, in this precedence (highest first):

1. the `--specs-format` CLI value;
2. the `[specs]` section in `.git-paw/config.toml`.

git-paw SHALL NOT probe the filesystem to infer the spec system. When neither an `[specs]` section nor `--specs-format` is provided, spec scanning SHALL fail with an actionable error naming both remedies (add a `[specs]` section, or pass `--specs-format`). When `--specs-format` names a format but no `dir` is configured, the format's conventional directory SHALL be supplied (`.specify/specs` for `speckit`, `docs/superpowers/plans` for `superpowers`).

#### Scenario: Unconfigured repo errors even when layouts exist on disk

- **GIVEN** a repo with `.specify/specs/` and `docs/superpowers/plans/*.md` present on disk, no `[specs]` section, and no `--specs-format`
- **WHEN** spec scanning runs
- **THEN** it SHALL fail with an error naming `[specs]` and `--specs-format`
- **AND** it SHALL NOT infer a spec system from the filesystem

#### Scenario: Config [specs] is used verbatim regardless of on-disk layout

- **GIVEN** `[specs] type = "markdown"`, `dir = "specs"` in config, and a `.specify/specs/` directory also present on disk
- **WHEN** spec scanning runs
- **THEN** the Markdown backend SHALL be used (the `.specify/` layout is ignored)

#### Scenario: --specs-format supplies the format's conventional dir

- **WHEN** `--specs-format speckit` is passed with no configured `dir`
- **THEN** `specs.dir` SHALL default to `.specify/specs`
- **AND** `--specs-format superpowers` SHALL likewise default `specs.dir` to `docs/superpowers/plans`

### Requirement: Scan changes directory for pending changes

The `OpenSpecBackend` SHALL scan the configured directory for immediate subdirectories, treating each as a pending change.

#### Scenario: Directory with multiple changes
- **WHEN** `scan()` is called on a directory containing subdirectories `add-auth`, `fix-session`, and `add-logging`
- **THEN** it SHALL return a `SpecEntry` for each subdirectory

#### Scenario: Empty changes directory
- **WHEN** `scan()` is called on an empty directory
- **THEN** it SHALL return an empty `Vec`

#### Scenario: Directory with files only (no subdirectories)
- **WHEN** `scan()` is called on a directory containing only files (no subdirectories)
- **THEN** it SHALL return an empty `Vec`

#### Scenario: Archive directory is ignored
- **WHEN** `scan()` is called on a directory containing an `archive/` subdirectory
- **THEN** the `archive` entry SHALL NOT be included in results

### Requirement: Extract prompt content from tasks.md

The `OpenSpecBackend` SHALL read `tasks.md` from each change directory as the primary prompt content.

#### Scenario: Change with tasks.md
- **WHEN** a change directory contains `tasks.md`
- **THEN** the `SpecEntry.prompt` SHALL contain the full content of `tasks.md`

#### Scenario: Change without tasks.md
- **WHEN** a change directory does not contain `tasks.md`
- **THEN** the change SHALL be skipped (not included in results) and a warning SHALL be printed to stderr

### Requirement: Append spec content to prompt

The `OpenSpecBackend` SHALL append content from `specs/` subdirectory to the prompt when present.

#### Scenario: Change with specs directory
- **WHEN** a change directory contains `specs/<capability>/spec.md` files
- **THEN** the `SpecEntry.prompt` SHALL contain `tasks.md` content followed by each spec file's content under a heading

#### Scenario: Change without specs directory
- **WHEN** a change directory has `tasks.md` but no `specs/` subdirectory
- **THEN** the `SpecEntry.prompt` SHALL contain only the `tasks.md` content

#### Scenario: Multiple spec files
- **WHEN** a change has `specs/auth/spec.md` and `specs/session/spec.md`
- **THEN** both spec files SHALL be appended to the prompt with their capability names as headings

### Requirement: Extract paw_cli from frontmatter (OpenSpec)

The `OpenSpecBackend` SHALL extract an optional `paw_cli` field from YAML frontmatter in `tasks.md`.

#### Scenario: tasks.md with paw_cli frontmatter
- **WHEN** `tasks.md` starts with `---`, contains `paw_cli: gemini`, and ends frontmatter with `---`
- **THEN** `SpecEntry.cli` SHALL be `Some("gemini")`

#### Scenario: tasks.md without frontmatter
- **WHEN** `tasks.md` does not start with `---`
- **THEN** `SpecEntry.cli` SHALL be `None`

#### Scenario: tasks.md with frontmatter but no paw_cli
- **WHEN** `tasks.md` has frontmatter that does not contain `paw_cli`
- **THEN** `SpecEntry.cli` SHALL be `None`

### Requirement: Extract file ownership

The `OpenSpecBackend` SHALL extract an optional file ownership list from `tasks.md` content.

#### Scenario: tasks.md declares owned files
- **WHEN** `tasks.md` contains a line starting with `Files owned:` or `Owned files:` followed by a markdown list
- **THEN** `SpecEntry.owned_files` SHALL be `Some` containing the listed file paths

#### Scenario: tasks.md without file ownership
- **WHEN** `tasks.md` does not contain file ownership declarations
- **THEN** `SpecEntry.owned_files` SHALL be `None`

### Requirement: Spec id derived from directory name

The `SpecEntry.id` SHALL be the name of the change subdirectory.

#### Scenario: Change directory name becomes id
- **WHEN** a change exists at `changes/add-auth/`
- **THEN** `SpecEntry.id` SHALL be `"add-auth"`

### Requirement: Frontmatter excluded from prompt content

The `SpecEntry.prompt` SHALL NOT include the frontmatter block — only the content after the closing `---`.

#### Scenario: Prompt excludes frontmatter
- **WHEN** `tasks.md` has frontmatter followed by task content
- **THEN** `SpecEntry.prompt` SHALL contain only the task content, not the frontmatter delimiters or fields

### Requirement: SpecEntry backend tagging for OpenSpec entries

The `OpenSpecBackend` SHALL set `SpecEntry.backend = SpecBackendKind::OpenSpec` on every `SpecEntry` it returns from `scan()`. The field is non-optional on `SpecEntry`; the backend SHALL populate it for every entry without exception.

This enables downstream consumers (initially `build_task_prompt`, future governance and dispatch logic) to specialise behaviour based on which backend produced an entry without re-reading configuration. Backend identity is a per-entry property recorded at scan time, not a global property looked up by callers.

The `SpecBackendKind` enum SHALL be defined in `src/specs/mod.rs` (the same module as `SpecEntry` and `SpecBackend`). Initial variants are `OpenSpec` and `Markdown`; additional variants (e.g. `SpecKit`) MAY be added by future spec-backend changes without modifying this requirement.

#### Scenario: OpenSpec-scanned entries are tagged with the OpenSpec backend variant

- **GIVEN** an OpenSpec changes directory containing a pending change `add-auth/` with a valid `tasks.md`
- **WHEN** `OpenSpecBackend::scan()` is called on the directory
- **THEN** the returned `SpecEntry` for `add-auth` SHALL have `backend == SpecBackendKind::OpenSpec`

#### Scenario: Every entry in a multi-change scan carries the OpenSpec backend tag

- **GIVEN** an OpenSpec changes directory containing three pending changes (`add-auth/`, `fix-session/`, `add-logging/`), each with a valid `tasks.md`
- **WHEN** `OpenSpecBackend::scan()` is called on the directory
- **THEN** every returned `SpecEntry` SHALL have `backend == SpecBackendKind::OpenSpec`
- **AND** no returned entry SHALL carry any other `SpecBackendKind` variant

#### Scenario: Backend tag is independent of frontmatter or file ownership

- **GIVEN** an OpenSpec change whose `tasks.md` declares `paw_cli: gemini` in frontmatter and lists `Files owned: src/foo.rs` in body
- **WHEN** `OpenSpecBackend::scan()` returns the entry
- **THEN** `SpecEntry.backend` SHALL be `SpecBackendKind::OpenSpec` regardless of the CLI override or file-ownership values
- **AND** `SpecEntry.cli` and `SpecEntry.owned_files` SHALL be populated as the existing frontmatter/ownership requirements specify

### Requirement: The system SHALL parse markdown spec frontmatter schema

The markdown spec format SHALL use YAML frontmatter (delimited by `---`) with `paw_status` (required: `pending`, `done`, `in-progress`), `paw_branch` (optional kebab-case branch suffix), and `paw_cli` (optional CLI override) fields.

#### Scenario: All frontmatter fields present
- **WHEN** a file has `paw_status`, `paw_branch`, and `paw_cli` in frontmatter
- **THEN** all three fields SHALL be parsed and mapped to the corresponding `SpecEntry` fields

#### Scenario: Only required field present
- **WHEN** a file has only `paw_status: pending` in frontmatter
- **THEN** `SpecEntry.id` SHALL fall back to filename stem, `SpecEntry.cli` SHALL be `None`

#### Scenario: Unknown frontmatter fields are ignored
- **WHEN** a file has additional frontmatter fields not in the schema (e.g., `author: alice`)
- **THEN** the unknown fields SHALL be silently ignored

### Requirement: Scan directory for pending markdown specs

The `MarkdownBackend` SHALL scan the configured directory for `.md` files with `paw_status: pending` frontmatter.

#### Scenario: Directory with pending specs
- **WHEN** `scan()` is called on a directory containing `.md` files with `paw_status: pending`
- **THEN** it SHALL return a `SpecEntry` for each pending file

#### Scenario: Directory with no pending specs
- **WHEN** `scan()` is called on a directory where all `.md` files have `paw_status: done`
- **THEN** it SHALL return an empty `Vec`

#### Scenario: Empty directory
- **WHEN** `scan()` is called on an empty directory
- **THEN** it SHALL return an empty `Vec`

#### Scenario: Files without paw_status are ignored
- **WHEN** `scan()` is called on a directory containing `.md` files without `paw_status` frontmatter
- **THEN** those files SHALL NOT be included in results

#### Scenario: Non-markdown files are ignored
- **WHEN** the directory contains `.txt`, `.toml`, or other non-`.md` files
- **THEN** those files SHALL NOT be included in results

#### Scenario: Subdirectories are ignored
- **WHEN** the directory contains subdirectories
- **THEN** subdirectories SHALL NOT be traversed

### Requirement: Parse paw_status from frontmatter

The `MarkdownBackend` SHALL read the `paw_status` field from YAML frontmatter to determine if a spec is pending.

#### Scenario: paw_status is pending
- **WHEN** a file has `paw_status: pending` in frontmatter
- **THEN** it SHALL be included in scan results

#### Scenario: paw_status is done
- **WHEN** a file has `paw_status: done` in frontmatter
- **THEN** it SHALL NOT be included in scan results

#### Scenario: paw_status is in-progress
- **WHEN** a file has `paw_status: in-progress` in frontmatter
- **THEN** it SHALL NOT be included in scan results

#### Scenario: No frontmatter
- **WHEN** a file has no YAML frontmatter delimiters
- **THEN** it SHALL NOT be included in scan results

### Requirement: Derive spec id from paw_branch or filename

The `MarkdownBackend` SHALL use `paw_branch` frontmatter for the spec id, falling back to the filename stem.

#### Scenario: File with paw_branch
- **WHEN** a file has `paw_branch: add-auth` in frontmatter
- **THEN** `SpecEntry.id` SHALL be `"add-auth"`

#### Scenario: File without paw_branch
- **WHEN** a pending file named `fix-session.md` has no `paw_branch` in frontmatter
- **THEN** `SpecEntry.id` SHALL be `"fix-session"` (filename stem)

### Requirement: Extract paw_cli from frontmatter (Markdown)

The `MarkdownBackend` SHALL extract an optional `paw_cli` field for per-spec CLI override.

#### Scenario: File with paw_cli
- **WHEN** a file has `paw_cli: gemini` in frontmatter
- **THEN** `SpecEntry.cli` SHALL be `Some("gemini")`

#### Scenario: File without paw_cli
- **WHEN** a file has no `paw_cli` in frontmatter
- **THEN** `SpecEntry.cli` SHALL be `None`

### Requirement: Use file body as prompt content

The `SpecEntry.prompt` SHALL contain the full file content after frontmatter, excluding the frontmatter block itself.

#### Scenario: File with frontmatter and body
- **WHEN** a file has frontmatter followed by markdown content
- **THEN** `SpecEntry.prompt` SHALL contain only the body content after the closing `---`

#### Scenario: File with only frontmatter
- **WHEN** a file has frontmatter but no body content
- **THEN** `SpecEntry.prompt` SHALL be an empty string

### Requirement: File ownership is not supported in markdown format

The `MarkdownBackend` SHALL always set `SpecEntry.owned_files` to `None`.

#### Scenario: Owned files always None
- **WHEN** any markdown spec is scanned
- **THEN** `SpecEntry.owned_files` SHALL be `None`

### Requirement: SpecEntry backend tagging for Markdown entries

The `MarkdownBackend` SHALL set `SpecEntry.backend = SpecBackendKind::Markdown` on every `SpecEntry` it returns from `scan()`. The field is non-optional on `SpecEntry`; the backend SHALL populate it for every entry without exception.

This enables downstream consumers (initially `build_task_prompt`, future governance and dispatch logic) to specialise behaviour based on which backend produced an entry without re-reading configuration. Backend identity is a per-entry property recorded at scan time, not a global property looked up by callers.

The `SpecBackendKind` enum is defined in `src/specs/mod.rs` (the same module as `SpecEntry` and `SpecBackend`). Initial variants are `OpenSpec` and `Markdown`.

#### Scenario: Markdown-scanned entries are tagged with the Markdown backend variant

- **GIVEN** a Markdown specs directory containing a pending file `add-auth.md` whose frontmatter declares `paw_status: pending`
- **WHEN** `MarkdownBackend::scan()` is called on the directory
- **THEN** the returned `SpecEntry` for `add-auth` SHALL have `backend == SpecBackendKind::Markdown`

#### Scenario: Every pending Markdown file in a multi-file scan carries the Markdown backend tag

- **GIVEN** a Markdown specs directory containing three pending `.md` files (`add-auth.md`, `fix-session.md`, `add-logging.md`), each with `paw_status: pending`
- **WHEN** `MarkdownBackend::scan()` is called on the directory
- **THEN** every returned `SpecEntry` SHALL have `backend == SpecBackendKind::Markdown`
- **AND** no returned entry SHALL carry any other `SpecBackendKind` variant

#### Scenario: Non-pending Markdown files are filtered before the backend tag is applied

- **GIVEN** a Markdown specs directory containing `done.md` (`paw_status: done`) and `pending.md` (`paw_status: pending`)
- **WHEN** `MarkdownBackend::scan()` is called on the directory
- **THEN** the returned `SpecEntry` list SHALL contain exactly one entry (for `pending.md`) with `backend == SpecBackendKind::Markdown`
- **AND** no entry SHALL be returned for `done.md`

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

### Requirement: Boot-prompt assembly (Spec Kit)

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

### Requirement: Boot-prompt assembly (Superpowers)

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

### Requirement: README documents `[specs] type` accepts all three backends

The README's configuration excerpt for the `[specs]` section SHALL
document `type` as accepting `"openspec"`, `"markdown"`, AND
`"speckit"`. The previous v0.4 listing of only `"openspec"` and
`"markdown"` SHALL be replaced.

#### Scenario: README specs example lists all three backends

- **WHEN** the README's `[specs]` configuration excerpt is inspected
- **THEN** it contains the substrings `"openspec"`, `"markdown"`, AND `"speckit"` as documented values of `type`

### Requirement: Spec-driven launch chapter documents the Spec Kit backend

`docs/src/user-guide/spec-driven-launch.md` SHALL include a
section documenting the Spec Kit backend. The section SHALL
cover:

1. The `[specs] type = "speckit"` configuration value.
2. The auto-detection rule: when `.specify/` exists at the
   repository root and no `[specs]` configuration is set, the
   system defaults to `type = "speckit"` and
   `dir = ".specify/specs"`.
3. A minimal worked example showing how `[P]` markers in
   `tasks.md` decompose into per-task worktrees and how
   non-`[P]` tasks consolidate into a single `phase/...`
   worktree.
4. A reference to the constitution auto-wiring into
   `[governance]` (one sentence; the detail lives in
   `configuration/README.md#governance` or similar).

#### Scenario: Spec-driven launch chapter describes Spec Kit auto-detection

- **WHEN** the Spec Kit section is inspected
- **THEN** it contains the substring `.specify/` (the directory git-paw probes)
- **AND** it documents the auto-detection behaviour

#### Scenario: Spec-driven launch chapter explains [P] decomposition

- **WHEN** the Spec Kit section is inspected
- **THEN** it explains that `[P]` markers in `tasks.md`
  decompose into per-task worktrees
- **AND** it explains that non-`[P]` tasks consolidate into a
  single `phase/...` worktree
