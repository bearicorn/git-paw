## ADDED Requirements

### Requirement: sweep.sh approve re-confirms a live prompt before sending keys

The bundled `assets/scripts/sweep.sh` `approve <pane>` subcommand SHALL pass through the `broker-mediated-approvals` approval-send gate. Immediately before sending the sticky-yes keystrokes (`Down` then `Enter`), the subcommand SHALL run a fresh `tmux capture-pane` of the target pane and SHALL confirm a live permission-prompt marker is present within the last 4 non-blank lines of that capture. When the re-confirm capture shows no live prompt in the tail (the prompt has cleared), the subcommand SHALL send NO keystrokes and SHALL report that the prompt cleared.

#### Scenario: approve sends keys only when the prompt is still live

- **GIVEN** `sweep.sh approve <pane>` is invoked for a coding-agent pane whose fresh capture shows a permission-prompt marker within the last 4 non-blank lines
- **WHEN** the subcommand runs
- **THEN** it SHALL send `Down` then `Enter` to the pane via `tmux send-keys`

#### Scenario: approve sends nothing when the prompt has cleared

- **GIVEN** `sweep.sh approve <pane>` is invoked for a pane whose fresh capture no longer shows a permission-prompt marker in the last 4 non-blank lines
- **WHEN** the subcommand runs
- **THEN** it SHALL send NO keystrokes to the pane
- **AND** it SHALL report that the prompt has cleared so no keys were sent

### Requirement: sweep.sh approve refuses pane 0

The `sweep.sh approve <pane>` subcommand SHALL refuse to send keystrokes when the supplied pane index is 0 (the supervisor's own pane). It SHALL send no keystrokes and SHALL report that pane 0 is excluded from blind send-keys.

#### Scenario: approve 0 is rejected

- **GIVEN** `sweep.sh approve 0` is invoked
- **WHEN** the subcommand runs
- **THEN** it SHALL send NO keystrokes to pane 0
- **AND** it SHALL report that pane 0 is excluded from blind send-keys
