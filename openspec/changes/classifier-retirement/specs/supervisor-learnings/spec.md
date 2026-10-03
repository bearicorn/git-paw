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
- **WHEN** a learning is published that way
- **THEN** the invocation SHALL begin with the stable helper path
  `.git-paw/scripts/sweep.sh`, so a permission rule scoped to that exact path
  can name it
- **AND** no broad `curl *` grant SHALL be required to publish a learning

## REMOVED Requirements

### Requirement: Permission-pattern signal

**Reason**: The signal counted `agent.status` messages tagged `auto_approved`, which nothing publishes once git-paw stops approving prompts.
**Migration**: Recurring prompts are surfaced by the manual-decision log, `git paw approvals`, and the first-seen `permission_pattern` learning (`approval-pattern-surfacing`). The `permission_pattern` category and the rendering of `permission_pattern` records are unchanged.
