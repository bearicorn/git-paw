## ADDED Requirements

### Requirement: Worktree entry records the spec it was launched from

The `WorktreeEntry` struct SHALL include an optional field `spec_id: Option<String>` recording the id of the `SpecEntry` an agent was launched from. Every launch path that creates an agent from a spec — the spec-driven launch with or without supervisor mode, and `git paw add --from-spec` — SHALL set it to that spec's id; a launch without a spec (bare `git paw start`, `git paw add <branch>`) SHALL leave it `None`. The field SHALL be annotated `#[serde(default, skip_serializing_if = "Option::is_none")]` so session files written before this field existed load with `None` and an entry without a spec serializes byte-identically to before.

#### Scenario: A spec-driven launch records each agent's spec id

- **GIVEN** a spec-driven launch of specs `003-user-list` and `004-error-handling`
- **WHEN** the session is saved and loaded back
- **THEN** each agent's worktree entry SHALL carry the `spec_id` of the spec it was launched from

#### Scenario: A launch without a spec records no spec id

- **GIVEN** a bare `git paw start` with explicit branches
- **WHEN** the session is saved and the JSON inspected
- **THEN** no worktree entry SHALL contain a `spec_id` key

#### Scenario: Older session files load with no spec id

- **GIVEN** a session JSON file whose worktree entries have no `spec_id` field
- **WHEN** the session is loaded
- **THEN** loading SHALL succeed without error
- **AND** every entry's `spec_id` SHALL be `None`
