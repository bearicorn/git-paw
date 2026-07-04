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

## Adding and removing branches mid-session

Before v0.6.0 the branch set was frozen at `git paw start` time: changing
it meant `stop`/`purge` + re-`start`, which destroyed every agent's
in-flight conversation. `git paw add` and `git paw remove` edit a running
supervisor session's branch set in place — the other agents keep working
undisturbed.

### `git paw add` — hot-attach an agent

```bash
git paw add feat/new-thing              # attach a worktree + pane, session's default CLI
git paw add feat/api --cli codex        # choose the CLI for the new pane
git paw add --from-spec add-export      # derive branch + CLI from a discovered spec
```

What it does:

1. Resolves the branch and CLI — from the positional argument, or (with
   `--from-spec`) from a discovered spec across the OpenSpec / Markdown /
   Spec Kit backends (same resolution as `git paw start --specs NAME`). An
   unknown spec name errors with the discovered candidate list; an unknown
   `--cli` errors with the detected CLI ids — and in both cases nothing is
   created.
2. Enforces the 25-agent cap **before** touching anything. Adding the 26th
   agent fails with the same "split into multiple sessions" message
   `start` uses.
3. Creates the worktree (same naming and idempotent-create behaviour as
   `start`; re-adding an existing worktree reuses it) and attaches the
   agent through the **same** boot pipeline a start-time agent uses — the
   broker boot block, the sidecar spec body (`.git-paw/AGENTS.local.md`), and
   the initial prompt are byte-identical.
4. Re-tiles the agent grid to the layout a `start` of that many agents
   would have produced. Existing panes keep their indices, so any
   in-flight `send-keys` targeting (supervisor sweeps) still lands on the
   right pane; the new pane gets the next index. The re-tile preserves
   **every** other agent's pane — no existing agent loses its pane — and
   each agent row is rebalanced to equal width, so the added grid matches a
   start-time grid of the same count in both pane count and pane widths.
   After the re-tile, git-paw reconciles the session JSON against the live
   panes and warns if any agent has no live pane (a JSON↔tmux desync), so it
   is visible and recoverable rather than silent.
5. Registers the branch in the session so subsequent `status`, `stop`,
   `purge`, and `pause` include it.

The new agent's boot block is injected only once its CLI reaches an
interactive ready state (the launch-readiness gate), not after a blind
fixed sleep — the same protection `git paw start` applies.

The supervisor is **not** signalled directly. The new agent
auto-registers with the broker (filesystem watcher + its own boot-block
heartbeat) and the supervisor discovers it on its next sweep — no
restart, at most one sweep-interval of latency.

**Paused sessions:** if the session is paused when you `add`, the new
pane is created but its prompt is held unsubmitted — the agent stays
paused with the rest of the session and begins on the next
`git paw start`.

### `git paw remove` — detach a single agent

```bash
git paw remove feat/done-thing          # close pane, remove worktree, drop from session
git paw remove feat/wip --force         # remove even with uncommitted changes
git paw remove feat/keep --keep-worktree  # detach pane only; leave worktree + branch on disk
```

What it does:

1. Locates the agent by branch (errors with the live-agent list if it
   isn't in the session).
2. **Uncommitted-work safety:** refuses to delete a worktree with
   uncommitted changes, listing the changed files, unless you pass
   `--force`. This mirrors `git worktree remove`'s own safety.
   `--keep-worktree` skips the check entirely (nothing is deleted).
3. Kills the agent's pane and re-tiles the grid for the smaller agent
   count so it re-flows without leaving a hole. The target pane is
   resolved by mapping the removed branch's worktree to a live pane via
   `pane_current_path` and killed by its tmux pane id — **regardless of the
   process running in it** (a bare shell from a failed/never-started CLI, a
   CLI, or anything else). Killing by resolved pane id (not by a position
   computed from the session JSON) guarantees only the removed agent's pane
   is closed and never a different agent's pane, even if a stale orphan pane
   has shifted the grid. The re-tile preserves every other agent's pane and
   rebalances each row to equal width. The branch→pane mapping for the
   survivors is re-derived from `pane_current_path` on the supervisor's next
   sweep, so a mid-grid removal is safe.
4. Removes the worktree (reusing `git paw purge`'s per-worktree teardown)
   unless `--keep-worktree`, then drops the branch from the session.

`git paw remove supervisor` is refused — to end the whole session use
`git paw stop` (or `git paw purge`). The supervisor notices a removed
agent passively: its heartbeat stops, and the supervisor drops it from
its coordination scope on the next `/status` poll.

> **Scope (v0.6.0):** `add` / `remove` operate on **supervisor-mode**
> sessions (the default). Bare (`--no-supervisor`) sessions report an
> actionable error — stop and re-start them with the full branch set.
> One branch per invocation; bulk add/remove and configurable layouts
> are tracked for a later release.
