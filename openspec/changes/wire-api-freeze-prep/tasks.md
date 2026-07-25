# Tasks — wire-api-freeze-prep

## 1. Lenient list-field deserialization (broker wire, relax-only)
- [ ] Add `#[serde(default)]` (and ONLY `#[serde(default)]` — no `skip_serializing_if`) to `StatusPayload.modified_files` in `src/broker/messages.rs`
- [ ] Add `#[serde(default)]` to `ArtifactPayload.exports`
- [ ] Add `#[serde(default)]` to `ArtifactPayload.modified_files`
- [ ] Leave `IntentPayload.files` and `FeedbackPayload.errors` REQUIRED (no default) — they carry non-empty contracts (`EmptyIntentFiles`, `EmptyErrors`); omitting them must stay a clean missing-field parse error
- [ ] Confirm no `skip_serializing_if` was added to any of the three defaulted fields (serialization output must stay byte-identical)

## 2. Internal-library-API doc note (Cargo/lint task — NOT a spec requirement)
- [ ] Add a crate-level `//!` note to `src/lib.rs` stating the library API is internal, exists to serve the binary + tests, and is NOT covered by semver
- [ ] Confirm the decision recorded: crate-level doc note chosen over `#[doc(hidden)]` on ~23 modules (the lighter option — one edit, no per-module churn, no docs.rs hiding)

## 3. Pin MSRV (Cargo task — NOT a spec requirement)
- [ ] Add `rust-version = "1.97"` (current stable) to the `[package]` section of `Cargo.toml`
- [ ] Verify `cargo build` succeeds and `Cargo.lock` still resolves

## 4. Enforce the doc-comment mandate (lint task — NOT a spec requirement)
- [ ] Add a one-line `///` doc comment to `agents::inject_section_into_file` in `src/agents.rs` (the sole doc-coverage gap; precondition for the lint to pass clean)
- [ ] Add `#![warn(missing_docs)]` to `src/lib.rs`
- [ ] Verify `just check` (clippy `-D warnings`) passes with no missing-docs warning

## 5. Tests (behavioral)
- [ ] Add a test: `{"status":"idle"}` deserializes as `StatusPayload` with empty `modified_files`
- [ ] Add a test: minimal `agent.status` message (payload omits `modified_files`) parses via `from_json` into `BrokerMessage::Status`
- [ ] Add a test: artifact payload JSON omitting both `exports` and `modified_files` deserializes with both empty
- [ ] Add a test: `ArtifactPayload` with empty vecs still serializes `exports` and `modified_files` as `[]` (serialization unchanged)
- [ ] Add a test: `agent.feedback` payload omitting `errors` fails to parse as a missing required field (NOT defaulted); an explicit `"errors": []` is still rejected with the empty-errors error
- [ ] Add a test: `agent.intent` payload omitting `files` fails to parse as a missing required field (NOT defaulted); an explicit `"files": []` is still rejected with the empty-files error

## 6. Backward compatibility
- [ ] Verify every existing broker-messages test passes unchanged (populated messages round-trip byte-equivalently)
- [ ] Verify the `status_payload_v050_shape_round_trips_byte_equivalent` byte-equivalence test still passes (frozen `StatusPayload.message` behaviour untouched)
- [ ] Confirm the bundled producer `assets/scripts/broker.sh` (emits `modified_files: []`) continues to parse

## 7. Docs
- [ ] Confirm no CLI `--help` change is needed (no CLI surface change)
- [ ] Confirm README / mdBook "MSRV: current stable" statements stay consistent with the pinned `rust-version`
- [ ] Confirm the configuration reference needs no update (no config fields added)

## 8. Verification (five gates)
- [ ] Gate 1 — Testing: `cargo test --no-fail-fast` for the new broker-messages tests passes
- [ ] Gate 2 — Regression: full suite green diffed against the merge-base
- [ ] Gate 3 — Spec audit: every `Lenient list-field deserialization` scenario maps to a test; no other broker-messages requirement is contradicted (esp. the frozen serialization scenarios)
- [ ] Gate 4 — Doc audit: crate doc note present, `--help`/README/mdBook consistent, MSRV surfaced, `mdbook build docs/` succeeds
- [ ] Gate 5 — Security: no secrets; relax-only parsing change introduces no unsafe shell/path handling; least-privilege unchanged
- [ ] `just check` green; `cargo fmt` before commit
- [ ] `openspec validate wire-api-freeze-prep --strict` passes (confirm by real exit code)
