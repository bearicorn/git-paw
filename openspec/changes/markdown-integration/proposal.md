## Why

Not every project uses OpenSpec. Many teams write specs as standalone markdown files with simple frontmatter to indicate status and assignment. The markdown backend gives git-paw a lightweight alternative — drop `.md` files in a directory with `paw_status: pending` frontmatter, and `--from-specs` picks them up automatically. No tooling or directory structure required beyond the files themselves.

## What Changes

- Implement `MarkdownBackend` for the `SpecBackend` trait in a new `src/specs/markdown.rs` submodule
- Scan the configured `specs_dir` for `.md` files
- Parse YAML frontmatter for `paw_status`, `paw_branch`, and `paw_cli` fields
- Only files with `paw_status: pending` are included (other statuses are ignored)
- `paw_branch` provides the spec identifier for branch derivation (falls back to filename stem)
- `paw_cli` provides an optional per-spec CLI override
- Prompt content is the full file body after frontmatter

## Capabilities

### New Capabilities
- `markdown-integration`: Markdown-format backend for spec scanning — file traversal, frontmatter parsing, status filtering

### Modified Capabilities
_(none — implements the stub from `spec-scanner`, no trait or type changes)_

## Impact

- **New files**: `src/specs/markdown.rs`
- **Modified files**: `src/specs/mod.rs` (replace stub with real import)
- **No new dependencies** — reuses the same lightweight frontmatter parser from `openspec-integration`
- **Depends on**: `spec-scanner` (provides `SpecBackend` trait and `SpecEntry` struct)
