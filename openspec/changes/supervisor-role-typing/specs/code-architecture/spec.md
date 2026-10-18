## ADDED Requirements

### Requirement: Session surface roles are an explicit enum resolved once from the recorded layout

The system SHALL represent the role of each surface in a session's window as a closed enum (`SurfaceRole`) produced by a single resolver over a core layout record (`SessionLayout`) that is derived from the recorded session — its mode, whether a dashboard exists, and the dashboard's index — together with the surface's owning working directory. Coding-agent roles SHALL be resolved by matching the owning directory against the session's worktree paths before any reserved index is consulted. Every reserved-index fact (the first coding-agent index, the dashboard index, any role-reserved index, and every reserved index a launch records in session or discovery state) SHALL be answered by the layout record; decision code SHALL match on the resolved role and SHALL NOT compare a pane index to a literal, nor re-derive a role by comparing an agent-id string such as `"supervisor"`. A comparison against the broker wire identity `"supervisor"` that identifies a message sender, a roster row or a pane label — not the role of a surface — is not a role decision and is outside this rule. The broker wire identity for a role SHALL be produced in one place, and the terminal adapter modules SHALL NOT depend on supervisor modules for layout geometry. For every layout the system can launch, the resolved roles and every sweep, escalate, keystroke-target, skip, exit and pane-offset decision SHALL be identical to the behavior before this requirement.

#### Scenario: Roles resolve from the layout record

- **GIVEN** a supervised session layout with a supervisor surface at index 0, the dashboard at index 1 and coding agents from index 2, and a bare broker-enabled layout with the dashboard at index 0 and coding agents from index 1
- **WHEN** each surface of each layout is resolved
- **THEN** the supervised layout SHALL yield `Supervisor`, `Dashboard`, then `Coding` roles in index order
- **AND** the bare layout's index 0 SHALL yield `Dashboard`
- **AND** a surface at a non-reserved index whose owner matches no worktree SHALL yield `Unknown`

#### Scenario: Coding agents resolve by owner, not index

- **GIVEN** a coding agent whose surface sits at an index the layout reserves, running in that agent's worktree
- **WHEN** the surface is resolved
- **THEN** it SHALL resolve to that agent's `Coding` role

#### Scenario: No pane-index literal or role-string comparison in decision code

- **WHEN** a source audit scans the supervisor, coordination, session and command modules
- **THEN** it SHALL find no comparison of a pane index against a numeric literal and no comparison of an agent id against `"supervisor"` used to decide a role, outside the layout module
- **AND** comparisons that identify a broker message sender, a roster row or a pane label as `"supervisor"` SHALL be allowed by name
- **AND** the terminal adapter modules SHALL NOT import a supervisor module for layout geometry

#### Scenario: Recorded indices come from the layout record

- **GIVEN** a bare broker-enabled launch and a supervised launch
- **WHEN** each saves its session and per-repo discovery file
- **THEN** the recorded `dashboard_pane`, every agent's `pane_index` and any orchestrator `pane_index` SHALL equal the values the layout record answers for that layout
- **AND** they SHALL equal the values the same launch recorded before this requirement

#### Scenario: Behavior is unchanged

- **GIVEN** the typed-role refactor
- **WHEN** the unattended-drive, sweep-helper approve, add/remove and coordination-inventory suites run without modification
- **THEN** they SHALL pass
- **AND** the drive loop SHALL sweep, escalate for, send nudge or submit keystrokes to, and exit on exactly the surfaces it did before the refactor
