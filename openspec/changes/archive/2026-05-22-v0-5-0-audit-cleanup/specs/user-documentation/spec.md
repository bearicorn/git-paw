## ADDED Requirements

### Requirement: AGENTS.md dependency table SHALL list v0.5.0 dependencies and note `dirs` as intentionally absent

The top-level `AGENTS.md` "Dependencies" table SHALL include rows for `schemars`, `serde_yaml`, `chrono`, and `regex` — the four dependencies added during the v0.5.0 cycle. Each row's "Purpose" cell SHALL be one sentence (e.g. `schemars` → "JSON Schema derivation for governance config validation").

The `dirs` row SHALL be moved out of the approved-dependencies table and into a "Notable exclusions" sub-section beneath the table. The exclusion entry SHALL state: "Replaced by a homegrown `src/dirs.rs` because the upstream crate's license is not FOSS-compatible. Do not re-add."

#### Scenario: AGENTS.md lists schemars in dependency table

- **WHEN** `AGENTS.md` is read
- **THEN** the dependencies table SHALL contain a row with `schemars` in the Crate column
- **AND** the Purpose column for that row SHALL mention JSON Schema or governance config

#### Scenario: AGENTS.md lists serde_yaml, chrono, regex

- **WHEN** the dependencies table is inspected
- **THEN** rows for `serde_yaml`, `chrono`, and `regex` SHALL exist
- **AND** each row's Purpose cell SHALL be non-empty

#### Scenario: AGENTS.md notes `dirs` as intentionally absent

- **WHEN** `AGENTS.md` is read
- **THEN** the `dirs` row SHALL NOT appear in the approved-dependencies table
- **AND** a separate "Notable exclusions" section SHALL describe `dirs` as replaced by `src/dirs.rs` due to a non-FOSS license

### Requirement: AGENTS.md commit-conventions scopes SHALL include v0.5.0 scope names and document the compound-scope form

The "Commit Conventions" section of `AGENTS.md` SHALL list the following scope names in addition to the v0.4 set: `user-guide`, `worktree`, `governance`, `learnings`, `pause`. The section SHALL also document the compound-scope syntax `(scope1,scope2,...)` with at least one inline example (e.g. `feat(cli,config): ...`).

#### Scenario: Scopes line includes v0.5 names

- **WHEN** the AGENTS.md Scopes line is inspected
- **THEN** it SHALL contain each of `user-guide`, `worktree`, `governance`, `learnings`, `pause` (as inline-code-style backtick names)

#### Scenario: Compound-scope syntax is documented

- **WHEN** the Commit Conventions section is inspected
- **THEN** there SHALL be at least one example using compound-scope syntax `(scope1,scope2)`
- **AND** the section SHALL state that compound scopes are permitted when a commit legitimately touches more than one scope

### Requirement: `docs/src/user-guide/supervisor.md` SHALL consolidate v0.5.0 supervisor surfaces

The user-guide supervisor chapter SHALL include the following subsections (or equivalent headings) introduced in v0.5.0:

1. **Spec audit governance sub-step** — references `docs/src/user-guide/governance.md` and the five doc-checklist examples (DoD, ADR, security, test-strategy, constitution).
2. **Common dev-command allowlist** — describes the preset, opt-out via `[supervisor.common_dev_allowlist].enabled = false`, and the `extra` field; cross-links to `docs/src/configuration/README.md`.
3. **Repo-configurable gate commands** — names the six `[supervisor]` gate-command keys (`test_command`, `lint_command`, `build_command`, `doc_build_command`, `spec_validate_command`, `fmt_check_command`, `security_audit_command`) and the `(not configured)` graceful-skip behaviour; cross-links to `docs/src/configuration/README.md`.
4. **Broker-side conflict detector** — names the three failure shapes (forward, in-flight, ownership) and the `[conflict-detector]` token; cross-links to `docs/src/user-guide/conflict-detection.md`.
5. **Learnings aggregator** — at minimum a one-line cross-link to `docs/src/user-guide/learnings.md`.
6. **When the user types in your pane** — mirrors the bundled-skill section of the same name, covering status questions, directives, and judgement-call asks.
7. **Merge orchestration** — mirrors the bundled-skill section, covering the topological order from `agent.blocked` events, per-branch `git merge --ff-only` + test loop, cycle handling.

#### Scenario: Supervisor user-guide names governance sub-step + cross-link

- **WHEN** `docs/src/user-guide/supervisor.md` is inspected
- **THEN** the content SHALL contain a heading or paragraph naming the governance sub-step inside spec audit
- **AND** SHALL link to `docs/src/user-guide/governance.md`

#### Scenario: Supervisor user-guide names the common dev-command allowlist

- **WHEN** the file is inspected
- **THEN** it SHALL contain a section with a heading approximately "Common dev-command allowlist" or equivalent
- **AND** SHALL mention `[supervisor.common_dev_allowlist]` and the `extra` field

#### Scenario: Supervisor user-guide names the gate-command templating

- **WHEN** the file is inspected
- **THEN** it SHALL contain prose stating that supervisor skill gate commands are repo-configurable
- **AND** SHALL name at least three of the six new `[supervisor]` gate-command keys
- **AND** SHALL mention the `(not configured)` graceful-skip behaviour

#### Scenario: Supervisor user-guide names the broker-side conflict detector

- **WHEN** the file is inspected
- **THEN** it SHALL contain a section describing the broker-side conflict detector
- **AND** SHALL name the three failure shapes (forward, in-flight, ownership)
- **AND** SHALL link to `docs/src/user-guide/conflict-detection.md`

#### Scenario: Supervisor user-guide cross-links the learnings aggregator chapter

- **WHEN** the file is inspected
- **THEN** the content SHALL include a link to `docs/src/user-guide/learnings.md`

#### Scenario: Supervisor user-guide mirrors "When the user types in your pane"

- **WHEN** the file is inspected
- **THEN** the content SHALL include a section approximately named "When the user types in your pane" (or substantively equivalent)
- **AND** SHALL describe at least the three categories of user input (status question, directive, judgment-call ask)

#### Scenario: Supervisor user-guide mirrors "Merge orchestration"

- **WHEN** the file is inspected
- **THEN** the content SHALL include a section describing supervisor-driven merge orchestration
- **AND** SHALL mention the topological order derived from `agent.blocked` events
- **AND** SHALL mention `git merge --ff-only` as the per-branch merge command

### Requirement: `docs/src/user-guide/coordination.md` SHALL mirror the skill's editing-phase structure

The user-guide coordination chapter SHALL include a section that mirrors the bundled `assets/agent-skills/coordination.md`'s "Before you start editing" and "While you're editing" phased structure. The mirrored section SHALL include:

- **Before you start editing**: read the spec, publish `agent.intent` listing files + summary + TTL, poll once for warnings, decide between wait/split/escalate on overlap.
- **While you're editing**: re-publish intent if scope grows; on a peer's intent for a same-module file, send `agent.question` rather than racing; MUST NOT pairwise check-ins, MUST NOT wait for explicit go-ahead, MUST NOT block on broker silence.

#### Scenario: Coordination user-guide mirrors the editing phases

- **WHEN** `docs/src/user-guide/coordination.md` is inspected
- **THEN** the content SHALL contain a heading approximately "Workflow phases" or "Before you start editing" or substantively equivalent
- **AND** the section SHALL describe `agent.intent` publishing as the pre-edit step
- **AND** SHALL describe re-publishing on scope growth as the mid-edit rule
