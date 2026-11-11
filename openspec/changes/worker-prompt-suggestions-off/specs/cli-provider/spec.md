## ADDED Requirements

### Requirement: Claude-family workers launch without prompt suggestions

A claude-family worker's operator isolation SHALL also turn off the CLI's prompt suggestions, so an idle worker's input box never holds a predicted message that a lone Enter from git-paw (buffered-input recovery, `sweep.sh tell`, an approval) could submit as if someone had typed it. The opt-out `inherit_operator_config = true` SHALL leave prompt suggestions as the operator configured them.

#### Scenario: An isolated claude worker gets no prompt suggestions

- **GIVEN** a claude-family CLI without `inherit_operator_config`
- **WHEN** its worker pane command is built
- **THEN** its `env` prefix SHALL include `CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false`

#### Scenario: An opted-out claude worker keeps the operator's setting

- **GIVEN** `[clis.claude] command = "claude"` with `inherit_operator_config = true`
- **WHEN** its worker pane command is built
- **THEN** it SHALL NOT contain `CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION`

## MODIFIED Requirements

### Requirement: Each agent CLI resolves into one record

The system SHALL resolve each agent CLI, once per agent, into one record: its id, display name, launch command and arguments, provider family, and facets (approval flags, session-id, resume and headless templates, MCP capability, prompt-shape profile). The id SHALL be the leading whitespace-delimited token of the CLI identity (a `[clis.<name>]` key, or a name passed as `--cli`). Consumers SHALL read facets from the record, never re-derive them from the launch command.

#### Scenario: A known CLI resolves its record

- **GIVEN** `claude` resolves on PATH and no `[clis.claude]` entry exists
- **WHEN** the record for `claude` is resolved
- **THEN** its id SHALL be `claude`, its executable the resolved path, and its family `claude`

#### Scenario: A custom entry resolves its record

- **GIVEN** `[clis.my-agent] command = "/opt/tools/my-agent"` with that file present
- **WHEN** the record for `my-agent` is resolved
- **THEN** its id SHALL be `my-agent` and its executable `/opt/tools/my-agent`

#### Scenario: An identity with trailing flags keeps its base CLI's facets

- **GIVEN** no `[clis.claude]` entry and the identity `claude --model x` (for example from
  `--cli "claude --model x"`)
- **WHEN** a worker using it is launched at `agent_approval = "full-auto"`
- **THEN** its id SHALL be `claude` and its family `claude`
- **AND** the pane command SHALL be `env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --model x` followed by
  `--setting-sources project,local`, `--dangerously-skip-permissions` and the session-id arguments

#### Scenario: Overrides resolve by id when the executable differs

- **GIVEN** `[clis.fast] command = "/opt/tools/agent"`, `submit_delay_ms = 2500`, and a
  `prompt_profile` override
- **WHEN** a pane for `fast` is launched and its settle delay and profile are resolved
- **THEN** both SHALL come from `[clis.fast]`, although the launch command begins with
  `/opt/tools/agent`

#### Scenario: The launch command's leading token is never the id

- **GIVEN** `[clis.foo] command = "claude"` with no `family` key
- **WHEN** its record is resolved
- **THEN** its id SHALL be `foo` and its family `generic`, although its launch command begins with `claude`

### Requirement: The ambiguous shim is reported before a launch

`git paw doctor` SHALL report one row per `[clis.<name>]` entry, failing with the refusal and the
fix for every entry a launch would refuse as an ambiguous PATH shim. A dry-run plan SHALL apply the
same check and SHALL show the exact command each worker pane would run.

#### Scenario: Doctor flags an ambiguous shim

- **GIVEN** a `[clis.<name>]` entry a launch would refuse as an ambiguous shim
- **WHEN** `git paw doctor` runs
- **THEN** that entry's row SHALL fail, naming both paths and telling the user to rename the key or
  set `command` to the full path they mean

#### Scenario: A dry run shows the real launch command

- **GIVEN** `[clis.sonnet] command = "claude"`, `family = "claude"`, `args = "--model sonnet"`
- **WHEN** `git paw start --dry-run` plans a worker using `sonnet`
- **THEN** the plan SHALL show the pane command beginning `env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --model sonnet --setting-sources project,local`

#### Scenario: A built-in CLI still launches by name

- **GIVEN** a detected `codex` with no `[clis.codex]` entry
- **WHEN** a worker using `codex` is launched
- **THEN** the pane command SHALL begin with `codex`, not with the detected absolute path

### Requirement: Operator isolation is a provider facet

Each provider family SHALL declare its operator-isolation launch environment and arguments: what keeps a worker from loading the operator's personal configuration and extensions and from writing memory the operator's sessions share. An environment entry MAY assign the empty value to clear an inherited variable. A family that encodes none SHALL contribute nothing, so the `generic` fallback launches unchanged.

#### Scenario: The claude family declares its isolation

- **GIVEN** the `claude` provider family
- **WHEN** its operator isolation is resolved
- **THEN** the arguments SHALL be `--setting-sources project,local`
- **AND** the environment SHALL be `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`, `CLAUDE_CODE_PLUGIN_DIRS=` (empty) and `CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false`, in that order

Test: `agent_cli::tests::the_claude_family_declares_its_isolation`

#### Scenario: A family without isolation facts contributes nothing

- **GIVEN** `[clis.agent] command = "/opt/agent"`, `args = "--quiet"` and no `family` (so `generic`)
- **WHEN** its worker pane command is built at `agent_approval = "manual"`
- **THEN** the command SHALL be exactly `/opt/agent --quiet`, with no `env` prefix and no assignment before the executable

Test: `agent_cli::tests::a_family_without_isolation_facts_contributes_nothing`

### Requirement: Operator isolation is placed by the launch builder

The launch builder SHALL write the isolation environment as `env NAME=value …` before the executable, so it works in every pane shell, and the isolation arguments after the CLI's `args` and before any native sandbox arguments and the approval flags. This SHALL hold on every worker launch path that builds through the one builder, including recovery with a resume id.

#### Scenario: A claude worker is launched isolated

- **GIVEN** `[clis.opus] command = "claude"`, `family = "claude"`, `args = "--model opus"`, and `agent_approval = "auto"`
- **WHEN** the pane command of a worker using `opus` is built for a fresh launch assigned the session id `abc`
- **THEN** it SHALL be `env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude --model opus --setting-sources project,local --permission-mode acceptEdits --session-id abc`

Test: `agent_cli::tests::a_claude_worker_is_launched_isolated`

#### Scenario: Recovery keeps the isolation

- **GIVEN** a stopped session whose entry has `cli = "claude"`, no `[clis.claude]` entry, and a recorded session id
- **WHEN** the session is recovered
- **THEN** the pane command SHALL start with `env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false claude`
- **AND** it SHALL contain `--setting-sources project,local` before `--resume <id>`

Test: `agent_cli::tests::recovery_keeps_the_isolation`, `tests/worker_operator_isolation_e2e.rs::recovery_through_start_keeps_the_isolation`

#### Scenario: Start and add stay identical

- **GIVEN** the same claude-family CLI, approval level and configuration
- **WHEN** one worker is launched by `git paw start` and another by `git paw add`
- **THEN** both pane commands SHALL carry the same isolation environment and arguments

Test: `tests/worker_operator_isolation_e2e.rs::start_and_add_launch_claude_workers_with_identical_isolation`
