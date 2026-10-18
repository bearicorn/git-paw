## ADDED Requirements

### Requirement: Dashboard shows a stalled-wave indicator

The dashboard SHALL show a stalled-wave indicator in its status line, including how long the wave has been idle, whenever the broker's wave staleness — the `wave` object `GET /status` reports — has `stalled = true`, and SHALL clear it on the first refresh that reports `stalled = false`.

#### Scenario: Stalled wave is shown

- **GIVEN** a running dashboard whose broker reports `stalled = true` with `idle_seconds = 3600`
- **WHEN** the dashboard renders a frame
- **THEN** the status line SHALL contain a stalled indicator naming 1h of idleness

#### Scenario: Indicator clears on progress

- **GIVEN** a dashboard showing the stalled indicator
- **WHEN** the next refresh reports `stalled = false`
- **THEN** the indicator SHALL no longer be rendered
