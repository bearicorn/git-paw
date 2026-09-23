## MODIFIED Requirements

### Requirement: Bundled `sweep.sh learn` subcommand

The bundled `sweep.sh` helper SHALL provide a `learn <category> <title>
<body-json>` subcommand that publishes an `agent.learning` broker message
with `agent_id = "supervisor"`. The subcommand SHALL reuse the helper's
existing broker-URL discovery (`.git-paw/config.toml [broker]`, default
`127.0.0.1:9119`) and its internal `publish()` path. The supervisor skill
SHALL NOT hand-roll a raw `curl …/publish` call to emit `agent.learning`.

The subcommand SHALL pass the `<category>` and `<title>` arguments and the
`<body-json>` argument through to the `agent.learning` payload's `category`,
`title`, and `body` fields respectively, leaving the body shape to the
caller (the skill documents the per-category body).

#### Scenario: learn publishes an agent.learning through the helper

- **GIVEN** a running broker reachable via the helper's URL discovery
- **WHEN** `sweep.sh learn tooling_friction "Commit step re-prompts every sweep" '{"friction":"git commit re-prompts","occurrences":3,"suggestion":"pre-approve worktree-confined git commit"}'` is run
- **THEN** the broker SHALL receive an `agent.learning` message with
  `agent_id = "supervisor"`, `category = "tooling_friction"`, the given
  `title`, and the given `body` object

#### Scenario: learn resolves the broker URL from config

- **GIVEN** `.git-paw/config.toml` sets `[broker] port = 9200`
- **WHEN** `sweep.sh learn <category> <title> <body-json>` is run
- **THEN** the publish SHALL target the configured port, not a hardcoded one

#### Scenario: learn needs no broad curl grant

- **GIVEN** the command `.git-paw/scripts/sweep.sh learn tooling_friction "t" '{}'`
- **WHEN** the safe-command classifier evaluates it
- **THEN** it SHALL classify as safe under the managed helper-script rule (the
  script path `.git-paw/scripts/sweep.sh`)
- **AND** no broad `curl *` whitelist entry SHALL be required to publish a learning

### Requirement: User guide includes a Learnings Mode chapter

The mdBook user guide SHALL include a chapter at
`docs/src/user-guide/learnings.md` documenting learnings mode.
The chapter SHALL cover the opt-in `[supervisor] learnings = true`
flag, the location and append-only shape of
`.git-paw/session-learnings.md`, and the five deterministic
categories tracked (stuck duration, recovery-cycle
count, forward conflicts, in-flight conflicts, ownership
violations). The chapter SHALL state that when the broker is
enabled each record is additionally published as an
`agent.learning` broker message, and that the MCP
`get_learnings()` tool reads the learnings file rather than the
broker.

The chapter SHALL be linked from `docs/src/SUMMARY.md` under the
User Guide group.

#### Scenario: Learnings chapter exists and is linked

- **WHEN** `docs/src/SUMMARY.md` is inspected
- **THEN** it contains a link to `user-guide/learnings.md` under the User Guide section

#### Scenario: Learnings chapter documents the opt-in flag

- **WHEN** `docs/src/user-guide/learnings.md` is inspected
- **THEN** it contains the substring `[supervisor]` and references the `learnings` flag
- **AND** it states the default value is `false` (or equivalent — "opt-in")

#### Scenario: Learnings chapter names the output file

- **WHEN** `docs/src/user-guide/learnings.md` is inspected
- **THEN** it contains the substring `.git-paw/session-learnings.md`

#### Scenario: Learnings chapter enumerates the deterministic categories

- **WHEN** `docs/src/user-guide/learnings.md` is inspected
- **THEN** it mentions stuck duration (or "where agents got stuck")
- **AND** it mentions recovery-cycle count (or "recovery cycles")
- **AND** it mentions forward conflicts
- **AND** it mentions in-flight conflicts
- **AND** it mentions ownership violations

#### Scenario: Learnings chapter documents broker publication and file-sourced get_learnings

- **WHEN** `docs/src/user-guide/learnings.md` is inspected
- **THEN** it states that records are additionally published as `agent.learning` broker messages when the broker is enabled
- **AND** it states that `get_learnings()` reads the Markdown file, not the broker
- **AND** it does NOT state that the `agent.learning` variant is deferred

### Requirement: Deterministic id for idempotent re-emission

The aggregator SHALL produce a deterministic `id` for each
`agent.learning` record. The system SHALL compute the id as the
64-bit FNV-1a hash (offset basis `0xcbf29ce484222325`, prime
`0x100000001b3`) of the UTF-8 bytes of a canonical serialisation,
rendered as 16 lowercase hexadecimal characters. The canonical
serialisation SHALL be `<category>|<branch_id>|<body>|<hour>`, where
`<branch_id>` is empty for a record with no branch, `<hour>` is the
UTC hour bucket (`YYYY-MM-DDTHH`), and `<body>` is the
category-specific body fields in a stable order: an object as
`{key:value,…}` with its keys in ascending byte order and written
unquoted, an array as `[item,…]` in its own order, and every other
value as its compact JSON text. Re-publishing the same logical
record within the same hour SHALL produce the same id. The id SHALL
NOT depend on the Rust toolchain or the platform; in particular it
SHALL NOT be computed with the standard library's `DefaultHasher`,
whose algorithm is unspecified and may change between Rust releases.
Because earlier releases computed the id with `DefaultHasher`, ids
change once on upgrade: a record emitted before the upgrade and the
same logical record emitted after it carry different ids.

#### Scenario: Same record within the hour gets the same id

- **WHEN** the aggregator commits the same logical record twice
  within a single UTC hour
- **THEN** both broker messages SHALL carry identical `id`
  values

#### Scenario: Same record across hour boundaries gets different ids

- **GIVEN** a record committed at 13:59 UTC and the same record
  committed at 14:01 UTC
- **WHEN** both publish
- **THEN** the two messages SHALL carry different `id` values

#### Scenario: A known record has a pinned id

- **GIVEN** a record with category `conflict_event`, no branch id,
  body `{"file":"src/lib.rs","agents":["feat-a","feat-b"]}`, committed
  at `2026-01-01T13:30:00Z`
- **WHEN** its id is computed
- **THEN** the canonical serialisation SHALL be
  `conflict_event||{agents:["feat-a","feat-b"],file:"src/lib.rs"}|2026-01-01T13`
- **AND** the id SHALL be `351c85e586ece5f0`

## REMOVED Requirements

### Requirement: MCP get_learnings consumes the variant

**Reason**: Never implemented, and contradicted by the shipped contract. The MCP
`get_learnings` tool only parses `.git-paw/session-learnings.md` and returns
`{ sections: [{ category, entries }] }`, with no broker path and no `source` field
(`src/mcp/tools/session.rs`). That is what `mcp-server`'s scenario "get_learnings parses the
session-learnings.md file" and the learnings user guide specify.
**Migration**: None for MCP clients, whose behaviour is unchanged. The `mcp-server` capability
owns the `get_learnings` contract. A tool that wants broker-side records reads `agent.learning`
messages from the broker (`/messages/<branch-id>` or `/log`), as the learnings chapter documents.
