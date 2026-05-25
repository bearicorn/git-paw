## Context

`worktree-resume-fix` solved the "tmux server crashed mid-session"
recovery story by making `create_worktree` idempotent on the existence
check. It explicitly deferred any handling of *stale* worktrees — see
that change's Open Questions: "Should the resume path detect that the
branch's HEAD has changed since the worktree was created? Out of scope
for v0.5.0."

v0.5.0 dogfood made the answer "yes". The supervisor's job description
includes "advance main while agents are working" — drift 48 in
`MILESTONE.md` documents three commits I landed on `feat/v0.5.0-specs`
during a live session. Each agent's worktree was at the old baseline.
End-of-cycle merge required per-branch rebase to reconcile.

This change makes `git paw start` rebase each agent branch onto current
`main` before opening (or re-opening) its worktree. The rebase is the
default; `--no-rebase` opts out for users who prefer the v0.4/v0.5
behaviour.

## Goals / Non-Goals

**Goals:**

- After `git paw start`, every agent worktree's branch HEAD shares an
  ancestor at current `main` (modulo new commits the branch already
  had).
- Rebase failures are visible (the user sees the stderr) and
  non-destructive (the branch is left at its original commit).
- The change is opt-out, not opt-in: dogfood proved "no rebase" is
  the surprising default.

**Non-Goals:**

- No `git fetch` or `git pull` for `main` itself. The rebase target is
  whatever `origin/HEAD`'s tracked branch is *locally*; the user owns
  remote-sync.
- No interactive conflict resolution. Conflict → abort → error.
- No multi-step rebase strategies (`--rebase-merges`,
  `--interactive`, etc.). Plain
  `git rebase <main-branch>` is the only invocation.
- No support for non-default base branches per agent. All agent
  branches rebase onto the single repo-default branch.

## Decisions

### D1. Place the rebase block inside `create_worktree`

**Choice:** Add a `rebase_onto_main: bool` parameter to
`create_worktree`. The rebase block runs at the top of the function,
before the existing existence check.

**Why:**

- `create_worktree` already owns the contract "make sure this worktree
  exists at the right state for this branch". Adding "and the branch
  is rebased onto main" extends that contract naturally rather than
  forcing every caller to remember to rebase separately.
- All call sites currently sit in `cmd_start` / `cmd_start_from_specs`.
  Putting the rebase inline at each call site would duplicate the
  logic. Moving it into `create_worktree` keeps a single source of
  truth.
- The idempotency check from `worktree-resume-fix` and the rebase block
  share a common precondition (branch exists in the local repo) and a
  common cleanup site (return the same `WorktreeCreation` struct).

**Alternatives considered:**

- *Helper function `rebase_branch_onto_main(repo_root, branch)` called
  by `cmd_start` before `create_worktree`* — visible to the user in
  the launch output, but requires every caller to remember the order.
  Rejected; centralising the contract is cleaner.
- *Free function in `cmd_start` only* — couples the launch flow to
  internal git plumbing that `create_worktree` already wraps. Rejected.

### D2. Use the existing `default_branch()` helper for "main"

**Choice:** The rebase target is the result of
`default_branch(repo_root)` (`src/git.rs:93`), which reads
`git symbolic-ref refs/remotes/origin/HEAD` and strips the
`refs/remotes/origin/` prefix.

**Why:**

- Already in the codebase; already tested.
- Matches the convention that "main" in this project means "whatever
  the remote considers default" — handles `main` vs `master` vs custom
  default branches transparently.
- No new code paths; no risk of drift between `default_branch()` and
  whatever the rebase block hard-codes.

**Alternatives considered:**

- *Hard-code `"main"` as the rebase target* — breaks any repo whose
  default branch is `master` or `trunk`. Rejected.
- *Read a `[git] base_branch` config field* — adds config surface for
  a case the existing helper already handles. Rejected; revisit in a
  future change if dogfood shows the need.

### D3. Conflict handling: abort the rebase, return an error

**Choice:** When `git rebase <main>` exits non-zero (which for
`git rebase` means conflicts requiring resolution, or any other
failure), `create_worktree` SHALL run `git rebase --abort` in the same
repo and return
`Err(PawError::WorktreeError("rebase onto main failed: <stderr>"))`.

**Why:**

- Leaving a branch half-rebased is the worst outcome — every subsequent
  `git` operation on that branch reports "you have unmerged paths" or
  similar, and the agent in that worktree has no idea why nothing
  works.
- `--abort` is documented to restore the branch to its
  pre-rebase HEAD; the caller sees a branch that looks exactly like
  it did before the launch attempted to rebase.
