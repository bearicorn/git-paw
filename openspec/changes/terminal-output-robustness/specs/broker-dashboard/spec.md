## ADDED Requirements

### Requirement: Dashboard state stays visible without colour

The dashboard SHALL mark the selected broker-log row and the active filter chips with text attributes (reverse video, with bold for active chips) so both remain distinguishable when colours are disabled. When colours are disabled (`NO_COLOR` set to a non-empty value) the dashboard SHALL set no foreground or background colour on any style.

#### Scenario: Selection visible under NO_COLOR

- **GIVEN** colours are disabled and the broker log shows several rows
- **WHEN** the user moves the selection
- **THEN** the selected row SHALL render with the reverse attribute and no colour

#### Scenario: Active chip visible under NO_COLOR

- **GIVEN** colours are disabled and the `status` chip is active
- **WHEN** the chip row renders
- **THEN** the `status` chip SHALL carry reverse and bold attributes and the inactive chips SHALL not

### Requirement: Dashboard renders a notice below its minimum size

The dashboard SHALL compute its minimum usable size from its fixed layout constraints (the agent table's minimum column widths and height plus the title and status lines, and the broker-log panel's minimum when it is visible). When the terminal is smaller in either dimension it SHALL render a single notice stating the current size and the required minimum instead of the clipped layout, SHALL keep handling keys, and SHALL render the full layout again on the first frame after the terminal grows past the minimum.

#### Scenario: Tiny terminal shows the notice

- **GIVEN** a 30×8 terminal
- **WHEN** the dashboard renders
- **THEN** the frame SHALL contain the too-small notice with `30×8` and the required minimum, and no agent table

#### Scenario: Growing the terminal restores the layout

- **GIVEN** the dashboard showing the too-small notice
- **WHEN** the terminal is resized above the minimum
- **THEN** the next frame SHALL render the agent table and status line

#### Scenario: Quit still works when too small

- **GIVEN** the dashboard showing the too-small notice
- **WHEN** the user presses `q`
- **THEN** the dashboard SHALL quit

### Requirement: Filter chip row fits the panel width

The filter chip row SHALL fit the broker-log panel's width: it SHALL use full chip labels when they fit, otherwise an abbreviated `digit:short-label` form, otherwise the hotkey digits alone, and SHALL NOT be clipped mid-chip.

#### Scenario: Narrow panel abbreviates chips

- **GIVEN** a broker-log panel 60 columns wide
- **WHEN** the chip row renders
- **THEN** every chip SHALL be visible in an abbreviated form and no chip SHALL be cut off
