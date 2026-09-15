## MODIFIED Requirements

### Requirement: A CLI prompt-shape profile describes one CLI's terminal surface

The system SHALL define a per-CLI prompt-shape profile that carries the literals and patterns describing how one agent CLI's terminal surface appears. The profile SHALL cover, at minimum: readiness markers, approval-prompt markers, live-prompt markers, mid-response markers, the command-header form, the file-prompt pattern, the option-line form, the broad-grant marker, the input-line sigil, the permission-mode markers, the stream-error, context-bloat and paste-buffer markers, and the CLI-unavailability markers (`cli_unavailable_markers` — the phrasings the CLI shows when it has stopped serving the session because a usage or rate limit was reached or authentication was lost).

The profile SHALL describe only how the terminal *looks*. It SHALL NOT carry any decision about what is permitted.

#### Scenario: Profile carries prompt-shape fields

- **WHEN** a CLI prompt-shape profile is resolved
- **THEN** it SHALL provide readiness markers, approval markers, mid-response markers, the command-header form, the file-prompt pattern, the option-line form, the input sigil, the mode markers, and the CLI-unavailability markers for that CLI

#### Scenario: Profile carries no permission policy

- **WHEN** a CLI prompt-shape profile is inspected
- **THEN** it SHALL NOT contain the danger list, protected-path rules, worktree-boundary rules, the broad-grant eligibility rule, or any other determination of whether a command may run

#### Scenario: CLI-unavailability markers are overridable and readable by the helper script

- **GIVEN** a `[clis.<name>].prompt_profile` override that sets `cli_unavailable_markers`
- **WHEN** the profile is resolved for that CLI and `git paw __prompt-profile <name> cli_unavailable_markers` is run
- **THEN** the resolved markers SHALL be the override's values
- **AND** the hidden command SHALL print them, so the bundled helper script and the compiled loop read one source

### Requirement: The Claude Code profile is embedded as the default

The system SHALL embed a Claude Code profile compiled into the binary, and SHALL use it as the default for every field. When no profile configuration is present, resolved behaviour SHALL be identical to the behaviour before profiles existed. The embedded profile's `cli_unavailable_markers` SHALL include Claude Code's session/usage-limit and authentication-failure phrasings, including `You've hit your session limit`. The embedded profile's `stream_error_markers` SHALL additionally include the mid-response connection-drop phrasings `API Error` and `Connection closed mid-response`, which denote a transient drop whose conversation context survives — a nudge resumes the worker — and SHALL NOT be placed in `cli_unavailable_markers`, which is reserved for terminal conditions requiring human intervention.

#### Scenario: Absent configuration reproduces current behaviour

- **GIVEN** an installation with no profile configuration
- **WHEN** readiness, approval detection, mid-response detection, mode detection, and prompt-shape identification run
- **THEN** each SHALL resolve the same literals it used before profiles were introduced

#### Scenario: Single-binary distribution preserved

- **WHEN** the binary is distributed without any accompanying profile file
- **THEN** the embedded Claude Code profile SHALL still resolve

#### Scenario: The embedded profile recognises the Claude Code session limit

- **GIVEN** no profile configuration and a pane capture whose tail contains `You've hit your session limit`
- **WHEN** the capture is matched against the resolved `cli_unavailable_markers`
- **THEN** it SHALL match

#### Scenario: A mid-response connection drop is a stream error, not CLI unavailability

- **GIVEN** no profile configuration and a pane capture whose tail contains `API Error: Connection closed mid-response`
- **WHEN** the capture is matched against the resolved marker sets
- **THEN** it SHALL match `stream_error_markers`
- **AND** it SHALL NOT match `cli_unavailable_markers`
- **AND** the wave SHALL NOT exit with outcome `cli-unavailable` on account of it
