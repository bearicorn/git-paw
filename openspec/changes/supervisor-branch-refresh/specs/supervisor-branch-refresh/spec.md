## ADDED Requirements

### Requirement: Branch refresh is opt-in and default-off

The supervisor SHALL NOT rebase a live worker branch unless branch refresh has been explicitly enabled in configuration. The configuration field SHALL default to disabled, SHALL be declared with `#[serde(default)]` and `skip_serializing_if`, and SHALL be absent from configurations written by earlier versions without error. When branch refresh is disabled, the supervisor's post-merge behaviour SHALL be byte-identical to the behaviour before this capability existed.

#### Scenario: Refresh disabled by default

- **GIVEN** a configuration that does not mention branch refresh
- **WHEN** the supervisor completes a successful merge into the default branch
- **THEN** no worker branch SHALL be rebased
- **AND** the supervisor's observable post-merge behaviour SHALL be unchanged

#### Scenario: Existing configuration loads unchanged

- **WHEN** a configuration written before this capability existed is loaded
- **THEN** it SHALL load without error
- **AND** branch refresh SHALL be disabled

### Requirement: Refresh is attempted only after a successful merge

When branch refresh is enabled, the supervisor SHALL evaluate worker branches for refresh only after a successful merge into the default branch, and SHALL NOT refresh on a timer, on an interval, or at arbitrary points during an agent's turn. A merge is the event that makes a worker branch stale, and is therefore the only event that justifies the risk of rewriting a live branch.

#### Scenario: Refresh evaluated after a merge

- **GIVEN** branch refresh is enabled
- **WHEN** the supervisor completes a successful merge into the default branch
- **THEN** each live worker branch SHALL be evaluated against the refresh preconditions

#### Scenario: No refresh without a merge

- **GIVEN** branch refresh is enabled
- **AND** no merge into the default branch has occurred
- **WHEN** time passes and agents continue working
- **THEN** no worker branch SHALL be rebased

### Requirement: All refresh preconditions are a conjunction

The supervisor SHALL rebase a worker branch only when ALL of the following hold simultaneously. If ANY precondition fails, the branch SHALL be left untouched, and the supervisor SHALL NOT attempt a partial, degraded, or retried rebase within the same merge event:

1. The worker's working tree is clean — it has no uncommitted changes.
2. The worker's pane is not mid-response.
3. The supervisor holds the pane claim for that worker.
4. The rebase is predicted to be conflict-free.
5. The branch is behind the default branch.

#### Scenario: Clean, idle, claimed, conflict-free, behind branch is refreshed

- **GIVEN** branch refresh is enabled and a merge has just succeeded
- **AND** a worker's working tree is clean, its pane is not mid-response, the pane claim is held, the rebase is predicted conflict-free, and the branch is behind the default branch
- **WHEN** refresh is evaluated for that branch
- **THEN** the branch SHALL be rebased onto the default branch

#### Scenario: Dirty working tree is never rebased

- **GIVEN** branch refresh is enabled and a merge has just succeeded
- **AND** a worker has uncommitted changes in its working tree
- **WHEN** refresh is evaluated for that branch
- **THEN** the branch SHALL NOT be rebased
- **AND** the supervisor SHALL NOT stash, commit, or otherwise modify the worker's uncommitted changes

#### Scenario: Mid-response pane is not rebased

- **GIVEN** branch refresh is enabled and a merge has just succeeded
- **AND** a worker's working tree is clean but its pane is mid-response
- **WHEN** refresh is evaluated for that branch
- **THEN** the branch SHALL NOT be rebased

#### Scenario: Idle but dirty is still skipped

- **GIVEN** a worker whose pane is not mid-response
- **AND** whose working tree has uncommitted changes
- **WHEN** refresh is evaluated for that branch
- **THEN** the branch SHALL NOT be rebased, because idleness does not imply cleanliness

#### Scenario: Branch already current is not rebased

- **GIVEN** a worker branch that is already at or ahead of the default branch
- **WHEN** refresh is evaluated for that branch
- **THEN** the branch SHALL NOT be rebased

### Requirement: Cleanliness is read from the watcher's tracked state

