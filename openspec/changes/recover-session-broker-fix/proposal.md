## Why

The session recovery feature currently fails to recreate the dashboard pane when recovering a session that was originally created with broker enabled. This happens because `recover_session` only checks the current config's `broker.enabled` flag instead of checking whether the original session had broker enabled (indicated by `broker_port` being `Some` in the session state). This regression was identified in audit-regression.md as "recover_session drops dashboard" and needs to be fixed to ensure broker/dashboard functionality works correctly across stop/start cycles.

## What Changes

- Modify `recover_session` function in `src/main.rs` to check the original session's broker state (via `existing.broker_port.is_some()`) instead of relying solely on current config
- Add dashboard pane and broker environment variable when recovering a session that originally had broker enabled
- Ensure the recovery behavior matches the original session creation behavior from `cmd_start`

## Capabilities

### New Capabilities
- None (this is filling a specification gap, not adding new capabilities)

### Modified Capabilities
- `session-state`: Add requirement for session recovery to recreate dashboard pane using original session's broker configuration

## Impact

- `src/main.rs`: `recover_session` function will be modified
- Session recovery will correctly recreate dashboard panes for sessions originally created with broker enabled
- No breaking changes to existing APIs or behavior
- Fixes the regression identified in audit-regression.md