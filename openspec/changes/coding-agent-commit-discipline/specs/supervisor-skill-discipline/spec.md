# supervisor-skill-discipline Specification — delta for coding-agent-commit-discipline

## MODIFIED Requirements

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
