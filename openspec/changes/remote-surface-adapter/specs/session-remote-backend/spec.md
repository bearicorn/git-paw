## Purpose

Defines git-paw's remote terminal backend: an implementation of the terminal port in which a
worker's surface is a coding session that a remote agent vendor runs on its own machines, started
from the branch's pushed base. git-paw never runs the work locally; it follows the session from the
session's broker host. It escalates the session's questions to the orchestrator, sends the
orchestrator's answers back once per question, renders the session's events for capture and
status, and stops following at a time cap or on persistent failure. This capability covers the
surface mapping, the vendor-neutral remote agent service port, the follow, its persisted state and
the session-file record. The launch gate, landing the session's patch and recovery are covered by
`remote-worker-landing`; the Jules adapter by `remote-worker-jules`.

## ADDED Requirements

### Requirement: A remote surface is a vendor session that git-paw follows

The remote backend SHALL implement the terminal port such that a surface is a coding session run by a remote agent vendor, which git-paw follows and never runs locally. Spawning a surface with a remote task plan that names no vendor session SHALL create exactly one vendor session from the plan's repository, base branch, title and task prompt, and SHALL record the session's name and URL before returning; spawning with a plan that names a recorded vendor session SHALL re-attach to that session and SHALL create none. Spawning without a remote task plan SHALL fail with an error naming the surface and SHALL create nothing. Listing a session's surfaces SHALL report each remote surface with the local worktree its work lands in as its owner and with no layout position. Every remote surface SHALL report that it does not present an interactive screen, and SHALL report its activity as unknown. No local process SHALL be started for a remote surface.

#### Scenario: Spawning creates one vendor session

- **GIVEN** the fake remote agent service and a remote task plan for worktree `/repo/.git-paw/worktrees/feat-r` that names no vendor session
- **WHEN** a remote surface is spawned with that plan
- **THEN** the fake SHALL have recorded exactly one session creation carrying the plan's repository, base branch, title and prompt
- **AND** listing the session's surfaces SHALL report the surface with owner `/repo/.git-paw/worktrees/feat-r` and no position
- **AND** the surface SHALL be reported non-interactive with activity unknown

#### Scenario: Re-attaching creates nothing

- **GIVEN** the fake remote agent service and a remote task plan naming the recorded vendor session `sessions/abc123`
- **WHEN** a remote surface is spawned with that plan
- **THEN** the fake SHALL have recorded no session creation
- **AND** every later read for the surface SHALL address `sessions/abc123`

#### Scenario: A spec without a remote task plan is refused

- **GIVEN** the remote backend
- **WHEN** a surface is spawned without a remote task plan
- **THEN** the spawn SHALL fail with an error naming the surface
- **AND** no session creation SHALL have been recorded

### Requirement: Text and Enter send one message; operations a remote surface lacks are unsupported

The remote backend SHALL treat sent text as an unsent draft of the surface: `send_text` SHALL append to the draft, `send_key(Enter)` SHALL send the whole draft to the vendor session as one message and empty the draft, and the clear-line key SHALL discard the draft. An `Enter` on an empty draft SHALL send nothing. The draft SHALL persist across git-paw processes. Every other key, leaving text in a composer, re-running a launch command, choosing a dialog option, and starting, stopping or probing a service SHALL fail with the typed unsupported-operation error naming the remote backend and the operation, and SHALL send nothing; reading the foreground process SHALL report that no process can be read.

#### Scenario: Text then Enter is one message

- **GIVEN** a remote surface with an empty draft on the fake remote agent service
- **WHEN** the text `please continue` is sent and then `Enter`
- **THEN** the fake SHALL have recorded exactly one message, with the content `please continue`, to that surface's vendor session

#### Scenario: Enter on an empty draft sends nothing

- **GIVEN** a remote surface with an empty draft
- **WHEN** `Enter` is sent
- **THEN** the fake SHALL have recorded no message

#### Scenario: Interactive-only and service operations are unsupported

- **GIVEN** a remote surface
- **WHEN** a digit key is sent, text is left in its composer, its launch command is re-run, dialog option 1 is chosen and a service is spawned
- **THEN** each call SHALL return the unsupported-operation error naming the remote backend
- **AND** the fake SHALL have recorded no message and no other vendor call for them

#### Scenario: A draft written by one process is sent by another

