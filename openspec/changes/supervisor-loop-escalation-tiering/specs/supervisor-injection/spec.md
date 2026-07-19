## ADDED Requirements

### Requirement: Drive-loop coordination in the supervisor boot context

When a session runs `--unattended` (an in-process drive loop is auto-approving classifier-safe prompts), git-paw SHALL inject into the supervisor's boot context a directive stating that:

- a drive loop is running and owns mechanical approval of classifier-safe prompts;
- the supervisor SHALL consume the loop's escalations rather than blanket-approving prompts by sweeping panes;
- the supervisor handles the reasoning-level work the loop cannot — escalated non-safe prompts, verification, merge orchestration, and conflict handling.

When the session is NOT unattended (no drive loop), the boot context SHALL NOT contain this directive, and the supervisor operates as the sole approver (full sweep + approve).

#### Scenario: Unattended supervisor boot context announces the drive loop

- **GIVEN** a supervisor session started with `--unattended`
- **WHEN** the supervisor's boot context is assembled
- **THEN** it SHALL contain the directive that a drive loop owns safe-prompt approval and the supervisor consumes escalations

#### Scenario: Attended supervisor boot context omits the drive-loop directive

- **GIVEN** a supervisor session started WITHOUT `--unattended`
- **WHEN** the supervisor's boot context is assembled
- **THEN** it SHALL NOT contain the drive-loop coordination directive
