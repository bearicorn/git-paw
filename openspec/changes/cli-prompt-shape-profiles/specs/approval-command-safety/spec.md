## ADDED Requirements

### Requirement: Prompt markers are sourced from the resolved CLI profile

Permission-prompt detection SHALL obtain its approval markers, command-header form, file-prompt pattern, option-line form and broad-grant marker from the prompt-shape profile resolved for the CLI running in the pane being inspected, rather than from constants compiled into the detector. When no profile is configured for that CLI, the embedded default SHALL resolve, so detection outcomes are unchanged from before profiles existed.

This changes only where the literals come from. The detection algorithm, the permission-type classification outcomes, the capture rate limiting, and every safety gate SHALL be unaffected.

#### Scenario: Detection uses the pane's CLI profile

- **GIVEN** a pane running a CLI with a configured prompt-shape profile
- **WHEN** permission-prompt detection runs against that pane
- **THEN** the markers used SHALL come from that CLI's resolved profile

#### Scenario: Unconfigured CLI detects as before

- **GIVEN** a pane running a CLI with no configured profile
- **WHEN** permission-prompt detection runs against that pane
- **THEN** the embedded default markers SHALL be used
- **AND** the detection outcome SHALL match the outcome before profiles were introduced

#### Scenario: Safety gates unaffected by profile sourcing

- **GIVEN** any resolved profile
- **WHEN** a captured prompt is classified
- **THEN** the danger determination, protected-path rules, worktree-boundary rules and broad-grant eligibility SHALL be evaluated from compiled logic
- **AND** SHALL NOT be influenced by profile content
