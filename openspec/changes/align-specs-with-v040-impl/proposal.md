## Why

Several main specs in `openspec/specs/` describe the v0.3.0 surface of types and functions that have evolved during v0.4.0 work. Implementation has shipped, gates are green, and downstream code depends on the new shape — but the spec text still describes the old signatures. CLAUDE.md's "Match specs exactly" rule is currently violated for a handful of public APIs, even though the runtime behavior is correct.

This change documents the as-built v0.4.0 surface so the spec set is coherent before tagging the release. No production code changes ship with this spec-alignment change; only spec text moves.

## What Changes

- `broker-server`: update `start_broker` signature to include the `watch_targets: Vec<WatchTarget>` parameter introduced by hook-injection, and acknowledge that callers wrap state in `Arc<BrokerState>` rather than relying on `BrokerState: Clone`.
- `dashboard`: update `run_dashboard` signature to take `&Arc<BrokerState>` plus a `&AtomicBool` shutdown flag, and document that the tick interval is poll-driven for input responsiveness rather than fixed at 1s.
- `error-handling`: extend `BrokerError::PortInUse` to carry the underlying `std::io::Error` (`source` field) so the diagnostic surfaces the actual bind failure cause.
- `configuration`: reconcile the `[specs]` section field names with `spec-scanning` and the implementation: the fields are `dir` and `type` (not `specs_dir` / `enabled`), and `[logging]` is documented to match the implementation surface.

## Capabilities

### Modified Capabilities
- `broker-server`: signature update + state-handle clarification
- `dashboard`: signature update + tick-cadence clarification
- `error-handling`: `PortInUse` carries `source`
- `configuration`: `[specs]` field names match impl + `spec-scanning`

### New Capabilities
- None

## Impact

**Affected Components:**
- `openspec/specs/broker-server/spec.md`
- `openspec/specs/dashboard/spec.md`
- `openspec/specs/error-handling/spec.md`
- `openspec/specs/configuration/spec.md`

**Dependencies:**
- None. No code changes; spec text alignment only.

**Breaking Changes:**
- None. Production binaries already ship the documented behavior; this change merely makes the specs match.

**Configuration:**
- Documents existing `[specs] dir = "..."` / `[specs] type = "..."` field names. No user-visible config change.
