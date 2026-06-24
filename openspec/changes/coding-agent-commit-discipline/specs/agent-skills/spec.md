# agent-skills Specification — delta for coding-agent-commit-discipline

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

## ADDED Requirements

### Requirement: Coordination skill — stand-by after final commit

The embedded `coordination.md` skill SHALL include a positive "stand by after your
final commit" protocol that tells the coding agent what to do *instead* of reaching
for verification or archive once its work is committed. This complements the existing
`opsx-role-gating` forbidden-commands block (which forbids `/opsx:verify` and
`/opsx:archive` from a coding-agent worktree) by supplying the actionable next step.
The protocol SHALL:

1. Instruct the agent that after its final commit it SHALL **stand by**: rely on the
   automatic `agent.artifact { status: "committed" }` publish (or publish a manual
   `agent.artifact { status: "done" }` for code-less work) and then **wait**.
2. State that while standing by the agent SHALL NOT run `/opsx:verify` or
   `/opsx:archive` — these are supervisor-only — and SHALL cross-reference the
   role-gating guidance rather than restate the full forbidden-commands list.
3. State what the agent waits *for*: an `agent.verified`, `agent.feedback`, or
   further `agent.intent` from the supervisor; on `agent.feedback` the agent fixes the
   listed errors and re-publishes `agent.artifact`.

#### Scenario: Coordination skill instructs standing by after the final commit

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains explicit guidance to stand by (wait) after the final commit rather than proceed to verification or archive
- **AND** it states that the agent waits for `agent.verified`, `agent.feedback`, or further `agent.intent` from the supervisor

#### Scenario: Stand-by protocol forbids self-verify and self-archive

- **WHEN** the stand-by guidance is inspected
- **THEN** it states the agent SHALL NOT run `/opsx:verify` or `/opsx:archive` while standing by
- **AND** it cross-references the supervisor-only / role-gating guidance rather than re-deriving its own enforcement

### Requirement: Coordination skill — releasable-unit commit discipline with amend fixups

The embedded `coordination.md` skill's "Commit cadence" section SHALL teach
releasable-unit commit discipline so agents stop producing micro-commit noise that the
supervisor must hand-squash at release. The section SHALL:

1. State that each commit MUST build and pass its own gates on its own — a commit is a
   releasable unit, not a checkpoint of in-progress work.
2. State that a small follow-up to the commit the agent *just* made (one that has not
   yet been verified and that the agent has not moved past) SHOULD be folded into that
   commit with `git commit --amend` rather than landed as a separate `fix typo` /
   `address feedback` micro-commit.
3. State the narrowing caveat: the agent SHALL NOT `--amend` an already-verified commit
   or an earlier commit from a previous group — amend applies ONLY to the
   most-recent, not-yet-verified commit.
4. Tie the rationale to git-paw's orchestration model: per-commit verification treats
   each committed artifact as a verification boundary, and the supervisor curates the
   release changelog from these commits, so a clean releasable-unit history avoids the
   manual squashes seen in prior release cycles.

#### Scenario: Commit cadence requires each commit to be a releasable unit

- **WHEN** the embedded coordination skill's "Commit cadence" section is inspected
- **THEN** it states that each commit must build and pass its own gates on its own (a releasable unit)

#### Scenario: Commit cadence prefers amend for just-made-commit fixups

- **WHEN** the "Commit cadence" section is inspected
- **THEN** it instructs the agent to fold a small follow-up to the just-made commit in with `git commit --amend` rather than add a separate micro-commit

#### Scenario: Commit cadence forbids amending verified or earlier commits

- **WHEN** the "Commit cadence" section is inspected
- **THEN** it states the agent SHALL NOT `git commit --amend` an already-verified commit or an earlier commit from a previous group
