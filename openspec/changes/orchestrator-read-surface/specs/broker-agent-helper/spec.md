## ADDED Requirements

### Requirement: sweep.sh events long-polls the broker log

The bundled `sweep.sh` SHALL provide an `events <since> [wait]` subcommand that calls the session broker's `GET /log?since=<since>&wait=<wait>` (default wait 30 seconds) and the broker's `/status`, and prints one JSON object `{entries, last_seq, wave}` to stdout. It SHALL resolve the session and broker the same way its other subcommands do, SHALL print a JSON object with empty `entries` and `no_session: true` when no broker is reachable, and SHALL send no keystrokes and publish nothing.

#### Scenario: events prints new entries and the cursor

- **GIVEN** a live broker at sequence 3
- **WHEN** `sweep.sh events 3 5` runs and an agent publishes during the wait
- **THEN** stdout SHALL be a JSON object whose `entries` holds the new message and whose `last_seq` is 4

#### Scenario: events reports a stalled wave on timeout

- **GIVEN** a live broker whose wave is stalled and no publisher
- **WHEN** `sweep.sh events 3 1` runs
- **THEN** stdout SHALL contain `"entries": []`, `last_seq` 3 and a `wave` object with `stalled` true

#### Scenario: events without a session degrades

- **GIVEN** no active session for the repository
- **WHEN** `sweep.sh events 0` runs
- **THEN** stdout SHALL be a JSON object with `no_session` true and the exit status SHALL be 0
