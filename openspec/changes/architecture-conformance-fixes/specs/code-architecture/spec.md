## ADDED Requirements

### Requirement: Source audits read every production line

A source audit SHALL check every production line of each file it scans. It SHALL skip only `#[cfg(test)]` items, measured by their braces, and comment lines, so code after an early `#[cfg(test)]` item is still audited. All source audits SHALL share one implementation of that rule.

#### Scenario: Production code after an early test item is audited

- **GIVEN** a source file whose first `#[cfg(test)]` item is followed by production code that matches the audit's forbidden pattern
- **WHEN** the audit scans the file
- **THEN** the audit SHALL report that production line

#### Scenario: Test items and comments are not audited

- **GIVEN** a source file whose only matches of the forbidden pattern are inside a `#[cfg(test)]` item or on a comment line
- **WHEN** the audit scans the file
- **THEN** the audit SHALL report nothing for that file

#### Scenario: main.rs is a positive control

- **WHEN** the shared production-code filter is applied to `src/main.rs`
- **THEN** a known production line after the file's first `#[cfg(test)]` SHALL remain visible to the audit

#### Scenario: An if-let on a spec-format variant is caught

- **GIVEN** the line `if let SpecBackendKind::OpenSpec = kind {` in production code outside `src/specs/`
- **WHEN** the spec-format dispatch audit runs
- **THEN** it SHALL report that line

### Requirement: Core modules report warnings as values

A module in the core or agent-CLI layer SHALL return a warning to its caller as a value, not print it to stderr. It SHALL NOT keep process-global state to deduplicate warnings, and SHALL NOT read the process working directory. The caller SHALL emit the warning through its own output sink and deduplicate it per launch.

#### Scenario: An unsandboxed full-auto launch warns once per launch through the sink

- **GIVEN** a CLI that declares no native sandbox, launched at the full-auto level for two workers in one `start`
- **WHEN** the launch runs with a capturing output sink
- **THEN** the sink SHALL hold the unsandboxed-full-auto warning exactly once

#### Scenario: A captured launch does not write the warning to stderr

- **GIVEN** a launch that produces the unsandboxed-full-auto warning, run with a capturing output sink as the MCP server runs it
- **WHEN** the launch completes
- **THEN** the warning SHALL be in the sink
- **AND** the process SHALL NOT have written it to stderr

#### Scenario: Two launches in one process each warn

- **GIVEN** two launches of the same unsandboxed CLI at full-auto in one process
- **WHEN** each runs with its own capturing output sink
- **THEN** each sink SHALL hold the warning once

#### Scenario: A profile that fails to compile is reported to the caller

- **GIVEN** an agent recorded with a CLI whose prompt-profile override does not compile
- **WHEN** the agent profile map records it
- **THEN** the record call SHALL return a warning naming the agent
- **AND** the agent SHALL resolve the `generic` profile

### Requirement: Keystrokes reach a surface through the backend's own dispatcher

Every terminal backend SHALL act as a keystroke dispatcher through one shared implementation, and the submit-and-verify actuator SHALL take that dispatcher by shared reference. No command or test SHALL wrap a backend in its own dispatcher adapter.

#### Scenario: A send uses the backend as capturer and dispatcher

- **GIVEN** `__send` delivering a message to a fake backend's surface
- **WHEN** the message is typed and submitted
- **THEN** the fake SHALL record the text operation and the `Enter` key operation in order, from the same backend the screen was read through

#### Scenario: Drive-loop keystrokes are unchanged

- **GIVEN** the drive loop wired to a fake backend that is both its capturer and its dispatcher
- **WHEN** it submits buffered input left on an idle input box
- **THEN** the fake SHALL record the same key operations as before this refactor

### Requirement: The styled screen read is pinned by a golden argv test

The tmux adapter's styled screen read SHALL be pinned by a golden argv test, like every other adapter operation, and the fake backend SHALL record a styled read as its own operation.

#### Scenario: The styled read's argv is pinned

- **WHEN** `TmuxBackend::capture_styled` reads a surface
- **THEN** it SHALL run `tmux capture-pane -p -e -t <surface>` and nothing else

#### Scenario: The fake records a styled read

- **GIVEN** a fake backend with a scripted screen for a surface
- **WHEN** a caller reads that surface with `capture_styled`
- **THEN** the fake SHALL record a styled-capture operation for that surface, distinct from a plain screen-tail read
