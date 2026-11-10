## MODIFIED Requirements

### Requirement: A CLI prompt-shape profile describes one CLI's terminal surface

The system SHALL define a per-CLI prompt-shape profile of the literals and patterns showing how one CLI's terminal surface looks: readiness, approval-prompt, live-prompt, mid-response and permission-mode markers; the command-header, option-line and file-prompt forms; the broad-grant marker; the input sigil; stream-error, context-bloat and paste-buffer markers; the menu cursor glyphs and footer markers; and `cli_unavailable_markers`. It SHALL NOT carry any decision about what is permitted.

#### Scenario: Profile carries prompt-shape fields

- **WHEN** a CLI prompt-shape profile is resolved
- **THEN** it SHALL provide readiness markers, approval markers, mid-response markers, the command-header form, the file-prompt pattern, the option-line form, the input sigil, the mode markers, and the CLI-unavailability markers for that CLI

#### Scenario: Profile carries no permission policy

- **WHEN** a CLI prompt-shape profile is inspected
- **THEN** it SHALL NOT contain the danger list, protected-path rules, worktree-boundary rules, the broad-grant eligibility rule, or any other determination of whether a command may run

#### Scenario: CLI-unavailability markers are overridable and readable by the helper script

- **GIVEN** a `[clis.<name>].prompt_profile` override that sets `cli_unavailable_markers`
- **WHEN** the profile is resolved for that CLI and `git paw __prompt-profile --cli <name> --field cli_unavailable_markers` is run
- **THEN** the resolved markers SHALL be the override's values
- **AND** the hidden command SHALL print them, so the bundled helper script and the compiled loop read one source
