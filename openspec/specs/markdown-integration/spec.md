## Purpose

Parse markdown spec files with YAML frontmatter to discover pending specs for spec-driven launches, extracting branch names, CLI overrides, and prompt content from frontmatter fields and file bodies.
## Requirements
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

### Requirement: Extract paw_cli from frontmatter

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

