## Why

git-paw v0.2.0 adds `--from-specs` which auto-creates branches and sessions from spec files. Before launching, git-paw needs to scan a directory for pending specs, determine which ones need work, and derive branch names from them. This scanning logic is format-agnostic — it defines a trait that concrete backends (OpenSpec, Markdown) implement. The scanner is the foundation that both `openspec-integration` and `markdown-integration` build on.

## What Changes

- New `src/specs.rs` module providing:
  - `SpecEntry` struct: represents a discovered spec with branch name, CLI override, prompt content, and optional file ownership
  - `SpecBackend` trait: `scan(dir: &Path) -> Result<Vec<SpecEntry>>` — format-specific backends implement this
  - `scan_specs(config: &PawConfig) -> Result<Vec<SpecEntry>>` — reads `[specs]` config, selects the right backend, scans, and returns pending entries
  - Branch name derivation: `branch_prefix` + spec identifier (e.g., `spec/add-auth`)
- The module does NOT implement any specific format — that's `openspec-integration` and `markdown-integration`
- Returns an error if the configured `specs_dir` does not exist or is not a directory

## Capabilities

### New Capabilities
- `spec-scanning`: Trait-based spec discovery, pending detection, branch derivation, and format-agnostic scanning

### Modified Capabilities
_(none)_

## Impact

- **New files**: `src/specs.rs`
- **Modified files**: `src/main.rs` or `src/lib.rs` (add `mod specs;`)
- **No new dependencies** — uses `std::path`, `std::fs`, and existing crate types
- **Consumers**: `start --from-specs` flow calls `scan_specs()` to get the list of specs, then creates worktrees and sessions from the results
