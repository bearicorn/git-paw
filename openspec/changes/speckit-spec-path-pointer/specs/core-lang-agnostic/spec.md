## ADDED Requirements

### Requirement: Spec-path doctrine points Spec Kit workers at the Spec Kit spec location

The rendered spec-path doctrine (the `SPEC_PATH_DOCTRINE` skill substitution) SHALL name a Spec-Kit-specific spec location for a Spec Kit backend — the Spec Kit feature spec directory (`specs/<feature>/`) or the injected per-worktree sidecar — and SHALL NOT direct a Spec Kit worker to the OpenSpec `openspec/changes/<id>/` path, which does not exist under Spec Kit. When multiple backends are active, each backend's doctrine line SHALL name its own correct location.

#### Scenario: Spec Kit doctrine names the Spec Kit spec location

- **WHEN** the spec-path doctrine is rendered for a Spec Kit backend
- **THEN** it SHALL name the Spec Kit spec location (e.g. `specs/<feature>/` or the injected sidecar)
- **AND** it SHALL NOT direct the worker to `openspec/changes/<id>/`

#### Scenario: OpenSpec doctrine is unchanged

- **WHEN** the spec-path doctrine is rendered for an OpenSpec backend
- **THEN** it SHALL still name the OpenSpec `openspec/changes/<change-name>/` path
