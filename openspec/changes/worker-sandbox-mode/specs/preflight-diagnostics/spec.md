## ADDED Requirements

### Requirement: Doctor reports each worker CLI's declared sandbox mode

`git paw doctor` SHALL report, for every CLI a worker of the current configuration can resolve to
(the configured default, spec-driven and preset CLIs), its declared sandbox mode — `native`,
`wrapper` or `none` — labelled as declared rather than verified. It SHALL report ⚠ with a remedy
pointing at the sandbox documentation when the effective worker approval level is `full-auto` and a
CLI it reports declares no sandbox. This check SHALL never report ✗, SHALL be read-only, and SHALL
NOT invoke any CLI or sandbox.

#### Scenario: A declared sandbox is reported

- **GIVEN** `default_cli = "sbx"` and `[clis.sbx] sandbox = "wrapper"`
- **WHEN** `git paw doctor` runs
- **THEN** it SHALL report `sbx` with sandbox mode `wrapper (declared)`

#### Scenario: Unsandboxed full-auto warns

- **GIVEN** `agent_approval = "full-auto"` and `default_cli = "claude"` with no `[clis.claude].sandbox`
- **WHEN** `git paw doctor` runs
- **THEN** the sandbox-mode check SHALL report ⚠ naming `claude` with a remedy pointing at the
  sandbox documentation
- **AND** it SHALL NOT report ✗

#### Scenario: Sandboxed full-auto passes

- **GIVEN** `agent_approval = "full-auto"` and `default_cli = "sbx"` with `[clis.sbx] sandbox = "wrapper"`
- **WHEN** `git paw doctor` runs
- **THEN** the sandbox-mode check SHALL report ✓
