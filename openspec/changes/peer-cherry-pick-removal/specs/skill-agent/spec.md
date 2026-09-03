## MODIFIED Requirements

### Requirement: Embedded coordination skill

The embedded `coordination.md` skill content SHALL reflect the v0.5 state in which agents publish `agent.intent` before editing as the primary coordination signal, while `agent.status` publishing remains automated by the filesystem watcher and `agent.artifact` publishing remains automated by the post-commit git hook. The embedded content SHALL therefore:

1. NOT contain the legacy "MUST publish agent.status" instruction. Status publishing is automatic — agents do not curl `/publish` for `agent.status` themselves.
2. Include a note explaining that git-paw automatically publishes the agent's working status when the agent edits files and automatically publishes an `agent.artifact` when the agent runs `git commit`. The note SHALL state that agents only need to publish manually if they are blocked, want to announce explicit exports, or are signalling intent.
3. Retain the `agent.blocked` curl example as an opt-in operation for blocked agents.
4. Retain the `agent.artifact` curl example with `exports`, documented as the manual escape hatch when the agent wants to advertise specific exports beyond what the post-commit hook captures automatically.
5. Include a `### When you depend on a peer's work` section (or equivalent heading) that instructs the agent, when a peer's `agent.artifact` arrives naming work the agent depends on, to publish `agent.blocked` naming the peer and what it needs, then continue on unblocked work or wait. The section SHALL state that the agent MUST NOT cherry-pick a peer's commit into its own branch, and SHALL explain why: grafting a peer commit creates a second copy of a commit the supervisor will later merge from the peer's own branch, leaving the agent's branch divergent and unmergeable. The section SHALL direct the agent to the `agent.advanced-main` discipline as the supported way to take on peer work once that work has landed on the base branch. The embedded content SHALL NOT contain the substring `git cherry-pick`.
6. Include a `### Messages you may receive` section that documents the two supervisor-originated message variants:
   - `agent.verified` — the agent's work has been verified by the supervisor. No action required.
   - `agent.feedback` — the agent's work has issues. The `errors` field lists problems to fix; the agent SHALL address them and re-publish `agent.artifact`.
7. Continue to use `{{BRANCH_ID}}` and `{{GIT_PAW_BROKER_URL}}` placeholders, retaining the existing polling example `GET {{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}`.
8. Include a `### Before you start editing` section that instructs the agent to: (a) read its spec or task; (b) publish `agent.intent` listing the specific files it plans to touch with a one-line summary and a TTL; (c) poll once for warnings; (d) on overlap, decide whether to wait, split scope, or escalate via `agent.question`. The section SHALL include a `curl` example that publishes `agent.intent` with `files`, `summary`, and `valid_for_seconds`.
9. Include a `### While you're editing` section that instructs the agent to: (a) re-publish `agent.intent` if scope grows to include files not in the original list; (b) on seeing a peer's `agent.intent` for a file in the same module, send `agent.question` rather than racing. The section SHALL state explicitly that agents MUST NOT do pairwise check-ins on every change, MUST NOT wait for explicit go-ahead from peers when no conflict signal exists, and MUST NOT block on broker silence.
10. Include a `### Working heartbeat` section (or equivalent heading) that instructs the agent to publish a lightweight `agent.status` heartbeat with `status: "working"` every **5 tool uses** while actively working. The section SHALL:
    - State the cadence explicitly as "every 5 tool uses" (or substantively equivalent text naming the number 5).
    - Provide a `curl` example that publishes `agent.status` with `status: "working"` and `modified_files: []` (or the current dirty file list) so the broker treats it identically to watcher-driven status updates.
    - Explain *why* an agent-side heartbeat is needed in addition to the automatic filesystem watcher: the watcher cannot observe read-only tool uses (Read, Grep, Glob), permission-prompt waits, or LLM-only deliberation between tool calls, so `last_seen` would otherwise stay stale during active work.
    - Frame the cadence as a SHOULD (recommended floor): publishing more often is fine, publishing less often defeats the purpose.
11. Include a `### References & terminology` section (or equivalent heading) that documents the two related forms of agent identifier used throughout the broker protocol and names `slugify_branch` as the canonical conversion. The section SHALL:
    - Identify the **branch-name** form (e.g. `feat/no-supervisor-flag`) as the original git ref used in `git checkout`, `git worktree`, and other git-side operations.
    - Identify the **`agent_id`** form (e.g. `feat-no-supervisor-flag`) as the dashed slug used in every `/publish` payload, every `/messages/<id>` URL, and the `target` field of `agent.feedback` / `agent.question` payloads.
    - State explicitly that `agent_id` is the **slugified** form of the branch name, and name the conversion function as `slugify_branch`.
    - Describe the slugify rule's effect (lowercase, non-`[a-z0-9_]` chars become `-`, consecutive `-` collapse to one, empty fallback to `agent`) so a reader can predict the conversion without reading the source.
    - State which form to use in which context: the `agent_id` form in every broker payload `target` field; the branch-name form in git operations.
