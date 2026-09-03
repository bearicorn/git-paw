## ADDED Requirements

### Requirement: A CLI prompt-shape profile describes one CLI's terminal surface

The system SHALL define a per-CLI prompt-shape profile that carries the literals and patterns describing how one agent CLI's terminal surface appears. The profile SHALL cover, at minimum: readiness markers, approval-prompt markers, live-prompt markers, mid-response markers, the command-header form, the file-prompt pattern, the option-line form, the broad-grant marker, the input-line sigil, the permission-mode markers, and the stream-error, context-bloat and paste-buffer markers.

The profile SHALL describe only how the terminal *looks*. It SHALL NOT carry any decision about what is permitted.

#### Scenario: Profile carries prompt-shape fields

- **WHEN** a CLI prompt-shape profile is resolved
- **THEN** it SHALL provide readiness markers, approval markers, mid-response markers, the command-header form, the file-prompt pattern, the option-line form, the input sigil, and the mode markers for that CLI

#### Scenario: Profile carries no permission policy

- **WHEN** a CLI prompt-shape profile is inspected
- **THEN** it SHALL NOT contain the danger list, protected-path rules, worktree-boundary rules, the broad-grant eligibility rule, or any other determination of whether a command may run

### Requirement: Security decisions are never profile-sourced

The danger list, protected-path violation rules, worktree-boundary resolution, the arbitrary-code-runner / broad-grant eligibility rule, and the approval send gate SHALL remain compiled and SHALL NOT be readable or overridable from any profile, configuration file, or worktree-writable location. These determine whether an agent's command is permitted; a profile that could alter them would let a gated agent widen the gate that governs it.

#### Scenario: Danger determination ignores profile content

- **GIVEN** a profile that attempts to declare a dangerous command safe
- **WHEN** that command is classified
- **THEN** the classification SHALL be unaffected by the profile
- **AND** the command SHALL still be treated as dangerous

#### Scenario: Broad-grant eligibility ignores profile content

- **GIVEN** a profile that attempts to mark an arbitrary-code-runner command as eligible for a durable grant
- **WHEN** option selection runs for that prompt
- **THEN** the durable-grant option SHALL NOT be selected

### Requirement: The Claude Code profile is embedded as the default

The system SHALL embed a Claude Code profile compiled into the binary, and SHALL use it as the default for every field. When no profile configuration is present, resolved behaviour SHALL be identical to the behaviour before profiles existed.

#### Scenario: Absent configuration reproduces current behaviour

- **GIVEN** an installation with no profile configuration
- **WHEN** readiness, approval detection, mid-response detection, mode detection, and prompt-shape identification run
- **THEN** each SHALL resolve the same literals it used before profiles were introduced

#### Scenario: Single-binary distribution preserved

- **WHEN** the binary is distributed without any accompanying profile file
- **THEN** the embedded Claude Code profile SHALL still resolve

### Requirement: Per-field fallback to the embedded default

A profile SHALL be permitted to specify a subset of fields. Any field a profile does not specify SHALL fall back to the embedded default rather than resolving empty, so a partial profile can never silently disable detection.

#### Scenario: Partial profile falls back per field

- **GIVEN** a profile that specifies only approval markers
- **WHEN** readiness markers are resolved for that CLI
- **THEN** they SHALL resolve to the embedded default values
- **AND** SHALL NOT resolve to an empty set

#### Scenario: Empty field does not disable detection

- **GIVEN** a profile field that is absent
- **WHEN** the corresponding detection runs
- **THEN** detection SHALL behave as though the embedded default were configured

### Requirement: Rust and the supervisor helper script read one source

The profile SHALL be the single source of prompt-shape literals for both the compiled code paths and the bundled supervisor helper script. The helper script SHALL NOT carry its own copies of the markers or regexes. A prompt-shape literal SHALL NOT be duplicated between the two.

#### Scenario: Helper script sources markers from the profile

- **WHEN** the bundled supervisor helper script evaluates a prompt shape
- **THEN** it SHALL read the markers from the resolved profile
- **AND** it SHALL NOT use a locally defined copy of those markers

#### Scenario: No duplicated literal between shell and Rust

- **WHEN** the bundled helper script and the compiled code are compared for prompt-shape literals
- **THEN** no prompt-shape literal SHALL appear independently defined in both

### Requirement: Detection algorithms are unchanged by profile sourcing

Moving the literals into a profile SHALL NOT change any detection algorithm: the tail and block capture windows, the last-header-wins command extraction, the option-index selection rule, the rate limiting, and the re-confirm gate SHALL behave as before. Only the origin of the literals changes.

#### Scenario: Extraction semantics preserved

- **GIVEN** a capture that previously yielded a particular extracted command
- **WHEN** the same capture is processed with profile-sourced markers resolving to the same literals
- **THEN** the extracted command SHALL be identical

#### Scenario: Rate limiting preserved

- **WHEN** captures are taken with profile-sourced markers
- **THEN** the capture rate limiting SHALL behave as it did before profiles were introduced

### Requirement: An unrecognised CLI resolves a usable profile

When a CLI has no profile of its own, the system SHALL resolve the embedded default rather than failing or resolving an empty profile, so an unrecognised CLI degrades to current behaviour rather than to no detection at all.

#### Scenario: Unknown CLI falls back to the embedded default

- **GIVEN** a CLI with no profile defined
- **WHEN** its prompt-shape profile is resolved
- **THEN** the embedded default profile SHALL be returned
- **AND** detection SHALL remain operative
