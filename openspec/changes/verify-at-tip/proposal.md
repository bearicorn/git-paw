## Why

The supervisor's change-level verification can run its five gates against a
**stale snapshot** of an agent's branch. The bundled `supervisor.md` recipe
creates the isolated verify worktree with `git worktree add --detach "$VERIFY"
"$SHA"`, where `$SHA` is the commit carried by the *first* `committed` event it
consumed. Agents commit incrementally — implementation first, then tests, then
docs — so by the time the doc-audit gate runs, the docs may already be
committed at the branch **tip** while the verify worktree is still pinned to the
first commit. In a v0.9.0 dogfood run this produced a false negative: the doc
audit flagged docs as MISSING when they were present at the tip, nearly
triggering a wasted fix-request to a correct agent. The same staleness corrupts
regression and security gates, which inspect "the diff" relative to whatever the
worktree happens to point at.

## What Changes

- The change-level (five-gate) verification SHALL re-resolve the agent's branch
  **tip** (`git rev-parse <branch>`) at the moment it runs, and check out that
  tip in the isolated verify worktree — instead of pinning to a captured
  `committed`-event SHA. If new commits land after the worktree is established,
  the supervisor SHALL re-resolve and re-check-out the tip before re-running the
  gates rather than reporting against the old snapshot.
- Docs/tests that are present at the branch tip SHALL NOT be reported as missing
  by the doc-audit or spec-audit gates.
- The "what did this change add" determination (spec audit, security-audit diff
  review) SHALL diff the branch tip against the **merge-base** with the
  integration target, not against a stale integration tip, so that rebased or
  behind-tip branches do not show spurious mass deletions/additions.
- The bundled `supervisor.md` recipe and the "Isolated verify worktrees"
  guidance SHALL be updated to teach tip re-resolution and merge-base diffing.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `agent-skills`: the "Supervisor skill — five verification gates" requirement
  is modified so the gates run against the freshly re-resolved branch tip (not a
  captured commit SHA), and the "what changed" diff is taken against the
  merge-base with the integration target.
- `supervisor-skill-discipline`: the "Isolated verification worktrees use a
  repo-local gitignored scratch dir" requirement is modified so the recipe
  checks out the re-resolved branch tip and re-resolves it before re-running
  gates, rather than detaching at a pinned SHA.

## Impact

- `assets/agent-skills/supervisor.md` — the "Isolated verify worktrees" recipe
  (`git worktree add --detach "$VERIFY" "$SHA"`) and the five-gate prose.
- Skill-content tests under `tests/` that assert supervisor.md verification
  guidance (must assert the tip-resolution and merge-base wording).
- No code path in `src/` changes — verification is supervisor-skill behaviour,
  not binary logic. No config schema change. Fully backward compatible.
