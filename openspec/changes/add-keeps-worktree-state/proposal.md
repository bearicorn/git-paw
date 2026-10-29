## Why

Swapping a worker's CLI is `git paw remove <branch> --keep-worktree` followed by
`git paw add <branch> --cli <other>`. In the 0.18.0 dogfood (finding GP-87) the second step failed
whenever the kept worktree had uncommitted changes: `add` always asks `create_worktree` to rebase an
existing branch onto the default branch (`src/commands/add.rs` passes `no_rebase: false`), so git
refused with "cannot rebase: You have unstaged changes" after `remove` had already detached the
agent. Nothing was lost, but the worker stayed out of the session until its tree was clean. On a
clean tree with a moved default branch the same swap succeeded but rebased the worker's commits onto
the new tip in the middle of its task. `start` has a `--no-rebase` opt-out; `add` has none.

## What Changes

- **`git paw add --no-rebase`** — same meaning as `start --no-rebase`: the existing branch is not
  rebased onto the default branch before its worktree is opened or reused. Without the flag, `add`
  keeps rebasing an existing branch, as it does today.
- **`add` never rebases over uncommitted work.** When the branch already has a worktree and that
  worktree holds uncommitted changes (judged by the same detection `remove`'s safety check uses,
  which ignores git-paw's own injected files), `add` skips the rebase, prints a one-line notice on
  stderr naming the skip and `--no-rebase`, and attaches the agent on the branch's current base.
  It never stashes, commits, resets or discards anything in that worktree.
- No new verb. An atomic "swap a worker's CLI" command is recorded as an open question in the design.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `git-add-remove-branch`: *git paw add subcommand* gains `--no-rebase`; *Worktree creation reuses
  start conventions* states `add`'s rebase step with `start`'s semantics; a new requirement,
  *add never rebases over uncommitted work*, covers the dirty-worktree case.

## Impact

- **Code:** `src/cli.rs` (the `add` arguments: `--no-rebase` with a `help` string and a
  `long_about` example), `src/commands/add.rs` (thread the flag into `AttachContext.no_rebase`;
  check the reused worktree with `git::uncommitted_files` filtered by `agents::is_managed_path`
  before `attach_agent`, and pass `no_rebase = true` with a notice when it is dirty). `create_worktree`,
  `rebase_branch_onto_default_with` and `start` are unchanged.
- **CLI:** one new optional flag on `add`; the default behaviour on a clean worktree is unchanged.
- **Enum-variant ripple:** none.
- **Security / safety:** the change only removes a git write (a rebase) in one case and adds an
  opt-out; it introduces no new process spawn beyond a read-only `git status` the `remove` path
  already runs. Uncommitted work in a reused worktree can no longer be the target of a rebase from
  `add`. Export-agnosticism: nothing exported changes; no toolchain or CLI is named.
- **Cross-version touchpoints:** v0.21.0 `uniform-json-output` adds `git paw add --json` (an ADDED
  requirement, not one this change modifies). The skip notice goes to stderr so that `--json`
  stdout stays one document; that change needs no edit. No later spec branch modifies the two
  requirements this change modifies.
- **Dependencies:** none.
