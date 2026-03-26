## Context

The `agents-md-injection` change provides marker-based injection into `AGENTS.md`. The `worktree-agents-md` change generates per-worktree AGENTS.md. Neither handles repos that use `CLAUDE.md` — either exclusively or alongside `AGENTS.md`.

Users may maintain separate `CLAUDE.md` and `AGENTS.md` with intentionally different content (different instructions for Claude Code vs other CLIs). Automatically symlinking them together would destroy that separation. The default behavior must respect this.

## Goals / Non-Goals

**Goals:**
- Handle all four file combinations (CLAUDE.md only, AGENTS.md only, both, neither) at root and worktree level
- Default to "separate" mode: inject git-paw section into each file independently
- Opt-in "symlink" mode via config for users who want unified content
- Detect existing symlinks to avoid double-symlinking or overwriting

**Non-Goals:**
- Merging divergent content between CLAUDE.md and AGENTS.md
- Supporting Windows native symlinks (WSL uses Unix symlinks)
- Handling `.claude/` directory or other Claude Code config beyond CLAUDE.md

## Decisions

### Decision 1: Config-driven behavior with `claude_md` field, set during init

```toml
claude_md = "separate"   # default — inject into each file independently
claude_md = "symlink"    # opt-in — symlink so both resolve to same content
```

When `git paw init` detects `CLAUDE.md` in the repo, it prompts the user to choose between "Keep separate" and "Symlink together" using dialoguer. The choice is persisted to `.git-paw/config.toml`. On subsequent runs, if the field is already set, the prompt is skipped. If no `CLAUDE.md` exists, no prompt is shown and the default (`separate`) applies.

**Why:** Users who have different CLAUDE.md and AGENTS.md content need the default to be safe. Making it an interactive choice during init ensures users understand the tradeoff. An enum field is clearer than a boolean because it reads as intent in the config file.

**Alternative considered:** Always default to separate with no prompt, require manual config edit for symlink mode. Rejected — the init flow is the natural moment to make this choice, and users may not know the config field exists.

### Decision 2: "Separate" mode injects into both files independently

In separate mode:
- `init` injects the git-paw section into `AGENTS.md` (create if missing)
- If `CLAUDE.md` also exists, inject the git-paw section there too
- Each file keeps its own content; only the git-paw marker section is managed

**Why:** Both CLIs get the git-paw instructions without affecting user-managed content in either file. The marker system ensures only the git-paw section is touched.

### Decision 3: "Separate" mode in worktrees copies rather than symlinks

In separate mode when the worktree CLI is `claude`:
- `AGENTS.md` is generated as usual (root content + assignment)
- `CLAUDE.md` is created as a copy of the worktree `AGENTS.md` content

**Why:** Claude Code reads `CLAUDE.md`. Without either a symlink or a copy, it wouldn't see the worktree assignment. A copy maintains file independence. The worktree files are ephemeral anyway — no content drift risk since they're regenerated each session.

### Decision 4: "Symlink" mode follows the original design

In symlink mode:
- Root: if only one file exists, symlink the other → it
- Worktree: if CLI is claude, symlink `CLAUDE.md → AGENTS.md`

Same safety checks as before: `symlink_metadata()` to detect existing symlinks, no overwriting of regular files.

### Decision 5: `AgentFileState` enum for detection

```rust
enum AgentFileState {
    ClaudeMdOnly,
    AgentsMdOnly,
    BothExist,
    NeitherExists,
}
```

Used by both modes to determine what files exist before acting.

**Why:** Exhaustive matching ensures every combination is handled.

### Decision 6: `ClaudeMdMode` enum for config

```rust
#[derive(Default)]
enum ClaudeMdMode {
    Symlink,
    Copy,
    #[default]
    Skip,
}
```

Three modes:
- **Symlink** — `AGENTS.md → CLAUDE.md` symlink, both filenames resolve to same content
- **Copy** — copy CLAUDE.md content to AGENTS.md, inject git-paw into both, user manages independently
- **Skip** — create fresh AGENTS.md with only git-paw section, inject git-paw into CLAUDE.md separately

Deserialized from the `claude_md` string in config. Defaults to `Skip` when absent.

## Risks / Trade-offs

**[Copy vs symlink in separate mode]** → Copying AGENTS.md to CLAUDE.md in worktrees means the files can drift if something modifies one. → Acceptable — worktree files are ephemeral and regenerated per session.

**[Double injection in separate mode]** → Init injects into both CLAUDE.md and AGENTS.md independently. If both exist, the git-paw section appears in both. → This is intentional — each file is self-contained.

**[Config field added to PawConfig]** → The `cli-selection` and `init-command` changes also add fields to PawConfig. → All are additive `Option` fields with `serde(default)`, merge cleanly.
