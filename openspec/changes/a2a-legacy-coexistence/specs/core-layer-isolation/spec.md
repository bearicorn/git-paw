## ADDED Requirements

### Requirement: A2A is an opt-in layer with a broker-host-only surface

git-paw SHALL resolve an A2A layer, which `[broker.a2a] enabled = true` enables, as part of a session's layer set.

The A2A layer SHALL require the Broker layer. It SHALL NOT imply it. When `[broker.a2a] enabled = true` but the session does not resolve the Broker layer, git-paw SHALL NOT enable the A2A layer, and SHALL print one notice line stating that A2A needs the broker.

The A2A layer's surface SHALL be exactly two things on the broker host: the A2A routes and the Agent Card route. The A2A layer SHALL add no file to the repository or to any worktree. It SHALL add nothing to any git hook, managed block, rendered skill, boot block, first prompt or session environment.

The CI layer matrix SHALL include a Manual, Broker and A2A entry. The modes chapter SHALL document the A2A layer.

#### Scenario: Resolution rows for A2A

- **WHEN** the layers are resolved for each row's configuration
- **THEN** the result SHALL be:
  - `[broker] enabled = true` with `[broker.a2a] enabled = true` → Manual, Broker, A2A
  - `[broker.a2a] enabled = true` with no `[broker]` section and not coordinated → Manual, with one notice line that A2A needs the broker
  - `--coordinated` with `[broker.a2a] enabled = true` → Manual, Broker, Coordinated, A2A

#### Scenario: A2A adds nothing to worktrees

- **WHEN** the surface plans for Manual and Broker, and for Manual, Broker and A2A, are compared
- **THEN** they SHALL differ only by the broker host's A2A routes and Agent Card route

#### Scenario: Matrix entry proves agent-side surfaces are unchanged

- **WHEN** the Manual, Broker and A2A matrix entry runs `git paw init` and `git paw start` with a stub agent CLI
- **THEN** the files, hooks, sidecar blocks, first prompts and session environment SHALL equal those of the Manual and Broker entry
- **AND** the broker host SHALL answer `GET /.well-known/agent-card.json` with `200`

#### Scenario: Modes chapter documents the layer

- **WHEN** the modes chapter is checked against the layer list
- **THEN** it SHALL name the A2A layer, its enabling key, its Broker requirement, its host-only surface and how to turn it off
