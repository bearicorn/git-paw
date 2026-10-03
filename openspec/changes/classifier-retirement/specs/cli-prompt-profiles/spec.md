## MODIFIED Requirements

### Requirement: Security decisions are never profile-sourced

The live-prompt gate's anchoring discipline (its tail and block windows and the bounded trailing region), the approval send gate (the fresh-capture re-confirm and the pane-0 refusal), and the `sweep.sh approve` rule that only option `1` is ever sent SHALL remain compiled, or for the bundled helper fixed in its script. They SHALL NOT be readable or overridable from any profile, configuration file, or worktree-writable location. A profile supplies only the literals these gates match. A profile that could change the gates themselves would let a gated agent widen the gate that governs it.

#### Scenario: A profile cannot widen the live-prompt window

- **GIVEN** a profile whose markers and option-line form are overridden
- **WHEN** the live-prompt gate evaluates a capture
- **THEN** the tail window, the block window and the trailing-region bound SHALL be those compiled into the gate

#### Scenario: A profile cannot make approve select another option

- **GIVEN** a profile that overrides `option_line_pattern` and `broad_grant_marker`
- **WHEN** `sweep.sh approve <pane>` runs against a live prompt
- **THEN** it SHALL send either `1` followed by `Enter`, or nothing
- **AND** it SHALL NOT send any other option digit

#### Scenario: The pane-0 refusal ignores profile content

- **GIVEN** any resolved profile
- **WHEN** `sweep.sh approve 0` runs
- **THEN** it SHALL send no keystrokes and report that pane 0 is excluded
