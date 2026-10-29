## MODIFIED Requirements

### Requirement: Spec Kit boot task-prompt does not point at the OpenSpec artifacts path

The per-agent task prompt built by `build_task_prompt` for a `SpecBackendKind::SpecKit` entry SHALL NOT direct the worker to an `openspec/changes/<id>/` sibling directory — the Spec Kit decomposition already embeds the feature spec, implementation plan, and task phase inline in the sidecar, so no such directory exists to read.

#### Scenario: Spec Kit task prompt does not reference the OpenSpec artifacts path

- **WHEN** the per-agent task prompt is built for a `SpecBackendKind::SpecKit` entry
- **THEN** it SHALL still point the agent at the gitignored sidecar
- **AND** it SHALL NOT contain `openspec/changes/`

## ADDED Requirements

### Requirement: Markdown boot task-prompt does not point at the OpenSpec artifacts path

The per-agent task prompt for a `SpecBackendKind::Markdown` entry SHALL point the worker at the gitignored sidecar, which carries the full spec body, and SHALL state that the single spec file is the whole contract. It SHALL NOT direct the worker to an `openspec/changes/<id>/` directory, which a Markdown spec does not have.

#### Scenario: Markdown task prompt does not reference the OpenSpec artifacts path

- **WHEN** the per-agent task prompt is built for a `SpecBackendKind::Markdown` entry
- **THEN** it SHALL point the agent at the gitignored sidecar
- **AND** it SHALL NOT contain `openspec/changes/`

#### Scenario: No non-OpenSpec task prompt names the OpenSpec artifacts path

- **WHEN** the per-agent task prompt is built for a Markdown, Spec Kit and Superpowers entry
- **THEN** none of the three prompts SHALL contain `openspec/changes/`
