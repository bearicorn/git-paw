## Why

The `spec-scanner` change defines a `SpecBackend` trait with a stub `OpenSpecBackend`. This change implements the real OpenSpec backend — scanning `openspec/changes/` for pending changes, extracting prompt content from `tasks.md` and spec files, and returning `SpecEntry` results that the `--from-specs` flow uses to create worktrees and sessions.

## What Changes

- Implement `OpenSpecBackend` for the `SpecBackend` trait in a new `src/specs/openspec.rs` submodule
- Scan the configured specs directory for subdirectories (each subdirectory = one change)
- Status detection: a change in `changes/` is pending; a change in `archive/` is done (ignored)
- Prompt extraction: read `tasks.md` as the primary prompt; if `specs/` subdirectory exists, append spec content
- Per-change CLI override: if `proposal.md` or `tasks.md` contains a `paw_cli: <name>` frontmatter field, use it
- File ownership extraction: if `tasks.md` or `design.md` declares owned files, extract them

## Capabilities

### New Capabilities
- `openspec-integration`: OpenSpec-format backend for spec scanning — directory traversal, status detection, prompt extraction from `tasks.md` and `specs/`

### Modified Capabilities
_(none — implements the stub from `spec-scanner`, no trait or type changes)_

## Impact

- **New files**: `src/specs/openspec.rs`
- **Modified files**: `src/specs.rs` (convert to `src/specs/mod.rs` if needed, replace stub with real import)
- **No new dependencies** — uses `std::fs`, `std::path` for directory traversal and file reading
- **Depends on**: `spec-scanner` (provides `SpecBackend` trait and `SpecEntry` struct)
