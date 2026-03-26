## Why

Claude Code reads `CLAUDE.md`, while other AI CLIs read `AGENTS.md`. Repos may have one, both, or neither. git-paw needs to handle all four combinations correctly — both at `init` time (root repo) and at worktree creation time — so that every CLI gets the git-paw instructions regardless of which file convention the project uses.

Users may have intentionally different content in `CLAUDE.md` and `AGENTS.md` (different instructions per CLI). Symlinking them together would destroy that separation. So symlink behavior must be opt-in via config, not automatic.

## What Changes

- New config field: `claude_md = "separate" | "symlink"` (default: `"separate"`)
  - `"separate"` — git-paw injects its section into whichever files exist, independently. No symlinks created.
  - `"symlink"` — git-paw creates symlinks so both filenames resolve to the same content.
- Root repo handling (during `git paw init`):
  - Inject the git-paw section into `AGENTS.md` (create if missing)
  - If `CLAUDE.md` exists → also inject the git-paw section into `CLAUDE.md`
  - If `claude_md = "symlink"` and only one file exists → create a symlink for the missing one
- Worktree handling (during `git paw start`):
  - Always write `AGENTS.md` in the worktree
  - If `claude_md = "symlink"` and CLI is `claude` → symlink `CLAUDE.md → AGENTS.md`
  - If `claude_md = "separate"` and CLI is `claude` → copy `AGENTS.md` content to `CLAUDE.md` (so Claude Code sees it without a symlink)
- Symlink safety: check for existing files/symlinks before creating, never overwrite

## Capabilities

### New Capabilities
- `claude-md-compat`: Config-driven CLAUDE.md handling for all file-existence combinations in root repos and worktrees

### Modified Capabilities
- `configuration`: New `claude_md` field on `PawConfig`

## Impact

- **Modified files**: `src/agents.rs` (add CLAUDE.md handling functions), `src/config.rs` (add `claude_md` field)
- **No new files** — logic lives in `agents.rs` alongside the injection functions it depends on
- **No new dependencies** — uses `std::os::unix::fs::symlink`, `std::fs::symlink_metadata`
- **Platform note**: symlink functions are Unix-only (only used when `claude_md = "symlink"`), matching git-paw's platform support
