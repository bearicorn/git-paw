# opsx-role-gating Specification

## Purpose
TBD - created by archiving change opsx-role-gating. Update Purpose after archive.
## Requirements
### Requirement: Role-gating is scoped to the OpenSpec spec engine

The role-gating capability SHALL apply only when the session's spec
engine is OpenSpec. This covers both the post-commit archive-activity
guard and the forbidden-command skill sections. The system MUST treat
the configured spec engine (`[specs] type`) as the gate: a value of
`"openspec"` activates the capability, and any other value disables it.

The rationale is that the `/opsx:verify` and `/opsx:archive` commands,
and the `openspec/changes/` / `openspec/specs/` archive-activity diff
shapes, exist only under the OpenSpec engine; the Spec Kit and markdown
engines have no equivalent commands or paths, so applying the guard or
rendering the skill sections there would be meaningless and misleading.

When `[specs] type` is `"speckit"` or `"markdown"` (or any non-OpenSpec
engine), the system SHALL NOT render the forbidden-command skill
sections described by the skill requirements below, and the post-commit
archive-activity guard SHALL be inactive (it SHALL NOT classify,
warn, or block on any commit) regardless of the configured warn/block
mode. The other requirements in this capability describe behaviour that
applies only under the OpenSpec engine.

#### Scenario: Non-OpenSpec engine omits the skill sections

- **GIVEN** a session configured with `[specs] type = "speckit"` (or
  `"markdown"`)
- **WHEN** the bundled coordination and supervisor skills are rendered
- **THEN** the `/opsx:verify` / `/opsx:archive` forbidden-command
  sections SHALL be omitted from both skills

#### Scenario: Non-OpenSpec engine disables the guard

- **GIVEN** a session configured with `[specs] type = "markdown"` and
  role-gating mode set to `block`
- **WHEN** any worktree commit lands (including one whose diff touches
  `openspec/`-shaped paths)
- **THEN** the post-commit guard SHALL NOT classify it as archive
  activity and SHALL NOT warn or revert

#### Scenario: OpenSpec engine activates the capability

- **GIVEN** a session configured with `[specs] type = "openspec"`
- **WHEN** the skills are rendered and worktree commits land
- **THEN** the forbidden-command skill sections SHALL be present and
  the post-commit guard SHALL behave as the remaining requirements
  specify

### Requirement: Coding-agent skill names the forbidden commands

The bundled coordination skill SHALL include an explicit
"Commands you must not run" section enumerating
`/opsx:verify` and `/opsx:archive` with a one-paragraph
rationale describing the spec-lifecycle harm and a reference
to the role-gating guard.

#### Scenario: Coordination skill lists the forbidden commands

- **WHEN** the bundled `coordination.md` is inspected
- **THEN** it SHALL contain a section listing both
  `/opsx:verify` and `/opsx:archive` as supervisor-only,
  paired with a rationale paragraph and a reference to the
  role-gating guard

### Requirement: Supervisor skill restates the role boundary

The bundled supervisor skill SHALL include a "Commands you
must run (not coding agents)" section restating the
supervisor-only contract with MUST / MUST NOT framing. The
section SHALL instruct the supervisor to call out observed
violations via `agent.feedback`.

#### Scenario: Supervisor skill carries the must/must-not framing

- **WHEN** the bundled `supervisor.md` is inspected
- **THEN** it SHALL contain a section using MUST / MUST NOT
  framing for the two commands and SHALL include an
  instruction to call out coding-agent violations via
  `agent.feedback`

### Requirement: Post-commit archive-activity detection

The system SHALL detect archive-activity commits via a
post-commit watcher hook. The detection SHALL classify a
commit as archive activity when EITHER the commit message
matches the canonical archive pattern OR the diff includes
moves into `openspec/changes/archive/<name>/` and/or
additions to `openspec/specs/<capability>/spec.md`.

#### Scenario: Canonical archive commit message is detected

- **GIVEN** a commit whose message matches
  `chore(specs): archive <name>; sync deltas to main specs`
- **WHEN** the post-commit watcher runs
- **THEN** the system SHALL classify the commit as archive
  activity

#### Scenario: Diff-shape detection catches a non-canonical message

