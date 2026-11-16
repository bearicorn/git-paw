## MODIFIED Requirements

### Requirement: Coordination skill — stand-by after final commit

The embedded `coordination.md` skill SHALL include a positive "stand by after your
final commit" protocol that tells the coding agent what to do *instead* of reaching
for verification or archive once its work is committed. This complements the existing
`opsx-role-gating` forbidden-commands block (which forbids `/opsx:verify` and
`/opsx:archive` from a coding-agent worktree) by supplying the actionable next step.
The protocol SHALL:

1. Instruct the agent that after its final commit it SHALL publish exactly one
   change-complete signal, `agent.artifact { status: "done" }` through the agent helper's
   `artifact` subcommand, for code and code-less work alike, and SHALL then **stand by** and
   **wait**. It SHALL state that the automatic `agent.artifact { status: "committed" }` the hook
   publishes per commit is not the change-complete signal.
2. State that while standing by the agent SHALL NOT run `/opsx:verify` or
   `/opsx:archive` — these are orchestrator-only — and SHALL cross-reference the
   role-gating guidance rather than restate the full forbidden-commands list.
3. State what the agent waits *for*: an `agent.verified`, `agent.feedback`, or
   further `agent.intent` from the orchestrator; on `agent.feedback` the agent fixes the
   listed errors, commits, and publishes the change-complete signal again.
4. State that while standing by the agent SHALL NOT start work the orchestrator did not
   route to it.

#### Scenario: Coordination skill instructs standing by after the final commit

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains explicit guidance to stand by (wait) after the final commit rather than proceed to verification or archive
- **AND** it states that the agent waits for `agent.verified`, `agent.feedback`, or further `agent.intent` from the orchestrator

#### Scenario: Stand-by protocol forbids self-verify and self-archive

- **WHEN** the stand-by guidance is inspected
- **THEN** it states the agent SHALL NOT run `/opsx:verify` or `/opsx:archive` while standing by
- **AND** it cross-references the orchestrator-only / role-gating guidance rather than re-deriving its own enforcement

#### Scenario: The skill names done as the change-complete signal

- **WHEN** the stand-by guidance is inspected
- **THEN** it instructs the agent to publish `agent.artifact { status: "done" }` once after its final commit, including for code changes
- **AND** it does not describe the automatic `committed` artifact as the done signal

## ADDED Requirements

### Requirement: Coordination skill — a frozen branch takes no commits

The coordination skill SHALL tell the coding agent that an `orchestrator.freeze` message, or a commit refused because the branch is under verification, means its branch is frozen. It SHALL NOT commit, amend, rebase or reset while frozen, and SHALL wait for `agent.verified` or `agent.feedback`.

#### Scenario: Skill explains the freeze

- **WHEN** the embedded coordination skill is inspected
- **THEN** it names `orchestrator.freeze`, forbids commits, amends, rebases and resets while frozen, and names the two verdicts that lift it
