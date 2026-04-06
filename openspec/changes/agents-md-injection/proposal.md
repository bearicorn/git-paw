## Why

AI coding CLIs read instruction files like `AGENTS.md` to understand project context. git-paw needs a reusable module that can inject a git-paw section into per-worktree AGENTS.md files at session launch time. This module provides the core read/write/inject logic with marker-based idempotency.

## What Changes

- New `src/agents.rs` module providing:
  - Generate the git-paw marker-delimited section content
  - Detect whether a file already contains a git-paw section (via `<!-- git-paw:start -->` marker)
  - Inject a section into a file: append if absent, replace-in-place if present
  - Replace the content between markers while preserving surrounding content
  - Read file content with proper error handling
- All functions are pure or take `&Path` — no CLI, no config, no side effects beyond file I/O
- This module is consumed by `worktree-agents-md` (per-worktree AGENTS.md generation)

## Capabilities

### New Capabilities
- `agents-md-injection`: Marker-based section injection into markdown files with detection, append, replace, and content generation

### Modified Capabilities
_(none — this is a new library module with no CLI or config changes)_

## Impact

- **New files**: `src/agents.rs`
- **Modified files**: `src/main.rs` or `src/lib.rs` (add `mod agents;`), `src/error.rs` (add `AgentsMdError` variant)
- **No new dependencies** — uses only `std::fs` and `std::path`
- **No CLI changes** — this is a library module, not a command
- **Consumer**: `worktree-agents-md` calls these functions for per-worktree AGENTS.md generation
