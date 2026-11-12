# cli-provider Specification

## Purpose
Defines how git-paw knows what each agent CLI is and how to drive it. Every agent CLI — a detected
binary or a `[clis.<name>]` entry — resolves once into a record carrying its identity, the
executable and arguments to launch, its provider family, and the per-CLI facets the rest of git-paw
consumes (native approval flags, session-id / resume / headless templates, MCP capability, prompt
shape). Provider families are compiled in-tree as a Strategy with a generic Null Object fallback, so
an unrecognised CLI degrades to launch-and-observe rather than failing, and a variant of a known CLI
can declare its family to inherit that CLI's behaviour.

## Requirements

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
- **AND** the pane command SHALL be `env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= claude --model x` followed by
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

### Requirement: Provider families are compiled in-tree with a generic fallback

The system SHALL define provider families as a closed, compile-time set with one in-tree
implementation each, plus a `generic` family that contributes no approval flags, templates or MCP
capability and keeps the embedded default prompt-shape profile. Every CLI SHALL resolve to exactly
one family, `generic` when it has none of its own. Families SHALL NOT be loadable at runtime or
definable in configuration. Each family's record SHALL be constructible without a PATH lookup or a
configuration file.

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

When a `[clis.<name>]` key that is not a known CLI name also resolves on PATH to a different file
than the entry's `command` resolves to (compared after resolving symlinks), launching that CLI SHALL
fail with an error naming both paths and telling the user to point `command` at the one they mean,
and no pane SHALL run either executable. A known CLI name SHALL NOT be refused, because pointing it
at another install is an explicit override. Otherwise panes launch as `cli-resolution` specifies.

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
- **THEN** the plan SHALL show the pane command beginning `env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= claude --model sonnet --setting-sources project,local`

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
- **THEN** the pane command SHALL be the family's operator-isolation `env` prefix, the executable, `--model sonnet`, then the isolation arguments

#### Scenario: Two profiles of one executable

- **GIVEN** `[clis.sonnet]` and `[clis.opus]` both with `command = "claude"` and different `args`
- **WHEN** one worker uses `sonnet` and another uses `opus`
- **THEN** each pane command SHALL carry its own entry's `args`

### Requirement: Every worker launch path builds its command through one builder

Every worker launch path SHALL build the pane command with one builder — bare start, spec-driven
start, supervised start, `git paw add`, session recovery, and the launch-readiness relaunch alike —
in this order: the family's operator-isolation environment as an `env NAME=value …` prefix (absent
when the family declares none or the entry sets `inherit_operator_config`), the executable, the
CLI's `args`, the operator-isolation arguments, the native sandbox arguments, the approval flags
resolved for the effective approval level, then the session-id arguments for a fresh launch or the resume arguments
for a recovery with a recorded session id. An empty part SHALL contribute no separator.

#### Scenario: Start and add build identical commands

- **GIVEN** the same CLI, approval level and configuration
- **WHEN** one worker is launched by `git paw start` and another by `git paw add`
- **THEN** both pane commands SHALL be identical apart from the session id

#### Scenario: Recovery builds the full command

- **GIVEN** a stopped session whose entry has `cli = "claude"` and a recorded session id, and
  `agent_approval = "auto"`
- **WHEN** the session is recovered
- **THEN** the pane command SHALL contain `--setting-sources project,local`, then
  `--permission-mode acceptEdits`, then `--resume <id>`

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

### Requirement: The detection roster is part of the provider records

The set of binary names auto-detected on PATH SHALL be defined once, as part of the provider
records: each roster name SHALL map to exactly one family (`generic` when the code encodes no facts
for it). No other list of known CLI names SHALL exist in the code.

#### Scenario: Every roster name maps to a family

- **WHEN** each auto-detected binary name is resolved
- **THEN** each SHALL map to exactly one family
- **AND** the auto-detected set SHALL equal the names listed in `cli-resolution`'s known-CLI
  requirement

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