- **GIVEN** a remote surface to which one git-paw process sent the text `answer: use option B` without `Enter`
- **WHEN** a second git-paw process sends `Enter` to the same surface
- **THEN** one message with the content `answer: use option B` SHALL be sent

### Requirement: Capture and streaming render the vendor session's events as plain text

`capture_tail` on a remote surface SHALL return the last lines of the rendering of the vendor session's events, and `stream_to` SHALL append to its destination the rendering of every event git-paw observes after the call. Events SHALL be ordered by the time the vendor created them, never by their position in a vendor listing, and each event SHALL be rendered once. A rendering SHALL contain no control character other than the line break. A read that fails SHALL be reported as no output rather than as empty output. The destination SHALL be created owner-only when absent, appended to when present, and SHALL NOT be written when it is a symbolic link; a destination that cannot be written SHALL be reported in the follow log and SHALL NOT stop the follow.

#### Scenario: Events render in creation order

- **GIVEN** a fake session whose event listing returns an event created at 10:02 before an event created at 10:01
- **WHEN** the surface's tail is captured
- **THEN** the 10:01 event's rendering SHALL precede the 10:02 event's rendering

#### Scenario: Control characters are removed

- **GIVEN** a fake session whose agent-message event text contains an ANSI escape sequence and a carriage return
- **WHEN** the surface's tail is captured
- **THEN** the result SHALL contain the message's printable text
- **AND** it SHALL contain no escape byte and no carriage return

#### Scenario: A failed read is not an empty tail

- **GIVEN** a fake session whose event listing fails with a network error
- **WHEN** the surface's tail is captured
- **THEN** the result SHALL be no output, distinguishable from an empty tail

#### Scenario: Streaming appends each event once

- **GIVEN** a remote surface streaming to a capture file, and a fake session that gains one event between two follow reads
- **WHEN** both reads complete
- **THEN** the capture file SHALL hold each event's rendering exactly once, in creation order

#### Scenario: A symlinked destination is refused

- **GIVEN** a stream destination that is a symbolic link to a file outside the session record
- **WHEN** the follow observes a new event
- **THEN** the link target SHALL be unchanged
- **AND** the follow SHALL continue

### Requirement: Ending a remote surface stops git-paw's follow and never claims to stop the vendor's work

Because git-paw cannot cancel a vendor session, killing a remote surface SHALL end git-paw's follow of it and remove its local follow state, and SHALL call no vendor operation that changes the session. Every command that ends a remote surface or a session holding one — `remove`, `stop` and `purge` — SHALL print, for each such surface, its label, the vendor session's URL and that the session may keep running on the vendor. Because a removed worker's vendor session is no longer followed or re-attached, `remove` SHALL refuse a remote worker whose vendor session's last-known state is neither completed nor failed unless `--force` is given, as it refuses uncommitted work, naming the worker, the URL and that the session may still be running, and SHALL then change nothing. Closing a session SHALL end the follow of every remote surface in it. Presenting a session to the user SHALL print each remote surface's label, last-known vendor state and URL, SHALL send nothing to the vendor, and SHALL then return. Detaching clients, applying a layout and re-flowing the grid SHALL be no-ops for the remote backend.

#### Scenario: Killing a remote surface calls nothing that changes the session

- **GIVEN** a followed remote surface on the fake remote agent service
- **WHEN** the surface is killed
- **THEN** the fake SHALL have recorded no message and no other call that changes the session
- **AND** the surface SHALL no longer be listed and its follow state SHALL no longer exist

#### Scenario: remove names the session that keeps running

- **GIVEN** a session with remote worker `feat-r` whose vendor session URL is `https://vendor.example/s/abc123`
- **WHEN** `git paw remove feat/r --force` runs
- **THEN** its output SHALL name `feat-r` and `https://vendor.example/s/abc123`
- **AND** it SHALL state that the session may keep running on the vendor

#### Scenario: remove refuses a worker whose session may still be running

- **GIVEN** a session with remote worker `feat-r` whose vendor session's last-known state is working, at `https://vendor.example/s/abc123`
- **WHEN** `git paw remove feat/r` runs without `--force`
- **THEN** it SHALL exit with code 1 naming `feat-r`, `https://vendor.example/s/abc123` and `--force`
- **AND** the worker's worktree, session entry and follow SHALL be unchanged

#### Scenario: Presenting a session prints the remote sessions

