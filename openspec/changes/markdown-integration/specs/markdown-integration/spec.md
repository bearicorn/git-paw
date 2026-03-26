## ADDED Requirements

### Requirement: Markdown spec frontmatter schema

The markdown spec format SHALL use YAML frontmatter (delimited by `---`) with the following fields:

| Field | Required | Values | Description |
|-------|----------|--------|-------------|
| `paw_status` | Yes | `pending`, `done`, `in-progress` | Controls whether git-paw picks up this spec. Only `pending` is actionable. |
| `paw_branch` | No | kebab-case string | Branch name suffix. Combined with `branch_prefix` from config to form the full branch name (e.g., `spec/add-auth`). Falls back to filename stem if omitted. |
| `paw_cli` | No | CLI name string | Override which AI CLI to use for this spec's session. Overrides `default_spec_cli` from config. |

Example spec file:
```markdown
---
paw_status: pending
paw_branch: add-auth
paw_cli: claude
---

## Authentication Module

The system SHALL implement JWT-based authentication...
```

Minimal spec file (only required field):
```markdown
---
paw_status: pending
---

## Fix Login Bug

The login endpoint returns 500 when...
```

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