- **GIVEN** a commit whose message does not match the
  canonical pattern but whose diff moves files into
  `openspec/changes/archive/feat-x/` and updates
  `openspec/specs/<capability>/spec.md`
- **WHEN** the post-commit watcher runs
- **THEN** the system SHALL classify the commit as archive
  activity via the diff-shape signal

### Requirement: Agent attribution determines violation

The system SHALL resolve the publishing worktree's
`agent_id` from the session JSON when archive activity is
detected. The system SHALL treat the activity as a violation
when the resolved `agent_id` is anything other than
`"supervisor"`. The system SHALL treat unresolvable
worktrees as coding-agent activity (conservative default).

#### Scenario: Supervisor's own archive is not a violation

- **GIVEN** the supervisor commits an archive
- **WHEN** the watcher runs
- **THEN** the system SHALL NOT classify the activity as a
  violation (agent_id == "supervisor")

#### Scenario: Coding agent's archive is a violation

- **GIVEN** a coding-agent worktree (agent_id != "supervisor")
  commits an archive
- **WHEN** the watcher runs
- **THEN** the system SHALL classify the activity as a
  violation

#### Scenario: Unresolvable worktree treated as violation

- **GIVEN** a commit on a worktree the session JSON does
  not list
- **WHEN** the watcher runs
- **THEN** the system SHALL treat the activity as a
  violation (conservative default)

### Requirement: Warn-mode default behaviour

The system SHALL provide a `warn` mode that publishes an
`agent.feedback` to the offending agent AND publishes an
`agent.learning` record with category
`permission_pattern`. The system SHALL set `warn` as the
default mode when no `[opsx].role_gating` config is
present.

#### Scenario: Warn mode publishes feedback and learning

- **GIVEN** a violation detected in warn mode
- **WHEN** the guard fires
- **THEN** the broker SHALL receive both an
  `agent.feedback` targeting the violator and an
  `agent.learning` with category `permission_pattern`

#### Scenario: No config selects warn mode by default

- **GIVEN** a `.git-paw/config.toml` with no
  `[opsx]` section
- **WHEN** a violation occurs
- **THEN** the system SHALL apply warn-mode behaviour

### Requirement: Block-mode revert flow

The system SHALL provide a `block` mode that performs the
warn-mode actions AND additionally publishes an
`agent.feedback` targeted at `"supervisor"` requesting the
violator's commit be reverted. The system SHALL route the
revert through the supervisor's existing merge-orchestration
skill rather than executing the revert directly from
git-paw code.

#### Scenario: Block mode signals the supervisor to revert

- **GIVEN** a violation detected in block mode
- **WHEN** the guard fires
- **THEN** the broker SHALL receive a feedback targeted at
  `"supervisor"` requesting revert of the offending commit,
  in addition to the warn-mode feedback + learning

#### Scenario: Block mode does not directly mutate git

- **WHEN** the block-mode guard fires
- **THEN** git-paw code SHALL NOT directly run
  `git revert` or any equivalent destructive command — the
  revert action SHALL be the supervisor agent's
  responsibility per its merge-orchestration skill

### Requirement: Off-mode disables the guard

The system SHALL provide an `off` mode that disables the
guard entirely. In off mode the system SHALL NOT publish
feedback or learnings on archive activity regardless of
agent role.

#### Scenario: Off mode produces no broker traffic on violations

- **GIVEN** `[opsx].role_gating = "off"`
- **WHEN** a coding-agent archive commit lands
- **THEN** the system SHALL produce no feedback or
  learning broker messages from the guard

### Requirement: Diagnosable warning text

The warning text published by the guard SHALL identify the
commit (short SHA), the violating agent_id, and the
classification reason (which signal triggered — message
match vs. diff shape). The text SHALL be plain-prose
enough that the user can identify false positives at a
glance.

#### Scenario: Warning identifies the trigger reason

- **WHEN** the guard fires on a canonical-message commit
- **THEN** the feedback text SHALL include the short SHA,
  the agent_id, and a line identifying the trigger as the
  commit-message match

#### Scenario: Diff-shape trigger names the diff signal

- **WHEN** the guard fires on a diff-shape commit (no
  matching message)
- **THEN** the feedback text SHALL identify the trigger as
  the diff-shape signal and name at least one of the
  detected paths

