## ADDED Requirements

### Requirement: git paw finish subcommand

The system SHALL provide `git paw finish <branch>` with an optional `--into <target>` and the mutually exclusive flags `--merge-commit` and `--ff-only`. It SHALL integrate `<branch>` into the target branch and then tear down the branch's agent. It SHALL run only from the repository's main working tree: invoked from inside a linked worktree it SHALL exit non-zero with a message naming the main working tree, and SHALL change nothing. `<branch>` SHALL exist as a local branch, otherwise the command SHALL exit non-zero and change nothing. The subcommand SHALL carry `about`, `long_about` with examples, and a `help` string on every flag.

#### Scenario: Finish from the main working tree integrates and tears down

- **GIVEN** an agent `feat/x` in the session whose worktree is clean and whose branch is ahead of the default branch by fast-forwardable commits
- **WHEN** the operator runs `git paw finish feat/x` from the main working tree
- **THEN** the default branch SHALL point at `feat/x`'s tip
- **AND** the agent's surface SHALL be closed, its worktree removed, and its entry dropped from the session

#### Scenario: Finish refuses to run inside a linked worktree

- **GIVEN** the current directory is inside the agent worktree of `feat/x`
- **WHEN** `git paw finish feat/x` runs
- **THEN** it SHALL exit non-zero naming the main working tree
- **AND** no ref, worktree or session entry SHALL change

#### Scenario: Unknown branch is refused

- **WHEN** the operator runs `git paw finish feat/ghost` and no local branch `feat/ghost` exists
- **THEN** the command SHALL exit non-zero and change nothing

#### Scenario: Conflicting policy flags are rejected

- **WHEN** the operator runs `git paw finish feat/x --merge-commit --ff-only`
- **THEN** argument parsing SHALL fail with a usage error

### Requirement: Finish refuses uncommitted or detached work

