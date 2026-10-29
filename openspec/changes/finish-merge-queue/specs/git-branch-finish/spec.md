## ADDED Requirements

### Requirement: Finish integrates an ordered queue of branches

`git paw finish` SHALL accept one or more branch names and SHALL process them strictly in the order given, each through the same preconditions, integration policy and teardown as a single-branch finish, against the target as advanced by the branches before it. With `--verified` and no branch names, the queue SHALL be every agent of the live session whose broker status resolves to the verified phase, in session order; `--verified` without a reachable broker SHALL exit non-zero and change nothing, and `--verified` combined with explicit branch names SHALL be a usage error.

#### Scenario: Branches integrate in the given order

- **GIVEN** `feat/a` and `feat/b` both fast-forwardable onto the default branch in sequence (`feat/b` built on `feat/a`)
- **WHEN** the operator runs `git paw finish feat/a feat/b`
- **THEN** the default branch SHALL end at `feat/b`'s tip and both agents SHALL be torn down

#### Scenario: Verified selection

- **GIVEN** a live session with agents `feat/a` (verified), `feat/b` (working) and `feat/c` (verified)
- **WHEN** the operator runs `git paw finish --verified`
- **THEN** only `feat/a` then `feat/c` SHALL be processed

#### Scenario: Verified selection needs a broker

- **GIVEN** no reachable broker
- **WHEN** the operator runs `git paw finish --verified`
- **THEN** the command SHALL exit non-zero and change nothing

### Requirement: Mechanical rebase only of provably disjoint branches

With `--rebase`, a queued branch that is not fast-forwardable onto the current target SHALL be rebased onto the target by git-paw only when ALL of the following hold: no path changed on the branch since its merge base with the target was also changed on the target since that merge base; `git merge-tree --write-tree` of target and branch reports no conflict; the branch's worktree is clean; when the branch belongs to a live agent, the agent's surface is not mid-response and git-paw holds that surface's claim for the whole rebase-and-test window; and a test command is configured. If any condition fails the branch SHALL be skipped untouched. Without `--rebase`, a non-fast-forwardable branch SHALL be skipped as under the single-branch policy.

#### Scenario: Disjoint branch is rebased and integrated

- **GIVEN** `feat/a` and `feat/b` cut from the same base, touching different files, and a configured test command that passes
- **WHEN** the operator runs `git paw finish --rebase feat/a feat/b`
- **THEN** `feat/a` SHALL fast-forward the target
- **AND** `feat/b` SHALL be rebased onto the new target, re-tested, and fast-forwarded
- **AND** the report SHALL mark `feat/b` as integrated after rebase

#### Scenario: Overlapping paths are skipped even when merge-tree is clean

- **GIVEN** `feat/a` and `feat/b` both edit `CHANGELOG.md` in non-conflicting hunks
- **WHEN** `git paw finish --rebase feat/a feat/b` runs
- **THEN** `feat/b` SHALL be skipped with the overlapping path named and SHALL NOT be rebased

#### Scenario: Busy agent is skipped

- **GIVEN** `feat/b` needs a rebase and its agent's surface is mid-response
- **WHEN** the queue reaches `feat/b`
- **THEN** `feat/b` SHALL be skipped with reason "agent busy" and its worktree SHALL be untouched

#### Scenario: No test command means no mechanical rebase

- **GIVEN** no `[supervisor] test_command` is configured
- **WHEN** `git paw finish --rebase feat/a feat/b` reaches a disjoint but non-fast-forwardable `feat/b`
- **THEN** `feat/b` SHALL be skipped with a reason stating no test command is configured to re-verify it

### Requirement: A rebased branch is re-tested before the target moves

After a mechanical rebase the queue SHALL run the configured test command inside the branch's worktree and SHALL integrate the branch only when it exits 0. When the rebase itself fails, or the test command fails, the queue SHALL return the branch and its worktree to the pre-rebase tip, leave no rebase in progress, skip the branch with the failure as its reason, and continue with the next branch. The target branch SHALL NOT move for a branch whose re-test did not pass.

#### Scenario: Failing re-test restores the branch

- **GIVEN** a disjoint `feat/b` whose tests fail after rebasing onto the advanced target
- **WHEN** the queue processes `feat/b`
- **THEN** `feat/b` SHALL point at its pre-rebase tip with a clean worktree and no rebase in progress
- **AND** the target SHALL NOT include `feat/b`
- **AND** the report SHALL give the test failure as the skip reason

