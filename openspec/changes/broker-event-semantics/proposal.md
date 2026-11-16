## Why

The broker's events say things that did not happen, and stay silent about things that did. The
v0.19.0 and v0.20.0 dogfood runs and the peer-4-poker run found:

- **A rebase looks like new work** (DF-35, DF-50, DF-56, DV-13). The post-commit hook runs for
  every commit a rebase replays, so each one is published as `agent.artifact committed` and
  draws a `verify-now`: 24 and 27 false events for two orchestrator rebases. One 27-commit rebase
  stopped on the worktree's `index.lock`, which the watcher's two-second `git status` and the
  hook's `git diff HEAD~1` both take.
- **The orchestrator's commits are published as the worker's** (DF-35, DV-08, DV-10). The hook
  reads the worktree's marker, not who committed.
- **There is no "change complete" signal** (DF-21). Workers commit per section, the nudge fires
  per commit, and the skill calls every commit "the canonical done signal".
- **There is no "frozen for verification" signal** (DF-37, DF-40). Workers amend the branch while
  the orchestrator's gates run on it. Workers also keep committing after their terminal commit,
  and `tell` refuses a mid-response worker, so scope creep cannot be stopped (PK-12, PK-16).
- **A conflict acknowledgement does not stick** (DV-09, DV-10). It stops the question but not the
  repeated in-flight warning, and the question returns after the overlap's record ages out.
- **`git paw remove` raises false conflicts** (DV-15). The removed agent's watcher publishes the
  teardown as edits, and its last file set stays in the conflict tracker: eight false questions.
- **A busy wave reads `stalled`** (DV-17). A worker visibly mid-turn for 30 minutes, editing
  nothing, is indistinguishable from a dead one.
- **Merge truth is not checked** (PK-13, PK-14). Merging a tip newer than the verified one goes
  unnoticed, and the drive-loop summary calls a merged branch unmerged once the worker commits
  after the merge.
