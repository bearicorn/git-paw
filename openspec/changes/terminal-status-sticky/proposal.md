## Why

The terminal status sticky behavior prevents important agent states ("done", "verified", "blocked", "committed") from being accidentally overwritten by transient states like "working". This ensures that once an agent reaches a terminal state, that information is preserved unless explicitly changed to another terminal state. This behavior was identified in audit-regression.md as needing formal specification and test coverage.

## What Changes

- Add formal specification for terminal status protection behavior
- Add comprehensive test coverage for terminal status scenarios
- Update documentation to reflect this behavior
- No breaking changes - this formalizes existing behavior

## Capabilities

### New Capabilities
- `terminal-status-protection`: Specifies that terminal agent states cannot be overwritten by non-terminal states

### Modified Capabilities
- `message-delivery`: Add requirement that `update_agent_record` must preserve terminal states

## Impact

- Affects: `src/broker/delivery.rs` (implementation already exists)
- Affects: `src/broker/messages.rs` (status definitions)
- Affects: Test suite (new tests needed)
- No breaking changes - this formalizes existing behavior