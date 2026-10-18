## ADDED Requirements

### Requirement: Session records its pane layout

The `Session` struct SHALL include an optional field recording the window's pane layout, written as the agent grid by every start, recovery and restart that builds or rebuilds the window. The field SHALL be annotated `#[serde(default, skip_serializing_if = "Option::is_none")]` so session files written earlier load unchanged. A session without the field SHALL be read as the agent grid when its mode is supervised, and as the earlier tiled layout when its mode is bare; consumers that depend on the geometry (`add`, `remove`) SHALL decide from this field and the mode, never from the mode alone.

#### Scenario: New sessions record the grid

- **GIVEN** a bare `git paw start` with two branches
- **WHEN** the session state is saved
- **THEN** the session file SHALL record the grid layout

#### Scenario: Earlier session files load unchanged

- **GIVEN** a session file written before this field existed
- **WHEN** it is loaded
- **THEN** loading SHALL succeed and the layout field SHALL be absent

#### Scenario: An earlier bare session reads as tiled

- **GIVEN** a loaded bare session without the layout field
- **WHEN** `git paw add` inspects its layout
- **THEN** it SHALL be treated as the tiled layout

#### Scenario: An earlier supervised session reads as the grid

- **GIVEN** a loaded supervised session without the layout field
- **WHEN** `git paw add` inspects its layout
- **THEN** it SHALL be treated as the agent grid
