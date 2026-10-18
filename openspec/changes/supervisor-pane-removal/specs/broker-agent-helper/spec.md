## MODIFIED Requirements

### Requirement: sweep.sh approve refuses pane 0

The `sweep.sh approve <pane>` subcommand SHALL send keystrokes only to a pane that the session's per-repo discovery roster (`agents[]` in `.git-paw/sessions/<session>.json`) lists as a coding agent's `pane_index`. For any other pane — the dashboard, or a pane no roster entry claims — it SHALL send no keystrokes and SHALL report that the pane is excluded from blind send-keys because it is not a coding agent. When no discovery roster can be read, the subcommand SHALL fall back to refusing pane 0. The roster check SHALL run before the live-prompt re-confirm and the option-1 guard, which are otherwise unchanged (*sweep.sh approve re-confirms a live prompt before sending keys*).

#### Scenario: approve of the dashboard pane is rejected

- **GIVEN** a broker-enabled session whose discovery roster lists coding agents at panes 1 and 2
- **WHEN** `sweep.sh approve 0` is invoked
- **THEN** it SHALL send NO keystrokes to pane 0
- **AND** it SHALL report that pane 0 is excluded from blind send-keys because it is not a coding agent

#### Scenario: approve of a pane no roster entry claims is rejected

- **GIVEN** a discovery roster listing coding agents at panes 1 and 2
- **WHEN** `sweep.sh approve 5` is invoked
- **THEN** it SHALL send NO keystrokes

#### Scenario: approve of a roster agent's pane proceeds

- **GIVEN** a discovery roster listing a coding agent at pane 1 showing a live permission prompt whose option `1` is a one-time "Yes"
- **WHEN** `sweep.sh approve 1` is invoked
- **THEN** it SHALL send `1` and then `Enter` to pane 1

#### Scenario: Without a roster pane 0 is still refused

- **GIVEN** no readable discovery roster for the session
- **WHEN** `sweep.sh approve 0` is invoked
- **THEN** it SHALL send NO keystrokes to pane 0
