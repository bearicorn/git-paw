## Context

git-paw needs to inject instructions into per-worktree `AGENTS.md` files during `start`. This requires marker detection, section replacement, and content generation logic. This module provides that shared foundation.

The existing codebase has no markdown file manipulation. `error.rs` has `PawError` with string-carrying variants. The module follows the same pattern.

## Goals / Non-Goals

**Goals:**
- Reusable library functions for marker-based markdown section injection
- Idempotent injection: append if absent, replace if present
- Clean API surface for `worktree-agents-md` (per-worktree AGENTS.md generation)

**Non-Goals:**
- Per-worktree content generation with branch/CLI/spec context (belongs to `worktree-agents-md`)
- File permission management or git operations

## Decisions

### Decision 1: Pure functions + thin I/O wrapper

Core logic is implemented as pure `&str → String` functions:
- `has_git_paw_section(content: &str) -> bool`
- `replace_git_paw_section(content: &str, new_section: &str) -> String`
- `inject_into_content(content: &str, section: &str) -> String` (append or replace)

A thin I/O wrapper handles file reads/writes:
- `inject_section_into_file(path: &Path, section: &str) -> Result<(), PawError>`

**Why:** Pure functions are trivially testable without tempfiles. The I/O wrapper is tested separately with `tempfile`. This also lets `worktree-agents-md` compose with the pure functions directly when it needs custom content.

### Decision 2: Marker format

```
<!-- git-paw:start — managed by git-paw, do not edit manually -->
...content...
<!-- git-paw:end -->
```

The start marker includes a human-readable note. Detection uses `<!-- git-paw:start` as the prefix (ignoring the trailing comment) for forward compatibility.

**Why:** HTML comments are invisible in rendered markdown. The prefix-based detection allows changing the trailing note in future versions without breaking detection.

### Decision 3: Section replacement strategy

When replacing an existing section:
1. Find the line containing `<!-- git-paw:start`
2. Find the line containing `<!-- git-paw:end -->`
3. Replace everything from start line through end line (inclusive) with the new section
4. If end marker is missing, replace from start marker to EOF

**Why:** Inclusive replacement ensures the markers themselves are refreshed. The missing-end-marker fallback handles corrupted files gracefully rather than erroring.

### Decision 4: AgentsMdError variant

Add `AgentsMdError(String)` to `PawError` for all file I/O errors in this module. The string carries context like the file path and operation that failed.

**Why:** Distinct from `InitError`. The error message should say "AGENTS.md error" to clearly identify the source of the failure.

## Risks / Trade-offs

**[Marker in user content]** → A user could accidentally write `<!-- git-paw:start` in their own AGENTS.md content. → Low risk — the marker is tool-specific.

**[Large file performance]** → Reading entire AGENTS.md into memory for string operations. → Acceptable — these files are typically < 10KB. No streaming needed.

**[Missing end marker]** → Replacing from start to EOF could delete user content below a corrupted section. → Acceptable trade-off — the alternative (erroring) leaves the user stuck. The content was already below git-paw's managed section.
