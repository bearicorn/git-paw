## MODIFIED Requirements

### Requirement: Drive-loop coordination in the supervisor boot context

When a session runs `--unattended` (an in-process drive loop that escalates every live permission prompt and approves none), git-paw SHALL inject into the supervisor's boot context a directive stating that:

- a drive loop is running; it sweeps every pane, records each live permission prompt, judgment call and merge decision on the broker, and hands it to this pane;
- no git-paw component approves permission prompts. The supervisor SHALL relay each escalated permission prompt to the human and SHALL run `sweep.sh approve <pane>` only when the human explicitly tells it to;
- the supervisor handles the reasoning-level work the loop cannot: relaying prompts, verification, merge orchestration, and conflict handling.

When the session is NOT unattended (no drive loop), the boot context SHALL NOT contain this directive. The supervisor then finds stuck-on-prompt agents through its own sweep and relays their prompts to the human in the same way.

#### Scenario: Unattended supervisor boot context announces the drive loop

- **GIVEN** a supervisor session started with `--unattended`
- **WHEN** the supervisor's boot context is assembled
- **THEN** it SHALL contain the directive that a drive loop escalates prompts to this pane and that no component approves them
- **AND** it SHALL tell the supervisor to relay each permission prompt to the human

#### Scenario: Attended supervisor boot context omits the drive-loop directive

- **GIVEN** a supervisor session started WITHOUT `--unattended`
- **WHEN** the supervisor's boot context is assembled
- **THEN** it SHALL NOT contain the drive-loop coordination directive

#### Scenario: No boot context tells the supervisor to approve by pattern

- **WHEN** the supervisor's boot context is assembled for an attended or an `--unattended` session
- **THEN** it SHALL NOT instruct the supervisor to approve prompts it judges safe
- **AND** it SHALL NOT instruct the supervisor to select a "don't ask again" option
