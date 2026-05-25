## ADDED Requirements

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