12. Include a `### Stash hygiene` section (or equivalent heading) that instructs the agent how to safely handle stashes in a multi-worktree environment. The section SHALL contain three rules in order:
    - **List before pop** — always run `git stash list` first.
    - **Inspect before pop** — use `git stash show -p stash@{N}` to inspect any candidate entry's patch contents before popping.
    - **Pop only your own** — only pop stash entries you authored on the current worktree; if authorship is uncertain, leave the stash alone and escalate via `agent.question` rather than risk a destructive pop.

    The section SHALL state that `git stash pop` SHOULD NOT be run blindly. The section MAY include a cautionary narrative referencing a real dogfood incident where a blind pop wiped in-flight work.
13. NOT hardcode a specific commit-MESSAGE format as a mandatory convention. The "Commit cadence" section SHALL defer message format to the host project's injected `AGENTS.md` (e.g. "follow the project's commit-message conventions; see the project's `AGENTS.md`") rather than prescribe Conventional Commits (`feat(<scope>):`, `fix(<scope>):`, …) as the required format. A Conventional-Commits prefix MAY still appear as an illustrative example, but the prose SHALL NOT state that the agent MUST use that format — message format is a per-project convention, not a git-paw-bundled-skill rule.

#### Scenario: Coordination skill documents automatic status publishing

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains text indicating that `agent.status` publishing is automatic
- **AND** it does NOT contain the substring "MUST publish agent.status"

#### Scenario: Coordination skill retains blocked and artifact curl examples

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains a `curl` example for publishing `agent.blocked`
- **AND** it contains a `curl` example for publishing `agent.artifact`

#### Scenario: Coordination skill directs peer dependencies to escalation, not cherry-pick

- **WHEN** the embedded coordination skill is inspected
- **THEN** it does NOT contain the substring `git cherry-pick`
- **AND** it does NOT contain a `Cherry-pick peer commits` heading
- **AND** it contains guidance directing an agent that depends on a peer's work to publish `agent.blocked` naming the peer and what it needs

#### Scenario: Coordination skill explains why grafting a peer commit is unsafe

- **WHEN** the embedded coordination skill's peer-dependency section is inspected
- **THEN** it states that the agent MUST NOT cherry-pick a peer's commit into its own branch
- **AND** it explains that doing so duplicates a commit the supervisor will later merge from the peer's own branch, leaving the agent's branch divergent and unmergeable
- **AND** it directs the agent to the `agent.advanced-main` discipline for taking on peer work after it lands on the base branch

#### Scenario: Coordination skill retains supervisor-side integration language

- **WHEN** the embedded coordination skill's terminal-action guidance is inspected
- **THEN** it still describes the supervisor as the party that integrates the agent's branch onto the release line
- **AND** the removal of agent-side cherry-pick guidance does NOT remove the statement that verification and archival belong to the supervisor

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

#### Scenario: Coordination skill instructs working heartbeat every five tool uses

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains a heading or sub-section naming the working heartbeat (e.g. `Working heartbeat`, `Heartbeat`, or equivalent)
- **AND** it contains the literal cadence text `every 5 tool uses` (or substantively equivalent text naming the number `5`)
- **AND** it contains a `curl` example that publishes an `agent.status` message with `status` set to `"working"`

#### Scenario: Coordination skill explains why heartbeats supplement the filesystem watcher

- **WHEN** the embedded coordination skill's working-heartbeat section is inspected
- **THEN** it explains that the filesystem watcher does not see read-only tool uses, permission-prompt waits, or LLM-only deliberation
- **AND** it states that the heartbeat keeps `last_seen` fresh during active work that does not touch files

#### Scenario: Coordination skill documents agent_id vs branch slugify rule

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains a heading or sub-section naming references / terminology (e.g. `References & terminology`, `Terminology`, or equivalent)
- **AND** it contains the substring `agent_id`
- **AND** it contains the substring `slugify_branch`
- **AND** it instructs the agent to use the `agent_id` (dashed) form in broker `target` fields and the branch-name (slashed) form in git operations
- **AND** it describes the slugify rule sufficiently for a reader to predict the conversion (lowercase, non-`[a-z0-9_]` chars to `-`, collapse, fallback to `agent` on empty)

#### Scenario: Coordination skill documents stash hygiene rules

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains a heading or sub-section naming stash hygiene (e.g. `Stash hygiene`, `Stash safety`, or equivalent)
- **AND** it contains the substring `git stash list`
- **AND** it contains the substring `git stash show -p`
- **AND** it instructs the agent to pop only stash entries the agent authored on the current worktree
- **AND** it states that `git stash pop` SHOULD NOT be run blindly (or substantively equivalent language)

#### Scenario: Coordination skill defers commit-message format to the project AGENTS.md

- **WHEN** the embedded coordination skill's "Commit cadence" section is inspected
- **THEN** it instructs the agent to follow the host project's commit-message conventions and references the project's `AGENTS.md`
- **AND** it does NOT state that the agent MUST use a specific commit-message format (e.g. it does NOT prescribe Conventional Commits as mandatory)