- The user gets an actionable error: the branch name, the fact that
  rebase failed, and git's own stderr (which usually lists the
  conflicting files).
- Single-attempt is simpler than retry loops or recovery flows; if the
  human cares they can resolve manually and re-launch.

**Alternatives considered:**

- *Skip the rebase and continue* (treat conflict as a non-fatal skip)
  — hides the problem; the agent ends up on a stale baseline silently.
  The whole point of the change is to surface divergence. Rejected.
- *Drop the user into `git mergetool` or similar* — interactive flow
  that doesn't compose with the non-TTY launch path documented in
  `cli-parsing`. Rejected.
- *Keep the half-rebased state and require manual `--continue`* —
  poisons the next launch attempt. Rejected.

### D4. When NOT to rebase

The rebase block runs only when **all** of these hold:

- `rebase_onto_main == true` (the caller asked for it).
- The target branch exists in the local repo (otherwise there's
  nothing to rebase — the existing-branch check happens before the
  rebase invocation; if the branch doesn't exist yet, the function
  proceeds to the existing
  `git worktree add -b <branch>` fallback, which creates the branch
  *from* current HEAD, so it's already rebased by construction).
- `default_branch(repo_root)` returns successfully. If the repo has no
  `origin/HEAD` (e.g. a brand-new repo with no remote), the function
  returns the same `BranchError` the helper produces. Documented as a
  limitation; not a regression because v0.5.0 doesn't rebase at all.

If the branch is already up-to-date with `main`, `git rebase` reports
"Current branch <branch> is up to date" and exits zero. Treat that as
success (no error, no special-case branch HEAD comparison needed).

If the branch and `main` share no common history (orphan branch),
`git rebase` will likely error out; the conflict-abort path handles
that the same as any other rebase failure.

### D5. Skip rebase for the just-created branch case

When `create_worktree` falls through to the `git worktree add -b
<branch>` path (the branch does not yet exist in the local repo), the
new branch is created from current HEAD by definition. There's nothing
to rebase. The rebase block SHALL run only when the branch *already*
exists in the local repo at the time `create_worktree` is invoked.

The check is: before the existing existence guard, run
`git rev-parse --verify refs/heads/<branch>`. If it succeeds, the
branch exists and the rebase block runs. If it fails, skip the rebase
and proceed straight to the existence guard / `git worktree add`.

### D6. CLI flag: `--no-rebase` on `start`, default `false`

**Choice:** Add a `--no-rebase` boolean flag to `git paw start` with
default `false`. The parsed value is stored on `StartArgs.no_rebase`.
The dispatch passes `!args.no_rebase` as the `rebase_onto_main`
parameter to each `create_worktree` call.

**Why:**

- "Off-by-default with a `--no-rebase` opt-out" is the standard CLI
  ergonomic for "the safe default that 95% of users want, with an
  escape hatch for the 5%".
- Naming it `--no-rebase` instead of `--rebase` makes the v0.4/v0.5
  behaviour explicitly the opt-out path, which is what we want users
  to think of as "the old way".
- The flag lives on `start` (and inherits to `start --from-specs`
  via the shared `StartArgs`). No other subcommand creates worktrees,
  so no other subcommand needs the flag.

**Alternatives considered:**

- *No flag — always rebase* — too aggressive for users who script
  against the v0.4/v0.5 behaviour. Rejected.
- *Default to no-rebase, flag = `--rebase`* — keeps backward
  compatibility but contradicts the dogfood evidence that "no rebase"
  is the surprising default. Rejected.
- *Config field `[git] rebase_on_start = true`* — config surface for
  what should be a per-launch decision. Rejected; revisit if users
  ask for it.

### D7. Interaction with the worktree-resume-fix idempotency check

The rebase block runs **before** the idempotency check. This means:

1. The branch is rebased (if applicable, per D4 / D5).
2. The idempotency check then runs against the now-rebased branch.
3. If a worktree already exists at the expected path AND is registered
   for the (rebased) branch, the function returns
   `Ok(WorktreeCreation { branch_created: false })` — the existing
   worktree directory survives, but its checked-out branch now points
   to the rebased SHA.

Order matters: rebasing first means the surviving worktree's branch
ref is updated transparently. If the idempotency check ran first and
returned early, the rebase would never happen on resume — defeating
the whole change.

Note: the working tree files in the surviving worktree directory do
NOT automatically update when the branch ref moves. The agent in that
pane sees stale files until it re-checks-out or pulls. This is the
same trade-off `git rebase` always makes; the docs note it as a
caveat.

### D8. Backward compatibility and the breaking-change posture

The change has three breakage surfaces:

1. **Default behaviour changes**: `git paw start` now rebases. Users
   who relied on "no rebase" must pass `--no-rebase`. This IS the
   intended change; the dogfood evidence (drift 48) justifies the
   default flip.
2. **`create_worktree` signature changes**: the new
   `rebase_onto_main: bool` parameter is a breaking change for any
   external library consumer of the `git-paw` crate. Only the binary
   itself consumes this function today, so the practical impact is
   nil; the changelog notes the signature change.
3. **`--no-rebase` is a new CLI flag**: not a breaking change for
   users; old scripts without the flag pick up the new default.

The CLI `--help` text for `start` SHALL document the new flag with a
one-line explanation and a pointer to the changelog. The README SHALL
mention the new default in the "Smart start" paragraph.

## Risks / Trade-offs

- **[Rebase rewrites SHAs, breaking any external tool that pinned a
  pre-rebase SHA]** → Mitigation: this is rebase's standard caveat;
  agents working on `feat/<change>` should never have their SHAs
  pinned externally. If a user does pin SHAs (e.g. CI tracking a PR),
  they pass `--no-rebase` and accept the v0.5 behaviour.

- **[The branch contains a force-push or unusual history that makes
  rebase hostile]** → Mitigation: conflict-abort handles it. The user
  gets an error, the branch is left at the pre-rebase HEAD, and they
  can investigate manually.

- **[Surviving worktree's working tree files diverge from the now-
  rebased branch ref]** → Documented in D7. Standard `git rebase`
  caveat. The agent in the pane sees its old uncommitted edits and the
  new branch HEAD; they reconcile via `git status` and a manual
  decision.

- **[The repo has no remote `origin/HEAD` (rare; brand-new repos)]** →
  `default_branch()` fails and `create_worktree` returns the same
  error it would have returned in v0.5 when called for a branch with
  no upstream. Acceptable; document in CLI `--help` that rebase
  requires `origin/HEAD` to be set, and the workaround is
  `--no-rebase`.

- **[The rebase runs once per agent branch at launch — N agents means
  N rebases]** → Each rebase is cheap when the branch is already
  up-to-date (single `git rebase` invocation that exits zero). The
  worst case is a per-branch rebase that needs to replay K commits;
  still seconds per branch. Not a real cost.

## Migration Plan

Single-function code change plus one CLI flag. No data, no config,
no schema migration.

1. **Code changes**:
   - `src/git.rs`: extend `create_worktree` signature, add rebase
     block before existing existence check.
   - `src/cli.rs`: add `--no-rebase` to `StartArgs`.
   - `src/main.rs` (or wherever `cmd_start` / `cmd_start_from_specs`
     live): update every `create_worktree` call site to pass
     `!args.no_rebase`.

2. **Tests** (per `tasks.md`):
   - 4 unit tests covering happy path, up-to-date no-op, conflict
     abort, and `rebase_onto_main = false`.
   - 1 integration test: launch session, advance main, restart with
     defaults, assert agent worktree HEADs include the new main
     commits.

3. **Rollback** — revert the two files. `git paw start` post-rollback
   behaves like v0.5.0 (no rebase). The new CLI flag disappears; users
   who scripted `--no-rebase` see a "unknown argument" error and need
   to drop the flag.

## Open Questions

- *Should the rebase be `git rebase --onto <main> <upstream>` rather
  than plain `git rebase <main>`?* Plain `rebase <main>` rewrites the
  branch's commits on top of `main`'s tip, which matches the desired
  semantics. `--onto` would matter if we wanted to detach a chunk of
  commits from one base and re-attach to another, which is not the
  case here. Sticking with plain `git rebase`.

- *Should we capture and stash uncommitted changes in a surviving
  worktree before rebasing?* Out of scope; the surviving worktree's
  branch ref moves but the working tree doesn't (D7), so uncommitted
  changes are untouched by the rebase itself. Stashing would change
  the visible files in the pane, surprising the agent.

- *Should `--no-rebase` also accept a per-branch list (e.g.
  `--no-rebase=feat/a,feat/b`)?* Not for v0.5/v0.6. The boolean flag
  covers the all-or-nothing case; granular control is a follow-up.

- *Should there be a `git paw rebase` subcommand to rebase
  individually after the fact?* Not in scope. The user can `cd
  ../<worktree>` and `git rebase main` directly; we don't need a
  dedicated subcommand.
