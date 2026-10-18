## MODIFIED Requirements

### Requirement: Dashboard pane layout

The system SHALL arrange tmux panes with the dashboard in a full-width top row and the coding-agent panes in the agent grid below it (row-major, at most five per row, each row at equal widths, per `tmux-orchestration`'s "Every pane-less session uses the agent grid"). This layout SHALL be applied when the session is built, re-applied when an agent is added or removed, and maintained throughout the session.

#### Scenario: Dashboard takes full width at top

- **GIVEN** a tmux session with a dashboard pane and 3 coding-agent panes
- **WHEN** the session is built
- **THEN** the dashboard pane occupies the full window width as the top row
- **AND** the 3 coding-agent panes form one equal-width row below the dashboard

#### Scenario: Layout is maintained after add and remove

- **GIVEN** a dashboard session with the grid applied
- **WHEN** an agent is added or removed
- **THEN** the dashboard SHALL remain the full-width top row and the agent rows SHALL re-flow to the grid for the new count
