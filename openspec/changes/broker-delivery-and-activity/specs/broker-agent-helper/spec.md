## ADDED Requirements

### Requirement: Root-side message verbs report the sequence number

`sweep.sh feedback-gate`, `verified`, `freeze`, `answer` and `conflict-ack` SHALL print the sequence number of each message the broker accepted, as `seq <N>`, in their success line. A broker that returns no sequence number SHALL leave the success line without it.

#### Scenario: feedback-gate prints the seq

- **GIVEN** a broker that answers the publish with `{"seq": 42}`
- **WHEN** the orchestrator runs `sweep.sh feedback-gate feat-x testing "rebase onto main"`
- **THEN** the output SHALL contain `seq 42`

### Requirement: sweep.sh delivery reports whether a message was read

`sweep.sh delivery <agent> [<seq>]` SHALL read `GET /status` and print `read` or `unread` for the given `seq`. Without a `seq` it SHALL list the agent's unread orchestrator messages with their `seq` and age. It SHALL exit `0` when everything asked about is read and `1` otherwise.

#### Scenario: Unread feedback is reported

- **GIVEN** orchestrator feedback `seq` 42 for `feat-x` that `feat-x` has not polled past
- **WHEN** the orchestrator runs `sweep.sh delivery feat-x 42`
- **THEN** the output SHALL be `unread` and the exit status SHALL be `1`

#### Scenario: Read feedback is reported

- **GIVEN** `feat-x` has polled with `since=42`
- **WHEN** the orchestrator runs `sweep.sh delivery feat-x 42`
- **THEN** the output SHALL be `read` and the exit status SHALL be `0`
