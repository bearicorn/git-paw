## Purpose

Defines git-paw's adapter for Google's Jules, the first remote agent vendor behind the remote agent
service port: how it reaches the Jules REST API (pinned host, no redirects, bounded requests), how
it handles the API key, how it creates sessions from a pushed base without pull requests, how it
maps Jules states and activities onto git-paw's remote domain, how it recognises limit failures,
and how git-paw keeps its own use of Jules within the account's daily and concurrent task limits
and counts that use. Several Jules behaviours are not documented publicly; where a rule depends on
one, the rule is written to hold either way.

## ADDED Requirements

### Requirement: The Jules adapter talks only to its pinned API host

The Jules adapter SHALL send every request over HTTPS to the host `jules.googleapis.com` under the base path `/v1alpha`, both pinned in code, and SHALL refuse to build a request for any other scheme, host or base path. It SHALL NOT follow a redirect: a redirect response SHALL fail the request as a bad response. Every request SHALL time out after at most 20 seconds, and a response body larger than 16 MB, or one that is not a JSON object, SHALL fail the request as a bad response. A vendor session name that is not `sessions/` followed by one or more of the characters `A`–`Z`, `a`–`z`, `0`–`9`, `_` and `-` SHALL be refused before any request is built from it. A paged listing SHALL follow page tokens up to a fixed number of pages and SHALL fail as a bad response beyond it.

#### Scenario: Another host is refused

- **GIVEN** the Jules adapter with a recording transport
- **WHEN** it is asked to build a request for `https://example.invalid/v1alpha/sessions`
- **THEN** it SHALL refuse with an error
- **AND** the transport SHALL have recorded no request

#### Scenario: A redirect fails the request

- **GIVEN** a recording transport that answers a session read with a `302` redirect to another host
- **WHEN** the adapter reads the session
- **THEN** the read SHALL fail as a bad response
- **AND** the transport SHALL have recorded exactly one request, to `jules.googleapis.com`

#### Scenario: A malformed session name is refused

- **GIVEN** the Jules adapter
- **WHEN** it is asked to read the session `sessions/../sources`
- **THEN** it SHALL refuse with a bad-response error
- **AND** the transport SHALL have recorded no request

#### Scenario: Pages are followed

- **GIVEN** a recording transport whose activity listing returns two pages joined by a page token
- **WHEN** the adapter lists the session's events
- **THEN** the result SHALL contain the activities of both pages

### Requirement: The Jules key comes only from JULES_API_KEY and never leaves its header

The Jules adapter SHALL read its API key only from the `JULES_API_KEY` environment variable of the git-paw process that makes the request, and SHALL send it only in the `X-Goog-Api-Key` header of requests to the pinned host. The key SHALL NOT appear in any process's arguments, and SHALL NOT be written to any log, error, escalation, broker message, session file, follow state, ledger, capture or session record; every vendor-supplied text git-paw records or prints SHALL have any occurrence of the key's value replaced by `***`. When the variable is unset or empty, an operation that needs the key SHALL fail with an error naming `JULES_API_KEY` and printing no part of any key. A key that contains a line break or any other control character SHALL be refused the same way before any request is built or any process is started with it. git-paw SHALL remove `JULES_API_KEY` from the environment of every git process it runs to check, apply or commit a remote worker's work, including the hooks those processes run.

#### Scenario: The key travels only in the header

- **GIVEN** `JULES_API_KEY` set to `test-key-abc123` and a recording transport
- **WHEN** the adapter reads a session
- **THEN** the recorded request SHALL carry `X-Goog-Api-Key: test-key-abc123`
- **AND** no recorded process argument, URL or query string SHALL contain `test-key-abc123`

#### Scenario: An echoed key is masked

- **GIVEN** a recording transport whose error body echoes `test-key-abc123`
- **WHEN** the adapter's resulting error is formatted and the follow logs it
- **THEN** the error text and the follow log SHALL contain `***` and SHALL NOT contain `test-key-abc123`

#### Scenario: A missing key is named, never shown

- **GIVEN** `JULES_API_KEY` unset
- **WHEN** an operation that needs the key runs
- **THEN** it SHALL fail with an error naming `JULES_API_KEY`

#### Scenario: A key with a control character is refused before any request

- **GIVEN** a recording transport
- **WHEN** an operation that needs the key runs with `JULES_API_KEY` set, in turn, to `test-key-abc123` followed by a carriage return, to `test-key-abc123` followed by a line feed, and to `test-key-abc123` with a tab inside it
- **THEN** each SHALL fail with an error naming `JULES_API_KEY`
- **AND** no error SHALL contain `test-key-abc123`
- **AND** the transport SHALL have recorded no request

#### Scenario: Git commands do not inherit the key

- **GIVEN** `JULES_API_KEY` set in git-paw's environment and a repository whose pre-commit hook records its environment
- **WHEN** git-paw commits a remote worker's patch
- **THEN** the hook's recorded environment SHALL contain no `JULES_API_KEY`

### Requirement: Jules sessions are created from the pushed base without pull requests or plan approval