- **GIVEN** a session holding remote worker `feat-r` whose last-known state is working
- **WHEN** the remote backend presents the session
- **THEN** it SHALL print `feat-r`, its state and its URL and return
- **AND** the fake SHALL have recorded no message

### Requirement: The remote agent service port speaks git-paw's terms

The remote backend SHALL reach a vendor only through a compile-time `RemoteAgentService` trait whose operations are: confirming that a repository is visible to the vendor without starting any work; creating a session from a repository, a base branch, a title and a prompt; reading a session's state and last-update time; listing a session's events; sending a session a message; and finding sessions by title. A session's state SHALL be one of a closed set — queued, planning, working, awaiting feedback, awaiting plan approval, paused, completed, failed or unknown. An event SHALL be one of a closed set — an agent message, a proposed plan, progress, a patch (carrying its base commit, its unified diff and a suggested commit message), completion, failure (carrying its reason) or other — and SHALL carry the vendor's identifier for it and its creation time. Failures SHALL be typed as limit, authentication, not found, bad response, network or other. A vendor value an adapter does not recognise SHALL map to the unknown state or the other event, never to a failure. The trait's identifiers SHALL name no vendor; its adapters SHALL be in-tree and selected at construction; and a fake implementation SHALL let every follow decision be tested without a network.

#### Scenario: An unrecognised state keeps the follow going

- **GIVEN** the fake remote agent service scripted to report the unknown state on every read
- **WHEN** the follow runs three reads within the time cap
- **THEN** it SHALL still be following the session
- **AND** it SHALL have published no escalation

#### Scenario: The port names no vendor

- **WHEN** the public identifiers of the `RemoteAgentService` trait and its associated types are inspected
- **THEN** none SHALL contain the tokens `jules`, `google`, `codex`, `copilot`, `claude` or `github`

### Requirement: Questions and plan approvals escalate, and git-paw approves nothing

While git-paw follows a remote worker's session, it SHALL publish on the worker's behalf one `agent.question` per distinct question when the session awaits feedback, and one per distinct plan when the session awaits plan approval. A question's text SHALL be the newest agent-message event created at or before the session's last update or, when there is none, a fixed text stating that the session awaits feedback. A plan's text SHALL be the newest proposed-plan event, and SHALL state that git-paw approves no plan and that the plan can be approved in the vendor's own interface. Each SHALL carry the session's URL and SHALL present the vendor's words as quoted text from the remote agent. A question or plan SHALL be distinct by its event's identifier or, when there is no event, by the state together with the session's last-update time. git-paw SHALL NOT call any vendor operation that approves a plan, and SHALL NOT answer any question itself.

#### Scenario: A question seen on three reads is escalated once

- **GIVEN** a fake session that awaits feedback with the same agent-message event on three consecutive reads
- **WHEN** the follow processes the three reads
- **THEN** exactly one `agent.question` SHALL have been published for the worker
- **AND** it SHALL carry the event's text and the session's URL

#### Scenario: Two questions are escalated separately

- **GIVEN** a fake session that awaits feedback with agent-message event `e1`, then works, then awaits feedback with agent-message event `e2`
- **WHEN** the follow processes those reads
- **THEN** two `agent.question` messages SHALL have been published, one for `e1` and one for `e2`

#### Scenario: A plan awaiting approval is escalated and never approved

- **GIVEN** a fake session that awaits plan approval with a proposed-plan event
- **WHEN** the follow processes the read
- **THEN** one `agent.question` SHALL have been published stating that git-paw approves no plan
- **AND** the fake SHALL have recorded no call that approves a plan and no message

#### Scenario: A question with no message uses the fixed text

- **GIVEN** a fake session that awaits feedback and has no agent-message event
- **WHEN** the follow processes the read
- **THEN** one `agent.question` SHALL have been published with the fixed awaits-feedback text and the session's URL

### Requirement: Answers are sent to the vendor session once per distinct question

When an `agent.answer` addressed to a remote worker is observed while that worker has an escalated question to which no answer has been sent, git-paw SHALL record that question as answered and then send the answer's text to the worker's vendor session as one message. Any other `agent.answer` addressed to the worker — one for a question already answered, or one observed while no escalated question is open — SHALL NOT be sent and SHALL be recorded in the follow log. Broadcast messages, `agent.blocked` and messages addressed to other workers SHALL NOT be sent to a vendor session. An `agent.feedback` addressed to a remote worker SHALL be sent only under the verify-continuation rule of remote-worker landing.

