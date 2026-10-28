## Purpose

Defines how git-paw knows what each agent CLI is and how to drive it. Every agent CLI — a detected
binary or a `[clis.<name>]` entry — resolves once into a record carrying its identity, the
executable and arguments to launch, its provider family, and the per-CLI facets the rest of git-paw
consumes (native approval flags, session-id / resume / headless templates, MCP capability, prompt
shape). Provider families are compiled in-tree as a Strategy with a generic Null Object fallback, so
an unrecognised CLI degrades to launch-and-observe rather than failing, and a variant of a known CLI
can declare its family to inherit that CLI's behaviour.

## ADDED Requirements

### Requirement: Each agent CLI resolves into one record

The system SHALL resolve each agent CLI, once per agent, into a single record carrying: the CLI id
(the `[clis.<name>]` key or known binary name, as recorded in session state), the display name, the
launch command (per v0.17.0's *Panes launch the configured command of a CLI*), the launch arguments, the provider family, and the per-CLI facets —
approval flags per approval level, the session-id, resume and headless templates, MCP capability,
and the prompt-shape profile. Consumers SHALL read these values from the record. A consumer SHALL
NOT re-derive a CLI's identity, overrides or facets from a launch-command string or its leading
token.

#### Scenario: A known CLI resolves its record

- **GIVEN** `claude` resolves on PATH and no `[clis.claude]` entry exists
- **WHEN** the record for `claude` is resolved
- **THEN** its id SHALL be `claude`, its executable the resolved path, and its family `claude`

#### Scenario: A custom entry resolves its record

- **GIVEN** `[clis.my-agent] command = "/opt/tools/my-agent"` with that file present
- **WHEN** the record for `my-agent` is resolved
- **THEN** its id SHALL be `my-agent` and its executable `/opt/tools/my-agent`

#### Scenario: Overrides resolve by id when the executable differs

- **GIVEN** `[clis.fast] command = "/opt/tools/agent"`, `submit_delay_ms = 2500`, and a
  `prompt_profile` override
- **WHEN** a pane for `fast` is launched and its settle delay and profile are resolved
- **THEN** both SHALL come from `[clis.fast]`, although the launch command begins with
  `/opt/tools/agent`

### Requirement: Provider families are compiled in-tree with a generic fallback

The system SHALL define provider families as a closed, compile-time set with one in-tree
implementation each, plus a `generic` family that contributes no approval flags, no templates and
no MCP capability, and whose prompt-shape profile and recognised first-run dialogs are the embedded
defaults that every CLI received before this change.
Every CLI SHALL resolve to exactly one family; a CLI with no family of its own SHALL resolve
`generic` and behave as unrecognised CLIs did before this change. Families SHALL NOT be loadable at
runtime or definable in configuration. Each family's record SHALL be constructible without a PATH
lookup or a configuration file, so its behaviour can be exercised directly against recorded inputs.

#### Scenario: An unknown CLI resolves the generic family

- **GIVEN** `[clis.my-agent]` with no `family` key
- **WHEN** its record is resolved
- **THEN** its family SHALL be `generic`
- **AND** its approval flags SHALL be empty for every level and it SHALL NOT be MCP-capable

#### Scenario: Every family is constructible without PATH or config

- **WHEN** a record is constructed for each family with no PATH lookup and no configuration file
- **THEN** construction SHALL succeed for every family
- **AND** each record SHALL expose all facets, using the family's built-in values

### Requirement: A custom CLI declares its provider family

`[clis.<name>]` SHALL accept an optional `family` naming one of the in-tree families (`claude`,
`codex`, `agy`, `qwen`, `cursor-agent`, `generic`). A value outside that set SHALL be rejected at config
load with an error naming the key and listing the accepted values. When `family` is absent, a known
CLI name SHALL resolve its own family and any other name SHALL resolve `generic`. The family SHALL
NOT be inferred from the basename of `command`.

#### Scenario: A declared variant inherits its family

- **GIVEN** `[clis.claude-oss] command = "claude-oss"` and `family = "claude"`
- **WHEN** its record is resolved
- **THEN** its family SHALL be `claude`

#### Scenario: An undeclared custom entry is unchanged

- **GIVEN** `[clis.claude-oss] command = "claude-oss"` with no `family` key
- **WHEN** its record is resolved
- **THEN** its family SHALL be `generic`

#### Scenario: The command basename is not used

- **GIVEN** `[clis.foo] command = "/usr/local/bin/claude"` with no `family` key
- **WHEN** its record is resolved
- **THEN** its family SHALL be `generic`

#### Scenario: An unknown family is rejected

- **GIVEN** `[clis.foo] family = "gpt"`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `family` and listing the accepted values

### Requirement: An ambiguous PATH shim is refused

Panes SHALL launch a CLI as `cli-resolution`'s *Panes launch the configured command of a CLI*
specifies: a configured entry by its `command`, a built-in CLI with no entry by its name, resolved
by the pane's shell. This requirement adds only a refusal. When a `[clis.<name>]` key that is not a
known CLI name also resolves on PATH to a different file than the entry's `command` resolves to
(compared after resolving symlinks), launching that CLI SHALL fail with an error naming both paths
and telling the user to point `command` at the one they mean, and no pane SHALL run either
executable. A key that is a known CLI name SHALL NOT be refused, because pointing it at another
install is an explicit override.

#### Scenario: An ambiguous shim is refused

- **GIVEN** `[clis.claude-oss-sonnet] command = "claude"` and a different executable named
  `claude-oss-sonnet` on PATH
- **WHEN** a worker using `claude-oss-sonnet` is launched
- **THEN** the launch SHALL fail with an error naming both resolved paths
- **AND** no pane SHALL run either executable

#### Scenario: A known name may point at another install

- **GIVEN** `[clis.claude] command = "/opt/alt/claude"` while `claude` on PATH resolves elsewhere
- **WHEN** a worker using `claude` is launched
- **THEN** the pane command SHALL begin with `/opt/alt/claude` and the launch SHALL succeed

#### Scenario: A built-in CLI still launches by name

- **GIVEN** a detected `codex` with no `[clis.codex]` entry
- **WHEN** a worker using `codex` is launched
- **THEN** the pane command SHALL begin with `codex`, not with the detected absolute path

### Requirement: Per-CLI launch arguments

`[clis.<name>]` SHALL accept an optional `args` string that is appended, verbatim and after the
executable, to every launch of that CLI on every worker launch path. When `args` is absent the
launch command SHALL be unchanged. Two `[clis.*]` entries MAY name the same `command` with different
`args`, so per-worker launch arguments are configured by giving each worker profile its own entry.

#### Scenario: Arguments are appended

- **GIVEN** `[clis.sonnet] command = "claude"`, `family = "claude"`, `args = "--model sonnet"`
- **WHEN** a worker using `sonnet` is launched at `agent_approval = "manual"`
- **THEN** the pane command SHALL be the executable followed by `--model sonnet`

#### Scenario: Two profiles of one executable

- **GIVEN** `[clis.sonnet]` and `[clis.opus]` both with `command = "claude"` and different `args`
- **WHEN** one worker uses `sonnet` and another uses `opus`
- **THEN** each pane command SHALL carry its own entry's `args`

### Requirement: Every worker launch path builds its command through one builder

Every worker launch path SHALL build the pane command with one builder — bare start, spec-driven
start, supervised start, `git paw add`, session recovery, and the launch-readiness relaunch alike —
in this order: the executable, the CLI's `args`, the approval flags resolved for the
effective approval level, then the session-id arguments for a fresh launch or the resume arguments
for a recovery with a recorded session id. An empty part SHALL contribute no separator.

#### Scenario: Start and add build identical commands

- **GIVEN** the same CLI, approval level and configuration
- **WHEN** one worker is launched by `git paw start` and another by `git paw add`
- **THEN** both pane commands SHALL be identical apart from the session id

#### Scenario: Recovery builds the full command

- **GIVEN** a stopped session whose entry has `cli = "claude"` and a recorded session id, and
  `agent_approval = "auto"`
- **WHEN** the session is recovered
- **THEN** the pane command SHALL contain `--permission-mode acceptEdits` followed by
  `--resume <id>`

#### Scenario: Empty parts leave no separators

- **GIVEN** a `generic` CLI with no `args` at `agent_approval = "full-auto"`
- **WHEN** its pane command is built
- **THEN** the command SHALL be exactly the quoted executable

### Requirement: Built-in per-CLI tables are keyed by provider family

The system SHALL look up the built-in rows for approval flags, session-id and resume templates,
headless invocation templates, and MCP capability by the record's provider family. Resolution SHALL
consult, in order: the `[clis.<id>]` override, the family's built-in row, then none. Override tables
SHALL remain keyed by the CLI id.

#### Scenario: A declared variant resolves its family's rows

- **GIVEN** `[clis.claude-oss] family = "claude"` with no overrides
- **WHEN** its full-auto approval flags and session-id template are resolved
- **THEN** they SHALL be `--dangerously-skip-permissions` and `--session-id {{SESSION_ID}}`

#### Scenario: An undeclared variant resolves none

- **GIVEN** `[clis.claude-oss]` with no `family` and no overrides
- **WHEN** its full-auto approval flags are resolved
- **THEN** they SHALL be empty

### Requirement: The boot-block form follows the record's MCP capability

The system SHALL render the MCP form of the boot block for a worker exactly when that worker's CLI
record is MCP-capable, and the helper-script form otherwise. The record's MCP capability SHALL be
resolved as `boot-block`'s *Per-CLI MCP capability override* specifies: an explicit `mcp = true` or
`mcp = false` on the CLI's `[clis.<id>]` entry SHALL win; only when `mcp` is unset SHALL the
provider family supply the default (the `claude`, `codex`, `agy` and `cursor-agent` families are
MCP-capable, `generic` and `qwen` are not). It SHALL never come from matching the CLI id against a
list of names.

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

### Requirement: The detection roster is part of the provider records

The set of binary names auto-detected on PATH SHALL be defined once, as part of the provider
records: each roster name SHALL map to exactly one family (`generic` when the code encodes no facts
for it). No other list of known CLI names SHALL exist in the code.

#### Scenario: Every roster name maps to a family

- **WHEN** each auto-detected binary name is resolved
- **THEN** each SHALL map to exactly one family
- **AND** the auto-detected set SHALL equal the names listed in `cli-resolution`'s known-CLI
  requirement
