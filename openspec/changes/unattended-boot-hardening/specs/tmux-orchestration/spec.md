## ADDED Requirements

### Requirement: Launch gate handles a first-run permission/trust acceptance dialog

The readiness gate SHALL, when the launch-readiness poll observes a pane presenting
a first-run acceptance dialog for its CLI — a one-time bypass-permissions
confirmation (e.g. the dialog Claude Code shows the first time
`--dangerously-skip-permissions` runs against a config directory that never accepted
it) or a "trust this folder?" prompt — either answer the dialog to reach the
interactive ready state, or fail the launch loudly. It SHALL NOT treat the dialog as a bare shell and relaunch
(which would re-open the dialog), and SHALL NOT treat it as ready and inject the
boot block on top of it. A pane left blocked on an unanswered acceptance dialog
SHALL NOT be reported as a healthy, running agent. Dialog detection SHALL be
per-CLI and conservative: a CLI whose acceptance dialog the gate does not recognise
SHALL fall back to the existing readiness / relaunch / fixed-budget behaviour, so
launch is never worse than before.

#### Scenario: First-run bypass acceptance dialog is answered, not relaunched

- **GIVEN** an agent pane launched with a full-auto bypass flag against a config directory that has never accepted bypass, showing the first-run acceptance dialog
- **WHEN** the readiness gate polls the pane
- **THEN** the gate SHALL answer the acceptance dialog (or fail the launch loudly) rather than relaunching the CLI into the dialog

#### Scenario: A pane stuck on an acceptance dialog is not reported healthy

- **GIVEN** an agent pane blocked on an unanswered first-run acceptance dialog after the launch budget elapses
- **WHEN** launch completes
- **THEN** the pane SHALL NOT be reported as a healthy running agent (the launch fails loudly or the pane's dead/blocked state is surfaced)

#### Scenario: Unrecognised CLI acceptance dialog falls back to prior behaviour

- **GIVEN** a custom CLI whose first-run dialog the gate does not recognise
- **WHEN** the readiness budget elapses
- **THEN** the gate SHALL fall back to the existing readiness / relaunch / fixed-budget injection behaviour
