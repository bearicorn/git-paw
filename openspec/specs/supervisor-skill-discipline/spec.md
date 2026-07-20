# supervisor-skill-discipline Specification

## Purpose
Encodes the operational disciplines the bundled supervisor skill teaches the supervisor agent: drive pane work through `sweep.sh` (never inline loops), never send-keys to its own pane, use `git -C` for cross-worktree git, nudge on commit cadence and rely on agents standing by post-commit, and create isolated verification worktrees under a repo-local gitignored scratch dir checked out at the re-resolved branch tip.
## Requirements
### Requirement: Mandate sweep.sh; forbid inline pane loops

The bundled supervisor skill SHALL include a section directing
the supervisor to use the bundled `.git-paw/scripts/sweep.sh`
helper for all pane capture, prompt approval, and send-keys.
The section SHALL explicitly forbid ad-hoc inline loops of the
form `for p in ...; do tmux ...; done`, stating that the
variable expansion trips the `simple_expansion` permission gate
and forces a human approval per iteration.

#### Scenario: Skill mandates sweep.sh for pane work

- **WHEN** the bundled supervisor.md is inspected
- **THEN** it SHALL contain a section directing all pane
  capture/approve/send-keys through `.git-paw/scripts/sweep.sh`

#### Scenario: Skill forbids inline pane loops

- **WHEN** the same section is read
- **THEN** it SHALL explicitly forbid `for p in ...; do tmux
  ...; done`-style inline loops, with the simple_expansion
  rationale

### Requirement: Never send-keys to the supervisor's own pane

The supervisor skill SHALL state that the supervisor sends
keystrokes only to agent panes and SHALL NEVER send-keys to
its own pane (pane 0), because doing so interrupts its own
in-flight command.

#### Scenario: Skill states the never-own-pane rule

- **WHEN** the supervisor.md pane-driving section is read
- **THEN** it SHALL state that the supervisor must not
  send-keys to its own pane, with the self-interrupt rationale

### Requirement: Cross-worktree git uses git -C, never cd

The supervisor skill SHALL include a rule that all git commands
against an agent worktree use `git -C <path> ...` and SHALL
forbid `cd <path> && git ...`. The rule SHALL state both
reasons: cd-before-git trips the untrusted-hooks warning, and
it leaks the working directory so a subsequent mutating git
command can land on the wrong branch.

#### Scenario: Skill mandates git -C

- **WHEN** the "Cross-worktree git" rule is read
- **THEN** it SHALL mandate `git -C <path>` for cross-worktree
  git and forbid `cd <path> && git`

#### Scenario: Rule states the cwd-leak rationale

- **WHEN** the rule is read
- **THEN** it SHALL cite both the untrusted-hooks warning and
  the wrong-branch (cwd-leak) risk as rationale

### Requirement: Reliable commit-cadence nudge

The supervisor skill SHALL state that when a sweep observes an
agent with more than a soft threshold (~10) of uncommitted
files, the supervisor publishes an `agent.feedback` nudging
the agent to commit its completed section. The threshold and a
sample nudge message SHALL be stated explicitly.

The supervisor skill SHALL ALSO state that the supervisor's verify-then-archive
workflow depends on coding agents **standing by** after their final commit: once an
agent has committed and published `agent.artifact { status: "committed" }` (or a manual
`status: "done"`), the supervisor — not the agent — runs `/opsx:verify` and
`/opsx:archive`. The skill SHALL cross-reference the agent-side stand-by-after-commit
protocol in `coordination.md` so the supervisor understands the post-commit signal is
its cue to begin verification, and that an agent should not be expected (or instructed)
to self-verify or self-archive.

#### Scenario: Skill states the nudge threshold and cue

- **WHEN** the coordination section is read
- **THEN** it SHALL state the ~10-uncommitted-file threshold
  and include a sample `agent.feedback` nudge message

#### Scenario: Skill states the supervisor relies on agents standing by post-commit

- **WHEN** the supervisor skill's commit-cadence / verification guidance is read
- **THEN** it SHALL state that the supervisor runs `/opsx:verify` and `/opsx:archive` after an agent's final commit, not the agent
- **AND** it SHALL cross-reference the agent-side stand-by-after-commit protocol in `coordination.md`

### Requirement: Stack-agnostic phrasing

The new/edited sections SHALL pass the no-language-leak audit
from [[lang-agnostic-assets]].

#### Scenario: No-leak audit passes

- **WHEN** the audit runs against the updated supervisor.md
- **THEN** it SHALL pass across all supported spec backends

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

### Requirement: Escalation-first, no blanket-approve when a drive loop is running

When the supervisor's boot context indicates a drive loop is running (an unattended session), the supervisor SHALL, each supervision cycle:

1. **Drain the drive loop's escalations first** — read the loop's escalation/review items from its broker inbox, reason about each, and either targeted-approve the specific escalated pane or publish feedback. This precedes the rest of the sweep so agents blocked on a prompt the loop could not classify safe are unblocked fastest.
2. **Then perform its normal sweep** — verification, merge orchestration, conflict handling, detect-stuck, and status publishing — as it otherwise would.

While a drive loop is running, the supervisor SHALL NOT blanket-approve classifier-safe prompts by sweeping panes: the loop owns safe-prompt approval, and the supervisor acts only on prompts the loop escalated. This keeps the two approvers' actions disjoint (see `unattended-operation`) and removes the approval-dispatch race.

When no drive loop is running (an attended supervisor session), the supervisor performs the full sweep INCLUDING approving classifier-safe prompts, as its sole-approver role requires — this preserves existing attended behaviour.

#### Scenario: With a loop running, escalations are handled before the sweep

- **GIVEN** a supervisor whose boot context indicates a drive loop is running
- **WHEN** it runs a supervision cycle
- **THEN** it SHALL process the loop's escalations (targeted approve / feedback) before its verify/merge/status sweep
- **AND** SHALL NOT blanket-approve classifier-safe prompts by sweeping panes

#### Scenario: With no loop, the supervisor approves safe prompts itself

- **GIVEN** a supervisor whose boot context does NOT indicate a drive loop
- **WHEN** it sweeps the panes
- **THEN** it SHALL approve classifier-safe prompts itself as the sole approver

