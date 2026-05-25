# Session Lifecycle

This chapter covers what `git paw start` does to your worktrees on each
launch (or relaunch), and how to control its behaviour when the defaults
don't fit.

## Rebase on start

Every `git paw start` rebases each existing agent branch onto the
repository's default branch (whatever `origin/HEAD` tracks — typically
`main`) **before** opening or reopening that branch's worktree. This
keeps agents starting from current `main`, which matters when the
supervisor (you) advances `main` while agents are still working on their
branches.

Why this is the default:

- A supervisor session typically lands commits on `main` while one or
  more agents are running on `feat/*` branches. Without rebasing on
  start, every subsequent agent commit chains from the stale baseline,
  forcing a per-branch rebase at merge time.
- The rebase is cheap when there's nothing to do: `git rebase` exits
  zero with no rewrite when the branch is already up to date.
- The branch ref is updated even in the resume case — `git paw start`
  on a session whose worktrees already exist on disk picks up the new
  `main` commits transparently.

Rebases that **don't** happen:

- A brand-new branch (one git-paw creates via `git worktree add -b`
  during this launch) is not rebased. It's already at the current
  default-branch tip by construction.
- If the repo has no `origin/HEAD` (a rare brand-new repo with no
  remote), the rebase step errors out cleanly. Workaround: run with
  `--no-rebase`, or push the default branch to `origin` and set
  `origin/HEAD` first.

## Opting out: `--no-rebase`

Pass `--no-rebase` when you want the pre-v0.6 behaviour — agent branches
opened at their current SHA with no rebase attempt:

```bash
git paw start --no-rebase
git paw start --no-rebase --cli claude --branches feat/auth,feat/api
git paw start --no-rebase --supervisor   # combines with other flags
```

Use cases:

- You are deliberately working against a stale baseline (e.g. an agent
  is reproducing a bug on a specific historical commit).
- An external tool has pinned a pre-rebase SHA on the agent branch and
  rewriting history would break it.
- You're scripting against the pre-v0.6 launch contract and don't want
  the new default behaviour.

## Rebase conflicts

If the rebase hits a conflict — typically because both `main` and the
agent branch modified the same line of the same file — `git paw start`:

1. Runs `git rebase --abort` on the affected branch.
2. Leaves the branch at its pre-rebase HEAD (no half-rebased state
   survives).
3. Exits with an error like
   `Error: rebase onto main failed: <git's stderr listing the
   conflicting files>`.

When you see this error:

1. `cd` into the affected agent's worktree (typically at
   `../<project>-<branch-slug>/`).
2. Inspect the conflicting files git named in the error.
3. Either reconcile manually (e.g. `git rebase main` followed by
   resolving conflicts and `git rebase --continue`), or decide the
   change should land via a merge commit instead.
4. Once the branch is in a clean state, rerun `git paw start` (with or
   without `--no-rebase` depending on what you decided).

The conflict-abort path is intentional and not configurable — leaving a
branch half-rebased poisons every subsequent git operation against it,
which is far worse for the agent in that pane than a clean error at
launch time.

## What rebase does NOT do

- It does not `git fetch` or `git pull` for `main`. The rebase target
  is whatever `origin/HEAD` tracks **locally**; the user owns the
  remote-sync decision.
- It does not rewrite the working tree files inside a surviving
  worktree. After a rebase-on-resume, the worktree directory's files
  may diverge from the branch's new HEAD until the agent does its own
  checkout or pull inside the worktree. This is the standard `git
  rebase` caveat — `git status` from inside the pane will surface it.
- It does not rebase one agent branch onto another agent branch. Only
  `feat/<change>` onto the repository's default branch.
- It does not loop or retry on failure. One attempt, abort on
  conflict, surface the error.

## Resume vs. fresh launch

The rebase step runs in both flows:

- **Fresh launch**: branch exists locally, no worktree yet. Branch is
  rebased onto `main`, then the worktree is created at the rebased
  SHA.
- **Resume** (worktree already on disk from a prior session): branch
  is rebased in the existing worktree, the existence check confirms
  the worktree is registered for that branch, and `git paw start`
  returns success with the worktree's branch ref now pointing at the
  rebased HEAD.

If you're debugging a launch and want to know whether the rebase
modified anything, `git rev-parse <branch>` from the main repo before
and after `git paw start` will show the SHA change (or lack thereof).
