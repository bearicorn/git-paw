## Context

Today nothing in git-paw integrates a branch. `git::merge_branch` (`git merge --no-ff --no-commit`)
has no production caller; supervised sessions integrate through prose in the bundled supervisor skill
("Merge orchestration": `git checkout main` + `git merge --ff-only feat/<branch>`), which an LLM runs
with raw shell. `git paw remove` (`commands/remove.rs::cmd_remove`) is the only agent-ending command:
dirty check (`git::uncommitted_files` filtered by `agents::is_managed_path`), pane kill by resolved id,
re-tile, `detach_worktree` (on_remove hook → `git worktree remove --force` → `git branch -D` when
`branch_created`), session save, discovery-file rewrite. `remove` refuses bare sessions
(`bare_mode_unsupported`, GP-49).

Building blocks that already exist: `git::default_branch` (single resolver, GP-12),
`git::branch_is_merged_into_with` (`merge-base --is-ancestor`), `git::predict_rebase_conflict_with`
(`merge-tree --write-tree`), `git::find_worktree_for_branch_with` (private; `worktree list --porcelain`),
`SessionLock`, `BrokerMessage::AdvancedMain` (the drive loop's `supervisor-branch-refresh` trigger).

After v0.18 the commands are a library (`cmd-library-surface`), terminals are reached through
`TerminalBackend` keyed by `SurfaceId` (`terminal-backend-seam`), and every session is pane-less
(`supervisor-pane-removal`). This design targets that shape.

## Goals / Non-Goals

**Goals:** one safe, scriptable command that completes the manual loop; a pure integration decision
that the merge queue reuses; distinct exit status for "integration refused, rebase and retry".

**Non-Goals:** pushing, PR creation (v1.5.0 provider integration); rebasing (that is
`finish-merge-queue`); integrating a
dirty worktree (`--force`); finishing several branches (queue change).

## Decisions

### D1 — Functional core: `decide_integration(facts, policy) -> IntegrationPlan`

Facts are observed first, through the version-control port: target exists?, target tip, branch tip,
`is_ancestor(branch, target)`, `is_ancestor(target, branch)`, target checked out in which worktree and
is that checkout clean, merge-tree clean. The pure decision returns one of:
`AlreadyIntegrated` · `CreateTarget { at }` · `FastForward { from, to }` · `MergeCommit` ·
`Refuse(NotFastForward | Conflict)`. The shell applies it. Unit tests are fixture-in/plan-out; one e2e
per plan against a real repo. Alternative — inline git calls in the command body like `cmd_remove` —
rejected: the queue needs the same decision per branch, and it is the target architecture.

### D2 — Integration mechanics never touch a checkout they do not own

- Target checked out in worktree W (usually the main working tree): run `git -C W merge --ff-only` /
  `git -C W merge --no-ff --no-edit` there. W must be clean (tracked changes); otherwise refuse
  (exit 1) naming W. git updates W's files consistently with the ref.
- Target checked out nowhere: fast-forward is `git update-ref refs/heads/<t> <new> <old>`
  (compare-and-swap — fails if the ref moved since observation, then finish aborts with exit 1, nothing
  changed). A merge commit is built without a checkout: `git merge-tree --write-tree <t> <b>` →
  `git commit-tree <tree> -p <t> -p <b> -m "Merge branch '<b>' into <t>"` → CAS `update-ref`. A
  conflicting merge-tree refuses with exit 3.
- Target missing: `git branch <t> <b>` (validated by `git check-ref-format --branch <t>`).

Alternative — always `git checkout <t>` at the repo root and merge — rejected: it switches the
operator's checkout and fails whenever the root is dirty. `merge-tree --write-tree` already gates `supervisor-branch-refresh`, so it
adds no new git-version floor (to verify: doctor's `MIN_GIT_VERSION` is 2.5 while `--write-tree`
needs 2.38 — a pre-existing gap noted for the doctor owner).

### D3 — Exit status: 3 means "refused by policy, nothing changed"

`PawError::IntegrationRefused(String)` → `exit_code::INTEGRATION_REFUSED = 3`. Every other failure
(dirty, unknown branch, CAS lost, target checkout dirty) stays 1; cancellation and v0.17.0's
`NotAttempted` stay 2 (the delta is written on `cli-setup-consent`'s text so it keeps
`NotAttempted`). An orchestrator
can branch on 3 ("rebase and retry") without parsing text. Alternative — reuse 1 — rejected: the queue
and an external orchestrator both need to distinguish the retryable case.

### D4 — Teardown reuses the shared per-worktree teardown

After integration, finish runs the same path as `remove` (surface close via the terminal port,
deregistration under `SessionLock`, discovery-file rewrite, `detach_worktree`). `detach_worktree`'s
branch rule from `purge-unmerged-branch-safety` ("delete only if created AND merged into the default
branch") is parameterised to take the comparison ref; finish passes the integration target, so a branch
finished `--into feat/pr` is deleted when created and contained in `feat/pr`. Only worktrees recorded in
the session file are removed; a branch checked out in an unmanaged worktree is integrated and left in
place with a note. The main working tree is never removed.

### D5 — `[finish] strategy`

New top-level table `FinishConfig { strategy: Option<FinishStrategy> }`, `FinishStrategy { FfOnly,
MergeCommit }`, `#[serde(default, skip_serializing_if)]`, absent → `ff-only`. Flags win over config.
A top-level table (not under `[supervisor]`) because finish belongs to every tier, supervised or not.

### D6 — Broker announcement

When the target is the default branch and the session's broker answers, finish publishes
`agent.advanced-main { from: "supervisor", merged_branch, new_main_sha (12 chars), base, merged_at,
summary }` — the same shape the skill publishes today, so `supervisor-branch-refresh` is unchanged.
Unreachable broker → one warning line, exit status unaffected.

### D7 — `--json` follows `cli-machine-output`, shaped for the queue

The document is a typed `FinishReport` in the shared output module from `uniform-json-output`:
`{schema_version, target, target_created, strategy, broker_notified, branches: [{branch, outcome,
reason, target_tip, worktree_removed, branch_deleted}]}`. `branches` is an array from the start so
`finish-merge-queue` extends the same document additively (more entries, more outcome values, a
`directives` key) inside schema version 1 — both changes ship in the same release, before the freeze.
Exit 3 is a *result* (like a failing `doctor` check): the document is printed. Exit 1 is a command
failure: stdout stays empty, per the contract. Alternative — print nothing on exit 3 — rejected: an
orchestrator needs the refusal reason without parsing stderr.

## Security and safety review (`security-and-safety-review`)

- **Irreversible git beyond scope:** no force, no push, no history rewrite. Ref moves are
  fast-forwards, new merge commits, or branch creation, each CAS-guarded or inside a clean checkout.
  Branch deletion keeps the fail-closed rule (created AND contained in target; undeterminable → keep).
  A dirty source worktree is never removed — there is no `--force`.
- **Rogue agent:** an agent could try to finish (integrate) its own unverified branch. Guards:
  (1) since v0.17.0 git-paw approves no permission prompt (`classifier-retirement`), so an agent's
  `git paw finish …` prompt is escalated to a human like every other prompt, and `git paw doctor`'s
  broad-grant advisory (`cli-setup-consent`, shape 5) lists a durable `git paw finish` grant left in
  the project's CLI settings; (2) finish refuses to run from a linked worktree
  (`git rev-parse --git-dir` ≠ `--git-common-dir`), so even a human-approved prompt in an agent pane
  cannot act. Residual: an agent whose CLI allow rules (or `full-auto` level) let it run raw
  `git -C <root> merge …` without a prompt can still move the target — that is the CLI's permission
  decision, pre-existing and out of scope; doctor's advisory flags the broad `git` grants that allow
  it.
- **Injection:** branch and target names come from argv / the session file and reach git as discrete
  `CommandRunner` arguments, never a shell; `--into` is validated with `git check-ref-format --branch`
  and must differ from `<branch>`. The merge message is an argv value, not interpolated into a shell.
- **Out-of-worktree effects:** the only writes outside the agent worktree are the target ref (and the
  target checkout's files when it is checked out and clean) — the purpose of the command, requested by
  the operator. `on_remove` is the consumer's existing hook, unchanged.
- **Dependencies / secrets:** none.

## Risks / Trade-offs

- [Target checkout dirty at the root blocks finish] → refuse with the path and the dirty files; the
  operator commits or stashes. Never merge into a dirty checkout.
- [CAS lost to a concurrent writer] → abort with exit 1, nothing changed; re-run.
- [Bare-session teardown depends on v0.18's GP-49 resolution] → if `supervisor-pane-removal` keeps the
  refusal, finish still integrates and removes the worktree but leaves the surface; tasks gate on it.

## Open Questions

- Should `finish` also accept agents of a *stopped* session (session file present, no terminal)? This
  design says yes (no surface to close); confirm.
- Should finish verify the branch is `verified` in the broker before integrating? Not here — finish is
  operator-invoked; verification gating belongs to the queue's `--verified` selection.
