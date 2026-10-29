## Context

`finish-command` integrates one branch under a fast-forward-only (or merge-commit) policy and refuses
with exit 3 when the target diverged. In a wave, integrating branch 1 advances the target, so branches
2..N all become non-fast-forwardable. Today's procedure (bundled skill, "Merge orchestration") skips
them and sends `agent.feedback` asking each agent to rebase, then waits — once per branch.

Existing mechanics to reuse: `git::predict_rebase_conflict_with` (`merge-tree --write-tree`),
`git::rebase_branch_onto_default_with` (rebase inside the worktree where the branch is checked out,
`rebase --abort` on failure), the `supervisor-branch-refresh` preconditions (clean tree, surface not
mid-response, claim held), `supervisor::claim` (claim files), `WorkerPhase::Verified`
(`drive.rs`), `[supervisor] test_command`.

## Goals / Non-Goals

**Goals:** one orchestrator-callable command that drains a batch of verified branches, rebasing only
when a rebase provably cannot change the branch's own patch, re-testing the result, and returning the
rest to their agents in one batch.

**Non-Goals:** merge ordering (topological order from `agent.blocked` stays orchestrator judgment and
is expressed by argument order); resolving conflicts; integrating from the drive loop automatically;
remote/PR merge queues.

## Decisions

### D1 — The queue is `git paw finish` with several branches

`git paw finish <b1> <b2> … [--rebase] [--into t]` or `git paw finish --verified [--rebase]`. Each
branch goes through the same `finish-command` pipeline (preconditions → decide → apply → teardown);
the queue only adds the per-branch rebase step and the summary. Alternative — a separate
`git paw merge-queue` verb — rejected: two commands with the same preconditions, policy and teardown
would drift.

### D2 — "Provably disjoint" means the branch's own patch survives the rebase unchanged

A branch is rebased mechanically only when (a) the set of paths changed on the branch since the merge
base and the set changed on the target since the merge base do not intersect, and (b)
`git merge-tree --write-tree <target> <branch>` reports no conflict. (a) guarantees the replayed commits
apply to files the target never touched, so the branch's diff is byte-identical before and after; (b) is
the existing cheap guard and catches rename/mode edge cases. Alternative — merge-tree alone (GP-09's
wording) — rejected as the default: a clean textual merge of overlapping files can still change the
branch's patch. This is also why the queue does not contradict `supervisor-branch-refresh`'s "a
verified branch awaiting merge is never refreshed": that rule protects verification from a *changed*
patch; under (a) the patch does not change, and the only thing that can break — integration with the new
base — is re-tested (D3).

### D3 — Re-test before the target moves; restore on failure

After a mechanical rebase the queue runs the configured `[supervisor] test_command` inside the branch's
worktree. Only on success is the branch integrated (now a fast-forward). On rebase failure
(`rebase --abort`) or test failure, the queue resets the worktree to the recorded pre-rebase tip — safe
because the worktree was clean before the rebase — and skips the branch. Without a configured test
command `--rebase` does not rebase (skip reason: "no test command to re-verify a rebased branch").
Alternative — integrate rebased branches untested — rejected: that silently drops the regression gate the
skill's merge loop had (it tested after each merge and reverted `main`). Testing in the worktree *before*
moving the target removes the `git reset --hard` on the target branch the skill used to need.

### D4 — Live agents: idle + claimed, exactly as branch refresh

When the branch belongs to a live session agent, the rebase requires the agent's surface not to be
mid-response and the surface claim to be held for the rebase-and-test window (released after). A
busy surface is a skip ("agent busy"), not a wait. Worktrees with no live agent need no claim.

### D5 — Batched directives

Skipped branches with a live agent each get exactly one `agent.feedback` from the root identity, after
the whole pass: `{from: "supervisor", errors: ["rebase <branch> onto <target> @ <sha>: <reason>"]}`
where reason is the overlapping paths, the conflicting paths, "agent busy", "rebase failed" or the
failing test summary. Publishing after the pass means every directive names the final target tip, so
an agent never rebases onto a tip that moves again within the same run.

### D6 — Exit status and stop rule

0: every branch integrated. 3: at least one branch skipped by policy (nothing about the skipped
branches changed). 1: a hard error (target checkout dirty, CAS lost, git failure); the queue stops at
that branch, keeps everything already integrated, and still prints the summary for processed branches.

### D7 — The drive loop does not merge

The unattended loop already hands merge decisions to the orchestrator (`orchestrator-escalation-handoff`).
Letting the loop call the queue itself would make an irreversible integration a mechanical decision with
no judgment in the path. The hand-off text may name the command; the decision stays outside.

## Security and safety review

- **Irreversible git:** the only history rewrite is a rebase of a git-paw-managed agent branch inside
  its own clean worktree, gated by D2 + D3, and fully restorable (pre-rebase tip recorded; failure path
  resets to it). The target only ever fast-forwards (or gets a merge commit under policy). No force-push,
  no push at all.
- **Rogue agent:** `git paw finish` is root-only (`finish-command`), so an agent cannot trigger the
  queue from its worktree, and git-paw approves no permission prompt (v0.17.0), so an agent's
  `git paw finish` prompt reaches a human; doctor lists a durable `finish` grant. `--verified` trusts broker status; after v0.18 `agent.verified` can be published
  only with the root credential (`broker-verdict-authority`), so a worktree agent cannot self-verify into
  the queue. Before that lands, `--verified` is as trustworthy as the broker's publish gate — documented.
- **Test command execution:** runs the consumer's configured `test_command` in the agent worktree — the
  same command the verification gates already run; no new command source. It is run as a discrete argv
  through the existing gate-command runner, not re-parsed from broker input.
- **Directives:** carry branch names, paths and SHAs produced by git, not free text from agents.

## Risks / Trade-offs

- [Disjointness is strict; shared files like a lockfile or CHANGELOG make every branch "overlapping"]
  → those fall back to today's directive path; a looser `merge-tree`-only mode is an Open Question.
- [Re-running the test command per rebased branch is slow] → it replaces an agent round-trip per
  branch, which is slower; branches that fast-forward are not re-tested.
- [A test command that is flaky skips a good branch] → skip is non-destructive; the directive says
  "tests failed after rebase", and the orchestrator can re-run.

## Open Questions

- Offer `--rebase=textual` (merge-tree only) for consumers who accept the weaker guarantee? Default
  stays file-disjoint.
- Should `--verified` order by verification time instead of session order? Session order chosen as
  deterministic; ordering judgment is expressed by naming branches explicitly.
