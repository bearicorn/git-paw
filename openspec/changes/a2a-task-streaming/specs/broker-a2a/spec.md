## ADDED Requirements

### Requirement: SubscribeToTask streams a task's updates over SSE

An authenticated `SubscribeToTask` for a task in a non-terminal state SHALL answer HTTP `200` with `Content-Type: text/event-stream`. Each SSE `data:` line SHALL be a JSON-RPC 2.0 response with the request's `id`, whose `result` is an A2A `StreamResponse`.

The first event SHALL carry the task's current state as `task`. Every later change to the task SHALL follow as a `statusUpdate` or an `artifactUpdate`, in broker order, with no event repeated and none missing. The stream SHALL close after the event that brings the task to a terminal state.

A task that is already terminal SHALL get JSON-RPC error `-32004`. An unknown task SHALL get JSON-RPC error `-32001`. Neither SHALL be a stream.

#### Scenario: Subscriber sees the snapshot then each update

- **GIVEN** a `WORKING` task for `feat-x`
- **WHEN** a client subscribes, and then `feat-x` publishes `agent.artifact` (`committed`) followed by `agent.question`
- **THEN** the first event SHALL carry `task` with state `TASK_STATE_WORKING`
- **AND** an `artifactUpdate` event SHALL follow
- **AND** a `statusUpdate` event with state `TASK_STATE_INPUT_REQUIRED` SHALL follow, in that order

#### Scenario: Stream closes at completion

- **GIVEN** a subscribed client on a task awaiting its verdict
- **WHEN** an authenticated `SendMessage` records `verdict = "verified"`
- **THEN** the stream SHALL deliver a `statusUpdate` with state `TASK_STATE_COMPLETED` and then close

#### Scenario: Subscribing to a terminal task is refused

- **GIVEN** a `COMPLETED` task
- **WHEN** a client calls `SubscribeToTask` for it
- **THEN** the response SHALL be a JSON-RPC error with code `-32004`, not an event stream

### Requirement: SendStreamingMessage delivers then streams

`SendStreamingMessage` SHALL validate and deliver its message exactly as `SendMessage` does. That includes every refusal, the verdict handling and `messageId` idempotency. It SHALL then answer with a stream of the target task in the form that `SubscribeToTask` defines. A refused request SHALL get a JSON-RPC error and SHALL NOT be a stream.

#### Scenario: Guidance then live updates

- **GIVEN** an `INPUT_REQUIRED` task for `feat-x`
- **WHEN** an authenticated `SendStreamingMessage` delivers "use the v2 parser"
- **THEN** the first event SHALL carry `task` with state `TASK_STATE_WORKING`
- **AND** the worker's next `agent.artifact` (`done`) SHALL arrive as an `artifactUpdate`, followed by a `statusUpdate` with state `TASK_STATE_INPUT_REQUIRED`

#### Scenario: Refused message is not a stream

- **WHEN** `SendStreamingMessage` targets a `COMPLETED` task
- **THEN** the response SHALL be a JSON-RPC error with code `-32004`

### Requirement: Concurrent streams receive identical sequences, and a lagging stream is closed

Every active stream for a task SHALL receive the same events in the same order. Closing one stream SHALL NOT affect another. Pending events SHALL be held in a bounded buffer. A subscriber that falls behind the buffer SHALL have its stream closed. It SHALL NOT receive a sequence with a gap.

#### Scenario: Two subscribers see the same order

- **GIVEN** two clients subscribed to `feat-x`'s task
- **WHEN** `feat-x` publishes three events and the first client then disconnects
- **THEN** both clients SHALL have received the same three updates in the same order
- **AND** the second client's stream SHALL stay open

#### Scenario: Lagging subscriber is closed, not gapped

- **GIVEN** a subscriber that reads nothing while more events than the buffer holds are published for its task
- **WHEN** it resumes reading
- **THEN** its stream SHALL end without delivering any event after the gap
- **AND** a new subscription SHALL begin with the current `task` snapshot

### Requirement: Agent Card advertises streaming while it is served

While streaming is served, the Agent Card SHALL set `capabilities.streaming` to `true`.

#### Scenario: Card shows streaming

- **GIVEN** the A2A face served with streaming
- **WHEN** the card is fetched
- **THEN** `capabilities.streaming` SHALL be `true`
