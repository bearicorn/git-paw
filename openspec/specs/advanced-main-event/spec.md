# advanced-main-event Specification

## Purpose
Defines the `agent.advanced-main` broker event the supervisor publishes after every successful merge to the main branch, plus the supervisor and coding-agent skill guidance that surrounds it. This tells running agents when main has advanced so they can decide whether to rebase — without auto-rebasing — and lets the dashboard surface the merge stream.

## Requirements
### Requirement: agent.advanced-main broker variant

The broker SHALL accept and route an `agent.advanced-main`
message variant. Each message SHALL carry the fields `from`,
`merged_branch`, `new_main_sha`, `base`, `merged_at`, and an
optional `summary`. The broker SHALL NOT validate the SHA's
existence or shape beyond its presence as a string.

#### Scenario: Broker accepts a well-formed advanced-main message

- **WHEN** the supervisor publishes an `agent.advanced-main`
  message with all required fields populated
- **THEN** the broker SHALL accept the message and SHALL
  include it in subsequent `/messages/<branch_id>` poll
  responses for every registered agent

#### Scenario: Missing required field is rejected

- **WHEN** a publish omits `merged_branch`, `new_main_sha`,
  `base`, or `merged_at`
- **THEN** the broker SHALL return a 400-class error
  identifying the missing field

#### Scenario: Optional summary is preserved when present

- **WHEN** the publisher includes a `summary` value
- **THEN** the routed message SHALL preserve the summary
  verbatim

### Requirement: Deterministic id for advanced-main events

The system SHALL produce a deterministic `id` for each
`agent.advanced-main` record using the same hashing pattern as
`agent.learning` from [[agent-learning-variant]]. The canonical
input SHALL include `merged_branch`, `new_main_sha`, `base`,
and the UTC hour bucket. Re-publishing the same merge within
the same hour SHALL produce an identical id.

#### Scenario: Same merge within the hour produces identical ids

- **WHEN** the supervisor publishes the same merge twice
  within a UTC hour
- **THEN** both broker messages SHALL carry identical `id`
  values

### Requirement: Supervisor publishes on merge to main

The bundled supervisor skill SHALL teach the LLM to publish an
`agent.advanced-main` event after every successful `git merge`
targeting `main` (or the configured `[git] main_branch`). The
skill SHALL include a concrete curl invocation example with
the message shape from this capability.

#### Scenario: Skill prose names the publish trigger explicitly

- **WHEN** the merge-orchestration section of supervisor.md is
  read
- **THEN** the prose SHALL include a publish step that fires
  immediately after a successful merge to main, with a
  concrete curl-to-`/publish` example

#### Scenario: Publish includes the resolved base name

- **WHEN** the skill prose shows the publish step
- **THEN** the `base` field SHALL be documented as the
  resolved `[git] main_branch` value, not hardcoded `"main"`

### Requirement: Agent skill teaches polling discipline

The bundled coordination skill SHALL include a "When main
advances" subsection teaching coding agents:
1. The event arrives on their normal
   `/messages/<branch_id>` poll
2. They SHALL NOT auto-rebase on receipt
3. The recommended decision process is fetch + inspect +
   decide
4. Any rebase action SHALL be preceded by a commit or stash
   to prevent loss

#### Scenario: Skill includes the four-step discipline

- **WHEN** the "When main advances" subsection is read
- **THEN** the prose SHALL contain the four items: polling
  source, no-auto-rebase rule, fetch+inspect+decide flow,
  and the commit-or-stash-first safety rule

#### Scenario: Skill explicitly forbids auto-rebase

- **WHEN** the polling-discipline subsection is read
- **THEN** the prose SHALL contain explicit "do not auto-
  rebase" language with a one-sentence safety rationale

### Requirement: Variant flows through dashboard automatically

The dashboard's [[dashboard-broker-log]] panel SHALL render
`agent.advanced-main` events without any code change to the
log panel — the existing watcher feed delivers the variant to
the ring buffer like any other message type. The filter-chip
bitmask SHALL gain a bit position for the new variant so
users can isolate the event stream.

#### Scenario: Advance event appears in the broker log

- **GIVEN** the dashboard's broker log panel is visible
- **WHEN** the supervisor publishes an `agent.advanced-main`
- **THEN** the event SHALL appear at the top of the panel
  within one frame tick, with the new variant's filter chip
  available in the header

### Requirement: Cross-reference with supervisor introspection

The publish trigger SHALL coordinate with
[[supervisor-introspection]] such that the `phase = "merge"`
status emitted before the merge and the `agent.advanced-main`
event emitted after a successful merge SHARE the
`merged_branch` value. This lets consumers correlate the two
events.

#### Scenario: Phase merge status and advance event share merged_branch

- **WHEN** the supervisor completes a successful merge of
  branch `feat/x`
- **THEN** the supervisor's preceding `phase = "merge"`
  status SHALL have `detail.branch == "feat/x"` and the
  resulting `agent.advanced-main` event SHALL have
  `merged_branch == "feat/x"`

### Requirement: Stack-agnostic phrasing

The new supervisor and coordination skill content SHALL pass
the no-language-leak audit from [[lang-agnostic-assets]]. The
content SHALL NOT use Rust-specific or any other stack-
specific language in its prose or examples.

#### Scenario: No-leak audit passes after the prose lands

- **WHEN** the no-leak audit runs against the updated
  supervisor.md and coordination.md
- **THEN** the audit SHALL pass on the rendered skills across
  all supported spec backends