The Jules adapter SHALL create a session with one `POST /sessions` whose body carries the prompt, the title, `sourceContext.source` set to `sources/github/<owner>/<repo>`, `sourceContext.githubRepoContext.startingBranch` set to the base branch, and `requirePlanApproval` set to false, and SHALL NOT set `automationMode` to any value that opens a pull request. It SHALL confirm that a repository is visible with `GET /sources/github/<owner>/<repo>`, and, when that answers not found, by listing sources and comparing names without regard to case; neither starts a session. It SHALL find sessions by title by listing sessions, and SHALL send a message with `POST /<session>:sendMessage` carrying the text as its prompt. It SHALL call no other endpoint that changes a session, and in particular SHALL NOT call plan approval.

#### Scenario: The creation request opens no pull request

- **GIVEN** a recording transport
- **WHEN** the adapter creates a session for `octo/demo`, base branch `main`, with a title and a prompt
- **THEN** the recorded body SHALL carry `sources/github/octo/demo`, `startingBranch` `main`, the title, the prompt and `requirePlanApproval` false
- **AND** it SHALL carry no `automationMode` that opens a pull request

#### Scenario: The access check starts nothing

- **GIVEN** a recording transport that answers the source read with not found and the source listing with `sources/github/Octo/Demo`
- **WHEN** the adapter checks that `octo/demo` is visible
- **THEN** it SHALL report the repository visible
- **AND** the transport SHALL have recorded no `POST`

#### Scenario: Plan approval is never called

- **WHEN** a source audit inspects the Jules adapter
- **THEN** it SHALL find no request to an `:approvePlan` endpoint

### Requirement: Jules states and activities map onto the remote domain

The Jules adapter SHALL map session states as follows: `QUEUED` to queued, `PLANNING` to planning, `IN_PROGRESS` to working, `AWAITING_USER_FEEDBACK` to awaiting feedback, `AWAITING_PLAN_APPROVAL` to awaiting plan approval, `PAUSED` to paused, `COMPLETED` to completed, `FAILED` to failed, and `STATE_UNSPECIFIED` or any other value to unknown. It SHALL map an `agentMessaged` activity to an agent message carrying its `agentMessage`, a `planGenerated` activity to a proposed plan, a `sessionCompleted` activity to completion, a `sessionFailed` activity to a failure carrying its `reason`, and any other activity to the other event carrying its description. Each `changeSet.gitPatch` in an activity's artifacts SHALL be reported as a patch event carrying its `baseCommitId`, `unidiffPatch` and `suggestedCommitMessage`, timed at the activity's `createTime`. Because it is not yet confirmed whether a session created without automatic pull requests returns its patch in its activities or in its `outputs`, the adapter SHALL also report each `changeSet.gitPatch` in the session's `outputs` as a patch event timed at the session's `updateTime`, so a patch is found in either place; a patch whose diff equals an earlier reported patch's diff SHALL be reported once, as the earlier. Activities SHALL be ordered by `createTime`, never by their position in a listing. An event's identifier SHALL be the activity's `id`, or its `name` when `id` is absent.

#### Scenario: States map, and an unknown state is unknown

- **WHEN** the adapter maps `IN_PROGRESS`, `AWAITING_USER_FEEDBACK`, `COMPLETED`, `STATE_UNSPECIFIED` and `SOMETHING_NEW`
- **THEN** the results SHALL be working, awaiting feedback, completed, unknown and unknown

#### Scenario: A patch is found in the activities

- **GIVEN** a session whose activities hold one artifact with a `changeSet.gitPatch` and whose `outputs` hold none
- **WHEN** the adapter lists the session's events
- **THEN** exactly one patch event SHALL be reported, with that patch's base commit, diff and suggested message

#### Scenario: A patch is found in the outputs

- **GIVEN** a session whose activities hold no patch and whose `outputs` hold one `changeSet.gitPatch`
- **WHEN** the adapter lists the session's events
- **THEN** exactly one patch event SHALL be reported, timed at the session's `updateTime`

#### Scenario: The same patch in both places is reported once

- **GIVEN** a session whose activities and `outputs` carry the same `unidiffPatch`
- **WHEN** the adapter lists the session's events
- **THEN** exactly one patch event SHALL be reported, timed at the activity's `createTime`

#### Scenario: Out-of-order activities are ordered by creation time

- **GIVEN** a listing that returns activities newest first
- **WHEN** the adapter lists the session's events
- **THEN** the events SHALL be in ascending `createTime` order

### Requirement: Jules limit failures are recognised in one place

The Jules adapter SHALL classify a failure as a limit failure when the response status is `429`, when the error body's `error.status` is `RESOURCE_EXHAUSTED`, or when the error body's `error.message` or a session failure's reason mentions a quota or a limit, matched case-insensitively against `quota`, `limit`, `RESOURCE_EXHAUSTED` and `429`. It SHALL classify `401` and `403` as authentication failures, `404` as not found, a transport failure or timeout as a network failure, and anything else as other. The classification SHALL be one pure function over the status and the body, with its patterns in one table.

#### Scenario: Limit shapes are recognised

