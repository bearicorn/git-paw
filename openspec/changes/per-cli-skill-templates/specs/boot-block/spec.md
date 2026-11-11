## MODIFIED Requirements

### Requirement: Paste handling instructions

The boot block SHALL include specific instructions for handling paste operations, particularly the requirement to send a second Enter key after pasted content, in both the helper and MCP forms. The explanation of how the agent's CLI displays pasted text SHALL come from the agent's CLI-family boot fragment: the `claude` fragment SHALL explain that the CLI collapses pasted text into `[Pasted text #N]`, and the `generic` fragment SHALL explain the behavior without naming a vendor.

#### Scenario: Paste handling instruction included

- **WHEN** the boot block is examined, for any CLI family
- **THEN** it SHALL contain instructions about paste detection
- **AND** it SHALL mention the need for a second Enter key

#### Scenario: Paste instruction format

- **GIVEN** an agent whose resolved CLI profile has family `claude`
- **WHEN** the paste handling section of its boot block is examined
- **THEN** it SHALL explain that pasted text is collapsed into `[Pasted text #N]`
- **AND** it SHALL instruct the agent to send an additional Enter after paste operations

#### Scenario: Paste instruction for a generic agent names no vendor

- **GIVEN** an agent whose resolved CLI profile has family `generic`
- **WHEN** the paste handling section of its boot block is examined
- **THEN** it SHALL instruct the agent to send an additional Enter after paste operations
- **AND** it SHALL NOT name a vendor product

### Requirement: Template variable substitution

The system SHALL support template variable substitution in boot blocks using the syntax `{{VARIABLE_NAME}}`. The system SHALL replace these variables with actual values at render time.

#### Scenario: Branch ID substitution

- **GIVEN** boot block template containing `{{BRANCH_ID}}`
- **WHEN** `build_boot_block("feat/errors", "http://localhost:9119", &profile)` is called
- **THEN** all occurrences of `{{BRANCH_ID}}` SHALL be replaced with `"feat-errors"`

#### Scenario: Broker URL substitution

- **GIVEN** boot block template containing `{{GIT_PAW_BROKER_URL}}`
- **WHEN** `build_boot_block("feat/errors", "http://localhost:9119", &profile)` is called
- **THEN** all occurrences of `{{GIT_PAW_BROKER_URL}}` SHALL be replaced with `"http://localhost:9119"`

### Requirement: Helper function signature

The `build_boot_block()` function SHALL take the branch id, the broker URL, and the agent's resolved CLI profile, and SHALL return the rendered boot block as a `String`:
```rust
pub fn build_boot_block(branch_id: &str, broker_url: &str, cli: &CliProfile) -> String
```
where `CliProfile` is the resolved per-CLI record of the `cli-provider` capability. The profile SHALL be the function's only source of per-CLI choice: its MCP capability picks the helper or the MCP form, and its family picks the CLI-family boot fragment. No caller SHALL pass a CLI name or a separately computed flag.

#### Scenario: Function accepts required parameters

- **WHEN** `build_boot_block("feat/errors", "http://localhost:9119", &profile)` is called with a resolved profile
- **THEN** it SHALL accept the parameters without error

#### Scenario: Function returns boot block string

- **WHEN** `build_boot_block("feat/errors", "http://localhost:9119", &profile)` is called
- **THEN** it SHALL return a `String` containing the boot instructions

#### Scenario: The generic profile yields the helper form with the generic fragment

- **GIVEN** the profile of family `generic` with no `mcp` override
- **WHEN** `build_boot_block` is called with it
- **THEN** the result SHALL be the helper form
- **AND** SHALL contain the `generic` boot fragment
