## Why

v0.13.0 is the last quality cycle before the v1.0.0 freeze locks the broker wire
format and the crate's public API surface. A handful of hardening changes are
**now-or-never**: they are *relax-only* (they accept strictly more, break no
existing producer) yet they become impossible to make compatibly once 1.0 ships.

- The broker wire format is a **frozen target**. Three required `Vec` payload
  fields with no non-empty contract have no `#[serde(default)]`, so serde treats
  a missing array as a hard parse error — a lean `{"status":"idle"}` heartbeat or
  a third-party publisher that omits an empty list is rejected today. Loosening a
  required field to optional is a compatible change; re-tightening one is not. If
  v1.0.0 is to accept lean/third-party publishers, this must land in the freeze
  cycle.
- The crate is published to crates.io as a **library** as well as a binary.
  `lib.rs` exposes 23 modules and ~807 bare-`pub` items that exist only to serve
  the bin + integration tests; at 1.0 every one becomes semver-frozen. The
  intent — "the library API is internal, not a stable dependency surface" — must
  be recorded before the freeze rather than discovered after it.
- The crate declares no `rust-version`; a downstream consumer on an old
  toolchain gets an opaque build failure instead of a clear MSRV error, and the
  resolver cannot pick MSRV-aware dependency versions.
- The AGENTS.md "`///` on all public items" mandate lives only in prose (doc
  coverage is 99.9%), so it can silently regress after the freeze with nothing
  to catch it.

All four items are behavior-preserving for existing producers and consumers; the
only observable change is that strictly more inputs now parse.

## What Changes

- **Lenient list-field deserialization (broker wire, relax-only).** Add
  `#[serde(default)]` to the three required `Vec` payload fields that have **no
  non-empty contract**: `StatusPayload.modified_files`, `ArtifactPayload.exports`,
  and `ArtifactPayload.modified_files`. An absent field deserializes to an empty
  vec. `IntentPayload.files` and `FeedbackPayload.errors` are deliberately
  **excluded** — they carry intentional non-empty contracts (`EmptyIntentFiles`,
  `EmptyErrors`), so defaulting them would be pointless (an absent field still
  gets rejected) and would only turn a clean serde "missing field" error into a
  defaulted-then-validation error while enlarging the frozen surface change.
  **Serialization output is unchanged** — no `skip_serializing_if` is added, so
  empty arrays are still emitted on the wire (preserving the frozen
  "`exports` present as empty array" and "`errors: []`" scenarios). This is the
  only item with a spec-requirement delta.
- **Internal-library-API doc note (Cargo/lint task, no spec delta).** Add a
  crate-level `//!` note to `src/lib.rs` stating the library API is internal and
  **not** covered by semver. Chosen over `#[doc(hidden)]` on ~23 modules as the
  lighter of the two options (one edit vs. touching every module; documents the
  semver intent without churn and without hiding items from docs.rs).
- **Pin MSRV (Cargo task, no spec delta).** Add `rust-version = "1.97"` (current
  stable) to `Cargo.toml`.
- **Enforce the doc mandate (lint task, no spec delta).** Add
  `#![warn(missing_docs)]` to `src/lib.rs` so doc coverage cannot regress. First
  document the single remaining gap (`agents::inject_section_into_file`) so the
  lint passes clean under CI's `-D warnings`.

No breaking changes. Nothing in the frozen serde "do-not-touch" set is altered
(see Impact).

## Capabilities

### New Capabilities
_None._

### Modified Capabilities
- `broker-messages`: adds one requirement — **Lenient list-field
  deserialization** — specifying that the three required `Vec` payload fields
  with no non-empty contract default to an empty vec when absent from the input
  JSON, while serialization continues to emit the arrays, and that the two
  non-empty-contract fields (`IntentPayload.files`, `FeedbackPayload.errors`)
  stay required. All existing broker-messages requirements are unchanged.

## Impact

- **Code:**
  - `src/broker/messages.rs` — `#[serde(default)]` on the three no-contract `Vec`
    fields, plus "minimal message parses" round-trip tests.
  - `src/lib.rs` — crate-level internal-API doc note and `#![warn(missing_docs)]`.
  - `src/agents.rs` — one-line `///` on `inject_section_into_file` (the sole doc
    gap; precondition for the lint to pass clean).
  - `Cargo.toml` — `rust-version = "1.97"`.
- **NOT enum-variant ripple:** no `BrokerMessage` or `SpecBackendKind` variant is
  added or removed. `#[serde(default)]` on three existing struct fields touches no
  exhaustive `match` (AGENTS.md checklist item 7 does not apply).
- **Frozen serde surfaces untouched** (per the principal-engineer audit): the
  `FileIntent` `#[serde(untagged)]` enum, the `AdvancedMain` `#[serde(flatten)]`
  payload, `StatusPayload.message`'s deliberate no-`skip_serializing_if`
  null-emitting behavior, the `Session.created_at` custom serializer, and the
  config `merged_with` default semantics are all left byte-identical.
- **Backward compatibility:** relax-only. The bundled producer
  `assets/scripts/broker.sh` already emits `modified_files: []`, so existing
  internal traffic is unaffected; every existing message still parses and
  round-trips byte-equivalently.
- **Docs:** no CLI surface change. The MSRV is surfaced by `Cargo.toml`
  metadata; README/mdBook already say "MSRV: current stable" and stay
  consistent. No configuration-reference change.
- **Tests:** additive round-trip / minimal-message tests for the three defaulted
  fields, plus missing-required-field parse-error tests for the two excluded
  fields (`IntentPayload.files`, `FeedbackPayload.errors`); all existing wire and
  byte-equivalence tests pass unchanged.
