## MODIFIED Requirements

### Requirement: Profile applies to supervisor and agent panes

The split-send + resolved delay SHALL apply to every launched coding-agent pane, each resolved from that agent's own CLI. No orchestrator pane exists in the session, so no boot block is submitted for an orchestrator CLI; the orchestrator's CLI is resolved only for git-paw-initiated headless invocations, never for a pane.

#### Scenario: Coding-agent boot blocks are submitted with their own CLI's delay

- **GIVEN** a supervised session with two coding agents running different CLIs
- **WHEN** each agent's boot block is injected
- **THEN** each SHALL be submitted via the split-send using that agent's CLI's resolved delay, so the agent begins work without a manual `Enter`

#### Scenario: No pane receives an orchestrator boot block

- **GIVEN** a supervised session with `[supervisor] cli` set
- **WHEN** the launch injects boot blocks
- **THEN** no split-send SHALL target a pane with the orchestrator CLI's resolved delay