- **WHEN** the classification is called for a `429` with any body, a `400` whose `error.status` is `RESOURCE_EXHAUSTED`, a `400` whose `error.message` is `Daily quota exceeded`, and a session failure whose reason is `Task limit reached`
- **THEN** each SHALL be classified as a limit failure

#### Scenario: Other statuses are classified

- **WHEN** the classification is called for `401`, `403`, `404` and `500`
- **THEN** the results SHALL be authentication, authentication, not found and other

### Requirement: Daily and concurrent task limits are enforced before a session is created

Before creating a Jules session, git-paw SHALL count the Jules sessions it has created on this machine, from any repository, in the last 24 hours — including those the same launch is about to create — and, among them, those whose last-known state is not completed or failed. It SHALL refuse to create a session when the first count would exceed the daily task limit or the second the concurrency limit, naming the limit, the count and the time the oldest counted session leaves the 24-hour window. The counts SHALL come from an owner-only ledger under the user data directory recording, for each session git-paw created, its name, its creation time and its last-known state, and no credential; the follow SHALL update a session's last-known state. The daily task and concurrency limits SHALL be those of the configured plan — `free` 15 and 3, `pro` 100 and 15, `ultra` 300 and 60 — unless `[remote.jules] daily_tasks` or `concurrent_tasks` overrides them. Sessions started outside git-paw are not counted. Re-attaching to a recorded session, adopting one by its title and sending a message to an existing session create no session, and SHALL NOT be refused by these limits or counted again against them.

#### Scenario: The daily limit refuses a launch

- **GIVEN** the `free` plan and a ledger holding 14 sessions created in the last 24 hours, all completed
- **WHEN** a launch plans two remote workers
- **THEN** it SHALL exit with code 1 naming the daily task limit of 15
- **AND** no Jules session and no worktree SHALL be created

#### Scenario: The concurrency limit refuses a launch

- **GIVEN** the `free` plan and a ledger holding 3 sessions created in the last 24 hours whose last-known state is working
- **WHEN** a launch plans one remote worker
- **THEN** it SHALL exit with code 1 naming the concurrency limit of 3

#### Scenario: Old sessions leave the window

- **GIVEN** the `free` plan and a ledger holding 15 completed sessions, each created more than 24 hours ago
- **WHEN** a launch plans one remote worker
- **THEN** the daily limit SHALL NOT refuse it

#### Scenario: An override replaces the plan's limit

- **GIVEN** the `pro` plan, `[remote.jules] daily_tasks = 20` and a ledger holding 20 sessions created in the last 24 hours
- **WHEN** a launch plans one remote worker
- **THEN** it SHALL exit with code 1 naming the daily task limit of 20

#### Scenario: A full ledger does not stop re-attaching

- **GIVEN** the `free` plan and a ledger holding 15 sessions created in the last 24 hours, one of them `sessions/abc123`, recorded by a stopped session's remote worker
- **WHEN** `git paw start` recovers that session
- **THEN** the worker SHALL NOT be refused
- **AND** no Jules session SHALL be created and the ledger SHALL still hold 15 sessions

### Requirement: A limit failure stops new remote launches for a cool-down

When creating a Jules session or reading a followed one fails with a limit failure, or a followed session fails with a limit-shaped reason, git-paw SHALL record in the ledger a cool-down lasting `[remote.jules] limit_cooldown_minutes`, and until it ends SHALL refuse every launch or attach that would create a Jules session, before anything is created, naming the time the cool-down ends. Following existing sessions, re-attaching to or adopting them, and sending them answers and the verify continuation SHALL continue during a cool-down.

#### Scenario: A limit on creation starts a cool-down

- **GIVEN** the default cool-down of 60 minutes and a fake transport whose session creation answers `429`
- **WHEN** a remote worker's session is created
- **THEN** the ledger SHALL record a cool-down ending 60 minutes later
- **AND** a launch of another remote worker within that hour SHALL exit with code 1 naming the cool-down's end

#### Scenario: Following continues during a cool-down

- **GIVEN** an active cool-down and a followed remote worker
- **WHEN** the follow's next read is due
- **THEN** the session SHALL be read

#### Scenario: Answers reach an existing session during a cool-down

- **GIVEN** an active cool-down and a followed remote worker with one escalated, unanswered question
- **WHEN** an `agent.answer` for the worker is observed
- **THEN** the answer SHALL be sent to the worker's session as one message
- **AND** no Jules session SHALL be created

### Requirement: Jules sessions count as remote tasks in usage

Each Jules session git-paw creates SHALL count as one remote task of the worker it was created for. `git paw status --json` SHALL report a remote worker's count in its `remote.tasks`, and SHALL add a `remote_tasks` field, the sum over the session's workers, to `usage_total`. A remote worker's token usage SHALL be reported as unknown, and a token or cost budget cap SHALL NOT count remote tasks.

#### Scenario: Status counts remote tasks

- **GIVEN** a session with two remote workers, one of which has had one vendor session and the other two
- **WHEN** `git paw status --json` runs
- **THEN** the workers' `remote.tasks` SHALL be `1` and `2`
- **AND** `usage_total.remote_tasks` SHALL be `3`
- **AND** each remote worker's `usage` SHALL be `null`