#### Scenario: An answer is sent once

- **GIVEN** a remote worker with one escalated, unanswered question
- **WHEN** an `agent.answer` for the worker with the text `use the existing helper` is observed
- **THEN** the fake SHALL have recorded exactly one message, `use the existing helper`, to the worker's session

#### Scenario: A second answer to the same question is not sent

- **GIVEN** a remote worker whose only escalated question has already had an answer sent
- **WHEN** another `agent.answer` for the worker is observed
- **THEN** no further message SHALL be sent
- **AND** the follow log SHALL record that the answer was not sent

#### Scenario: An answer with no open question is not sent

- **GIVEN** a remote worker with no escalated question
- **WHEN** an `agent.answer` for the worker is observed
- **THEN** no message SHALL be sent

#### Scenario: Broadcasts never reach a vendor session

- **GIVEN** a followed remote worker
- **WHEN** other workers publish `agent.artifact`, `agent.intent` and an `agent.blocked` naming the remote worker
- **THEN** no message SHALL be sent to the remote worker's session

### Requirement: The follow is bounded in time and in consecutive failures

git-paw SHALL read each followed vendor session's state at most once every 60 seconds, and SHALL list its events only on the first read of a follow and when the state or the last-update time changed since the previous read. It SHALL stop following a session, publishing on the worker's behalf one escalation — an `agent.question` carrying the verdict label and the session's URL — when: the follow has run longer than the configured follow time cap, measured from the latest of the session's creation, git-paw re-attaching to it and the last message git-paw sent to it (`remote-time-cap`); three consecutive reads have failed (`remote-unreachable`); a read has failed authentication, which ends the follow at once (`remote-auth`); or the session has failed, in which case the escalation SHALL carry the newest failure event's reason (`remote-failed`). A successful read SHALL reset the consecutive-failure count. A session that is queued, planning, working, paused or in the unknown state SHALL keep being followed until one of those conditions holds; a completed session SHALL be handled by remote-worker landing. A follow that has ended SHALL resume only when git-paw re-attaches to the session.

#### Scenario: Two failures are tolerated

- **GIVEN** a fake session whose reads fail twice and then succeed
- **WHEN** the follow processes the three reads
- **THEN** it SHALL still be following the session
- **AND** no escalation SHALL have been published

#### Scenario: The third consecutive failure ends the follow

- **GIVEN** a fake session whose reads fail three times in a row
- **WHEN** the follow processes the three reads
- **THEN** it SHALL have published one `remote-unreachable` escalation carrying the session's URL
- **AND** the fake SHALL record no further read of that session

#### Scenario: An authentication failure ends the follow at once

- **GIVEN** a fake session whose first read fails authentication
- **WHEN** the follow processes the read
- **THEN** it SHALL have published one `remote-auth` escalation and stopped following

#### Scenario: The time cap ends the follow

- **GIVEN** a follow time cap of 4 hours and a fake session that has been working since git-paw created it 4 hours and 1 minute ago, with no message sent since
- **WHEN** the follow decision runs
- **THEN** it SHALL publish one `remote-time-cap` escalation carrying the session's URL
- **AND** it SHALL read the session no further

#### Scenario: A failed session is escalated with its reason

- **GIVEN** a fake session that reports the failed state with a failure event whose reason is `could not install dependencies`
- **WHEN** the follow processes the read
- **THEN** it SHALL publish one `remote-failed` escalation carrying that reason and the session's URL
- **AND** it SHALL read the session no further

#### Scenario: Events are listed only when the session changed

- **GIVEN** a fake session whose state and last-update time are the same on two consecutive reads
- **WHEN** the follow processes both reads
- **THEN** the fake SHALL have recorded one event listing, for the first read

### Requirement: The follow publishes a remote worker's status when its session changes state

Because a remote agent cannot reach the broker, git-paw SHALL publish the remote worker's `agent.status` on its behalf each time the follow observes the vendor session enter a different state, and at no other time: with status `working` for the queued, planning, working, paused and unknown states, `waiting` for the awaiting-feedback and awaiting-plan-approval states, and `failed` for the failed state, and with an activity phrase naming the vendor state. The completed state SHALL be reported by remote-worker landing. git-paw SHALL publish no status for a remote worker whose state has not changed since its previous status.

