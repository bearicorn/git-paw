# supervisor-skill-discipline Specification

## Purpose
TBD - created by archiving change supervisor-skill-discipline-v0-6-x. Update Purpose after archive.
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

