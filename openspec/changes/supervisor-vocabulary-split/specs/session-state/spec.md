## ADDED Requirements

### Requirement: The session mode is read in either spelling

The session file's `mode` SHALL be read as coordinated mode when it holds `"coordinated"` or `"supervisor"`, and as bare mode when it holds `"bare"` or is absent. Until v1.0.0 git-paw SHALL keep writing `"supervisor"` for a coordinated session, so a session file it writes stays readable by releases that predate the `"coordinated"` spelling. Every consumer of the session mode — layout offsets, recovery, `add`, `remove`, status and the MCP session tools — SHALL behave identically for both spellings.

#### Scenario: Both spellings load as coordinated

- **GIVEN** two session files identical except that one has `"mode": "supervisor"` and the other `"mode": "coordinated"`
- **WHEN** both are loaded
- **THEN** both SHALL load as coordinated-mode sessions with equal fields

#### Scenario: A coordinated session is written with the compatible spelling

- **WHEN** a coordinated session is saved
- **THEN** its session file SHALL contain `"mode": "supervisor"`

#### Scenario: Recovery is unaffected by the spelling

- **GIVEN** a stopped coordinated session whose file says `"mode": "coordinated"`
- **WHEN** `git paw start` recovers it
- **THEN** it SHALL rebuild the coordinated layout exactly as for `"mode": "supervisor"`
