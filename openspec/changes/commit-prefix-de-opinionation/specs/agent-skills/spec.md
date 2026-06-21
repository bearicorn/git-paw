## MODIFIED Requirements

### Requirement: Coordination skill SHALL teach per-group commit cadence

The embedded `assets/agent-skills/coordination.md` skill SHALL contain a section (heading text approximately "Commit cadence" or "Per-group commit cadence") that instructs the coding agent to commit after completing each numbered task group (e.g. `## 1.`, `## 2.`) in the change's `tasks.md`. The section SHALL state:

1. The default unit of commit is the task GROUP, not the individual task. After all `- [ ]` items in a group are `- [x]`, the agent SHALL commit before starting the next group.
2. The agent SHALL NOT accumulate more than approximately ten uncommitted files at a time. If a single group's implementation produces more uncommitted files than that, the agent SHALL split into multiple commits using suffixes like `(part 1 of 2)`.
3. The commit-MESSAGE format SHALL defer ENTIRELY to the host project's injected `AGENTS.md` rather than be mandated, defaulted, or illustrated by the bundled skill. The section SHALL instruct the agent to follow the project's own commit-message conventions (e.g. "follow the project's commit-message conventions; see the project's `AGENTS.md`"). The bundled skill SHALL NOT present a Conventional-Commits prefix (`feat(<scope>):`, `fix(<scope>):`, …) as git-paw's example, default, or recommendation — Conventional Commits is git-paw's OWN repo convention (it belongs in git-paw's `AGENTS.md`, not in the asset the binary exports to every consumer). Any commit example the section needs (e.g. to demonstrate the `(part N of M)` split mechanism) SHALL use a FORMAT-NEUTRAL subject with no convention-specific prefix. This is the bundled-skill side of the "what the binary exports vs what is git-paw-repo-specific" separation, and keeps the requirement consistent with the "Embedded coordination skill" requirement (item 13) and the `lang-agnostic-skills` convention-neutrality audit.
4. The rationale: per-group commits protect against agent crashes, conflict mediation, and `/clear` resets losing unbounded work; they also map cleanly to the post-commit hook's `agent.artifact{status:committed}` event sequence the supervisor uses for verification.

#### Scenario: Coordination skill names the per-group cadence

- **WHEN** the embedded `coordination.md` skill is inspected
- **THEN** the content SHALL contain a heading naming the commit-cadence concept (e.g. "Commit cadence", "Per-group commit cadence", or substantively equivalent)
- **AND** the section's body SHALL mention the GROUP grain (i.e. the substring "group" or "section" appears at least once)
- **AND** SHALL name the ~10-file soft cap on uncommitted work

#### Scenario: Coordination skill defers commit-message format to the project AGENTS.md

- **WHEN** the commit-cadence section is inspected
- **THEN** it SHALL instruct the agent to follow the host project's commit-message conventions and SHALL reference the project's `AGENTS.md` as the source of the format rules
- **AND** it SHALL NOT state that the agent MUST use a specific commit-message format (it SHALL NOT prescribe Conventional Commits as the mandatory format)
- **AND** it SHALL NOT present a Conventional-Commits prefix (e.g. `feat(<scope>):`) as git-paw's example, default, or recommendation — any commit example shown SHALL use a format-neutral subject with no convention-specific prefix

#### Scenario: Per-group cadence and releasable-unit discipline remain unchanged

- **WHEN** the commit-cadence section is inspected
- **THEN** it SHALL still instruct the agent to commit per task group with the approximately-ten-uncommitted-file soft cap and `(part N of M)` split guidance
- **AND** the releasable-unit / `git commit --amend` fixup discipline (defined by the "Coordination skill — releasable-unit commit discipline with amend fixups" requirement) SHALL remain present and unaffected by this change
