## ADDED Requirements

### Requirement: A2A user chapter

The user guide SHALL contain an A2A chapter, `docs/src/user-guide/a2a.md`, listed in `SUMMARY.md`. The chapter SHALL cover:

- how to enable the face with `[broker.a2a] enabled`, and that it is served only on loopback;
- obtaining the credential with `git paw __authority` and sending it as a Bearer token;
- the required `A2A-Version: 1.0`;
- the card URL `<broker_url>/.well-known/agent-card.json`;
- the one-task-per-worker model and the task-state table;
- every supported method, and every unsupported method with its error code;
- blocking `SendMessage` and `returnImmediately`;
- recording verdicts through the coordination extension.

It SHALL include at least one worked JSON-RPC request and response for `ListTasks`, and one for a `SendMessage` verdict.

#### Scenario: Chapter covers the operational facts

- **WHEN** `docs/src/user-guide/a2a.md` is checked
- **THEN** it SHALL mention `[broker.a2a]`, `git paw __authority`, `A2A-Version`, `/.well-known/agent-card.json`, each of the eight task-state names, `GetTask`, `ListTasks`, `SendMessage`, `CancelTask`, the error codes `-32003` and `-32004`, `returnImmediately`, and the coordination extension URI
- **AND** it SHALL contain a JSON-RPC example whose `method` is `ListTasks` and one whose `method` is `SendMessage`

#### Scenario: Chapter is reachable and the book builds

- **WHEN** `mdbook build docs/` runs
- **THEN** it SHALL succeed with the A2A chapter listed in `SUMMARY.md`

### Requirement: Migration guide from the legacy broker wire to A2A

The user guide SHALL contain a migration guide, `docs/src/user-guide/a2a-migration.md`. It SHALL map each item below either to its A2A counterpart, or to a statement that it stays on the legacy wire:

- each legacy broker endpoint: `/publish`, `/watch`, `/messages/:agent_id`, `/status` and `/log`;
- each broker message type.

The guide SHALL show how a root-session orchestrator moves its work to A2A:

- from the `sweep.sh verified` and `feedback-gate` verbs, to a `SendMessage` with a verdict;
- from polling `/status` and `/log`, to `ListTasks`, `GetTask` and, when it is served, `SubscribeToTask`.

The guide SHALL also state:

- that workers keep `broker.sh` and the MCP publish tools unchanged;
- that the legacy wire is stable for the rest of the 1.x line and is not deprecated;
- that the Agent Card lives at `/.well-known/agent-card.json` (not `/.well-known/agent.json`), with one card per broker rather than one per agent.

#### Scenario: Every legacy message type is mapped

- **WHEN** the migration guide is checked against the broker message type list
- **THEN** every message type, from `agent.status` through `orchestrator.verify-now`, SHALL appear in its mapping table

#### Scenario: Legacy policy and corrected facts are stated

- **WHEN** the migration guide is read
- **THEN** it SHALL state that the legacy wire is not deprecated in the 1.x line
- **AND** it SHALL name `/.well-known/agent-card.json` as the card path and state that there is one card per broker

### Requirement: Coordination extension specification page

The documentation site SHALL publish the coordination extension's specification at the path its URI names. The source file is `docs/src/a2a/coordination/v1/index.md`. The page SHALL define:

- the URI;
- the task metadata fields;
- the message `event` values and their fields;
- the client `verdict` values and their effect;
- the requirement that the URI be listed in `message.extensions` for a verdict to apply;
- that `required` is `false` on the card;
- that a breaking change uses a new URI.

#### Scenario: Extension page defines the schema

- **WHEN** `docs/src/a2a/coordination/v1/index.md` is checked
- **THEN** it SHALL contain the URI `https://bearicorn.github.io/git-paw/a2a/coordination/v1`, the fields `agentId`, `generation`, `modifiedFiles` and `intents`, each `event` value, and the verdict values `verified` and `feedback`

### Requirement: Coordination and MCP chapters link to A2A

`docs/src/user-guide/coordination.md` SHALL state that the broker wire it documents is the agent-side transport, and SHALL link to the A2A chapter for external clients. `docs/src/user-guide/mcp.md` SHALL state that `git paw mcp` is served over stdio, and SHALL point HTTP-based agent integrations at the A2A chapter.

#### Scenario: Both chapters link to A2A

- **WHEN** `coordination.md` and `mcp.md` are checked
- **THEN** each SHALL contain a link to `a2a.md`
- **AND** `mcp.md` SHALL state that the MCP server uses stdio
