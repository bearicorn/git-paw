# Proposal: Add --force Flag to Start Command

## Summary

Add a `--force` flag to the `git paw start --from-specs` command that allows users to bypass the uncommitted-spec validation warning.

## Problem Statement

Currently, when users run `git paw start --from-specs` with uncommitted OpenSpec changes, the system should warn them but provides no way to proceed if they intentionally want to launch with uncommitted specs. This blocks workflows where users want to test or iterate on specs before committing them.

## Proposed Solution

Add a `--force` flag to the `start` command that:

1. **Bypasses the uncommitted-spec warning** when validation detects uncommitted changes
2. **Logs the force usage** for audit purposes
3. **Maintains safety** by still performing validation (just suppresses the warning)
4. **Follows existing patterns** from the `purge --force` implementation

## Impact Analysis

### Affected Components

- **CLI Layer** (`src/cli.rs`): Add `--force` flag to `Start` struct
- **Validation Logic** (`src/main.rs`): Modify `cmd_start_from_specs` to check `--force` before warning
- **Error Handling**: Add new error type for uncommitted specs
- **Testing**: Add unit tests for flag parsing and behavior

### User Experience

**Before:**
```bash
$ git paw start --from-specs
warning: Uncommitted spec changes detected in 'my-feature'
         Commit your changes or use --force to proceed
```

**After:**
```bash
$ git paw start --from-specs --force
# Proceeds without warning (but logs force usage)
```

### Backward Compatibility

- **No breaking changes**: Existing behavior preserved
- **Opt-in feature**: Only affects users who explicitly use `--force`
- **Default behavior**: Warning still shown by default

## Success Metrics

1. Users can proceed with uncommitted specs when using `--force`
2. Force usage is logged for audit trail
3. All existing tests continue to pass
4. New tests cover force flag scenarios

## Open Questions

1. Should force usage be persisted in session metadata?
2. Should there be additional confirmation for destructive operations?
3. Should the force flag apply to other validation checks in the future?

## Next Steps

1. ✅ Create OpenSpec change structure
2. Write formal specification
3. Implement CLI flag
4. Add validation logic
5. Write tests
6. Update documentation