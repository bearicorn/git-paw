## ADDED Requirements

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
