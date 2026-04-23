## Context

The session recovery feature was implemented to allow users to stop and restart git-paw sessions while preserving worktree state. However, the broker/dashboard functionality was not properly integrated into the recovery process. While the session state correctly persists broker configuration fields (`broker_port`, `broker_bind`, `broker_log_path`), the `recover_session` function fails to use this information to recreate the dashboard pane.

## Goals / Non-Goals

**Goals:**
- Fix session recovery to recreate dashboard pane when original session had broker enabled
- Use original session's broker configuration, not current repository config
- Maintain backward compatibility with existing sessions
- Add comprehensive tests for all recovery scenarios

**Non-Goals:**
- Changing how new sessions are created (cmd_start behavior remains unchanged)
- Modifying broker configuration persistence (already working correctly)
- Adding new broker features or changing broker protocol

## Decisions

**Decision 1: Use original session state, not current config**
- **Rationale**: Session recovery should restore the exact state that existed when the session was stopped, not apply current configuration changes. This ensures predictable behavior across stop/start cycles.
- **Alternatives considered**: Using current config would be inconsistent with user expectations and could break workflows if config changes between stop/start.

**Decision 2: Minimal code change in recover_session**
- **Rationale**: The fix only requires changing the condition from `broker_config.enabled` to checking `existing.broker_port.is_some()` and reconstructing the URL from saved fields. This minimizes risk and maintains code clarity.
- **Alternatives considered**: More extensive refactoring was rejected as unnecessary for this focused fix.

## Risks / Trade-offs

**[Risk] Session with partial broker fields**: If a session has `broker_port` but missing `broker_bind`, recovery will skip dashboard creation.
- **Mitigation**: The pattern `if let (Some(port), Some(bind)) = ...` ensures both fields are present, which is safe since they're always written together.

**[Risk] Backward compatibility**: v0.2.0 sessions without broker fields must still recover correctly.
- **Mitigation**: The `Option` pattern naturally handles `None` values, so old sessions recover without dashboard as expected.

## Migration Plan

No migration required. The change is backward compatible:
- Existing sessions without broker fields recover normally (no dashboard)
- Existing sessions with broker fields now recover correctly (with dashboard)
- New sessions created with broker enabled will recover properly

## Open Questions

None - the implementation is straightforward and all edge cases are covered by the existing test patterns.