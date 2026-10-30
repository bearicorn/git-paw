## ADDED Requirements

### Requirement: A CLI declares how its workers are sandboxed

`[clis.<name>]` SHALL accept an optional `sandbox` with the values `native` (git-paw passes the CLI's
own sandbox arguments at launch) and `wrapper` (the entry's `command` is the user's FS-sandbox
wrapper and git-paw passes nothing extra). Any other value SHALL be rejected at config load with an
error naming the key and the accepted values. When `sandbox` is absent the CLI SHALL be treated as
unsandboxed and its launch command SHALL be unchanged. Declaring a sandbox SHALL NOT change the
approval level any worker runs at.

#### Scenario: An undeclared CLI launches unchanged

- **GIVEN** a `[clis.<name>]` entry with no `sandbox` key
- **WHEN** a worker using it is launched
- **THEN** its pane command SHALL equal the command built before this change

#### Scenario: A wrapper declaration adds nothing to the command

- **GIVEN** `[clis.sbx] command = "/home/u/bin/claude-sandboxed"` and `sandbox = "wrapper"`
- **WHEN** a worker using `sbx` is launched
- **THEN** its pane command SHALL be the same as with no `sandbox` key

#### Scenario: A declaration does not raise the approval level

- **GIVEN** `agent_approval = "auto"` and `[clis.sbx] sandbox = "wrapper"`, `family = "claude"`
- **WHEN** a worker using `sbx` is launched
- **THEN** its approval flags SHALL be the `auto` flags, not the `full-auto` flags

#### Scenario: An unknown sandbox value is rejected

- **GIVEN** `[clis.sbx] sandbox = "docker"`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `sandbox` and listing `native` and `wrapper`

### Requirement: Native sandbox arguments are a provider facet

The system SHALL resolve a CLI's native sandbox arguments from, in order, `[clis.<name>].sandbox_args`
and the CLI family's built-in row; the `codex` family's built-in row SHALL be
`--sandbox workspace-write`. For a CLI declared `sandbox = "native"`, the launch builder SHALL place
the resolved arguments after the CLI's `args` and before its approval flags, and SHALL omit them when
the resolved approval flags already contain them verbatim. A `native` declaration for which no
arguments resolve SHALL be rejected at config load with an error naming `sandbox` and
`sandbox_args`.

#### Scenario: Codex receives its native sandbox

- **GIVEN** `[clis.codex] sandbox = "native"` and `agent_approval = "manual"`
- **WHEN** a worker using `codex` is launched
- **THEN** its pane command SHALL contain `--sandbox workspace-write`

#### Scenario: The sandbox argument is not duplicated

- **GIVEN** `[clis.codex] sandbox = "native"` and `agent_approval = "auto"`
- **WHEN** a worker using `codex` is launched
- **THEN** its pane command SHALL contain `--sandbox workspace-write` exactly once

#### Scenario: Native without arguments fails the load

- **GIVEN** `[clis.foo] sandbox = "native"` for a `generic` CLI with no `sandbox_args`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `sandbox` and `sandbox_args`

#### Scenario: An override supplies the arguments

- **GIVEN** `[clis.foo] sandbox = "native"` and `sandbox_args = "--confine ."`
- **WHEN** a worker using `foo` is launched
- **THEN** its pane command SHALL contain `--confine .`

### Requirement: Full-auto never disables a declared native sandbox

Each provider SHALL mark the built-in approval rows that disable the CLI's own sandbox; the `codex`
family's `full-auto` row SHALL be marked. When a CLI declared `sandbox = "native"` would be launched
with flags taken from a marked row, the launch SHALL be refused with an error naming the CLI, the
level, and `[clis.<name>].approval_args`, and no pane SHALL run the CLI. Flags supplied by an
`approval_args` override SHALL be used as given.

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

#### Scenario: A wrapper sandbox does not restrict full-auto

- **GIVEN** `[clis.sbx] sandbox = "wrapper"`, `family = "claude"` and `agent_approval = "full-auto"`
- **WHEN** a worker using `sbx` is launched
- **THEN** its pane command SHALL contain `--dangerously-skip-permissions`
