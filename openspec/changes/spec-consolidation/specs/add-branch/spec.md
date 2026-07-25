## MODIFIED Requirements

### Requirement: Paused-session interplay

When the session is in the paused state, an added pane SHALL also
start paused (boot block injected but the agent held), consistent
with the rest of the session. On the next `git paw start` (which
resumes a paused session — there is no separate `resume`
subcommand), the added agent SHALL begin working alongside the
others.

#### Scenario: Add while paused starts the new pane paused

- **GIVEN** a paused session
- **WHEN** the user runs `git paw add feat/x`
- **THEN** the new pane SHALL be in the paused state (not actively
  working) until the next `git paw start`

#### Scenario: Resuming starts the added agent

- **GIVEN** a paused session to which `feat/x` was added
- **WHEN** the user runs `git paw start` to resume the session
- **THEN** the `feat/x` agent SHALL submit its boot prompt and
  begin working alongside the resumed agents
