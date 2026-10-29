## MODIFIED Requirements

### Requirement: A custom CLI declares its provider family

`[clis.<name>]` SHALL accept an optional `family` naming one of the in-tree families (`claude`,
`codex`, `agy`, `qwen`, `cursor-agent`, `opencode`, `generic`). A value outside that set SHALL be rejected at config
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

#### Scenario: A wrapper entry selects the opencode family

- **GIVEN** `[clis.oc-route] command = "/opt/tools/oc-wrapper"` and `family = "opencode"`
- **WHEN** its record is resolved
- **THEN** its family SHALL be `opencode`, and its prompt-shape profile and start-up dialogs SHALL
  be opencode's
