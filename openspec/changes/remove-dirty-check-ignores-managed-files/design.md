## Context

`git paw remove <branch>` runs a D7 uncommitted-work safety check before
deleting an agent worktree (src/main.rs ~2575). It calls
`git::uncommitted_files(&worktree_path)` (src/git.rs ~575), which shells out to
`git status --porcelain` and returns every changed/untracked path. If that list
is non-empty and neither `--force` nor `--keep-worktree` was passed, remove
refuses and prints the files as "uncommitted changes".

The problem is what counts as "uncommitted". When `start` provisions an agent
worktree, `agents::setup_worktree_agents_md` (src/agents.rs ~269) writes a
gitignored **sidecar** `.git-paw/AGENTS.local.md` containing the injected
per-worktree assignment (the post-F10 design: the tracked `AGENTS.md` is no
longer touched; the injection lives in the sidecar). The function writes the
sidecar file first (line ~297) and only afterwards calls
`exclude_from_git(worktree_root, SIDECAR_REL_PATH)` (line ~311), which appends
the path to the common git dir's `info/exclude`.

Between those two steps — and in any worktree whose repo `.gitignore` predates
this file, where `info/exclude` is the only thing keeping the sidecar out of
`git status` — `git status --porcelain` reports `.git-paw/AGENTS.local.md` as
an untracked file. A `remove` issued in that window sees git-paw's own injected
state and refuses, listing `.git-paw/AGENTS.local.md` (whose first body line is
`**WARNING:` from the managed block, hence the confusing error text). No user
edit exists; git-paw is blocking on its own bookkeeping. This is the v0.8.0
regression behind the two flaky e2e tests, which both `start` then immediately
`remove` a clean agent.

Constraints: `--force`/`--keep-worktree` semantics, the session JSON shape, and
the CLI surface must not change. Genuine user uncommitted work MUST still block
removal. v0.2.0 sessions/configs must continue to load.

## Goals / Non-Goals

**Goals:**
- `remove` MUST NOT refuse a worktree whose only uncommitted entries are
  git-paw's own managed/injected files (the sidecar `.git-paw/AGENTS.local.md`
  and any residual managed `<!-- git-paw:start -->` block in `AGENTS.md`).
- `remove` MUST still refuse (sans `--force`) when a genuine user-authored
  uncommitted change is present, and the refusal MUST list only the user files.
- Close the write-then-exclude race so the sidecar is excluded the instant it
  lands, independent of the new filter (defense in depth).
- Make the two flaky e2e tests deterministic and add regression coverage.

**Non-Goals:**
- No change to `--force` (still removes regardless) or `--keep-worktree` (still
  bypasses the check entirely — nothing is deleted from disk).
- No change to how `start` injects the sidecar or to the managed-block format.
- No change to `purge`/`remove_worktree` (those already force-remove).
- No new config field, CLI flag, or session-JSON field.

## Decisions

### D1: Filter the dirty set at the remove call site, not inside `uncommitted_files`

`git::uncommitted_files` stays a faithful `git status --porcelain` reporter so
other callers and tests keep seeing raw status. The `remove` command filters
the returned vec against a git-paw-managed predicate before deciding to refuse.

A small helper `git_paw::agents::is_managed_path(rel: &str) -> bool` (or an
equivalent on the agents module) classifies a worktree-relative path as
git-paw-managed. It returns true for `SIDECAR_REL_PATH` (`.git-paw/AGENTS.local.md`)
and for `AGENTS.md` *only when* its current on-disk content still carries a
managed `<!-- git-paw:start -->` block AND the file is otherwise unmodified vs
HEAD (so a user editing AGENTS.md outside the block still blocks). Practically,
because the post-F10 design no longer injects into the tracked `AGENTS.md`, the
common case is the sidecar; the AGENTS.md branch covers self-healing of
worktrees provisioned by an older git-paw that still has the managed block.

**Alternative considered — filter inside `uncommitted_files`:** rejected. That
function's doc contract says it reports raw porcelain status; changing it would
ripple into unrelated callers and weaken its testability.

**Alternative considered — pass the managed set as a parameter to a new
`uncommitted_files_excluding`:** acceptable but heavier; a call-site filter
over the existing function is simpler and keeps the predicate co-located with
the injection logic in `agents.rs` where `SIDECAR_REL_PATH` already lives.

### D2: The filtered list drives both the refusal decision AND the message

After filtering, if the residual (user) list is empty, remove proceeds exactly
as the clean path. If it is non-empty, remove refuses and prints only the
residual user files — the managed files are never shown, so the user is never
told to commit git-paw's own injection.

### D3: Reorder `setup_worktree_agents_md` to exclude-before-write (race fix, defense in depth)

`exclude_from_git(worktree_root, SIDECAR_REL_PATH)` is moved to run BEFORE
`fs::write(&sidecar, …)`. `info/exclude` is a git-level ignore list; adding the
path before the file exists is valid and means `git status` never reports the
sidecar even for a single status call. This is belt-and-suspenders with D1: D1
is the authoritative fix (covers older worktrees, partial writes, and the
managed AGENTS.md case), D3 removes the timing window at the source.

**Alternative considered — only fix the race (D3) and skip the filter:**
rejected. The race fix alone does not cover worktrees provisioned by an older
git-paw, does not cover the managed AGENTS.md block, and is inherently timing
sensitive (any future reordering could reintroduce it). The filter is the
durable invariant; the reorder is hygiene.

### D4: De-flake the e2e tests by making the behavior deterministic, not by sleeping

The two flaky tests fail because the sidecar shows as dirty before exclusion.
With D1+D3 the clean-agent remove is deterministically successful regardless of
timing, so the tests need no sleeps or retries — they simply assert success,
which now always holds. Under `cargo llvm-cov` the instrumentation widened the
race window, which is exactly why they flaked worse there; removing the race
removes the llvm-cov sensitivity.

## Risks / Trade-offs

- [A user legitimately edits `.git-paw/AGENTS.local.md` and remove silently
  discards it] → Mitigation: the sidecar is git-paw-managed ephemeral state
  documented as gitignored and regenerated each session; it is never a user
  artifact. Treating it as non-blocking is correct by design.
- [The predicate is too broad and masks a real edit to `AGENTS.md`] →
  Mitigation: the AGENTS.md branch of `is_managed_path` only treats the file as
  managed when it is unmodified vs HEAD apart from the managed block; any user
  hunk outside the block keeps it in the residual list and blocks removal.
- [Reordering exclude-before-write breaks if the worktree `.git` dir is not yet
  resolvable] → Mitigation: by the time `setup_worktree_agents_md` runs the
  worktree is fully created and registered with git; `exclude_from_git` already
  resolves the common dir for linked worktrees. If exclude fails it returns
  `Err` exactly as today; ordering does not change its failure modes.

## Migration Plan

Pure behavior tightening; no data migration. Older worktrees with a stale
managed `AGENTS.md` block or an un-excluded sidecar are handled by D1 at remove
time and self-heal on the next `start`/`setup_worktree_agents_md` via D3.
Rollback is reverting the call-site filter and the reorder; no persisted state
changes.

## Open Questions

None.
