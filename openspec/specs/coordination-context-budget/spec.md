# coordination-context-budget Specification

## Purpose
TBD - created by archiving change coordination-context-budget. Update Purpose after archive.
## Requirements
### Requirement: Context budget section in coordination skill

The bundled coordination skill SHALL include a "Context budget"
section teaching agents how to manage their context window
across a task. The section SHALL be placed after the existing
"While you're editing" section. The section SHALL cover three
topics: a residual-budget heuristic, named compact / clear /
summarise moments, and a commit-before-compact discipline.

#### Scenario: Section exists with the three topics

- **WHEN** the bundled `coordination.md` is inspected
- **THEN** the file SHALL contain a "Context budget" heading
  with subsections covering the residual-budget heuristic,
  the named moments, and the commit-before-compact discipline

#### Scenario: Section placement after "While you're editing"

- **WHEN** the section ordering in coordination.md is checked
- **THEN** the "Context budget" section SHALL appear after the
  v0.5.0 "While you're editing" section

### Requirement: Residual-budget heuristic

The skill SHALL state that agents target at least ~60% of the
model's context window free for task work after boot blocks,
skill prose, and governance docs have loaded. The skill SHALL
phrase this as a heuristic in prose form, not as a numeric
config value.

#### Scenario: Heuristic stated in prose

- **WHEN** the "Context budget" section is read
- **THEN** the residual-budget heuristic SHALL be phrased as
  prose guidance referencing the target ratio (≥60% free
  post-boot), and the same content SHALL NOT introduce a new
  configuration field

### Requirement: Three named moments to compact / clear / summarise

The skill SHALL name three moments at which the agent reaches
for compact / clear / summarise, in priority order:
1. After each spec scenario completes (compact)
2. When the working set grows past ~40% of the window
   (compact)
3. When switching between sub-tasks that don't share state
   (clear)

The skill SHALL teach the agent to consult them top-to-bottom
and reach for the first applicable moment.

#### Scenario: Three moments documented in priority order

- **WHEN** the "Context budget" section is read
- **THEN** the three moments SHALL be listed in the documented
  order with their associated action (compact vs clear) labelled
  per moment

### Requirement: Commit-before-compact discipline

The skill SHALL state explicitly that compact / clear /
summarise operations MUST be preceded by a commit OR an
`agent.artifact` publish. The skill SHALL include language
explaining the safety rationale (compact reduces context; work
that isn't recorded in git or the broker can be lost).

#### Scenario: Discipline stated explicitly with safety rationale

- **WHEN** the "Context budget" section is read
- **THEN** the discipline SHALL appear as a clearly-marked
  statement (e.g. in bold or as a separate paragraph), and SHALL
  pair the rule with a one-sentence rationale about why the
  ordering matters

### Requirement: Per-CLI compact mechanism table

The skill SHALL include a small table identifying the
compact / clear mechanism per supported CLI. The table SHALL
include explicit entries for `claude` and `claude-oss` and a
generic "other" row directing agents to look for their CLI's
equivalent.

#### Scenario: Table includes claude and claude-oss explicitly

- **WHEN** the per-CLI table in the section is read
- **THEN** the table SHALL contain one row for `claude` and one
  for `claude-oss`, each naming the compact and clear command
  slash forms

#### Scenario: Generic "other" row points users at their CLI's equivalent

- **WHEN** the per-CLI table is read by a user on a CLI not
  explicitly listed
- **THEN** the table SHALL include a fallback row directing
  them to look for the CLI's `/compact`, `/save`, or `/reset`
  equivalent

### Requirement: Stack-agnostic phrasing

The new section SHALL pass the no-language-leak audit from
[[lang-agnostic-assets]]. The section SHALL NOT use
Rust-specific or any other stack-specific language in its
prose or examples.

#### Scenario: No-leak audit passes against the new section

- **WHEN** the no-leak audit is run after the section lands
- **THEN** the audit SHALL pass on the rendered coordination
  skill across all supported spec backends

