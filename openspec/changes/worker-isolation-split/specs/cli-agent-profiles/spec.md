## MODIFIED Requirements

### Requirement: A delegate's argv is built from its CLI record without a shell

A delegate's argv SHALL be built from its CLI record with no shell, each part split on whitespace, in this order: the CLI's `command`; the CLI's `args`; the profile's `args`; the CLI's operator-isolation arguments unless the CLI sets `inherit_operator_config = true`; the native sandbox arguments the record declares; the approval flags for the profile's level; the session-id arguments with a fresh id when the CLI has a template; then the headless arguments. The task SHALL NOT appear in argv.

#### Scenario: A delegate runs at the root with its arguments in order

- **GIVEN** `[clis.agent] command = "/opt/agent"` with `headless_args = "--batch"` and `[profiles.explorer]` with `kind = "delegate"`, `cli = "agent"`, `args = "--model small"`, `skill = "explorer"`
- **WHEN** `git paw delegate explorer --task "map the config loader"` runs from a subdirectory of the main working tree
- **THEN** exactly one process SHALL be spawned with argv `/opt/agent --model small --batch`
- **AND** its working directory SHALL be the main working tree root
- **AND** its stdin SHALL contain `map the config loader` and its argv SHALL NOT

#### Scenario: A delegate gets its CLI's operator isolation

- **GIVEN** `[clis.opus] command = "claude"`, `family = "claude"` and `[profiles.explorer]` with `kind = "delegate"`, `cli = "opus"`, `args = "--model small"`, `skill = "explorer"`
- **WHEN** `git paw delegate explorer --task x` runs
- **THEN** its argv SHALL be `claude --model small --setting-sources project,local --session-id <id> -p`
- **AND** its environment SHALL contain `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`

#### Scenario: An opted-out CLI's delegate keeps the operator configuration

- **GIVEN** the same profile and `[clis.opus] inherit_operator_config = true`
- **WHEN** `git paw delegate explorer --task x` runs
- **THEN** its argv SHALL NOT contain `--setting-sources` and its environment SHALL NOT contain `CLAUDE_CODE_PLUGIN_DIRS`
- **AND** its environment SHALL still contain `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` and `CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false`
