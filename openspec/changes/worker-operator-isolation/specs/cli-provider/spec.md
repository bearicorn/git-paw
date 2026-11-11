## ADDED Requirements

### Requirement: Operator isolation is a provider facet

Each provider family SHALL declare its operator-isolation launch environment and arguments: what keeps a worker from loading the operator's personal user-level configuration and from writing memory the operator's sessions share. A family that encodes none SHALL contribute nothing, so the `generic` fallback launches unchanged.

#### Scenario: The claude family declares its isolation

- **GIVEN** the `claude` provider family
- **WHEN** its operator isolation is resolved
- **THEN** the arguments SHALL be `--setting-sources project,local`
- **AND** the environment SHALL be `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`

#### Scenario: A family without isolation facts contributes nothing

- **GIVEN** `[clis.agent] command = "/opt/agent"`, `args = "--quiet"` and no `family` (so `generic`)
- **WHEN** its worker pane command is built at `agent_approval = "manual"`
- **THEN** the command SHALL be exactly `/opt/agent --quiet`, with no assignment before the executable

### Requirement: Operator isolation is placed by the launch builder

The launch builder SHALL write the isolation environment as `NAME=value` assignments before the executable, and the isolation arguments after the CLI's `args` and before any native sandbox arguments and the approval flags. This SHALL hold on every worker launch path that builds through the one builder, including recovery with a resume id.

#### Scenario: A claude worker is launched isolated

- **GIVEN** `[clis.opus] command = "claude"`, `family = "claude"`, `args = "--model opus"`, and `agent_approval = "auto"`
- **WHEN** the pane command of a worker using `opus` is built for a fresh launch assigned the session id `abc`
- **THEN** it SHALL be `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 claude --model opus --setting-sources project,local --permission-mode acceptEdits --session-id abc`

#### Scenario: Recovery keeps the isolation

- **GIVEN** a stopped session whose entry has `cli = "claude"`, no `[clis.claude]` entry, and a recorded session id
- **WHEN** the session is recovered
- **THEN** the pane command SHALL start with `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 claude`
- **AND** it SHALL contain `--setting-sources project,local` before `--resume <id>`

#### Scenario: Start and add stay identical

- **GIVEN** the same claude-family CLI, approval level and configuration
- **WHEN** one worker is launched by `git paw start` and another by `git paw add`
- **THEN** both pane commands SHALL carry the same isolation environment and arguments

### Requirement: A CLI may inherit the operator's configuration

`[clis.<name>]` SHALL accept an optional boolean `inherit_operator_config`, default `false`. When it is `true`, the launch builder SHALL add neither that CLI's isolation environment nor its isolation arguments. A config without the key SHALL load unchanged and SHALL NOT gain the key when written back.

#### Scenario: Opting out restores the previous launch

- **GIVEN** `[clis.claude] command = "claude"` with `inherit_operator_config = true` and `agent_approval = "manual"`
- **WHEN** a worker using `claude` is launched
- **THEN** its pane command SHALL contain neither `--setting-sources` nor `CLAUDE_CODE_DISABLE_AUTO_MEMORY`

#### Scenario: An existing config round-trips without the key

- **GIVEN** a config whose `[clis.*]` entries have no `inherit_operator_config`
- **WHEN** it is loaded and written back
- **THEN** the written file SHALL NOT contain `inherit_operator_config`
