## ADDED Requirements

### Requirement: Bundled skill text carries the agent's own CLI fragment

The system SHALL render per-CLI content into bundled templates through fragments: a package MAY register one per CLI family and slot at `assets/cli/<family>/{skill,boot}.md` (`<family>` is the family's config name), and a template SHALL include the selected one through `{{CLI_NOTES}}` (skill slot) or `{{CLI_BOOT_NOTES}}` (boot slot). A package using a CLI placeholder SHALL register a vendor-free `generic` fragment for that slot.

#### Scenario: A Claude-family agent gets the Claude fragment

- **GIVEN** an agent whose resolved CLI profile has family `claude`
- **WHEN** its coordination skill is rendered
- **THEN** the rendered text SHALL contain the `claude` skill fragment
- **AND** SHALL NOT contain another family's fragment

#### Scenario: Agents on different CLIs in one session get different fragments

- **GIVEN** one session with an agent on a `claude`-family CLI and an agent on an unrecognised CLI
- **WHEN** both agents' managed blocks are generated
- **THEN** the first SHALL carry the `claude` fragment and the second the `generic` fragment

### Requirement: The CLI fragment is selected from the agent's resolved CLI family

The system SHALL select a fragment by a pure function of the slot and the family of the agent's resolved CLI profile (`cli-provider`): the family's fragment when the package registers one, otherwise the `generic` fragment. A custom `[clis.<name>]` CLI SHALL use the family its resolved profile carries. Rendering code SHALL NOT match CLI or family names. A render with no agent CLI (the repository-root install, MCP `get_skill`) SHALL use `generic`.

#### Scenario: An unrecognised CLI gets the generic fragment

- **GIVEN** an agent whose resolved CLI profile has family `generic`
- **WHEN** its coordination skill is rendered
- **THEN** the rendered text SHALL contain the `generic` skill fragment

#### Scenario: A custom variant inherits its declared family

- **GIVEN** a `[clis.<name>]` entry declaring `family = "claude"`
- **WHEN** an agent using that CLI has its coordination skill rendered
- **THEN** the rendered text SHALL contain the `claude` skill fragment

#### Scenario: A family without a fragment falls back to generic

- **GIVEN** an agent whose family has no registered fragment for the skill slot
- **WHEN** its coordination skill is rendered
- **THEN** the rendered text SHALL contain the `generic` skill fragment

#### Scenario: A render with no agent uses generic

- **WHEN** the coordination skill is rendered for the repository root or served by MCP `get_skill`
- **THEN** the rendered text SHALL contain the `generic` skill fragment

### Requirement: Compact mechanism for the agent's own CLI

The coordination skill SHALL state the compact / clear mechanism for the agent's own CLI, from its family's skill fragment, not a table of CLIs. The `claude` fragment SHALL name `/compact` (compact) and `/clear` (clear). The vendor-free `generic` fragment SHALL direct the agent to its CLI's `/compact`, `/save`, or `/reset` equivalent. No bundled text SHALL contain a name one project chose for its own `[clis.<name>]` entry (such as `claude-oss`).

#### Scenario: Claude-family agent sees its slash commands

- **GIVEN** an agent whose resolved CLI profile has family `claude`
- **WHEN** the context-budget section of its rendered coordination skill is read
- **THEN** it SHALL name `/compact` and `/clear` as that agent's compact and clear commands

#### Scenario: Generic fallback points users at their CLI's equivalent

- **GIVEN** an agent on a CLI whose family has no skill fragment
- **WHEN** the context-budget section of its rendered coordination skill is read
- **THEN** it SHALL direct the agent to look for the CLI's `/compact`, `/save`, or `/reset` equivalent

#### Scenario: No custom variant is named

- **WHEN** the bundled coordination skill and all its fragments are inspected
- **THEN** none SHALL contain `claude-oss`

## REMOVED Requirements

### Requirement: Per-CLI compact mechanism table

**Reason**: Each agent now sees only its own CLI's compact / clear mechanism, rendered from its CLI-family fragment, so the coordination skill no longer carries a table with a `claude` row and a generic `other` row.

**Migration**: Replaced by *Compact mechanism for the agent's own CLI*. Its assertions carry over: the `claude` row becomes the `claude` fragment (`/compact`, `/clear`; scenario *Claude-family agent sees its slash commands*), the `other` row becomes the `generic` fragment (scenario *Generic fallback points users at their CLI's equivalent*), and the ban on naming `claude-oss` becomes scenario *No custom variant is named*. Tests `skills::tests::coordination_skill_per_cli_mechanism_table` and `tests/coordination_helper_only_skill_content.rs::coordination_skill_carries_no_git_paw_history_or_variant_names` move to the new requirement.
