## ADDED Requirements

### Requirement: Bundled skill text carries the agent's own CLI fragment

The system SHALL render per-CLI content into bundled templates through fragments: a package MAY register a fragment per CLI family and slot at `assets/cli/<family>/skill.md` and `assets/cli/<family>/boot.md`, and a template SHALL include the selected fragment through `{{CLI_NOTES}}` (skill slot) or `{{CLI_BOOT_NOTES}}` (boot slot). The fragment SHALL be selected by a pure function of the agent's resolved CLI profile's skill family and the slot: the family's fragment when the package registers one, otherwise the `generic` fragment. A custom `[clis.<name>]` CLI SHALL use the family its resolved profile declares; rendering code SHALL NOT match CLI names. A render with no agent CLI (the repository-root install, MCP `get_skill`) SHALL use `generic`. Every package that uses a CLI placeholder SHALL register a `generic` fragment for that slot, and the `generic` fragment SHALL name no vendor product.

#### Scenario: A Claude-family agent gets the Claude fragment

- **GIVEN** an agent whose resolved CLI profile has skill family `claude`
- **WHEN** its coordination skill is rendered
- **THEN** the rendered text SHALL contain the `claude` skill fragment
- **AND** SHALL NOT contain another family's fragment

#### Scenario: An unrecognised CLI gets the generic fragment

- **GIVEN** an agent whose CLI profile is the `generic` fallback
- **WHEN** its coordination skill is rendered
- **THEN** the rendered text SHALL contain the `generic` skill fragment

#### Scenario: A custom variant inherits its declared family

- **GIVEN** a `[clis.<name>]` entry whose resolved profile declares skill family `claude`
- **WHEN** an agent using that CLI has its coordination skill rendered
- **THEN** the rendered text SHALL contain the `claude` skill fragment

#### Scenario: A family without a fragment falls back to generic

- **GIVEN** an agent whose skill family has no registered fragment for the skill slot
- **WHEN** its coordination skill is rendered
- **THEN** the rendered text SHALL contain the `generic` skill fragment

#### Scenario: Agents on different CLIs in one session get different fragments

- **GIVEN** one session with an agent on a `claude`-family CLI and an agent on an unrecognised CLI
- **WHEN** both agents' managed blocks are generated
- **THEN** the first SHALL carry the `claude` fragment and the second the `generic` fragment

#### Scenario: A render with no agent uses generic

- **WHEN** the coordination skill is rendered for the repository root or served by MCP `get_skill`
- **THEN** the rendered text SHALL contain the `generic` skill fragment

## MODIFIED Requirements

### Requirement: Per-CLI compact mechanism table

The coordination skill SHALL state the compact / clear mechanism for the agent's own CLI, rendered from that CLI family's skill fragment in place of a table enumerating every CLI. The `claude` family fragment SHALL name `/compact` as the compact command and `/clear` as the clear command. The `generic` fragment SHALL direct the agent to look for its CLI's `/compact`, `/save`, or `/reset` equivalent and SHALL name no vendor. No bundled text SHALL name a specific custom CLI variant (such as `claude-oss`); a variant receives its family's fragment.

#### Scenario: Claude-family agent sees its slash commands

- **GIVEN** an agent whose CLI profile has skill family `claude`
- **WHEN** the context-budget section of its rendered coordination skill is read
- **THEN** it SHALL name `/compact` and `/clear` as that agent's compact and clear commands

#### Scenario: Generic fallback points users at their CLI's equivalent

- **GIVEN** an agent on a CLI with no family fragment
- **WHEN** the context-budget section of its rendered coordination skill is read
- **THEN** it SHALL direct the agent to look for the CLI's `/compact`, `/save`, or `/reset` equivalent

#### Scenario: No custom variant is named

- **WHEN** the bundled coordination skill and all its fragments are inspected
- **THEN** none SHALL contain `claude-oss`
