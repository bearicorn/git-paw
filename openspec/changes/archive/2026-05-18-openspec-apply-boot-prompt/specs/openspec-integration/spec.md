## ADDED Requirements

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