Before integrating, finish SHALL check the worktree in which `<branch>` is checked out, when there is one. It SHALL refuse, exiting non-zero without changing anything, when that worktree has uncommitted changes other than git-paw-managed files (the same managed-file rule as `git paw remove`'s uncommitted-work check), listing the offending files. It SHALL also refuse when that worktree's `HEAD` is not `<branch>`, for example while a rebase is in progress. Finish SHALL offer no option that discards uncommitted work.

#### Scenario: Dirty worktree is refused with the file list

- **GIVEN** `feat/x`'s worktree has an uncommitted change to `src/foo.rs`
- **WHEN** `git paw finish feat/x` runs
- **THEN** it SHALL exit non-zero listing `src/foo.rs`
- **AND** the target branch, the worktree and the session SHALL be unchanged

#### Scenario: Only git-paw-managed files is clean

- **GIVEN** `feat/x`'s worktree whose only uncommitted entry is `.git-paw/AGENTS.local.md`
- **WHEN** `git paw finish feat/x` runs
- **THEN** finish SHALL treat the worktree as clean and proceed

#### Scenario: Detached worktree is refused

- **GIVEN** `feat/x`'s worktree is in the middle of a rebase (its `HEAD` is detached)
- **WHEN** `git paw finish feat/x` runs
- **THEN** it SHALL exit non-zero stating the worktree is not on `feat/x`, and change nothing

### Requirement: Integration target resolution

The integration target SHALL be the `--into` branch when given, otherwise the repository's default branch resolved through the single default-branch resolver. A `--into` value that is not a valid branch name (per `git check-ref-format --branch`) or that equals `<branch>` SHALL be refused with a non-zero exit and no change. When the target branch does not exist, finish SHALL create it at `<branch>`'s tip, so the work is moved onto a fresh branch, and then tear down as for any successful integration.

#### Scenario: Default target is the default branch

- **GIVEN** a repository whose default branch is `master` and no `--into`
- **WHEN** `git paw finish feat/x` succeeds
- **THEN** `master` SHALL contain `feat/x`'s tip

#### Scenario: Missing target is created at the branch tip

- **GIVEN** no branch `pr/export` exists
- **WHEN** `git paw finish feat/x --into pr/export` runs on a clean worktree
- **THEN** `pr/export` SHALL exist pointing at `feat/x`'s tip
- **AND** the agent of `feat/x` SHALL be torn down

#### Scenario: Invalid or self target is refused

- **WHEN** `git paw finish feat/x --into feat/x` or `git paw finish feat/x --into "bad..name"` runs
- **THEN** finish SHALL exit non-zero and change nothing

### Requirement: Fast-forward-only integration by default

Unless the merge-commit policy is selected, finish SHALL integrate only by fast-forward: when the target is an ancestor of `<branch>` the target SHALL move to `<branch>`'s tip. When the target has commits that `<branch>` does not contain, finish SHALL refuse with exit code 3, a message naming the number of target commits missing from `<branch>` and suggesting to rebase `<branch>` onto the target or to pass `--merge-commit`, and SHALL change nothing. When `<branch>` is already contained in the target, finish SHALL report it as already integrated and proceed to teardown.

#### Scenario: Fast-forward succeeds

- **GIVEN** the target is an ancestor of `feat/x`
- **WHEN** `git paw finish feat/x` runs
- **THEN** the target SHALL point at `feat/x`'s tip and no merge commit SHALL be created

#### Scenario: Diverged branch is refused with exit 3

- **GIVEN** the target gained a commit after `feat/x` was cut
- **WHEN** `git paw finish feat/x` runs without `--merge-commit`
- **THEN** the process SHALL exit with code 3
- **AND** the message SHALL say the target has 1 commit not in `feat/x` and suggest rebasing or `--merge-commit`
- **AND** the target ref, the worktree and the session SHALL be unchanged

#### Scenario: Already-integrated branch only tears down

- **GIVEN** `feat/x`'s tip is already an ancestor of the target
- **WHEN** `git paw finish feat/x` runs
- **THEN** the target SHALL NOT move
- **AND** the output SHALL say the branch was already integrated, and the agent SHALL be torn down

### Requirement: Merge-commit integration is opt-in

The merge policy SHALL be read from `[finish] strategy` in `.git-paw/config.toml`, accepting `"ff-only"` (the default when the table or field is absent) and `"merge-commit"`. `--merge-commit` and `--ff-only` SHALL override the configured policy for that invocation. Under the merge-commit policy finish SHALL fast-forward when possible and otherwise create a merge commit whose parents are the target tip and `<branch>`'s tip. When the merge would conflict, finish SHALL refuse with exit code 3, name the conflicting paths, and leave the target, every checkout and the session unchanged. An existing config without `[finish]` SHALL load unchanged.

#### Scenario: Config selects merge commits

- **GIVEN** `[finish] strategy = "merge-commit"` and a target that diverged from `feat/x` without conflicts
- **WHEN** `git paw finish feat/x` runs
- **THEN** the target SHALL point at a new merge commit whose parents are the previous target tip and `feat/x`'s tip

#### Scenario: Flag overrides config

- **GIVEN** `[finish] strategy = "merge-commit"` and a diverged target
- **WHEN** `git paw finish feat/x --ff-only` runs
- **THEN** the process SHALL exit with code 3 and change nothing

#### Scenario: Conflicting merge is refused

- **GIVEN** the merge-commit policy and a target whose changes conflict with `feat/x` in `src/lib.rs`
- **WHEN** `git paw finish feat/x` runs
- **THEN** the process SHALL exit with code 3 naming `src/lib.rs`
- **AND** no merge commit, conflict marker or in-progress merge state SHALL remain anywhere

#### Scenario: Config without the finish table loads

- **GIVEN** a config file written before this change, with no `[finish]` table
- **WHEN** it is loaded
- **THEN** loading SHALL succeed and the policy SHALL resolve to `ff-only`

### Requirement: Integration leaves other checkouts consistent

When the target branch is checked out in some worktree (typically the main working tree), finish SHALL perform the integration in that worktree so its files follow the ref, and SHALL refuse with a non-zero exit, naming that worktree and its uncommitted files, when that worktree has uncommitted changes to tracked files. When the target is checked out nowhere, finish SHALL move the target ref with a compare-and-swap update against the tip it observed; if the ref moved in between, finish SHALL abort with a non-zero exit and change nothing. Finish SHALL never switch the branch checked out in any worktree, never force-update a ref, and never push.

#### Scenario: Target checked out at the root with a dirty checkout

- **GIVEN** the default branch is checked out in the main working tree, which has an uncommitted edit to `README.md`
- **WHEN** `git paw finish feat/x` runs
- **THEN** it SHALL exit non-zero naming the main working tree and `README.md`, and change nothing

#### Scenario: Target checked out at the root and clean

- **GIVEN** the default branch is checked out and clean in the main working tree
- **WHEN** `git paw finish feat/x` fast-forwards it
- **THEN** the main working tree's files SHALL match `feat/x`'s tip and its status SHALL be clean

#### Scenario: Target not checked out moves by compare-and-swap

- **GIVEN** `--into release` where `release` exists but is checked out in no worktree
- **WHEN** `git paw finish feat/x --into release` fast-forwards it
- **THEN** `release` SHALL point at `feat/x`'s tip and every worktree's checked-out branch SHALL be unchanged

### Requirement: Finished agent is torn down

After a successful integration (or an already-integrated branch), finish SHALL tear the agent down through the same per-worktree teardown `git paw remove` uses: close the agent's terminal surface when it belongs to the live session, drop its session entry under the session lock and rewrite the discovery file, run the configured `on_remove` hook, and remove the worktree. Finish SHALL delete `<branch>` only when git-paw created it and its tip is contained in the integration target; otherwise the branch SHALL be kept. Finish SHALL remove only worktrees recorded in the repository's git-paw session; a branch checked out in any other worktree SHALL be integrated and its worktree left in place with a note. Finish SHALL never remove the main working tree. When integration is refused or fails, no teardown step SHALL run.

#### Scenario: Created branch is deleted after integration

- **GIVEN** `feat/x` was created by git-paw
- **WHEN** `git paw finish feat/x --into pr/x` succeeds
- **THEN** `feat/x` SHALL be deleted, because its tip is contained in `pr/x`

#### Scenario: Pre-existing branch is kept

- **GIVEN** `feat/x` existed before the session started
- **WHEN** `git paw finish feat/x` succeeds
- **THEN** the worktree SHALL be removed and the branch `feat/x` SHALL remain

#### Scenario: Unmanaged worktree is left in place

- **GIVEN** `feat/y` is checked out in a worktree the operator created by hand, not recorded in the session
- **WHEN** `git paw finish feat/y` succeeds
- **THEN** the target SHALL contain `feat/y` and the hand-made worktree SHALL still exist
- **AND** the output SHALL say the worktree was left in place

#### Scenario: Refused integration tears nothing down

- **GIVEN** a diverged `feat/x` under the ff-only policy
- **WHEN** `git paw finish feat/x` exits with code 3
- **THEN** the agent's surface, worktree, session entry and branch SHALL all remain

### Requirement: Finish announces an advanced default branch

When finish moves the default branch and the session's broker is reachable, it SHALL publish one `agent.advanced-main` message from the root identity carrying the merged branch, the new tip (12-character abbreviated SHA), the default branch name as `base`, the merge time, and a one-line summary. An unreachable or absent broker SHALL produce a single warning line and SHALL NOT change the exit status. Integrating into any other target SHALL publish nothing.

#### Scenario: Live broker receives advanced-main

- **GIVEN** a session with a live broker
- **WHEN** `git paw finish feat/x` fast-forwards the default branch
- **THEN** the broker log SHALL contain one `agent.advanced-main` with `merged_branch = "feat/x"` and `base` equal to the default branch

#### Scenario: No broker does not fail finish

- **GIVEN** no reachable broker
- **WHEN** `git paw finish feat/x` succeeds
- **THEN** it SHALL print one warning that the broker could not be notified and exit 0

### Requirement: git paw finish reports its result as JSON

`git paw finish` SHALL accept `--json`. With it, the command SHALL follow the `cli-machine-output` contract and print one document containing `schema_version`, `target` (the integration target), `target_created` (whether finish created it), `strategy` (`"ff-only"` or `"merge-commit"`), `broker_notified` (whether an `agent.advanced-main` was published), and `branches`, an array with one entry per finished branch carrying `branch`, `outcome` (`"integrated"`, `"already_integrated"` or `"refused"`), `reason` (a string for `refused`, otherwise `null`), `target_tip` (the target's tip after the branch was processed), `worktree_removed`, and `branch_deleted`. An integration refused by the merge policy is a result, not a command failure: the document SHALL be printed with that branch's `outcome` `"refused"` and the process SHALL exit 3. Any other failure (exit 1) SHALL leave stdout empty with the error on stderr. Warnings that the human mode prints (an unreachable broker, an unmanaged worktree left in place) SHALL go to stderr. `--json` SHALL NOT add a prompt or change what finish does.

#### Scenario: finish --json reports an integration

- **GIVEN** a clean, fast-forwardable `feat/x` git-paw created
- **WHEN** `git paw finish feat/x --json` succeeds
- **THEN** stdout SHALL be one object with `schema_version` 1, the default branch as `target`, `target_created` false, `strategy` `"ff-only"`, and one `branches` entry with `branch` `"feat/x"`, `outcome` `"integrated"`, `reason` null, `target_tip` equal to `feat/x`'s tip, `worktree_removed` true and `branch_deleted` true

#### Scenario: A refused integration prints the document and exits 3

- **GIVEN** a diverged `feat/x` under the ff-only policy
- **WHEN** `git paw finish feat/x --json` runs
- **THEN** stdout SHALL be one object whose `branches` entry has `outcome` `"refused"` and a non-null `reason`
- **AND** the process SHALL exit 3

#### Scenario: A failed finish prints nothing on stdout

- **GIVEN** `feat/x`'s worktree has uncommitted changes
- **WHEN** `git paw finish feat/x --json` runs
- **THEN** stdout SHALL be empty, the error SHALL be on stderr, and the exit code SHALL be 1
