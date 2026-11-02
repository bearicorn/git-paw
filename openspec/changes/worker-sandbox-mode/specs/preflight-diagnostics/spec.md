## ADDED Requirements

### Requirement: Doctor reports each worker CLI's declared sandbox mode

`git paw doctor` SHALL report, for every CLI a worker of the current configuration can resolve to
(the configured default, spec-driven and preset CLIs), its declared sandbox mode — `native`,
`wrapper` or `none` — labelled as declared, never as verified. It SHALL be read-only and SHALL NOT
invoke any CLI or sandbox.

#### Scenario: A declared sandbox is reported

- **GIVEN** `default_cli = "sbx"` and `[clis.sbx] sandbox = "wrapper"`
- **WHEN** `git paw doctor` runs
- **THEN** it SHALL report `sbx` with sandbox mode `wrapper (declared)`

### Requirement: Doctor never passes a full-auto worker on a declaration alone

When the effective worker approval level is `full-auto`, the sandbox-mode check SHALL report ⚠ with
a remedy pointing at the sandbox documentation for every reported CLI: one declaring no sandbox, and
one declaring a sandbox, labelled `declared, not verified`. Neither SHALL be ✗.

#### Scenario: Unsandboxed full-auto warns

- **GIVEN** `agent_approval = "full-auto"` and `default_cli = "claude"` with no `[clis.claude].sandbox`
- **WHEN** `git paw doctor` runs
- **THEN** the sandbox-mode check SHALL report ⚠ naming `claude` with a remedy pointing at the
  sandbox documentation
- **AND** it SHALL NOT report ✗

#### Scenario: A declared sandbox at full-auto is not shown as verified

- **GIVEN** `agent_approval = "full-auto"` and `default_cli = "sbx"` with `[clis.sbx] sandbox = "wrapper"`
- **WHEN** `git paw doctor` runs
- **THEN** the sandbox-mode check SHALL report ⚠ for `sbx` labelled `declared, not verified`

### Requirement: Doctor flags sandbox declarations a launch cannot honour

The sandbox-mode check SHALL report ⚠ for a `wrapper` CLI whose `command`, by name or by the file it
resolves to on `PATH`, is a known agent CLI binary, and ✗ with the refusal for a CLI whose launch at
the effective level the native-sandbox rule would refuse.

#### Scenario: A wrapper that is the agent CLI itself warns

- **GIVEN** `default_cli = "sbx"` and `[clis.sbx] command = "claude"`, `sandbox = "wrapper"`
- **WHEN** `git paw doctor` runs
- **THEN** the sandbox-mode check SHALL report ⚠ for `sbx` naming `claude`

#### Scenario: Doctor reports a launch the sandbox rule would refuse

- **GIVEN** `agent_approval = "full-auto"`, `default_cli = "codex"` and `[clis.codex] sandbox = "native"`
- **WHEN** `git paw doctor` runs
- **THEN** the sandbox-mode check SHALL report ✗ naming `[clis.codex].approval_args`