#### Scenario: A state change publishes one status

- **GIVEN** a followed remote worker whose fake session moves from queued to working and then stays working for three reads
- **WHEN** the follow processes those reads
- **THEN** the broker log SHALL hold exactly two `agent.status` messages from the worker, `working` for queued and `working` for working, each with an activity phrase naming the vendor state

#### Scenario: Awaiting feedback reads as waiting

- **GIVEN** a followed remote worker whose fake session enters the awaiting-feedback state
- **WHEN** the follow processes the read
- **THEN** the worker's newest `agent.status` SHALL have status `waiting`

### Requirement: The follow runs in the broker host of every session with a remote worker

The broker host of a session that holds at least one remote worker SHALL run the follow of every remote worker of the session on a thread of its own, with its own cadence, in attended and unattended sessions alike, and SHALL begin following a remote worker added while it runs. The follow SHALL stop when the broker host stops, including when the session is paused, and SHALL leave every vendor session running.

#### Scenario: An attended session escalates without the drive loop

- **GIVEN** an attended session (no drive loop) with a remote worker whose fake session awaits feedback
- **WHEN** the broker host runs for one follow cadence
- **THEN** the broker log SHALL hold one `agent.question` from that worker

#### Scenario: A worker added later is followed

- **GIVEN** a running broker host for a session with no remote worker
- **WHEN** `git paw add` attaches a remote worker to the session
- **THEN** the broker host SHALL begin reading that worker's vendor session

#### Scenario: Pausing stops the follow

- **GIVEN** a session with a followed remote worker
- **WHEN** `git paw pause` runs
- **THEN** the fake SHALL record no read of the worker's session after the broker host stopped
- **AND** the fake SHALL have recorded no call that changes the session

### Requirement: Remote follow state is owner-only, outside the repository, and recorded before each vendor action

The remote backend SHALL keep each remote surface's draft and follow state — the vendor session's name, URL and title, the escalated question and plan keys, the answered question keys, the events already rendered, the follow's start, the last message sent and the consecutive-failure count — under the user data directory next to the session file, never inside the repository or any worktree. It SHALL create its directories with mode `0700` and its files with mode `0600`, and SHALL build every path from the validated session-name and branch-slug types. Before every vendor operation that sends a message, it SHALL record the message as sent, so a git-paw process that stops between recording and calling never sends it again; before every vendor operation that creates a session, it SHALL record the session's title, so a later process looks the session up by that title before creating another. The follow state SHALL hold no credential.

#### Scenario: Follow state permissions and location

- **GIVEN** a session with a followed remote worker
- **WHEN** its follow state is inspected
- **THEN** every directory SHALL have mode `0700` and every file mode `0600`
- **AND** no follow-state file SHALL be inside the repository

#### Scenario: A recorded answer is not sent again after a restart

- **GIVEN** a follow that recorded an answer as sent and stopped before the vendor call
- **WHEN** a new broker host resumes the follow
- **THEN** the fake SHALL have recorded no message carrying that answer

#### Scenario: Credentials are never in follow state

- **GIVEN** a git-paw process whose environment holds a vendor key `EXAMPLE_KEY_abc123`
- **WHEN** it follows a remote worker through creation, a question and an answer
- **THEN** no follow-state file SHALL contain `EXAMPLE_KEY_abc123`

### Requirement: The session file records each remote worker's vendor session

The `WorktreeEntry` of a remote worker SHALL record its vendor session in an optional `remote` field carrying the vendor's name, the vendor session's name and URL, the title git-paw gave it and the base commit it started from. The field SHALL be annotated `#[serde(default, skip_serializing_if = "Option::is_none")]`, so a session file without it loads unchanged and an entry of a tmux or headless worker serializes byte-identically to before. It SHALL hold no credential.

#### Scenario: A remote worker's record round-trips

- **GIVEN** a remote worker entry recording vendor `jules`, session `sessions/abc123`, its URL, its title and base commit `0f1e…`
- **WHEN** the session is saved and loaded back
- **THEN** the entry's `remote` field SHALL carry the same values

#### Scenario: Other entries are unchanged

- **GIVEN** a session whose worktree entries are all tmux workers
- **WHEN** it is saved by this release and by the previous release
- **THEN** the two files SHALL be byte-identical
