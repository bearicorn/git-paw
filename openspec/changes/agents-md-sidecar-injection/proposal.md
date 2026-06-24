## Why

During worktree setup, git-paw injects its managed `<!-- git-paw:start -->` skill block into the worktree's **tracked** `AGENTS.md` and then sets `git update-index --assume-unchanged AGENTS.md` (`src/agents.rs:281`) to stop `git add -A` from committing the ephemeral injection. The guard is **file-level**, so it silently hides *all* changes to AGENTS.md — including legitimate ones. The v0.7.0 dogfood hit this hard: an agent had to commit a real AGENTS.md edit (adding the `rmcp` approved-dependency row), but `git status` reported the file unmodified and the commit was blocked; it cost ~5 cycles to diagnose, and the same bit recurred during the release cherry-pick. This is general product behaviour — it bites any project where an agent must commit an AGENTS.md change mid-session.

## What Changes

- **Inject the managed skill block into a gitignored sidecar, not the tracked `AGENTS.md`.** Write the per-worktree assignment + skill section to a sidecar instruction file (e.g. `.git-paw/AGENTS.local.md` or a gitignored `AGENTS.paw.md`) that the agent reads alongside the project's tracked `AGENTS.md`, and have git-paw symlink/point the CLI's instruction file at the combined view. The tracked `AGENTS.md` is then **never** hidden, so legitimate edits to it stage and commit normally — no `assume-unchanged` on the tracked file.
- **Drop the file-level `assume-unchanged` on the tracked `AGENTS.md`** (and the `.git/info/exclude` entry for it) once the injection no longer lands in the tracked file.
- **Agent-side guidance (skill prose):** if an agent makes a legitimate change to the tracked `AGENTS.md` relevant to its PR, it commits it normally — no special handling needed once the injection is out of the tracked file.

## Capabilities

### New Capabilities
<!-- None. -->

### Modified Capabilities
- `agents-md-injection`: inject the managed block into a gitignored sidecar instead of the tracked `AGENTS.md`; stop setting `assume-unchanged` on the tracked file.
- `worktree-agents-md`: the worktree's effective agent-instruction view = tracked `AGENTS.md` + the sidecar; the tracked file is committable.

## Impact

- Affected code: `src/agents.rs` (injection target, `assume_unchanged`/`exclude_from_git` calls at ~241/276/281/1095), and how the CLI's instruction file is pointed at the combined content.
- Tests: a worktree-setup test asserting the tracked `AGENTS.md` is NOT `assume-unchanged` and that a hand-edit to it shows in `git status` + commits; the managed block still reaches the agent via the sidecar.
- Docs: worktree/AGENTS.md docs note the sidecar.
- Backward compatible: existing sessions re-inject on next `git paw start`; the sidecar is gitignored so nothing new is tracked. Resolves the v0.7.0 footgun (finding F10).
