## MODIFIED Requirements

### Requirement: The session mode is read in either spelling

The session file's `mode` SHALL be read as coordinated mode when it holds `"coordinated"` or `"supervisor"`, and as bare mode when it holds `"bare"` or is absent. git-paw SHALL write `"coordinated"` for a coordinated session; the spelling `"supervisor"`, written by releases before 1.0.0, SHALL remain readable so a session that is live or stopped across the upgrade keeps resolving. Every consumer of the session mode — layout offsets, recovery, `add`, `remove`, status and the MCP session tools — SHALL behave identically for both spellings.

#### Scenario: Both spellings load as coordinated

- **GIVEN** two session files identical except that one has `"mode": "supervisor"` and the other `"mode": "coordinated"`
- **WHEN** both are loaded
- **THEN** both SHALL load as coordinated-mode sessions with equal fields

#### Scenario: A coordinated session is written with the new spelling

- **WHEN** a coordinated session is saved
- **THEN** its session file SHALL contain `"mode": "coordinated"`

#### Scenario: A pre-1.0 session recovers after the upgrade

- **GIVEN** a stopped coordinated session whose file, written by a 0.x release, says `"mode": "supervisor"`
- **WHEN** `git paw start` recovers it
- **THEN** it SHALL rebuild the coordinated layout
- **AND** the re-saved file SHALL say `"mode": "coordinated"`
