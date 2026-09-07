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

### Requirement: Spec Kit boot task-prompt does not point at the OpenSpec artifacts path

The per-agent task prompt built by `build_task_prompt` for a `SpecBackendKind::SpecKit` entry SHALL NOT direct the worker to an `openspec/changes/<id>/` sibling directory — the Spec Kit decomposition already embeds the feature spec, implementation plan, and task phase inline in the sidecar, so no such directory exists to read. The `Markdown` backend's task prompt is unaffected.

#### Scenario: Spec Kit task prompt does not reference the OpenSpec artifacts path

- **WHEN** the per-agent task prompt is built for a `SpecBackendKind::SpecKit` entry
- **THEN** it SHALL still point the agent at the gitignored sidecar
- **AND** it SHALL NOT contain `openspec/changes/`
