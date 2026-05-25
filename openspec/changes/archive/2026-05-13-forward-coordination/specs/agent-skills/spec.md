## MODIFIED Requirements

### Requirement: Embedded coordination skill

The embedded `coordination.md` skill content SHALL reflect the v0.5 state in which agents publish `agent.intent` before editing as the primary coordination signal, while `agent.status` publishing remains automated by the filesystem watcher and `agent.artifact` publishing remains automated by the post-commit git hook. The embedded content SHALL therefore:

1. NOT contain the legacy "MUST publish agent.status" instruction. Status publishing is automatic — agents do not curl `/publish` for `agent.status` themselves.
2. Include a note explaining that git-paw automatically publishes the agent's working status when the agent edits files and automatically publishes an `agent.artifact` when the agent runs `git commit`. The note SHALL state that agents only need to publish manually if they are blocked, want to announce explicit exports, or are signalling intent.
3. Retain the `agent.blocked` curl example as an opt-in operation for blocked agents.
4. Retain the `agent.artifact` curl example with `exports`, documented as the manual escape hatch when the agent wants to advertise specific exports beyond what the post-commit hook captures automatically.
5. Include a `### Cherry-pick peer commits` section that gives the exact `git cherry-pick` command an agent should run when a peer's `agent.artifact` arrives in the agent's inbox.
6. Include a `### Messages you may receive` section that documents the two supervisor-originated message variants:
   - `agent.verified` — the agent's work has been verified by the supervisor. No action required.
   - `agent.feedback` — the agent's work has issues. The `errors` field lists problems to fix; the agent SHALL address them and re-publish `agent.artifact`.
7. Continue to use `{{BRANCH_ID}}` and `{{GIT_PAW_BROKER_URL}}` placeholders, retaining the existing polling example `GET {{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}`.
8. Include a `### Before you start editing` section that instructs the agent to: (a) read its spec or task; (b) publish `agent.intent` listing the specific files it plans to touch with a one-line summary and a TTL; (c) poll once for warnings; (d) on overlap, decide whether to wait, split scope, or escalate via `agent.question`. The section SHALL include a `curl` example that publishes `agent.intent` with `files`, `summary`, and `valid_for_seconds`.
9. Include a `### While you're editing` section that instructs the agent to: (a) re-publish `agent.intent` if scope grows to include files not in the original list; (b) on seeing a peer's `agent.intent` for a file in the same module, send `agent.question` rather than racing. The section SHALL state explicitly that agents MUST NOT do pairwise check-ins on every change, MUST NOT wait for explicit go-ahead from peers when no conflict signal exists, and MUST NOT block on broker silence.

#### Scenario: Coordination skill documents automatic status publishing

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains text indicating that `agent.status` publishing is automatic
- **AND** it does NOT contain the substring "MUST publish agent.status"

#### Scenario: Coordination skill retains blocked and artifact curl examples

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains a `curl` example for publishing `agent.blocked`
- **AND** it contains a `curl` example for publishing `agent.artifact`

#### Scenario: Coordination skill contains cherry-pick instructions

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains the substring `git cherry-pick`
- **AND** the cherry-pick guidance is reachable under a `Cherry-pick peer commits` heading or equivalent

#### Scenario: Coordination skill documents verification and feedback messages

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains the substring `agent.verified`
- **AND** it contains the substring `agent.feedback`
- **AND** it contains guidance describing how to handle feedback (fix the listed errors and re-publish `agent.artifact`)

#### Scenario: Coordination skill retains polling reference

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains `{{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}`

#### Scenario: Coordination skill contains Before you start editing section

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains a heading `Before you start editing` (or equivalent)
- **AND** it contains a `curl` example that publishes `agent.intent`
- **AND** the `agent.intent` example includes `files`, `summary`, and `valid_for_seconds` payload fields

#### Scenario: Coordination skill contains While you're editing section

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains a heading `While you're editing` (or equivalent)
- **AND** it instructs the agent to re-publish `agent.intent` when scope grows
- **AND** it instructs the agent to use `agent.question` (not pairwise blocking) when a peer's intent overlaps

#### Scenario: Coordination skill rejects pairwise over-coordination patterns

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains explicit guidance that agents MUST NOT perform pairwise check-ins on every change
- **AND** it contains explicit guidance that agents MUST NOT wait for go-ahead from peers when no conflict signal exists
- **AND** it contains explicit guidance that agents MUST NOT block on broker silence

### Requirement: Embedded supervisor skill

The embedded supervisor skill SHALL include a "Spec Audit Procedure" section that instructs the supervisor to verify implementation matches spec before publishing `agent.verified`. The procedure SHALL include:

- How to locate spec files for a given change
- How to extract WHEN/THEN scenarios from spec files
- How to search the codebase for matching tests
- How to verify struct fields, function signatures, and types match SHALL/MUST requirements
- How to compile gaps into an `agent.feedback` error list
- When to publish `agent.verified` (no gaps) vs `agent.feedback` (gaps found)

The spec audit SHALL run after the test command passes and before `agent.verified` is published.

The embedded supervisor skill SHALL ALSO include a "Watch peer intents" pointer that informs the supervisor that `agent.intent` messages arrive in its inbox alongside other peer events. The pointer SHALL state that programmatic conflict-warning logic is not part of this release and that the supervisor MAY inspect intents and prompt agents via `agent.feedback` or `agent.question` if it spots overlap manually. The pointer is intentionally advisory — full conflict-detection algorithms are owned by the `conflict-detection` change.

#### Scenario: Supervisor skill contains spec audit procedure

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains the substring `Spec Audit`
- **AND** it contains instructions to read `openspec/changes/` spec files
- **AND** it contains instructions to grep for matching tests
- **AND** it contains instructions to verify field names match spec

#### Scenario: Spec audit runs after tests, before verified

- **WHEN** the embedded supervisor skill workflow is inspected
- **THEN** the spec audit step appears after the test command step
- **AND** the spec audit step appears before the `agent.verified` publish step

#### Scenario: Supervisor skill mentions agent.intent

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains the substring `agent.intent`
- **AND** it contains a heading or section titled `Watch peer intents` (or equivalent)
- **AND** it indicates that automatic conflict-warning logic is not part of this release