### Requirement: Usage is a provider facet with an unknown fallback

A provider SHALL be able to declare how its CLI reports a session's token usage — input, output,
cache-read and cache-write tokens, and a cost figure only when the CLI itself reports one. The
`claude` family SHALL declare a usage source that locates a worker's report through the session id
git-paw assigned at launch. Every other family, and any session without an assigned id, SHALL
resolve usage as unknown with a stated reason.

#### Scenario: A known report yields figures

- **GIVEN** a recorded Claude-family usage report for a session
- **WHEN** it is parsed
- **THEN** the input, output, cache-read and cache-write token totals SHALL equal the sums in the
  report

#### Scenario: A generic CLI's usage is unknown

- **GIVEN** a worker running a `generic`-family CLI
- **WHEN** its usage is resolved
- **THEN** the result SHALL be unknown with a reason naming the CLI

#### Scenario: A session without an assigned id is unknown

- **GIVEN** a Claude-family worker with no git-paw-assigned session id
- **WHEN** its usage is resolved
- **THEN** the result SHALL be unknown with a stated reason

### Requirement: Usage reports are parsed without exposing their content

Turning a report's text into figures SHALL be a pure function. A report that cannot be parsed SHALL
yield unknown, never a partial or guessed figure. Only the usage figures SHALL be read into memory;
no other report content SHALL be logged, stored or shown in an unknown reason.

#### Scenario: A malformed report yields unknown

- **GIVEN** a Claude-family report whose usage records cannot be parsed
- **WHEN** it is parsed
- **THEN** the result SHALL be unknown, not a partial total

#### Scenario: An unknown reason carries no report content

- **GIVEN** a malformed report whose unparseable record contains transcript text
- **WHEN** it is parsed
- **THEN** the unknown reason SHALL NOT contain that text

### Requirement: A CLI entry's settings path locates its usage reports

For a `claude`-family entry, the parent directory of an absolute `[clis.<name>].settings_path`
(after `~` expansion) SHALL be the first directory searched for the session's usage reports, before
`CLAUDE_CONFIG_DIR` and `~/.claude`. The settings file itself SHALL NOT be read or written.

#### Scenario: A relocated variant's reports are found

- **GIVEN** `[clis.claude-oss]` with `family = "claude"` and `settings_path` inside a directory that
  holds the session's reports
- **WHEN** that worker's usage is resolved
- **THEN** the reports under that directory SHALL be read

### Requirement: Per-CLI names are compared only inside the agent CLI module

Production code outside `src/agent_cli/` SHALL NOT compare, match or test a value against a known CLI name (`claude`, `codex`, `agy`, `qwen`, `cursor-agent`), including a membership test with `contains`. Per-CLI behaviour SHALL be reached through the CLI's resolved record, such as its family. A source audit SHALL enforce this.

#### Scenario: A CLI-name comparison outside the module is caught

- **GIVEN** the line `if cli == "codex" {` in production code outside `src/agent_cli/`
- **WHEN** the CLI-name audit runs
- **THEN** it SHALL report that line

Test: `tests/source_audit.rs::cli_name_audit_catches_each_comparison_form`

#### Scenario: A CLI-name match arm outside the module is caught

- **GIVEN** a production `match` arm `"claude" => …` or `("claude", _) => …` outside `src/agent_cli/`
- **WHEN** the CLI-name audit runs
- **THEN** it SHALL report that line

Test: `tests/source_audit.rs::cli_name_audit_catches_each_comparison_form`

#### Scenario: A wrapped match or a prefix test is caught

- **GIVEN** a production line outside `src/agent_cli/` with `Some("claude") =>`, `(_, "codex") =>` or `.starts_with("claude")`
- **WHEN** the CLI-name audit runs
- **THEN** it SHALL report that line

Test: `tests/source_audit.rs::cli_name_audit_catches_each_comparison_form`

#### Scenario: A membership test is caught

