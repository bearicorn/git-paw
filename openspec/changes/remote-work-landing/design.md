## Context

The earlier changes of this release give git-paw:

- a remote surface and a follow in the broker host (`remote-surface-adapter`);
- per-worker backends in one session (`mixed-backend-sessions`);
- a Jules adapter with a ledger and cool-down (`jules-remote-vendor`).

What is missing is the boundary between the vendor's world and the local repository, in both
directions.

Fleet's design and reviewed plan settled most of that boundary for Fleet's runner; this change
carries over the fixes that apply to git-paw and leaves out what does not:

| Fleet decision / review fix | Here |
|---|---|
| Eligibility: GitHub remote, base is the remote tip, no local commits (design §6 step 1) | carried; checked with `git ls-remote` instead of the keyless GitHub API, so private repositories work with the user's own git credentials |
| `reuse_blocked` vs `ineligible_reason` (C1) | carried as "re-attach never re-gates a recorded session" |
| Phase-aware resume: mid-apply, mid-verify (I1) | carried as "complete an interrupted landing once" |
| Newest patch by `createTime`; cumulative assumed (I2) | ordering carried; cumulativeness **not** assumed (open question 3) |
| Patch policy: forbidden paths, symlinks, gitlinks, 5 MB, clean tree on failure (I11) | carried, with `.git-paw` in place of `.fleet` |
| Verify retry once, same session, patch after the message (I8, retry_after / saw_active) | carried as the one-continuation rule |
| Adopt a session by title after a crash (M11) | carried |
| Watching for 24 h after the run; pull-request marks; standing reply to questions; approve a plan once | **not** carried: git-paw follows only while its session lives, opens no pull request, escalates every question, approves nothing |
| Final review of the proof of concept, findings I-1 to I-6 | carried; see D9 |

## Goals / Non-Goals

**Goals:**
- No remote worker unless its vendor session starts from exactly what the local repository holds,
  without git-paw pushing anything.
- No vendor patch reaches the worktree unchecked; no failure leaves the worktree changed.
- Verify, gates and merge are untouched: a landed patch is just the worker's commit.
- One continuation per verification failure, never a second vendor session, no unbounded loop.

**Non-Goals:**
- Pull requests (v1.5.0), pushing, and closing or commenting on vendor pull requests.
- Remote workers without a spec (bare branches); see Open Questions.
- Repositories not on GitHub; vendors other than Jules.

## Decisions

### D1 — The gate checks the remote side with `git ls-remote`

`git ls-remote origin refs/heads/<base>` reads the remote tip with the user's own git credentials,
so it works for private repositories (which Jules supports through its GitHub app) and needs no
GitHub token or API. It writes nothing. git-paw never pushes; when the tip differs, the refusal
tells the user to push. `JULES_API_KEY` is stripped from its environment.

*Alternative rejected:* the keyless GitHub REST API (Fleet). It cannot see private repositories, and
it is a second network endpoint to pin.

### D2 — A remote worker needs a spec

A vendor session needs its task at creation, and the gate must hold at that moment. A spec-driven
worker has its task prompt at launch; a bare branch receives its task later, from the orchestrator,
by which time the base may have moved. Requiring a spec keeps the gate's facts true at creation.

### D3 — Create last

The local session is saved first; vendor sessions are created after. A local failure therefore never
orphans a vendor session (which cannot be cancelled), and a vendor failure leaves a recoverable
local session: `git paw start` revives it and creates the missing vendor session through the same
gate.

### D4 — The patch is untrusted

The check is pure and runs before anything touches the worktree. It takes paths from git's own
parser (`--numstat -z`, both sides of renames and copies) and from the header lines (modes), and
refuses on either. The apply needs a clean worktree, so restoring "as it was" is a reset to HEAD plus
removing files the attempt created, never a destructive clean of someone's work. The commit runs the
repository's hooks with the key stripped from the environment.

### D5 — Patch choice holds whatever open question 3's answer is

If Jules's newest patch is cumulative from the session's base, then "based on the starting commit
and covers every earlier path" holds and the patch is landed. If it is incremental, the rule fails
safe: nothing is landed, and `remote-patch-ambiguous` hands the session to the orchestrator with its
URL. Task 1.3 of `jules-remote-vendor` records the answer; if patches are incremental, this
requirement must be amended before release to apply them in order.

Only a completed session's patch is ever landed. A session that fails may have created a patch
before failing; that partial patch is escalated with the failure (the follow's `remote-failed`,
which then says the patch was not landed) and is never applied or committed (decided 2026-10-28).

### D6 — Replacing a landed patch

