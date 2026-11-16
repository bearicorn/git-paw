## ADDED Requirements

### Requirement: A merge is checked against the last verified tip

`agent.advanced-main` SHALL accept an optional `merged_sha`, the merged branch's tip. When it is present and differs from the `sha` of the last `agent.verified` recorded for that branch's agent, or no verified SHA is recorded, the broker SHALL put one `agent.question` tagged `[merge-check]`, naming the branch and both SHAs, into the orchestrator inbox.

#### Scenario: Merging a newer tip than the verified one is flagged

- **GIVEN** `feat-x` last verified at `sha = aaa111`
- **WHEN** an `agent.advanced-main` arrives with `merged_branch = "feat/x"` and `merged_sha = bbb222`
- **THEN** the orchestrator inbox SHALL receive one `[merge-check]` question naming `feat/x`, `aaa111` and `bbb222`

#### Scenario: Merging the verified tip raises nothing

- **GIVEN** `feat-x` last verified at `sha = aaa111`
- **WHEN** an `agent.advanced-main` arrives with `merged_branch = "feat/x"` and `merged_sha = aaa111`
- **THEN** no `[merge-check]` question SHALL be emitted

#### Scenario: An advanced-main without merged_sha is unchanged

- **GIVEN** any verified state
- **WHEN** an `agent.advanced-main` without `merged_sha` arrives
- **THEN** no `[merge-check]` question SHALL be emitted and routing SHALL be as before
