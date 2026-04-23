## Why

Standardized boot-prompt injection is essential for reliable agent behavior and consistent supervisor operation. Without a uniform boot instruction set, agents may fail to self-report their status, making monitoring and coordination difficult. This change establishes a foundation for predictable agent behavior across all usage modes (supervisor and manual broker).

## What Changes

- Introduce standardized boot instruction block format
- Create shared `build_boot_block()` helper function
- Implement boot-prompt injection in both supervisor auto-start and manual broker modes
- Add template substitution for `{{BRANCH_ID}}` and `{{GIT_PAW_BROKER_URL}}`
- Update supervisor skill template to include boot instructions

## Capabilities

### New Capabilities
- `boot-block-format`: Standard format and content for boot instruction blocks
- `template-substitution`: Variable substitution in boot blocks (branch ID, broker URL)
- `supervisor-injection`: Boot block prepending in supervisor auto-start mode
- `manual-injection`: Boot block injection in manual broker mode
- `shared-helper`: Common `build_boot_block()` function for consistency

### Modified Capabilities
- None (this introduces entirely new functionality)

## Impact

**Affected Components:**
- `src/skills.rs`: New `build_boot_block()` function
- `src/agents.rs`: Boot-prompt injection logic
- `src/main.rs`: Supervisor mode boot block integration
- `assets/agent-skills/supervisor.md`: Updated skill template
- `src/tmux.rs`: Manual mode boot block injection

**Dependencies:**
- No new external dependencies required

**Breaking Changes:**
- None (purely additive functionality)

**Configuration:**
- No new configuration required (boot blocks always enabled)