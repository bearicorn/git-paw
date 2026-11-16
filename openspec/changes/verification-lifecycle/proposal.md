## Why

The orchestrator cannot tell when a change is complete, cannot stop a worker while it verifies,
and cannot check that it merged what it verified. The v0.19.0 dogfood and the peer-4-poker run
found:

- **There is no "change complete" signal** (DF-21). Workers commit per section, the nudge fires
  per commit, and the skill calls every commit "the canonical done signal".
- **There is no "frozen for verification" signal** (DF-37, DF-40). Workers amend the branch while
  the orchestrator's gates run on it. Workers also keep committing after their terminal commit,
  and `tell` refuses a mid-response worker, so scope creep cannot be stopped (PK-12, PK-16).
- **Merge truth is not checked** (PK-13, PK-14). Merging a tip newer than the verified one goes
  unnoticed, and the drive-loop summary calls a merged branch unmerged once the worker commits
  after the merge.

This is the second of three changes split from `broker-event-semantics` (2026-11-16). It builds
on `commit-event-truth`; `broker-delivery-and-activity` is the third.

## What Changes

- **Change complete and frozen are explicit.** Workers publish `agent.artifact done` once after
  their final commit. That artifact draws a `verify-now` with `trigger: "complete"` and the tip
  SHA. A new `orchestrator.freeze` verdict marks a branch under verification. The worktree's
  pre-commit guard then refuses commits until `agent.verified` or `agent.feedback` lifts the
  freeze.
- **Merge truth.** `agent.verified` and `agent.advanced-main` carry the tips they mean. The
  broker raises a question when the merged tip is not the verified one. The drive-loop summary
  counts a branch merged when its recorded merged tip is on the default branch, and reports
  commits made after the merge separately.

Out of scope, proposed as its own change: interrupting a mid-response worker from `tell`
(PK-16's second half).

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `git-hook-injection`: ADDED the frozen-branch guard.
- `broker-watcher-and-state`: ADDED a merge check against the last verified tip.
- `broker-protocol`: ADDED the verified tip on `agent.verified`, and the `orchestrator.freeze`
  variant.
- `supervisor-skill-discipline`: ADDED the trigger and tip on `verify-now`, a nudge on change
  complete and none on a rewrite, and the skill's freeze-then-gate rule.
- `broker-agent-helper`: ADDED tips on `verified`, `advanced-main` and `broker.sh artifact`, and
  `sweep.sh freeze`.
- `skill-agent`: MODIFIED the stand-by protocol (an explicit `done` after the final commit);
  ADDED the frozen-branch rule.
- `supervisor-unattended-operation`: MODIFIED the exit summary's merged rule.

## Impact

- **Code:**
  - `assets/hooks/pre-commit.sh` (the frozen guard);
  - `src/broker/delivery.rs` (nudge triggers, freeze state, the merge check);
  - `src/broker/messages.rs` (optional `sha` on verified, `merged_sha` on advanced-main, the
    `Freeze` variant, `VerifyNow` fields);
  - `src/supervisor/drive.rs` (merge-state rule);
  - `assets/agent-skills/orchestrator/scripts/sweep.sh` (`freeze`, tips) and
    `assets/agent-skills/coordination/scripts/broker.sh` (`artifact` tip);
  - `assets/agent-skills/coordination/SKILL.md` and `assets/agent-skills/orchestrator/SKILL.md`.
- **Wire:** additive only. Optional fields: `sha` on `agent.verified`, `merged_sha` on
  `agent.advanced-main`, `trigger` and `sha` on the verify nudge. One new type,
  `orchestrator.freeze`. Old publishers and readers keep working.
- **Tests:** the hook e2e frozen guard, wire round-trips, nudge triggers and `[merge-check]`
  cases, `sweep.sh` freeze and tips, skill content tests, and drive-summary merge-state tests.
- **Docs:** the coordination chapter (the new type and fields) and the orchestrator skill.
- **Dependencies:**
  - **`commit-event-truth`**: the artifact `sha` and the `rewritten` status come from there.
  - **`supervisor-vocabulary-split`** and **`wire-format-dtos`** (this release), as for
    `commit-event-truth`.
  - **`spec-hygiene`** also MODIFIES "Exit summary" in `supervisor-unattended-operation`. Whichever archives second rebases its
    block.
  - **`finish-command`** should refuse an integration whose tip is not the last verified one.
    That is noted here, not specced.