The supervisor SHALL determine a worker's working-tree cleanliness from the broker watcher's tracked per-agent modified-file state, rather than issuing an independent working-tree probe at rebase time. A second source of truth could disagree with the detector about the one fact the safety argument depends on, and a supervisor that believes a worktree is clean while the watcher believes otherwise is the precondition for destroying uncommitted work.

#### Scenario: Cleanliness derived from watcher state

- **WHEN** the supervisor evaluates the cleanliness precondition for a worker
- **THEN** it SHALL use the watcher's tracked modified-file state for that agent
- **AND** it SHALL NOT issue a separate working-tree status probe to make the decision

### Requirement: Conflicts are predicted before the worktree is touched

The supervisor SHALL predict whether the rebase would conflict BEFORE invoking the rebase, and SHALL skip a branch whose rebase is predicted to conflict. Prediction keeps the common failure path from ever placing a live worktree into an in-progress rebase state that the running agent could observe or act on.

#### Scenario: Predicted conflict skips the branch

- **GIVEN** a worker branch whose rebase onto the default branch is predicted to conflict
- **WHEN** refresh is evaluated for that branch
- **THEN** the rebase SHALL NOT be invoked
- **AND** the worktree SHALL NOT be placed into a rebase state

### Requirement: A failed rebase restores the pre-rebase HEAD

If a rebase is invoked despite a clean prediction and then fails, the supervisor SHALL abort the rebase and leave the branch at its pre-rebase HEAD, so a mispredicted conflict never leaves a worker's branch in a partially rewritten or in-progress state.

#### Scenario: Mispredicted conflict is aborted and restored

- **GIVEN** a rebase predicted conflict-free that fails once invoked
- **WHEN** the failure is detected
- **THEN** the rebase SHALL be aborted
- **AND** the branch SHALL be left at the HEAD it had before the rebase began
- **AND** the worker's committed work SHALL NOT be lost

### Requirement: A verified branch awaiting merge is never refreshed

The supervisor SHALL NOT refresh a branch that has already passed verification and is awaiting merge. A branch verified at one base has not been verified at a different base, so rewriting it would silently invalidate the verification that authorised its merge.

#### Scenario: Verified branch is excluded from refresh

- **GIVEN** a worker branch that has passed verification and is awaiting merge
- **WHEN** refresh is evaluated for that branch
- **THEN** the branch SHALL NOT be rebased

### Requirement: A refreshed worker is told its HEAD moved

After a successful refresh, the supervisor SHALL notify the affected worker that its branch HEAD was rewritten, so the agent does not continue reasoning about file contents and commit SHAs from before the rewrite. The notification SHALL be carried on an existing broker message type with a distinguishing tag, and SHALL NOT introduce a new `BrokerMessage` variant. The notification SHALL NOT be sent as `agent.advanced-main`, because that event means the base moved and the worker-side discipline keys off that meaning; conflating it with "your own HEAD was rewritten" would corrupt that discipline.

#### Scenario: Worker notified after a successful refresh

- **GIVEN** a worker branch that has just been successfully rebased
- **WHEN** the refresh completes
- **THEN** the worker SHALL receive a notification stating that its branch HEAD was rewritten
- **AND** the notification SHALL carry a distinguishing tag identifying it as a branch refresh

#### Scenario: Refresh notification is not advanced-main

- **WHEN** a worker is notified of a branch refresh
- **THEN** the message SHALL NOT be an `agent.advanced-main` message
- **AND** no new `BrokerMessage` variant SHALL be required to carry it

### Requirement: A skipped branch is recorded, not retried

When a branch fails its preconditions, the supervisor SHALL record the skip and SHALL NOT retry the refresh within the same merge event. The branch SHALL be re-evaluated at the next successful merge. Repeated skips SHALL NOT escalate to the supervisor inbox as a question, because a busy or dirty agent is normal operation and a noisy signal is what makes a real warning unusable.

#### Scenario: Skip is recorded without retry

- **GIVEN** a worker branch that fails a refresh precondition
- **WHEN** refresh is evaluated for that branch
- **THEN** the skip SHALL be recorded
- **AND** no further refresh attempt SHALL be made for that branch during the same merge event

#### Scenario: Repeated skips do not escalate

- **GIVEN** a worker branch that has failed its refresh preconditions across several consecutive merges
- **WHEN** refresh is evaluated again
- **THEN** the supervisor SHALL NOT raise a question to the supervisor inbox about the repeated skips