- **GIVEN** a production line outside `src/agent_cli/` with `cli.contains("claude")` or `names.contains(&"codex")`
- **WHEN** the CLI-name audit runs
- **THEN** it SHALL report that line

Test: `tests/source_audit.rs::cli_name_audit_catches_each_comparison_form`

#### Scenario: Comparisons inside the module and in tests pass

- **GIVEN** CLI-name comparisons only inside `src/agent_cli/` or inside `#[cfg(test)]` items
- **WHEN** the CLI-name audit runs
- **THEN** it SHALL report nothing

Test: `tests/source_audit.rs::a_cli_name_comparison_in_a_test_item_is_not_a_finding`, `tests/source_audit.rs::cli_names_are_compared_only_inside_agent_cli`

#### Scenario: The current tree has no finding outside the module

- **WHEN** the CLI-name audit runs over the current tree
- **THEN** no production file outside `src/agent_cli/` SHALL have a finding

Test: `tests/source_audit.rs::cli_names_are_compared_only_inside_agent_cli`

### Requirement: Operator isolation is a provider facet

Each provider family SHALL declare its operator-isolation launch environment and arguments: what keeps a worker from loading the operator's personal configuration and extensions and from writing memory the operator's sessions share. An environment entry MAY assign the empty value to clear an inherited variable. A family that encodes none SHALL contribute nothing, so the `generic` fallback launches unchanged.

#### Scenario: The claude family declares its isolation

- **GIVEN** the `claude` provider family
- **WHEN** its operator isolation is resolved
- **THEN** the arguments SHALL be `--setting-sources project,local`
- **AND** the environment SHALL be `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` and `CLAUDE_CODE_PLUGIN_DIRS=` (empty), in that order

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
- **THEN** it SHALL be `env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= claude --model opus --setting-sources project,local --permission-mode acceptEdits --session-id abc`

Test: `agent_cli::tests::a_claude_worker_is_launched_isolated`

#### Scenario: Recovery keeps the isolation

- **GIVEN** a stopped session whose entry has `cli = "claude"`, no `[clis.claude]` entry, and a recorded session id
- **WHEN** the session is recovered
- **THEN** the pane command SHALL start with `env CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= claude`
- **AND** it SHALL contain `--setting-sources project,local` before `--resume <id>`

Test: `agent_cli::tests::recovery_keeps_the_isolation`, `tests/worker_operator_isolation_e2e.rs::recovery_through_start_keeps_the_isolation`

#### Scenario: Start and add stay identical

- **GIVEN** the same claude-family CLI, approval level and configuration
- **WHEN** one worker is launched by `git paw start` and another by `git paw add`
- **THEN** both pane commands SHALL carry the same isolation environment and arguments

Test: `tests/worker_operator_isolation_e2e.rs::start_and_add_launch_claude_workers_with_identical_isolation`

### Requirement: A CLI may inherit the operator's configuration

`[clis.<name>]` SHALL accept an optional boolean `inherit_operator_config`, default `false`. When it is `true`, the launch builder SHALL add neither that CLI's isolation environment nor its isolation arguments. A config without the key SHALL load unchanged and SHALL NOT gain the key when written back.

#### Scenario: Opting out restores the previous launch

- **GIVEN** `[clis.claude] command = "claude"` with `inherit_operator_config = true` and `agent_approval = "manual"`
- **WHEN** a worker using `claude` is launched
- **THEN** its pane command SHALL contain none of `--setting-sources`, `CLAUDE_CODE_DISABLE_AUTO_MEMORY` and `CLAUDE_CODE_PLUGIN_DIRS`

Test: `agent_cli::tests::opting_out_restores_the_previous_launch`

#### Scenario: An existing config round-trips without the key

- **GIVEN** a config whose `[clis.*]` entries have no `inherit_operator_config`
- **WHEN** it is loaded and written back
- **THEN** the written file SHALL NOT contain `inherit_operator_config`

Test: `config::tests::an_existing_config_round_trips_without_the_inherit_operator_config_key`
