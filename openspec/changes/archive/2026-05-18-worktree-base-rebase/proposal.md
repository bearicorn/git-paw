## Why

`worktree-resume-fix` (archived as `2026-05-11-worktree-resume-fix`) made
`create_worktree` idempotent on the existence check so `git paw start`
recovers from crashes when worktrees survive on disk. It deliberately did
NOT address a related problem: when the surviving worktree's branch is
behind `main`, the agent's next commits land on a stale baseline.

The drift surfaced during v0.5.0 dogfood. Three commits
(`prompt-submit-fix`, `worktree-resume-fix`, `boot-prompt-full-body`)
landed on `feat/v0.5.0-specs` while agents were running. Each agent's
worktree was created from its `feat/<change>` branch at the older
`c4f3deb` baseline. Agent commits chained from that baseline, missing
the three newer commits. End-of-cycle merge orchestration then required
either rebasing every agent branch onto the up-to-date `main` or
accepting merge commits — work the supervisor would not have had to do
if the worktrees had been rebased before the agents started writing.

The pattern is structural, not accidental. Supervisor work on `main`
during an agent session is the *expected* path (drift item 48 in
`MILESTONE.md`): the human is supposed to advance `main` while agents
are still working. Without a rebase step at launch, agents will always
diverge.

## What Changes

**Code change in `git::create_worktree`** (`src/git.rs`):

Add a `rebase_onto_main: bool` parameter to `create_worktree`. When
`true`, the function SHALL rebase the target branch onto current
`origin/HEAD`'s tracked branch BEFORE its existing existence check
(from `worktree-resume-fix`). The rebase MUST:

1. Skip if the branch was just created in this `cmd_start` invocation
   (no prior commits — nothing to rebase).
2. Skip / no-op if the branch is already at or ahead of `main`
   (`git rebase` exits zero with no work; treat that as success).
3. On conflict, run `git rebase --abort` and return
   `Err(PawError::WorktreeError("rebase onto main failed: <stderr>"))`
   so the branch is never left half-rebased.

**CLI change in `cli.rs`**:

Add a `--no-rebase` flag to `git paw start` (default `false`, i.e.
rebase is the default). When `--no-rebase` is passed, the dispatch
SHALL call `create_worktree` with `rebase_onto_main: false`, matching
the post-`worktree-resume-fix` v0.5.0 behaviour exactly.

**Call-site updates**: every caller of `create_worktree` in `cmd_start`
and `cmd_start_from_specs` passes the resolved `rebase_onto_main` flag
from the parsed CLI args.

**Not in scope:**

- Rebasing the **base branch** (`main`) itself onto a remote
  (`git fetch && git pull`). The user owns the local-vs-remote sync
  decision; this change rebases agent branches onto whatever `main`
  is locally, no network operations.
- Auto-resolving rebase conflicts. A conflict is a signal that the
  human needs to look at the branch.
- A retry loop or interactive prompt on conflict. Single attempt;
  abort cleanly; surface the error.
- Rebasing **between** worktrees (e.g. agent A's branch onto agent B's
  branch). Only `feat/<change>` onto `main`.

## Capabilities

### New Capabilities

*(none — extends existing capabilities)*

### Modified Capabilities

- `git-operations`: the existing "Create worktrees as siblings of the
  repository" requirement gains a `rebase_onto_main` parameter and four
  new scenarios covering the rebase-on-resume happy path, the
  up-to-date no-op, the conflict-abort path, and the `--no-rebase`
  opt-out. The `worktree-resume-fix` idempotency contract is preserved
  verbatim; the rebase block runs BEFORE the existence check.
- `cli-parsing`: the existing "Start subcommand with optional flags"
  requirement gains a `--no-rebase` flag (boolean, defaults to
  `false`). When passed, `StartArgs.no_rebase` SHALL be `true`.

## Impact

**Code**:

- `src/git.rs::create_worktree` — signature changes from
  `fn create_worktree(repo_root: &Path, branch: &str) -> Result<WorktreeCreation, PawError>`
  to
  `fn create_worktree(repo_root: &Path, branch: &str, rebase_onto_main: bool) -> Result<WorktreeCreation, PawError>`.
  ~40 lines added at the top of the function for the rebase block
  (skip-if-new-branch check, run `git rebase <main>`, conflict-abort
  handling). Reuses the existing `default_branch()` helper at
  `src/git.rs:93` for the main-ref discovery.
- `src/cli.rs::StartArgs` — adds `no_rebase: bool` with
  `#[arg(long, default_value_t = false)]`.
- `src/main.rs` (or wherever `cmd_start` / `cmd_start_from_specs` live)
  — every `create_worktree(repo_root, branch)` call site updates to
  `create_worktree(repo_root, branch, !args.no_rebase)`.

**Tests**:

- Unit: rebase happy path (branch 2 commits behind main → rebase
  succeeds → branch HEAD now ahead of main).
- Unit: rebase up-to-date no-op (branch == main HEAD → call returns
  Ok, no error, branch HEAD unchanged).
- Unit: rebase conflict aborts cleanly (induce a conflict, assert
  error, assert branch HEAD === pre-call HEAD, assert no `.git/rebase-*`
  state directory survives).
- Unit: `rebase_onto_main = false` preserves v0.4 behaviour (same
  branch SHA before and after, no rebase attempted).
- Integration: launch a session, advance `main` by 3 commits, restart
  with default flags, assert each worktree's HEAD chain now includes
  those 3 commits.

**Backward compatibility**:

- The CLI flag defaults to "rebase is on", which IS a behaviour
  change from v0.5.0 (worktree-resume-fix did not rebase). Users who
  depended on the v0.4/v0.5 no-rebase behaviour can pass
  `--no-rebase`. The dogfood evidence is that "no rebase" is the
  surprising behaviour, not the safe one; making rebase the default
  resolves the drift the change is filed against.
- The function signature change is a breaking change for **library**
  consumers of `git-paw` as a crate. Documented in the changelog;
  the only known caller is the binary itself.

**Mismatches resolved**:

- MILESTONE.md drift item 48 ("Agent worktree base divergence from
  main") — resolved by the default-on rebase.
- The "Smart start" `--help` description now also implies "and your
  agents start from current main".