After a continuation, the newer patch replaces the earlier commit: the branch returns to the commit
the earlier patch was applied on, and only when its tip is still git-paw's landing commit (the branch
was never pushed, and the earlier commit failed verification). Anything else (a refresh merged into
it, a person's commit) escalates `remote-branch-moved` rather than rewriting someone's history.

### D7 — One continuation, then escalate (v1.3.0's rule)

The first `agent.feedback` after a landing is sent once, with the failing output's tail. That is the
remote analogue of headless's single continuation. The time cap restarts, because the vendor is
doing new work. Fleet's `saw_active` guard is carried: a completion that still precedes the
continuation is not a "no new patch" outcome. Every further failure escalates `remote-verify-failed`.
No second vendor session is ever created for a failed verification, so a worker's task count grows
only by explicit user action.

### D8 — Recovery re-attaches

The session file records the vendor session (`WorktreeEntry.remote`). Recovery, restart from pause
and revival re-attach with a fresh cap; a title recorded before creation lets a crashed launch adopt
its session instead of creating another. An interrupted landing is completed from the follow state
(phase "landing", patch key, commit) and is idempotent: a patch already committed is never applied
twice.

### D9 — Lessons from the proof of concept's final review

Fleet's final review of its Jules route (`remote-workers-final-review.md`) confirmed six defects.
Each becomes a requirement here or in `jules-remote-vendor`:

- **I-1, reuse is never gated.** Fleet's daily budget blocked a resume and a verify retry from
  reusing their own session, which was then abandoned unwatched. Here the gate, the limits and the
  cool-down apply only where a session is to be created (*Picking up a worker's own vendor session
  is never gated as a new one*). The gate already ran when the session was created, and re-gating
  would refuse a branch that now holds a landed commit. Resuming without the key keeps the worker
  remote and its session recorded, ends the follow with `remote-auth`, and the next recovery with
  the key re-attaches (*A re-attach without the vendor key keeps the worker's session recorded*).
  A broker host keeps the environment it started with, so the key takes effect on a recovery or a
  restart from pause, not on reviving a live session. Moving the task to a local worker is
  impossible (a worker's backend is fixed, `mixed-backend-sessions`), and `remove` refuses a remote
  worker whose session may still be running unless `--force` (`remote-surface-adapter`).
- **I-2, a stop during verify.** The killed verification looked like a failure, and the resume then
  saw the retry patch as "not new". Here an interrupted gate run publishes no verdict, the landing
  stays "verifying", and recovery announces it again, never as `remote-no-patch` (*A verification
  interrupted by a stop is run again on recovery*).
- **I-3, a failed replacement.** Fleet reset to the base before applying the newer patch and lost
  the earlier one when the newer one failed. Here every check runs before the branch moves, the
  earlier landing commit is recorded, and any later failure puts it back (*A failed replacement
  puts the earlier landing back*).
- **I-4, a dirty worktree.** Fleet admitted a task with uncommitted local work and then cleaned the
  tree when the apply failed. Here the gate refuses a worktree holding uncommitted work, the apply
  refuses a dirty worktree, and no command discards a change or removes a file git-paw did not
  create.
- **I-5, a key with a line break.** It is refused before any request, in `jules-remote-vendor` and in
  doctor's remote check (Fleet's leak was through its check command). With `curl --config -`, a line
  break in the key would also start a new configuration line.
- **I-6, case.** `.git`, `.git-paw` and `.gitmodules` are compared without regard to case, because
  macOS's default filesystem is case-insensitive.

## Risks / Trade-offs

Threat analysis (`security-and-safety-review`).

- **L1 — A malicious or broken patch writes outside the work tree or into git's control files.** →
  Path, mode and size checks before any write; git's own parser; refusal escalated, never retried.
- **L2 — A patch that runs code at verification.** → Verification runs the vendor's code locally with
  the user's privileges, exactly as it runs a local worker's code. Documented in the posture chapter;
  gate commands keep whatever sandbox the user configured.
- **L3 — Destroying a person's work.** → Clean-worktree precondition at the gate and at the apply;
  restore to HEAD only, removing only files the attempt created; replacement only when the tip is
  git-paw's own landing commit, and a failed replacement puts that commit back (D9).
- **L4 — Spend.** → One continuation per landing; no second session for a failed verify; the ledger
  and cool-down from `jules-remote-vendor`.
- **L5 — Data egress.** → The task prompt, relayed answers and the failing output tail (with the key
  masked) go to the vendor; documented. The boot block and local paths are never sent.
- **L6 — A push by accident.** → No code path in this release runs `git push`; the gate's remote
  read is `ls-remote`. A source audit asserts no `push` invocation in the remote modules.
- **L7 — Hook bypass.** → The landing commit runs the repository's hooks (branch guard included);
  a refusal is a failed apply.
- **L8 — A vendor session left running unobserved.** → Re-attach is never gated, a missing key keeps
  the record, and `remove` needs `--force` while the session may still run (D9, I-1). `purge` and
  `remove --force` still drop the record by a person's choice; they print the URL, and the ledger
  keeps counting the session.
- **L9 — Training on the work.** Per the Jules privacy notice (read 2026-10-28,
  `jules-remote-vendor`), sessions on public repositories may train Google's models, private ones
  are not used for training, and reviewers may read samples. → Documented in the posture and
  remote-workers chapters; no gate, because the user chooses the repository.

## Migration Plan

1. Pure core: gate decision, patch check, patch choice, continuation decision; table tests.
2. Landing in the follow (apply, commit, announce, escalations) against the fake vendor and a real
   temporary repository.
3. Launch ordering, recovery and adoption; the `remote` value admitted by `--worker-backend`.
4. Doctor, posture, chapter, manifest entries.
5. Before release: fold in `jules-remote-vendor`'s live-run answers (questions 3 and 4 in
   particular).

Rollback: revert. Sessions with remote workers created meanwhile need this release to stop or purge.

## Open Questions

- **Cumulative patches** (proof-of-concept question 3). Landing is conservative until the live run
  answers it (D5).
- **No patch without `AUTO_CREATE_PR`** (question 4). If confirmed, this change cannot land work, and
  the user decides between pulling pull-request support forward and fetching the vendor's branch.
- **Remote workers without a spec.** *Recommendation:* later, if Fleet's git-paw mode needs bare
  tasks; it would need a task-file flag and the gate re-checked at creation.
- **Landing on the vendor's branch instead of a patch.** Not considered while sessions open no pull
  request (no vendor branch exists).
