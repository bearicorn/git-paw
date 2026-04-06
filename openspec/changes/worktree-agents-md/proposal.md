## Why

When git-paw creates worktrees for parallel AI sessions, each AI coding CLI needs to know its branch assignment, file ownership, and the spec it should implement. The root repo's `AGENTS.md` contains project-wide instructions, but each worktree needs additional per-session context. This change generates ephemeral per-worktree `AGENTS.md` files that combine root content with session-specific assignments, and prevents them from being accidentally committed.

## What Changes

- When `git paw start` creates worktrees, each worktree gets a generated `AGENTS.md`:
  1. Read the root repo's `AGENTS.md` content (preserves all project-wide instructions)
  2. Append a git-paw session assignment section with markers
  3. Write to the worktree root
  4. Add `AGENTS.md` to `.git/info/exclude` in the worktree (per-worktree gitignore) to prevent committing
- The assignment section includes: branch name, CLI name, spec content (if available), and file ownership (if available)
- On `purge`, worktree AGENTS.md files are cleaned up with the worktrees themselves
- On `stop`, worktrees (and their AGENTS.md) are preserved for session recovery

## Capabilities

### New Capabilities
- `worktree-agents-md`: Per-worktree AGENTS.md generation with root content + session assignment, and `.git/info/exclude` management

### Modified Capabilities
- `git-operations`: Worktree creation hooks — after creating a worktree, generate its AGENTS.md and update `.git/info/exclude`

## Impact

- **Modified files**: `src/agents.rs` (all worktree AGENTS.md generation and git exclude logic lives here)
- **New logic in `src/agents.rs`**: `generate_worktree_agents_md()` and `exclude_from_git()` functions (extends the `agents-md-injection` module). Includes a `resolve_git_dir()` helper that handles worktree `.git` files (which contain a `gitdir:` pointer to the actual git directory) so that `.git/info/exclude` can be correctly located in both regular repos and worktrees.
- **No new dependencies**
- **No CLI changes** — this is triggered automatically during worktree creation
