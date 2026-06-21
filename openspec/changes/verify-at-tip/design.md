## Context

The supervisor agent verifies each coding agent's branch through five gates
(testing, regression, spec audit, doc audit, security audit) before publishing
`agent.verified`. The bundled `assets/agent-skills/supervisor.md` teaches the
supervisor to do this in an **isolated verify worktree** under
`.git-paw/tmp/verify-<branch>/`. The current recipe is:

```sh
VERIFY=".git-paw/tmp/verify-${BRANCH//\//-}"
git worktree remove --force "$VERIFY" 2>/dev/null; git worktree prune
git worktree add --detach "$VERIFY" "$SHA"
# ... run the gate commands with -C "$VERIFY" ...
git worktree remove --force "$VERIFY"
```

`$SHA` is captured from the `agent.artifact { status: "committed" }` event (or
the `supervisor.verify-now` nudge) that *triggered* the verification. That event
fires on the agent's **first** commit. Agents then keep committing — tests, then
docs — so the branch tip advances while `$SHA` does not. When the doc-audit gate
runs minutes later against the detached `$SHA` worktree, it sees a tree without
the doc commits and reports `[doc audit] docs/src/...: missing` even though the
docs are committed at the tip. This is a false negative; it nearly drove a
wasted `agent.feedback` fix-request in a v0.9.0 dogfood run.

A second, related defect: gates that ask "what did this change add/remove"
(spec audit, security-audit diff scan) need a diff baseline. Diffing against a
stale integration tip (a `main` that has since moved, or a branch that is behind)
yields spurious mass deletions/additions. The correct baseline is the
**merge-base** of the branch tip and the integration target.

## Goals / Non-Goals

**Goals:**
- The change-level five-gate verification runs against the agent branch's
  current **tip**, re-resolved at verification time via `git rev-parse`.
- If commits land after the verify worktree exists, the supervisor re-resolves
  and re-checks-out the tip before re-running gates — no gate ever reports
  against a snapshot older than the current tip.
- The "what changed" diff uses the merge-base with the integration target.
- The fix is purely in the bundled supervisor skill content (and its assertion
  tests); no `src/` behaviour changes.

**Non-Goals:**
- Automating re-verification on every new commit inside the binary. The
  per-event verification cadence (verify on each `committed` event) is already
  specified in `per-commit-verification` and is unchanged here — this change is
  about *which tree* a given gate run reads, not *when* it is triggered.
- Changing the broker `supervisor.verify-now` nudge wire format.
- Changing the regression baseline definition (still "test outcome on `main`
  before any agent launched").

## Decisions

**Decision: Re-resolve the tip with `git rev-parse <branch>` at gate time,
not the captured event SHA.** The verify recipe changes from `add --detach
"$VERIFY" "$SHA"` to resolving `TIP=$(git rev-parse "$BRANCH")` immediately
before `git worktree add --detach "$VERIFY" "$TIP"`. Alternatives considered:
(a) check out the branch ref directly (not detached) — rejected, a non-detached
worktree on the branch ref blocks the agent's own worktree from holding that
branch and risks the supervisor moving the ref; detached-at-tip keeps the agent
worktree authoritative. (b) Keep `$SHA` but re-run only the failing gate against
the tip — rejected as more complex than just always pinning to the freshly
resolved tip for the whole sweep.

**Decision: Re-resolve before re-running.** When the supervisor re-runs the
gates (e.g. because a `verify-now` nudge arrived for a later commit, or it is
re-verifying after sending feedback), it SHALL drop and recreate the worktree at
the newly re-resolved tip. The recipe already does `git worktree remove --force
... ; git worktree prune` at the top, so re-resolution composes cleanly: each
sweep re-resolves and re-checks-out.

**Decision: Diff against the merge-base.** "What did this change add" is computed
as `git -C "$VERIFY" diff $(git merge-base <integration-target> <tip>)..<tip>`
(or `git diff <merge-base>...HEAD` triple-dot semantics in the detached
worktree). This isolates the branch's own contribution regardless of where the
integration tip has moved. Alternative: diff against the integration tip
directly — rejected because a behind-tip or rebased branch then shows the
integration tip's commits as spurious deletions.

**Decision: Spec the behaviour in two places.** The five-gate requirement lives
in `agent-skills` (which tree the gates read) and the worktree recipe lives in
`supervisor-skill-discipline` (the `.git-paw/tmp/verify-<branch>/` scratch-dir
requirement, which currently dictates `--detach ... "$SHA"`). Both have a
genuine requirement change, so both get a MODIFIED delta. `per-commit-verification`
is NOT modified — it governs verification *cadence/triggering* and the broker
nudge, none of which change.

## Risks / Trade-offs

- [Tip advances mid-gate, between gate 1 and gate 4] → The supervisor re-resolves
  once per sweep, so a single sweep is internally consistent at the tip it
  resolved. A commit landing mid-sweep is caught by the next `verify-now`-driven
  sweep, which re-resolves again. Acceptable: the failure mode we fix (stale
  *first*-commit snapshot) is eliminated; the residual race (tip moves during one
  sweep) self-corrects on the next event and never produces a *false missing*.
- [Detached worktree at a tip that the agent later force-pushes/rebases] → The
  re-resolve-before-rerun rule means the next sweep picks up the new tip; the
  previous detached checkout is torn down by `git worktree remove --force`.
- [Skill-only change has no compiler enforcement] → Mitigated by skill-content
  assertion tests that grep `supervisor.md` for the tip-resolution and
  merge-base wording, mapping each new scenario to a test.

## Migration Plan

Edit `assets/agent-skills/supervisor.md`; no data migration. Existing sessions
re-render the skill on next boot. Backward compatible — a session that never
hits a multi-commit branch sees identical behaviour.
