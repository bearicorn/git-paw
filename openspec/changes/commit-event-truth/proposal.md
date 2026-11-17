## Why

The broker's commit and edit events say things that did not happen. The v0.19.0 and v0.20.0
dogfood runs found:

- **A rebase looks like new work** (DF-35, DF-50, DF-56, DV-13). The post-commit hook runs for
  every commit a rebase replays, so each one is published as `agent.artifact committed` and
  draws a `verify-now`: 24 and 27 false events for two orchestrator rebases. One 27-commit rebase
  stopped on the worktree's `index.lock`, which the watcher's two-second `git status` and the
  hook's `git diff HEAD~1` both take.
- **The orchestrator's commits are published as the worker's** (DF-35, DV-08, DV-10). The hook
  reads the worktree's marker, not who committed.
- **Detector output reads as the orchestrator's rulings** (DV-08). The conflict detector sends
  its warnings and questions as `supervisor`, the root identity.
- **A conflict acknowledgement does not stick** (DV-09, DV-10). It stops the question but not the
  repeated in-flight warning, and the question returns after the overlap's record ages out.
- **`git paw remove` raises false conflicts** (DV-15). The removed agent's watcher publishes the
  teardown as edits, and its last file set stays in the conflict tracker: eight false questions.

This is the first of three changes split from `broker-event-semantics` (2026-11-16). The other
two are `verification-lifecycle` and `broker-delivery-and-activity`.

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
- **The detector speaks for itself.** Its messages carry the sender id `conflict-detector`. The
  acknowledgement filter checks the root identity, and the re-entrancy guard checks the detector
  id. The id is reserved: `/publish` refuses it and it never becomes a roster row.
- **Acknowledged conflicts stay quiet.** An acknowledgement also stops the in-flight warnings,
  and it lasts until the shape changes or an agent leaves.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `git-hook-injection`: ADDED replay suppression, one rewrite event per rebase, exact and
  lock-free commit file lists, and orchestrator-commit attribution.
- `broker-watcher-and-state`: ADDED a watcher that never takes the index lock, and silence during
  history operations.
- `broker-protocol`: ADDED the commit SHA on artifacts, and the reserved `conflict-detector`
  sender id.
- `broker-runtime`: ADDED watch-target withdrawal.
- `broker-conflict-detection`: ADDED acknowledgements that silence repeat warnings and outlive
  the overlap's absence, withdrawal of a departed agent, and a re-entrancy guard keyed on the
  detector id. REMOVED "Auto-emitted message conventions" and ADDED its replacement "Detector
  message conventions", whose sender is `conflict-detector`.
- `git-add-remove-branch`: ADDED withdrawal from the broker before teardown.

## Impact

- **Code:**
  - `assets/hooks/post-commit.sh`, a new `assets/hooks/post-rewrite.sh`, and `src/agents.rs`
    (the hook bodies and their managed-block installer, which gains the third hook);
  - `src/broker/watcher.rs` (`--no-optional-locks`, in-progress detection, cancel token);
  - `src/broker/messages.rs` (optional `sha` on artifacts, the `rewritten` status);
  - `src/broker/server.rs` (`DELETE /watch`);
  - `src/broker/conflict.rs` (acknowledgement scope and lifetime, agent withdrawal, the
    `conflict-detector` sender and the split of the acknowledgement filter from the guard);
  - `src/broker/server.rs` and `src/broker/learnings.rs` (refuse the reserved id at `/publish`;
    never count it as an agent);
  - `src/commands/remove.rs` (withdraw before `detach_worktree`);
  - the orchestrator's launch (`GIT_PAW_ACTOR`).
- **Wire:** an optional `sha` on `agent.artifact`, a `rewritten` artifact status and a new route,
  `DELETE /watch`. Old publishers and readers keep working. Detector messages change their sender
  from `supervisor` to `conflict-detector`. A reader that matched detector output on
  `from = "supervisor"` must match the new id or the `[conflict-detector]` tag.
- **Tests:** hook e2e (a real rebase of N commits publishes one event; the commit file list;
  orchestrator attribution), a watcher test with a held `index.lock` and a rebase in progress,
  conflict-tracker unit tests (acknowledgement lifetime, withdrawal, sender id, re-entrancy
  guard, acknowledgement filter), a `/publish` test for the reserved id, and a remove e2e with an
  overlapping peer.
- **Docs:** the coordination chapter (the `rewritten` status, `sha` and the `conflict-detector`
  sender), the user guide's conflict-detection chapter (acknowledgement lifetime and the
  detector's sender id), and the orchestrator and coordination skills where they name the
  detector's sender.
- **Dependencies:**
  - **`supervisor-vocabulary-split` (this release).** New text uses the orchestrator vocabulary.
    Land the split first, or map the names when rebasing this change. The detector's new
    requirements name "the root identity", so they hold under either spelling. The detector id
    is not an alias of the root identity, and the split's inbound normalization must leave it
    alone.
  - **`wire-format-dtos` (this release)** moves the message types into `broker::wire`. The new
    field goes there, and counts as an additive protocol-version change.
  - **`spec-kit-wave-planning`** also adds a conflict-detector exemption (PK-08). The two do not
    overlap.
  - **`verification-lifecycle`** builds on this change: it reads the artifact `sha` and relies on
    `rewritten` drawing no nudge.
