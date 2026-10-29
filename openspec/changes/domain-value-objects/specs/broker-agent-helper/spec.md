## ADDED Requirements

### Requirement: Helper agent-id slugging has a single shell definition that matches the binary

The bundled helpers SHALL compute a broker agent id from a branch name only through the shared preamble's `slugify` function in `_paw_common.sh`; no helper SHALL carry another copy of the rule, including inside an embedded Python snippet. A standing test SHALL run the shell `slugify` over the same input table used for the binary's agent-id rule — including mixed case, nested slashes, underscores, runs of separators, leading and trailing separators, non-ASCII characters, the empty string, and an all-separator string — and SHALL assert every output equals the binary's.

#### Scenario: Shell and binary produce the same agent id

- **GIVEN** the agent-id rule's input table
- **WHEN** each input is passed to `_paw_common.sh`'s `slugify` and to the binary's agent-id derivation
- **THEN** every pair of outputs SHALL be equal

#### Scenario: No helper carries a second copy of the rule

- **WHEN** the bundled helper scripts are inspected by the standing asset check
- **THEN** the agent-id slug rule (lowercasing plus the `[^a-z0-9_]` replacement) SHALL appear only in `_paw_common.sh`'s `slugify`
