## Context

Rebasing rewrites history. Doing it to a branch that is **checked out in a live
worktree, being edited by a running agent** is the dangerous case, and it is exactly the
case here. Three existing facts shape the design:

1. `rebase_branch_onto_default_with` (`src/git.rs:276`) already runs the rebase inside
   the worktree where the branch is checked out, and on failure runs `git rebase --abort`
   and leaves the branch at its pre-rebase HEAD (`:269-271`). The mechanics are solved;
   what is missing is *when to call it*.
2. The broker watcher already maintains each agent's `modified_files`, and the drive
   loop already detects a mid-response pane (`MID_RESPONSE_MARKERS`,
   `src/supervisor/drive.rs:1786`) and already serialises pane access through
   `PaneClaim` (`src/supervisor/claim.rs:62`).
3. The worker-side rule "do NOT auto-rebase" (`skill-agent`, *When main advances*) exists
   because a worker cannot know whether its own uncommitted edits will survive. That
   reasoning does not transfer to the supervisor, which can observe cleanliness and
   idleness from outside the agent.

## Goals / Non-Goals

**Goals:**

- Keep long-lived worker branches close to the base without asking the agent to
  interrupt itself.
- Make the unsafe cases *unreachable* rather than merely unlikely — every precondition
  is a gate, and failing any gate is a no-op, not a degraded attempt.
- Reuse the existing cleanliness, idleness, and claim signals so the supervisor cannot
  hold a different view of the worktree than the detector does.

**Non-Goals:**

- Rebasing a **dirty** worktree, by stashing or committing on the agent's behalf. Never.
  Touching an agent's uncommitted work is how work gets silently destroyed, and the
  worker-side skill explicitly warns about exactly this.
- Resolving rebase conflicts automatically.
- Changing the worker-side *When main advances* discipline, or reusing
  `agent.advanced-main` for the notification.
- Refreshing branches that are not participating in the current wave.
- Making this the default. A history rewrite under a live agent is opt-in, permanently.

## Decisions

**D1 — Gate on the watcher's `modified_files`, not a fresh `git status`.** A second
probe could disagree with the detector (the watcher polls; a direct probe races it), and
a supervisor that believes a worktree is clean while the detector believes otherwise is
the precondition for data loss. *Considered and rejected:* a dedicated `git status`
call at rebase time — marginally fresher, but it introduces a second source of truth for
the one fact the whole safety argument rests on.

**D2 — Predict the rebase before performing it.** Run a conflict prediction and skip the
branch entirely on a predicted conflict, rather than starting a rebase and relying on
`--abort`. Abort-based recovery is sound but leaves the worktree briefly in a rebase
state that a live agent can observe and react to (or worse, run git commands during).
Prediction keeps the common failure path from ever touching the worktree. `--abort`
recovery remains as the backstop for a prediction that proves wrong.

**D3 — Notify via tagged `agent.feedback`, not a new `BrokerMessage` variant.** A new
variant ripples across `messages.rs`, `dashboard/broker_log.rs`, `learnings.rs` and
`delivery.rs` (AGENTS.md enum-ripple checklist) — disproportionate for a notification.
The conflict detector already establishes the bracket-tag precedent. *Considered and
rejected:* reusing `agent.advanced-main` — it means "the base moved" and the worker-side
discipline keys off that meaning; overloading it with "your own HEAD was rewritten" would
corrupt the very rule this change depends on.

**D4 — Refresh after a merge, not on a timer.** A merge is the event that makes a branch
stale, so it is the moment refresh is worth its risk. A timer would rebase branches that
have not drifted and would fire at arbitrary points in an agent's turn.

**D5 — Idleness is necessary but not sufficient; cleanliness is the load-bearing gate.**
An idle-but-dirty worktree must be skipped. Idleness alone only means the agent is not
mid-response — it says nothing about uncommitted edits sitting in the tree. The gates are
a conjunction precisely so that neither is mistaken for the other.

**D6 — A skipped branch is recorded, not retried in a loop.** If gates fail, the branch
stays stale and the next merge re-evaluates. Retrying within one merge event would just
poll an agent that is busy or dirty for a reason.

## Risks / Trade-offs

- **The agent's context goes stale.** After a rebase the agent may hold file contents and
  SHAs from before the rewrite; a subsequent `git` operation can surprise it. → D3's
  notification tells it explicitly that HEAD moved. Residual risk is real and is the main
  reason the feature is opt-in.
- **A race between the cleanliness check and the rebase.** The agent could begin editing
  in the window between gate 1 and the rebase. → The pane claim (gate 3) narrows the
  window, and the agent being non-mid-response (gate 2) means it is not actively running
  tools. The window cannot be closed entirely; this is an accepted, disclosed risk and a
  further argument for default-off.
- **Predicted-clean rebases that still conflict.** → `--abort` restores the pre-rebase
  HEAD; the branch is left exactly as it was and the event is recorded.
- **A rebase invalidates a verification that already passed.** A branch verified at one
  base and rebased onto another has not been verified at its new base. → Refresh must not
  be applied to a branch that has already passed the five-gate verification and is
  awaiting merge; those branches are the supervisor's to merge, not to rewrite.
- **Opt-in means most users never get the benefit.** Accepted deliberately: the failure
  mode of an unsafe automatic rebase (silently destroyed agent work) is far worse than
  the failure mode of a stale branch (a manual rebase at merge time).

## Resolved Questions

**Refresh breadth: all live worker branches, not intent-overlap-scoped.** The
post-merge hook lives in the unattended drive loop (`src/supervisor/drive.rs`),
which already enumerates every live `AgentPane` each sweep but has no access to
the broker-internal intent tracker (`src/broker/conflict.rs`'s `ConflictTracker`
runs in the broker/dashboard process, reached only over HTTP with no query for
"declared file regions by agent"). Scoping refresh to intent-overlap would mean
either plumbing a new HTTP endpoint just for this feature or duplicating intent
tracking outside the broker — real cost for a narrowing that only reduces *how
much* history gets rewritten, not *whether* an unsafe rewrite can happen (the
five preconditions already make that unreachable). All five gates apply
per-branch regardless of breadth, so evaluating every live branch costs one
extra gate-check per idle branch and skips (rather than mis-refreshes) any
branch that is dirty, mid-response, unclaimed, conflicting, or already current.
Decided: **every live worker branch is evaluated on each merge**; each is still
independently gated, so a branch with no reason to refresh is simply a no-op,
not a skipped opportunity.

**Repeated skips do not escalate.** Confirmed silent, per task 4.7 and the
`git-operations`/`supervisor-branch-refresh` spec's "Repeated skips do not
escalate" scenario: a busy or dirty agent across several merges is normal
operation, and escalating it would recreate the noisy-detector problem that
made GP-18 invisible in the first place. The skip is recorded (for the
dashboard/broker log) but never raised as an `agent.question`.

**1.2 — no collision with the verified-branch exclusion.** The verified
exclusion (gate 5 of the "All refresh preconditions are a conjunction"
requirement) filters by the branch's own status (`"verified"`, meaning it
passed the five-gate supervisor verification and is awaiting merge), which is
orthogonal to breadth: every live branch is *considered*, and the verified
ones are then excluded by their own status. Because the exclusion is a status
predicate independent of which branches were considered, "evaluate all
branches" and "never refresh a verified-awaiting-merge branch" cannot select
the same branch for rewrite — the two rules compose rather than compete.
