## Context

git-paw v0.1.0's `create_worktree()` in `src/git.rs` creates a worktree and returns its path. There is no post-creation hook — worktrees get a bare checkout with no git-paw context. AI coding CLIs launching in these worktrees have no knowledge of their branch assignment, spec, or file ownership boundaries.

The `agents-md-injection` change provides the core marker-based injection functions. This change builds on top of it, adding worktree-specific content generation and git exclude management.

## Goals / Non-Goals

**Goals:**
- Generate per-worktree AGENTS.md that combines root content with session-specific assignment
- Prevent worktree AGENTS.md from being committed via `.git/info/exclude`
- Provide a clean API that the session launch flow can call after worktree creation
- Support optional spec content and file ownership injection (when available)

**Non-Goals:**
- Modifying the `create_worktree()` function signature (callers compose the two steps)
- CLAUDE.md symlink handling in worktrees (belongs to `claude-md-compat`)
- Reading specs or deriving file ownership (callers pass this data in)

## Decisions

### Decision 1: Extend `agents.rs` rather than new module

Add `generate_worktree_section()` and `setup_worktree_agents_md()` to `src/agents.rs` alongside the existing injection functions. Also add `exclude_from_git()`.

**Why:** The worktree functions reuse `inject_into_content()` and the marker format from the same module. A separate module would create a circular dependency or require extracting shared code into a third module — unnecessary complexity.

**Alternative considered:** New `src/worktree_agents.rs` module. Rejected — the functions share types, constants (markers), and logic with `agents.rs`.

### Decision 2: Caller passes context, module doesn't read specs

`generate_worktree_section()` takes a struct:
```rust
pub struct WorktreeAssignment {
    pub branch: String,
    pub cli: String,
    pub spec_content: Option<String>,
    pub owned_files: Option<Vec<String>>,
}
```

The module formats this into the marker-delimited section. It does not read spec files or config.

**Why:** Keeps the module pure — no dependency on spec-scanner, config, or filesystem layout. The session launch flow (in `main.rs` or `start` handler) is responsible for gathering the context and passing it in.

### Decision 3: `.git/info/exclude` for per-worktree gitignore

Git worktrees share the main repo's `.gitignore` but have their own `.git/info/exclude`. Adding `AGENTS.md` there prevents the worktree's generated file from showing up in `git status` without modifying the shared `.gitignore`.

**Why:** Per-worktree exclude is the correct git mechanism. Modifying `.gitignore` would affect all worktrees and the main repo.

### Decision 4: Read root AGENTS.md from main repo, not worktree

The root AGENTS.md is read from the main repo root (passed as a parameter), not from the worktree. The worktree might not have it yet, or it might be a symlink that doesn't resolve correctly in the worktree context.

**Why:** The main repo is the source of truth. Reading from there guarantees we get the committed version with any init-injected content.

## Risks / Trade-offs

**[Stale root content]** → If root AGENTS.md changes after worktrees are created, worktree copies become stale. → Acceptable — sessions are short-lived. Users can re-run `start` to refresh.

**[.git/info/exclude not always present]** → The `.git/info` directory might not exist in fresh worktrees. → Mitigation: create the directory if needed before writing the exclude file.

**[Large spec content]** → Spec content is injected verbatim into AGENTS.md. Very large specs could make the file unwieldy. → Acceptable for v0.2.0 — specs are typically < 5KB.