- **Feedback can go unread** (external feedback #33, v0.20.0 run). A rebase instruction sent
  with `feedback-gate` was never read, the worker committed without rebasing, and nothing told
  the orchestrator.

## What Changes

- **Commit events tell the truth.** The post-commit hook publishes nothing while a rebase
  replays commits. A finished rebase publishes one `rewritten` artifact, which draws no
  `verify-now`. The hook lists exactly the commit's files and its SHA, without touching the
  index. A commit made by the orchestrator (git-paw marks its environment) is not published as
  the worker's.
- **The watcher stays out of the way.** Its git calls never take the index lock, and it
  publishes nothing for a worktree with a rebase, merge or cherry-pick in progress.
- **`remove` withdraws the agent first.** Before the worktree is torn down, the broker stops
  watching it and drops the agent from the conflict tracker.
- **Change complete and frozen are explicit.** Workers publish `agent.artifact done` once after
  their final commit. That artifact draws a `verify-now` with `trigger: "complete"` and the tip
  SHA. A new `orchestrator.freeze` verdict marks a branch under verification. The worktree's
  pre-commit guard then refuses commits until `agent.verified` or `agent.feedback` lifts the
  freeze.
- **Acknowledged conflicts stay quiet.** An acknowledgement also stops the in-flight warnings,
  and it lasts until the shape changes or an agent leaves.
- **Merge truth.** `agent.verified` and `agent.advanced-main` carry the tips they mean. The
  broker raises a question when the merged tip is not the verified one. The drive-loop summary
  counts a branch merged when its recorded merged tip is on the default branch, and reports
  commits made after the merge separately.
- **Delivery is visible.** `POST /publish` returns the message's sequence number. The broker
  records how far each agent has read its inbox, and `/status` shows unread messages.
  `sweep.sh delivery` answers "was it read?". A commit made while orchestrator feedback is unread
  raises a question.
- **A busy agent is not a stall.** The broker host samples each agent's pane. A mid-response pane
  whose content is changing is *activity*: it keeps `stalled` false and is listed, but it is not
  progress.

Out of scope, proposed as their own changes: interrupting a mid-response worker from `tell`
(PK-16's second half), and worker sender authentication.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `git-hook-injection`: ADDED replay suppression, one rewrite event per rebase, exact and
  lock-free commit file lists, orchestrator-commit attribution, and the frozen-branch guard.
- `broker-watcher-and-state`: ADDED a watcher that never takes the index lock, silence during
  history operations, and a merge check against the last verified tip.
- `broker-protocol`: ADDED the commit SHA on artifacts, the verified tip on `agent.verified`,
  and the `orchestrator.freeze` variant.
- `broker-runtime`: ADDED the sequence number in the publish response, per-agent read cursors,
  escalation of a commit made while feedback is unread, watch-target withdrawal, and pane
  activity in the wave status.
- `broker-conflict-detection`: ADDED acknowledgements that silence repeat warnings and outlive
  the overlap's absence, and withdrawal of a departed agent.
- `git-add-remove-branch`: ADDED withdrawal from the broker before teardown.
- `supervisor-skill-discipline`: ADDED the trigger and tip on `verify-now`, a nudge on change
  complete and none on a rewrite, and the skill's freeze-then-gate rule.
- `broker-agent-helper`: ADDED tips on `verified` and `advanced-main`, `sweep.sh freeze`,
  sequence numbers in the verdict verbs' output, and `sweep.sh delivery`.
- `skill-agent`: MODIFIED the stand-by protocol (an explicit `done` after the final commit);
  ADDED the frozen-branch rule and an inbox poll before each commit.
- `supervisor-unattended-operation`: MODIFIED the exit summary's merged rule.

## Impact

- **Code:**
  - `assets/hooks/post-commit.sh`, `assets/hooks/pre-commit.sh`, a new
    `assets/hooks/post-rewrite.sh`, and `src/agents.rs` (the hook bodies and their managed-block
    installer, which gains the third hook);
  - `src/broker/watcher.rs` (`--no-optional-locks`, in-progress detection);
  - `src/broker/delivery.rs` (nudge triggers, read cursors, unread-commit escalation, freeze
    state, the merge check);
  - `src/broker/messages.rs` (optional `sha` fields, the `Freeze` variant, `VerifyNow` fields);
  - `src/broker/server.rs` (publish response body, `DELETE /watch`, `/status` fields);
  - `src/broker/conflict.rs` (acknowledgement scope and lifetime, agent withdrawal);
  - `src/broker/mod.rs` (wave activity);
  - `src/commands/broker_host.rs` (pane sampling);
  - `src/commands/remove.rs` (withdraw before `detach_worktree`);
  - the orchestrator's launch (`GIT_PAW_ACTOR`);
  - `src/supervisor/drive.rs` (merge-state rule);
  - `assets/agent-skills/orchestrator/scripts/sweep.sh` (`freeze`, `delivery`, tips, sequence
    numbers);
  - `assets/agent-skills/coordination/SKILL.md` and `assets/agent-skills/orchestrator/SKILL.md`.
- **Wire:** additive only. Optional fields: `sha` on `agent.artifact` and `agent.verified`,
  `merged_sha` on `agent.advanced-main`, `trigger` and `sha` on the verify nudge. One new type,
  `orchestrator.freeze`. A `{"seq": N}` body on the `202` response to `POST /publish`. A new
  route, `DELETE /watch`. Old publishers and readers keep working.
- **Tests:**
  - hook e2e: a real rebase of N commits publishes one event; the commit file list; orchestrator
    attribution; the frozen guard;
  - a watcher test with a held `index.lock` and a rebase in progress;
  - conflict-tracker unit tests (acknowledgement lifetime, withdrawal);
  - a remove e2e with an overlapping peer;
  - delivery tests (read cursor, unread escalation);
  - a wave-activity test with a fake pane backend;
  - drive-summary merge-state tests.
- **Docs:** the coordination chapter (the new type and fields, the read cursor), the user guide's
  conflict-detection chapter (acknowledgement lifetime), and the orchestrator skill.
- **Dependencies:**
  - **`supervisor-vocabulary-split` (this release).** New text uses the orchestrator vocabulary:
    the `orchestrator` wire identity and inbox, and `orchestrator.verify-now`. Land the split
    first, or map the names when rebasing this change: `"supervisor"` and
    `supervisor.verify-now` are the pre-split spellings, and the detector's sender check follows
    the root identity.
  - **`wire-format-dtos` (this release)** moves the message types into `broker::wire`. The new
    fields and the variant go there, and count as an additive protocol-version change.
  - **`spec-kit-wave-planning`** also adds a conflict-detector exemption (PK-08). The two do not
    overlap.
  - **`finish-command`** should refuse an integration whose tip is not the last verified one.
    That is noted here, not specced.
