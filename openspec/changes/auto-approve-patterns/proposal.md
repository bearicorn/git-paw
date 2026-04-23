## Why

Auto-approve patterns are essential to reduce the overwhelming permission prompt overhead that blocks agent productivity. During dogfood testing, each agent hit 4-8 permission prompts per task, requiring constant manual intervention from the supervisor. This change automates approval of safe, common commands to enable unattended operation and improve efficiency.

## What Changes

- Implement permission prompt detection via tmux capture-pane
- Create safe command classification system
- Add automatic approval for common development commands
- Implement shared allowlist for curl commands
- Add configuration for approval levels and patterns

## Capabilities

### New Capabilities
- `permission-detection`: Detect permission prompts in agent panes
- `safe-command-classification`: Classify commands as safe for auto-approval
- `automatic-approval`: Send approval key sequences to stuck agents
- `curl-allowlist`: Shared allowlist for broker curl commands
- `approval-configuration`: Configurable approval levels and patterns

### Modified Capabilities
- None (this introduces entirely new functionality)

## Impact

**Affected Components:**
- `src/broker/delivery.rs`: Stall detection integration
- `src/tmux.rs`: Permission prompt detection via capture-pane
- `src/config.rs`: Approval configuration options
- `assets/agent-skills/supervisor.md`: Updated skill template
- `src/main.rs`: Supervisor mode integration

**Dependencies:**
- No new external dependencies required

**Breaking Changes:**
- None (purely additive functionality)

**Configuration:**
- New `[supervisor] auto_approve` configuration section
- New `[supervisor] allowed_commands` pattern list