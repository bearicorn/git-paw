## ADDED Requirements

### Requirement: sweep.sh publishes the orchestrator's answers and main-advance events

The bundled `sweep.sh` helper SHALL provide two root-side publish subcommands, so the orchestrator skill needs no raw HTTP request for them, each reusing the helper's broker-URL discovery and its internal publish path:

- `answer <agent-id> <text> [re]` SHALL publish `agent.answer` with the top-level `agent_id` set to the recipient `<agent-id>` and a payload carrying `from = "supervisor"`, `answer = <text>` and, when given, `re`.
- `advanced-main <merged-branch> <summary>` SHALL publish `agent.advanced-main` with `from = "supervisor"`, `merged_branch = <merged-branch>`, `summary = <summary>`, and the `base`, `new_main_sha` (12-character abbreviation) and `merged_at` (ISO-8601 UTC) values the helper resolves itself from the repository's checked-out branch and `HEAD`, so `base` is the resolved default-branch name, never a hard-coded literal.

Both subcommands publish with supervisor authority, so the `setup-cli` grant table SHALL list them as never granted (like `verified`, `feedback-gate` and `conflict-ack`): the grant target is shared with worker sessions, and neither may be published by a worker without a prompt. Each SHALL exit non-zero with a diagnostic on stderr, publishing nothing, when a required argument is missing or empty.

#### Scenario: answer publishes an agent.answer to the recipient

- **GIVEN** a running broker reachable via the helper's URL discovery
- **WHEN** `.git-paw/scripts/sweep.sh answer feat-a "use RS256" q-auth` is run
- **THEN** the broker SHALL receive an `agent.answer` with `agent_id = "feat-a"`, `payload.from = "supervisor"`, `payload.answer = "use RS256"` and `payload.re = "q-auth"`

#### Scenario: advanced-main resolves base and SHA itself

- **GIVEN** a repository whose checked-out branch is its default branch, just fast-forwarded to a merged branch
- **WHEN** `.git-paw/scripts/sweep.sh advanced-main feat/auth "auth landed"` is run at the repository root
- **THEN** the broker SHALL receive an `agent.advanced-main` whose `merged_branch` is `feat/auth`, whose `base` is the checked-out branch's name and whose `new_main_sha` is the 12-character abbreviation of `HEAD`

#### Scenario: A missing argument publishes nothing

- **WHEN** `.git-paw/scripts/sweep.sh answer feat-a` is run without the answer text
- **THEN** the helper SHALL exit non-zero with a diagnostic on stderr
- **AND** no message SHALL be published

#### Scenario: Neither verb is granted by setup-cli

- **WHEN** the `setup-cli` allowlist item is planned for `claude`
- **THEN** no `permissions.allow` entry SHALL name `sweep.sh answer` or `sweep.sh advanced-main`
- **AND** both SHALL be listed as never granted
