# AGENTS.md Injection

git-paw uses `AGENTS.md` to pass context to AI CLIs running in worktrees. When launching a session (especially with `--from-specs`), git-paw generates an `AGENTS.md` file in each worktree containing the branch assignment, CLI name, spec content, and file ownership information.

## How It Works

1. **Worktrees:** When a session starts, each worktree gets its own `AGENTS.md` with branch-specific content injected.
2. **Exclusion:** Worktree `AGENTS.md` files are added to `.git/info/exclude` so they are never committed.

## Markers

git-paw manages its section using HTML comment markers:

```markdown
<!-- git-paw:start — managed by git-paw, do not edit manually -->

(git-paw content here)

<!-- git-paw:end -->
```

Content between these markers is replaced on each launch. Content outside the markers is preserved. If no markers exist, the section is appended to the end of the file.

## Worktree AGENTS.md Content

Each worktree's `AGENTS.md` includes:

- The root repo's `AGENTS.md` content (if any)
- A git-paw section with:
  - **Branch name** and **CLI name** for this worktree
  - **Spec content** (the prompt from the spec file, when using `--from-specs`)
  - **Owned files** list (from OpenSpec file ownership declarations)

