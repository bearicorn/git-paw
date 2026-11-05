## ADDED Requirements

### Requirement: Spec read tools serve each format's own files

The spec read tools SHALL read each spec from the location its format's backend reports. `get_specs` and `get_spec` SHALL report that location as `path`, relative to the repository root. `get_spec` SHALL return the artifacts found there. `get_tasks` SHALL parse the format's own tasks file. `title` SHALL be the first `# ` heading of the title file, else of the entry prompt, else the entry id. An entry its backend cannot locate SHALL NOT be served.

#### Scenario: A title falls back to the prompt heading, then the id

- **GIVEN** `[specs] type = "openspec"`, a change `add-auth` whose `proposal.md` starts with `## Why` and whose `tasks.md` starts with `# Add auth tasks`, and a change `bare` with no `# ` heading in either file
- **WHEN** the MCP client calls `get_specs()`
- **THEN** the `add-auth` title SHALL be `Add auth tasks` and the `bare` title SHALL be `bare`

#### Scenario: A Spec Kit task entry is served from its feature directory

- **GIVEN** `[specs] type = "speckit"`, `dir = "specs"`, and a feature `specs/001-login/` with `spec.md`, `plan.md`, a checklist and a phased `tasks.md` whose current phase yields the entry `001-login-T002`
- **WHEN** the MCP client calls `get_specs()`, `get_spec({ "id": "001-login-T002" })` and `get_tasks({ "spec": "001-login-T002" })`
- **THEN** the entry's `path` SHALL be `specs/001-login`
- **AND** `get_spec` SHALL return the `spec`, `plan` and `tasks` artifacts and the checklist
- **AND** `get_tasks` SHALL list every task in `specs/001-login/tasks.md` with its id, phase, `[P]` marker and completion state

#### Scenario: A Markdown spec is served from its file

- **GIVEN** `[specs] type = "markdown"`, `dir = "specs"`, and a pending `specs/fix-bug.md` whose body is `# Fix the bug` followed by `- [ ] reproduce`
- **WHEN** the MCP client calls `get_specs()` and `get_tasks({ "spec": "fix-bug" })`
- **THEN** the entry's `path` SHALL be `specs/fix-bug.md` and its `title` SHALL be `Fix the bug`
- **AND** `get_tasks` SHALL return one incomplete task whose description is `reproduce`

#### Scenario: A Superpowers plan is served from its file with its own title

- **GIVEN** `[specs] type = "superpowers"`, `dir = "plans"`, and an incomplete plan `plans/2026-07-20-add-auth.md` whose first heading is `# Add auth Implementation Plan`
- **WHEN** the MCP client calls `get_specs()` and `get_tasks({ "spec": "2026-07-20-add-auth" })`
- **THEN** the entry's `path` SHALL be `plans/2026-07-20-add-auth.md`
- **AND** its `title` SHALL be `Add auth Implementation Plan`
- **AND** `get_tasks` SHALL return the plan's `- [ ]` and `- [x]` steps with their completion state

#### Scenario: An OpenSpec change is served as before

- **GIVEN** `[specs] type = "openspec"`, `dir = "openspec/changes"`, and a change `add-auth` with `proposal.md`, `design.md`, `tasks.md` and a delta spec
- **WHEN** the MCP client calls `get_specs()`, `get_spec({ "id": "add-auth" })` and `get_tasks({ "spec": "add-auth" })`
- **THEN** the entry's `path` SHALL be `openspec/changes/add-auth`, its `title` the proposal's first heading
- **AND** `get_spec` SHALL return the proposal, design, tasks and delta-spec artifacts, and `get_tasks` the checkbox tasks of `tasks.md`

#### Scenario: Every reported path exists

- **GIVEN** one fixture spec for each of the four formats
- **WHEN** `get_specs()` reports each entry's `path`
- **THEN** that path, joined to the repository root, SHALL name an existing file or directory
