## ADDED Requirements

### Requirement: A CLI declares how its workers are sandboxed

`[clis.<name>]` SHALL accept an optional `sandbox` with the values `native` (git-paw passes the CLI's
own sandbox arguments at launch) and `wrapper` (the entry's `command` is the user's FS-sandbox
wrapper and git-paw passes nothing extra). Any other value SHALL be rejected at config load with an
error naming the key and the accepted values.

#### Scenario: An unknown sandbox value is rejected

- **GIVEN** `[clis.sbx] sandbox = "docker"`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `sandbox` and listing `native` and `wrapper`

### Requirement: An undeclared or wrapper sandbox leaves the launch command unchanged

When `sandbox` is absent the CLI SHALL be treated as unsandboxed and its launch command SHALL be
unchanged. A `wrapper` declaration SHALL add nothing to the launch command.

#### Scenario: An undeclared CLI launches unchanged

- **GIVEN** a `[clis.<name>]` entry with no `sandbox` key
- **WHEN** a worker using it is launched
- **THEN** its pane command SHALL equal the command built before this change

#### Scenario: A wrapper declaration adds nothing to the command

- **GIVEN** `[clis.sbx] command = "/home/u/bin/claude-sandboxed"` and `sandbox = "wrapper"`
- **WHEN** a worker using `sbx` is launched
- **THEN** its pane command SHALL be the same as with no `sandbox` key

### Requirement: A sandbox declaration does not change the approval level

Declaring a sandbox SHALL NOT change the approval level any worker runs at.

#### Scenario: A declaration does not raise the approval level

- **GIVEN** `agent_approval = "auto"` and `[clis.sbx] sandbox = "wrapper"`, `family = "claude"`
- **WHEN** a worker using `sbx` is launched
- **THEN** its approval flags SHALL be the `auto` flags, not the `full-auto` flags

### Requirement: Native sandbox arguments are a provider facet

The system SHALL resolve a CLI's native sandbox arguments from, in order, `[clis.<name>].sandbox_args`
and the CLI family's built-in row; the `codex` family's built-in row SHALL be
`--sandbox workspace-write`. A `native` declaration for which no arguments resolve SHALL be rejected
at config load with an error naming `sandbox` and `sandbox_args`.

#### Scenario: Native without arguments fails the load

- **GIVEN** `[clis.foo] sandbox = "native"` for a `generic` CLI with no `sandbox_args`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `sandbox` and `sandbox_args`

#### Scenario: An override supplies the arguments

- **GIVEN** `[clis.foo] sandbox = "native"` and `sandbox_args = "--confine ."`
- **WHEN** a worker using `foo` is launched
- **THEN** its pane command SHALL contain `--confine .`

### Requirement: Native sandbox arguments precede the approval flags once

For a CLI declared `sandbox = "native"`, the launch builder SHALL place the resolved arguments after
the CLI's `args` and before its approval flags, and SHALL omit them when the resolved approval flags
already contain them verbatim.

#### Scenario: Codex receives its native sandbox

- **GIVEN** `[clis.codex] sandbox = "native"` and `agent_approval = "manual"`
- **WHEN** a worker using `codex` is launched
- **THEN** its pane command SHALL contain `--sandbox workspace-write`

#### Scenario: The sandbox argument is not duplicated

- **GIVEN** `[clis.codex] sandbox = "native"` and `agent_approval = "auto"`
- **WHEN** a worker using `codex` is launched
- **THEN** its pane command SHALL contain `--sandbox workspace-write` exactly once

### Requirement: Full-auto never disables a declared native sandbox

Each provider SHALL name the flags that disable the CLI's own sandbox; for `codex` this is
`--dangerously-bypass-approvals-and-sandbox`. A `native` CLI SHALL be refused, naming the CLI, the
level and `[clis.<name>].approval_args`, before any pane runs it, when its effective approval flags
(built-in row or override) contain such a flag, or set the first word of its sandbox arguments to
another value.

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

### Requirement: An unsandboxed full-auto launch warns

A worker launch at `full-auto` on a CLI that declares no sandbox SHALL print a warning naming the CLI
and the `sandbox` key, once per CLI per run. The warning SHALL NOT refuse the launch.

#### Scenario: An unsandboxed full-auto launch warns

- **GIVEN** `agent_approval = "full-auto"` and no `[clis.codex].sandbox`
- **WHEN** `git paw start --cli codex` is run
- **THEN** stderr SHALL name `codex` and say it runs at full-auto with no declared sandbox, once
- **AND** the launch SHALL proceed

### Requirement: The headless hand-off honours a native sandbox

The headless orchestrator invocation SHALL pass a `native` CLI's sandbox arguments after its `args`
and SHALL be unavailable, with the refusal as its reason, where a worker launch at the
orchestrator's approval level would be refused.

#### Scenario: The headless hand-off keeps a native sandbox on

- **GIVEN** `[supervisor] cli = "codex"`, `approval = "manual"` and `[clis.codex] sandbox = "native"`
- **WHEN** the hand-off is resolved
- **THEN** its argv SHALL be `codex --sandbox workspace-write exec`

#### Scenario: The headless hand-off refuses a disabled native sandbox

- **GIVEN** `[supervisor] cli = "codex"`, `approval = "full-auto"` and `[clis.codex] sandbox = "native"`
- **WHEN** the hand-off is resolved
- **THEN** it SHALL be unavailable, naming `[clis.codex].approval_args`