#### Scenario: Failed rebase restores the branch

- **GIVEN** a rebase predicted clean that fails when run
- **WHEN** the queue processes the branch
- **THEN** the rebase SHALL be aborted, the branch SHALL be at its pre-rebase tip, and the branch SHALL be skipped

### Requirement: Skipped agents receive one batched rebase directive

After the whole queue has been processed, for every skipped branch that belongs to a live agent the queue SHALL publish exactly one `agent.feedback` to that agent from the root identity, naming the target, the target's final tip, and the skip reason (overlapping paths, conflicting paths, agent busy, rebase failed, test failure, or policy). All directives SHALL be published after the last branch is processed, so each names the final target tip. With no reachable broker the queue SHALL print the directives instead and SHALL NOT fail because of it.

#### Scenario: One directive per skipped agent after the pass

- **GIVEN** a queue of `feat/a`, `feat/b`, `feat/c` where `feat/b` and `feat/c` are skipped
- **WHEN** the queue finishes
- **THEN** `feat/b` and `feat/c` SHALL each receive exactly one `agent.feedback`
- **AND** both SHALL name the same final target tip
- **AND** `feat/a` SHALL receive none

### Requirement: Queue report and exit status

The queue SHALL print one line per processed branch stating `integrated`, `integrated after rebase`, `already integrated`, or `skipped` with its reason. It SHALL exit 0 when every branch was integrated or already integrated, 3 when at least one branch was skipped by policy and no error occurred, and 1 on an error. On an error the queue SHALL stop at that branch, keep every integration already made, and still print the lines for the branches it processed.

#### Scenario: Partial success exits 3

- **GIVEN** a queue where one branch integrates and one is skipped for overlapping paths
- **WHEN** the queue finishes
- **THEN** the process SHALL exit with code 3 and the report SHALL list both outcomes

#### Scenario: An error stops the queue

- **GIVEN** a queue of three branches where the target's checkout becomes dirty before the second
- **WHEN** the queue reaches the second branch
- **THEN** it SHALL exit 1 naming the dirty checkout
- **AND** the first branch SHALL remain integrated and the third SHALL be untouched

### Requirement: The drive loop never integrates branches itself

The unattended drive loop SHALL NOT invoke the finish queue or otherwise move an integration target. Integrating branches SHALL remain a decision made outside the loop by the operator or the orchestrator.

#### Scenario: All workers verified does not trigger integration

- **GIVEN** an unattended session in which every worker reaches the verified phase
- **WHEN** the drive loop runs its sweep
- **THEN** no target branch SHALL move and no worktree SHALL be rebased by the loop

### Requirement: Queue results extend the finish JSON document

With `--json`, a queue SHALL print the same `cli-machine-output` document as a single-branch finish, extended additively: `branches` SHALL hold one entry per processed branch in processing order; `outcome` SHALL additionally take `"integrated_after_rebase"`, with every skipped branch reported as `"refused"` and its skip reason in `reason`; a top-level `directives` array SHALL list each published (or, without a broker, printed) rebase directive with `branch` and `message`; and a top-level `error` SHALL be `null`, or the error message when the queue stopped on an error. Because a queue that stopped on an error has already changed the repository, its document SHALL still be printed when it exits 1 after integrating at least one branch; an error before any branch was processed SHALL leave stdout empty.

#### Scenario: Queue document lists every branch and directive

- **GIVEN** a queue of `feat/a` (fast-forward), `feat/b` (disjoint, rebased) and `feat/c` (overlapping, skipped) with a live broker
- **WHEN** `git paw finish --rebase --json feat/a feat/b feat/c` finishes
- **THEN** `branches` SHALL list `feat/a` `"integrated"`, `feat/b` `"integrated_after_rebase"`, `feat/c` `"refused"` with its overlap reason
- **AND** `directives` SHALL contain one entry for `feat/c`
- **AND** `error` SHALL be null and the process SHALL exit 3

#### Scenario: A queue stopped mid-way still reports what it did

- **GIVEN** a queue whose second branch hits a dirty target checkout after the first integrated
- **WHEN** it runs with `--json`
- **THEN** stdout SHALL be the document with the first branch `"integrated"` and a non-null `error`
- **AND** the process SHALL exit 1
