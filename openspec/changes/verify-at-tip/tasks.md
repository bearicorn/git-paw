## 1. Update the isolated-verify-worktree recipe (supervisor-skill-discipline)

- [ ] 1.1 In `assets/agent-skills/supervisor.md`, change the isolated-verify-worktree recipe to resolve `TIP=$(git rev-parse "$BRANCH")` immediately before `git worktree add --detach`, and pass `"$TIP"` (not the captured `"$SHA"`) as the checkout target; keep `--detach` so the agent worktree stays the authoritative branch-ref holder.
- [ ] 1.2 Add recipe prose stating that each (re-)run of the gates re-resolves the tip and re-creates the worktree, so the worktree never holds a snapshot older than the current tip; keep the `.git-paw/tmp/verify-<branch>/` path and the `git worktree remove`/`prune` cleanup.

## 2. Update the five-gate verification prose (agent-skills)

- [ ] 2.1 Add a paragraph to the five-gate section instructing the supervisor to re-resolve the branch tip with `git rev-parse <branch>` at verification time and run all five gates against that tip — explicitly NOT against the triggering `committed`-event / `supervisor.verify-now` SHA.
- [ ] 2.2 Add the re-verification rule: before re-running gates for a branch (later `committed` event, `verify-now` nudge, or re-verify after feedback), re-resolve and re-check-out the tip.
- [ ] 2.3 State that doc-audit and spec-audit gates read surfaces from the re-resolved tip and MUST NOT report docs/tests present at the tip as MISSING; cite the v0.9.0 false-negative as the motivating example.
- [ ] 2.4 Update the spec-audit and security-audit gate prose to compute the change's contribution from the merge-base diff (`git merge-base <integration-target> <tip>`), with the rationale that a stale integration tip causes spurious mass deletions/additions on behind-tip or rebased branches.
- [ ] 2.5 Update the Testing gate prose to say it runs inside the verify worktree checked out at the re-resolved tip.

## 3. Tests

- [ ] 3.1 Extend the supervisor.md skill-content assertion test(s) to grep for the tip-resolution wording (`git rev-parse`, "re-resolve", run-against-tip) — maps the "Gates run against the re-resolved branch tip" scenario.
- [ ] 3.2 Add an assertion that the recipe passes the re-resolved tip to `git worktree add --detach` and does NOT pass a captured `$SHA` — maps "Verify worktree checks out the re-resolved branch tip".
- [ ] 3.3 Add an assertion for the merge-base diffing wording (`git merge-base`) — maps "Change contribution is diffed against the merge-base".
- [ ] 3.4 Add an assertion that the doc-audit/spec-audit prose forbids reporting tip-present surfaces as missing and cites the v0.9.0 example — maps "Doc/test surfaces present at the tip are not reported missing".
- [ ] 3.5 Add an assertion for the re-verification re-resolve rule — maps "Re-verification re-resolves the tip before re-running gates" and "Recipe re-resolves the tip on re-run".

## 4. Quality gates

- [ ] 4.1 Run the no-language-leak audit against the edited supervisor.md (stack-agnostic phrasing must still pass).
- [ ] 4.2 `just check` (fmt + clippy + tests) passes.
- [ ] 4.3 `openspec validate verify-at-tip --strict` passes.
