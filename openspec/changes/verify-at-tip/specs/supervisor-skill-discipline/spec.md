## MODIFIED Requirements

### Requirement: Isolated verification worktrees use a repo-local gitignored scratch dir

The bundled supervisor skill SHALL instruct the supervisor to create
any isolated verification worktree under a repo-local, gitignored
scratch directory — `.git-paw/tmp/verify-<branch>/` — and SHALL NOT
direct it to `/tmp` or any path outside the repository. The skill
SHALL teach the cleanup step (`git worktree remove` / `git worktree
prune`) so scratch worktrees do not accumulate.

The recipe SHALL check out the agent branch's **re-resolved tip**, not a
pinned commit SHA captured from a `committed` event. The skill SHALL
instruct the supervisor to resolve `TIP=$(git rev-parse <branch>)`
immediately before `git worktree add --detach`, and to pass that
re-resolved tip (not a previously captured `$SHA`) as the checkout
target. The recipe SHALL re-resolve the tip and re-create the worktree
each time the gates are (re-)run for the branch, so the worktree never
holds a snapshot older than the branch's current tip. The detach mode
SHALL be preserved so the agent's own worktree remains the authoritative
holder of the branch ref.

The repository `.gitignore` SHALL ignore `.git-paw/tmp/` so the nested
verification worktree never appears in the parent worktree's status.

#### Scenario: Supervisor skill names the repo-local scratch path

- **WHEN** the bundled `supervisor.md` is inspected
- **THEN** it SHALL instruct creating the isolated verify worktree
  under `.git-paw/tmp/` (repo-local, gitignored)
- **AND** it SHALL NOT instruct using `/tmp` for verification scratch

#### Scenario: Scratch directory is gitignored

- **GIVEN** the repository `.gitignore`
- **WHEN** it is inspected
- **THEN** it SHALL contain an entry ignoring `.git-paw/tmp/`

#### Scenario: Verify worktree checks out the re-resolved branch tip

- **WHEN** the bundled `supervisor.md` isolated-verify-worktree recipe is inspected
- **THEN** it SHALL resolve the branch tip with `git rev-parse <branch>` immediately before `git worktree add --detach`
- **AND** it SHALL pass that re-resolved tip as the checkout target, NOT a commit SHA captured from a `committed` event

#### Scenario: Recipe re-resolves the tip on re-run

- **WHEN** the recipe's re-run / re-verification guidance is read
- **THEN** it SHALL state that each (re-)run of the gates re-resolves the branch tip and re-creates the worktree, so the worktree never holds a snapshot older than the current tip
