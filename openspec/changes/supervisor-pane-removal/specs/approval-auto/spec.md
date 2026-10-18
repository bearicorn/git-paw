## ADDED Requirements

### Requirement: Keystrokes reach only coding-agent surfaces

Every git-paw keystroke sender SHALL send keystrokes only to a surface whose resolved role is a coding agent, in every session mode. This covers the explicit `sweep.sh approve <pane>` action and the drive loop's nudge, stale-input recovery and buffered-input submit. The drive loop SHALL resolve the role from the session's recorded layout and the surface's owning worktree; `sweep.sh approve` SHALL resolve it from the session's discovery roster (see `broker-agent-helper`, *sweep.sh approve refuses pane 0*). The dashboard surface, a surface whose owner matches no agent worktree, and any other non-coding surface SHALL be refused: the sender SHALL send no keystrokes and SHALL report that the surface is excluded because it is not a coding agent. No sender SHALL hold an exception that lets it type into a non-coding surface, and the pane index alone SHALL NOT decide eligibility.

#### Scenario: Dashboard surface is refused in a bare session

- **GIVEN** a bare broker-enabled session whose dashboard is at pane 0, and whose discovery roster lists coding agents from pane 1
- **WHEN** `sweep.sh approve 0` runs, even if the dashboard's capture resembles a live prompt
- **THEN** it SHALL send NO keystrokes
- **AND** SHALL report that the surface is excluded because it is not a coding agent

#### Scenario: Dashboard surface is never nudged in a supervised session

- **GIVEN** a supervised `--unattended` session whose dashboard is at pane 0 and whose dashboard capture shows text resembling buffered input on an idle input box
- **WHEN** the drive loop sweeps the session
- **THEN** it SHALL send NO keystrokes to pane 0

#### Scenario: Unresolved surface is refused

- **GIVEN** a surface whose owner matches no agent worktree and whose capture shows buffered input on an idle input box
- **WHEN** the drive loop sweeps the session
- **THEN** it SHALL send NO keystrokes to that surface

#### Scenario: The first agent of a broker-less session is approvable

- **GIVEN** a session with the broker disabled, whose discovery roster lists its first coding agent at pane 0, showing a live permission prompt
- **WHEN** a human runs `sweep.sh approve 0`
- **THEN** the helper SHALL send `1` followed by `Enter` to that surface

#### Scenario: Coding agent surfaces are reached

- **GIVEN** a coding agent's surface whose idle input box holds buffered text
- **WHEN** the drive loop sweeps it
- **THEN** it SHALL submit the buffered text to that surface

## REMOVED Requirements

### Requirement: Blind send-keys excludes the supervisor pane 0

**Reason**: There is no supervisor pane, and pane 0 is the dashboard in a broker-enabled session but the first coding agent in a broker-less one, so an index rule is wrong in both directions (GP-46). The Rust approval-send gate it described was already deleted by `classifier-retirement`.

**Migration**: Replaced by the role-based *Keystrokes reach only coding-agent surfaces*. `sweep.sh approve` refuses any pane its discovery roster does not list as a coding agent (`broker-agent-helper`, *sweep.sh approve refuses pane 0*), and the drive loop types only into coding-agent surfaces (`supervisor-unattended-operation`).
