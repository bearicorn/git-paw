## Purpose

Scan an OpenSpec-style changes directory for pending changes, extracting prompt content from tasks.md and associated spec files, along with optional CLI overrides and file ownership declarations.
## Requirements
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

### Requirement: Extract paw_cli from frontmatter

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

