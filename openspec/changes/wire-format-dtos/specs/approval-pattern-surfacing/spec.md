## ADDED Requirements

### Requirement: Manual-decision log lines are versioned wire records

Each line of the manual-decision log SHALL be serialized from a dedicated wire type, separate from the in-memory manual-approval model, and SHALL carry an integer `schema_version` equal to 1 in addition to `timestamp`, `agent_id`, `pattern`, and `first_seen`. A line without `schema_version` SHALL be read as version 1. When aggregating, a line declaring a newer `schema_version` than the binary supports SHALL be skipped, and the number of skipped lines SHALL be reported once on stderr; malformed lines SHALL continue to be skipped as before.

#### Scenario: New lines carry the schema version

- **WHEN** the drive loop logs a live permission prompt it escalated
- **THEN** the appended line SHALL contain `"schema_version": 1` and the four existing fields

#### Scenario: Pre-change lines aggregate unchanged

- **GIVEN** a log written before this change (no `schema_version` on any line)
- **WHEN** `git paw approvals` aggregates it
- **THEN** the result SHALL equal the pre-change aggregation

#### Scenario: Newer lines are skipped with one notice

- **GIVEN** a log whose lines include two declaring `"schema_version": 2`
- **WHEN** `git paw approvals` aggregates it
- **THEN** those two lines SHALL be excluded from the counts
- **AND** stderr SHALL report once that two lines from a newer schema were skipped
