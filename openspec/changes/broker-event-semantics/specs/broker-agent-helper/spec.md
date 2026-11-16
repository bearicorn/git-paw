## ADDED Requirements

### Requirement: sweep.sh verified and advanced-main send the tips they mean

`sweep.sh verified <agent> <message>` SHALL include `sha`, the agent worktree's `HEAD`, unless `--sha <sha>` is given. `sweep.sh advanced-main <branch> <summary>` SHALL include `merged_sha`, the merged branch's tip resolved in the main checkout. An unresolvable tip SHALL exit non-zero without publishing.

#### Scenario: verified carries the worktree tip

- **GIVEN** agent `feat-x` whose worktree `HEAD` is `aaa111`
- **WHEN** the orchestrator runs `sweep.sh verified feat-x "all gates pass"`
- **THEN** the published `agent.verified` SHALL carry `sha = "aaa111"`

#### Scenario: advanced-main carries the merged tip

- **GIVEN** branch `feat/x` at `bbb222` just merged
- **WHEN** the orchestrator runs `sweep.sh advanced-main feat/x "landed x"`
- **THEN** the published `agent.advanced-main` SHALL carry `merged_sha = "bbb222"`

### Requirement: broker.sh artifact carries the worktree tip

`broker.sh artifact` SHALL include `sha`, the worktree's `HEAD`, in the `done` artifact it publishes. In a worktree with no commit it SHALL omit `sha` and still publish.

#### Scenario: The change-complete artifact names the tip

- **GIVEN** an agent worktree whose `HEAD` is `def456`
- **WHEN** the agent runs `broker.sh --agent feat-x artifact`
- **THEN** the published `agent.artifact` SHALL carry `status = "done"` and `sha = "def456"`

### Requirement: sweep.sh freeze marks a branch under verification

`sweep.sh freeze <agent>` SHALL write `paw-frozen`, holding the agent worktree's `HEAD`, into that worktree's linked gitdir and publish `orchestrator.freeze` with that SHA and the authority credential. `sweep.sh verified` and `sweep.sh feedback-gate` SHALL remove the marker before publishing.

#### Scenario: Freeze then verify

- **GIVEN** agent `feat-x` at `HEAD` `aaa111`
- **WHEN** the orchestrator runs `sweep.sh freeze feat-x` and later `sweep.sh verified feat-x "ok"`
- **THEN** `paw-frozen` SHALL hold `aaa111` between the two commands and SHALL be gone afterwards

#### Scenario: A failed freeze publish leaves no marker

- **GIVEN** a broker that rejects the publish
- **WHEN** the orchestrator runs `sweep.sh freeze feat-x`
- **THEN** the command SHALL exit non-zero and no `paw-frozen` SHALL remain

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
