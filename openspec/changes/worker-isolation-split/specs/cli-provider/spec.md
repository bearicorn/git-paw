## MODIFIED Requirements

### Requirement: Operator isolation is a provider facet

Each provider family SHALL declare its operator-isolation launch environment and arguments in two parts: *worker safety*, which keeps a worker from writing memory the operator's sessions share and from holding predicted input, and *operator config*, which keeps a worker from loading the operator's personal configuration and extensions. An environment entry MAY assign the empty value to clear an inherited variable. A family that encodes none SHALL contribute nothing, so the `generic` fallback launches unchanged.

#### Scenario: The claude family declares its isolation

- **GIVEN** the `claude` provider family
- **WHEN** its operator isolation is resolved
- **THEN** the worker-safety environment SHALL be `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` and `CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false`
- **AND** the operator-config environment SHALL be `CLAUDE_CODE_PLUGIN_DIRS=` (empty) and its arguments `--setting-sources project,local`
- **AND** a launch applying both parts SHALL emit the environment as `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false`, in that order

Test: `agent_cli::tests::the_claude_family_declares_its_isolation`

#### Scenario: A family without isolation facts contributes nothing

- **GIVEN** `[clis.agent] command = "/opt/agent"`, `args = "--quiet"` and no `family` (so `generic`)
- **WHEN** its worker pane command is built at `agent_approval = "manual"`
- **THEN** the command SHALL be exactly `/opt/agent --quiet`, with no `env` prefix and no assignment before the executable

Test: `agent_cli::tests::a_family_without_isolation_facts_contributes_nothing`

### Requirement: A CLI may inherit the operator's configuration

`[clis.<name>]` SHALL accept an optional boolean `inherit_operator_config`, default `false`. When it is `true`, the launch builder SHALL drop that CLI's operator-config environment and arguments and SHALL still apply its worker-safety environment. A config without the key SHALL load unchanged and SHALL NOT gain the key when written back.

#### Scenario: Opting out restores the previous launch

- **GIVEN** `[clis.claude-worker] command = "/home/u/bin/claude-worker"`, `family = "claude"`, `inherit_operator_config = true`, and `agent_approval = "manual"`
- **WHEN** a worker using `claude-worker` is launched
- **THEN** its pane command SHALL contain neither `--setting-sources` nor `CLAUDE_CODE_PLUGIN_DIRS`
- **AND** its `env` prefix SHALL be `env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false`

Test: `agent_cli::tests::opting_out_keeps_worker_safety`

#### Scenario: An existing config round-trips without the key

- **GIVEN** a config whose `[clis.*]` entries have no `inherit_operator_config`
- **WHEN** it is loaded and written back
- **THEN** the written file SHALL NOT contain `inherit_operator_config`

Test: `config::tests::an_existing_config_round_trips_without_the_inherit_operator_config_key`

### Requirement: Claude-family workers launch without prompt suggestions

A claude-family worker SHALL launch with the CLI's prompt suggestions off, so an idle worker's input box never holds a predicted message that a lone Enter from git-paw (buffered-input recovery, `sweep.sh tell`, an approval) could submit as if someone had typed it. This SHALL hold whether or not the CLI sets `inherit_operator_config = true`.

#### Scenario: An isolated claude worker gets no prompt suggestions

- **GIVEN** a claude-family CLI without `inherit_operator_config`
- **WHEN** its worker pane command is built
- **THEN** its `env` prefix SHALL include `CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false`

#### Scenario: An opted-out claude worker keeps the operator's setting

- **GIVEN** `[clis.claude] command = "claude"` with `inherit_operator_config = true`
- **WHEN** its worker pane command is built
- **THEN** its `env` prefix SHALL include `CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false`

Test: `agent_cli::tests::an_opted_out_claude_worker_still_gets_no_prompt_suggestions`
