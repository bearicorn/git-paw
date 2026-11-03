## ADDED Requirements

### Requirement: The headless hand-off carries the CLI's launch arguments

The headless orchestrator invocation SHALL build its argv as: the executable, the CLI's `args` split on whitespace, the native sandbox arguments when the CLI declares `sandbox = "native"`, the approval flags for `[supervisor].approval`, then the headless arguments. `args` SHALL be split on whitespace only, with no shell quoting, so an argument containing a space cannot be passed through `args`.

#### Scenario: The entry's args precede the approval flags and the headless form

- **GIVEN** `[supervisor] cli = "opus"`, `approval = "full-auto"` and `[clis.opus] command = "claude"`, `family = "claude"`, `args = "--model opus"`
- **WHEN** the headless hand-off is resolved
- **THEN** its argv SHALL be `claude`, `--model`, `opus`, `--dangerously-skip-permissions`, `-p`

#### Scenario: A CLI without args keeps its headless form

- **GIVEN** `[supervisor] cli = "claude"`, `approval = "manual"` and no `[clis.claude]` entry
- **WHEN** the headless hand-off is resolved
- **THEN** its argv SHALL be `claude`, `-p`

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
- **AND** the pane command SHALL be `claude --model x` followed by
  `--dangerously-skip-permissions` and the session-id arguments

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

### Requirement: The boot-block form follows the record's MCP capability

The system SHALL render the MCP form of the boot block for a worker exactly when that worker's CLI
record is MCP-capable, and the helper-script form otherwise. An explicit `mcp` on the CLI's
`[clis.<id>]` entry SHALL win; only when `mcp` is unset SHALL the family supply the default (the
`claude`, `codex`, `agy` and `cursor-agent` families are MCP-capable, `generic` and `qwen` are
not). It SHALL never come from matching the CLI id against a list of names.

#### Scenario: A declared Claude variant receives the MCP form

- **GIVEN** `[clis.claude-oss] family = "claude"`
- **WHEN** a worker using `claude-oss` is booted
- **THEN** its boot block SHALL use the MCP publish-tool form

#### Scenario: An undeclared custom CLI receives the helper form

- **GIVEN** `[clis.claude-oss]` with no `family` and no `mcp`
- **WHEN** a worker using `claude-oss` is booted
- **THEN** its boot block SHALL use the `broker.sh` helper form

#### Scenario: An explicit mcp key overrides the family default

- **GIVEN** `[clis.claude-oss] family = "claude"` with `mcp = false`, and `[clis.my-agent]` with no `family` and `mcp = true`
- **WHEN** a worker using each is booted
- **THEN** the `claude-oss` worker's boot block SHALL use the `broker.sh` helper form
- **AND** the `my-agent` worker's boot block SHALL use the MCP publish-tool form

#### Scenario: A declared family reaches the worker's boot prompt end to end

- **GIVEN** a broker-enabled config with `[clis.paw-stub] family = "claude"` and no `mcp` key, selected for a worker
- **WHEN** the session is launched with `git paw start`
- **THEN** the boot prompt delivered to that worker's pane SHALL contain the MCP publish-tool form, not the `broker.sh` helper invocations

### Requirement: Full-auto never disables a declared native sandbox

Each provider SHALL name the flags that disable the CLI's own sandbox; for `codex` this is
`--dangerously-bypass-approvals-and-sandbox`. A `native` CLI SHALL be refused, naming the CLI, the
level and `[clis.<name>].approval_args`, before any pane runs it, when its effective approval flags
(built-in row or override) contain such a flag, or set the first word of its sandbox arguments to
another value. A dry-run plan SHALL report the refusal in place of that worker's command.

#### Scenario: Codex full-auto under a native sandbox is refused

- **GIVEN** `[clis.codex] sandbox = "native"` and `agent_approval = "full-auto"` with no override
- **WHEN** a worker using `codex` is launched
- **THEN** the launch SHALL fail with an error naming `codex`, `full-auto` and
  `[clis.codex].approval_args`

#### Scenario: An explicit override is honoured

- **GIVEN** `[clis.codex] sandbox = "native"`, `agent_approval = "full-auto"` and
  `approval_args = { "full-auto" = "--example-no-prompts" }`
- **WHEN** a worker using `codex` is launched
- **THEN** the pane command SHALL contain `--sandbox workspace-write` and `--example-no-prompts`

#### Scenario: An override cannot re-add a sandbox-disabling flag

- **GIVEN** `[clis.codex] sandbox = "native"` and
  `approval_args = { "auto" = "--dangerously-bypass-approvals-and-sandbox" }`
- **WHEN** a worker using `codex` is launched at `auto`
- **THEN** the launch SHALL fail with an error naming that flag and `[clis.codex].approval_args`

#### Scenario: Conflicting sandbox settings are refused

- **GIVEN** `[clis.codex] sandbox = "native"`, `sandbox_args = "--sandbox read-only"` and
  `agent_approval = "auto"`
- **WHEN** a worker using `codex` is launched
- **THEN** the launch SHALL fail with an error naming `--sandbox` and `sandbox_args`

#### Scenario: A wrapper sandbox does not restrict full-auto

- **GIVEN** `[clis.sbx] sandbox = "wrapper"`, `family = "claude"` and `agent_approval = "full-auto"`
- **WHEN** a worker using `sbx` is launched
- **THEN** its pane command SHALL contain `--dangerously-skip-permissions`

#### Scenario: A dry run reports the refusal instead of a command

- **GIVEN** `[clis.codex] sandbox = "native"` and `agent_approval = "full-auto"` with no override
- **WHEN** `git paw start --dry-run --cli codex` plans a worker
- **THEN** the output SHALL contain the refusal naming `codex`, `full-auto` and `[clis.codex].approval_args`
- **AND** it SHALL NOT contain a `runs:` line for that worker
