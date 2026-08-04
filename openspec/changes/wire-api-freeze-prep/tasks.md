# Tasks — wire-api-freeze-prep

## 1. Lenient list-field deserialization (broker wire, relax-only)
- [x] Add `#[serde(default)]` (and ONLY `#[serde(default)]` — no `skip_serializing_if`) to `StatusPayload.modified_files` in `src/broker/messages.rs`
- [x] Add `#[serde(default)]` to `ArtifactPayload.exports`
- [x] Add `#[serde(default)]` to `ArtifactPayload.modified_files`
- [x] Leave `IntentPayload.files` and `FeedbackPayload.errors` REQUIRED (no default) — they carry non-empty contracts (`EmptyIntentFiles`, `EmptyErrors`); omitting them must stay a clean missing-field parse error
- [x] Confirm no `skip_serializing_if` was added to any of the three defaulted fields (serialization output must stay byte-identical)

## 2. Internal-library-API doc note (Cargo/lint task — NOT a spec requirement)
- [x] Add a crate-level `//!` note to `src/lib.rs` stating the library API is internal, exists to serve the binary + tests, and is NOT covered by semver
- [x] Confirm the decision recorded: crate-level doc note chosen over `#[doc(hidden)]` on ~23 modules (the lighter option — one edit, no per-module churn, no docs.rs hiding)

## 3. Pin MSRV (Cargo task — NOT a spec requirement)
- [x] Add `rust-version = "1.97"` (current stable) to the `[package]` section of `Cargo.toml`
- [x] Verify `cargo build` succeeds and `Cargo.lock` still resolves

## 4. Enforce the doc-comment mandate (lint task — NOT a spec requirement)
- [x] Add a one-line `///` doc comment to `agents::inject_section_into_file` in `src/agents.rs` (the sole doc-coverage gap; precondition for the lint to pass clean) — **no edit needed**: the function already carries a `///` doc comment (`src/agents.rs:776`), so the precondition was already satisfied; `#![warn(missing_docs)]` compiles clean with zero gaps
- [x] Add `#![warn(missing_docs)]` to `src/lib.rs`
- [x] Verify `just check` (clippy `-D warnings`) passes with no missing-docs warning

## 5. Tests (behavioral)
- [x] Add a test: `{"status":"idle"}` deserializes as `StatusPayload` with empty `modified_files`
- [x] Add a test: minimal `agent.status` message (payload omits `modified_files`) parses via `from_json` into `BrokerMessage::Status`
- [x] Add a test: artifact payload JSON omitting both `exports` and `modified_files` deserializes with both empty
- [x] Add a test: `ArtifactPayload` with empty vecs still serializes `exports` and `modified_files` as `[]` (serialization unchanged)
- [x] Add a test: `agent.feedback` payload omitting `errors` fails to parse as a missing required field (NOT defaulted); an explicit `"errors": []` is still rejected with the empty-errors error
- [x] Add a test: `agent.intent` payload omitting `files` fails to parse as a missing required field (NOT defaulted); an explicit `"files": []` is still rejected with the empty-files error

## 6. Backward compatibility
- [x] Verify every existing broker-messages test passes unchanged (populated messages round-trip byte-equivalently)
- [x] Verify the `status_payload_v050_shape_round_trips_byte_equivalent` byte-equivalence test still passes (frozen `StatusPayload.message` behaviour untouched)
- [x] Confirm the bundled producer `assets/scripts/broker.sh` (emits `modified_files: []`) continues to parse — it emits the key explicitly (`assets/scripts/broker.sh:213`, `:245`), and an explicit `[]` was already accepted and still is (relax-only widening only adds the absent-key case)

## 7. Docs
- [x] Confirm no CLI `--help` change is needed (no CLI surface change) — `src/cli.rs` untouched; no flag, subcommand, or help string altered
- [x] Confirm README / mdBook "MSRV: current stable" statements stay consistent with the pinned `rust-version` — README's only MSRV text is the `MSRV: stable` badge pointing at `rust-toolchain.toml` (`channel = "stable"`); rustc 1.97.0 IS current stable, so the badge stays accurate and no mdBook chapter states an MSRV
- [x] Confirm the configuration reference needs no update (no config fields added) — no `[section]`/field was added to the config schema

## 8. Verification (five gates)
- [x] Gate 1 — Testing: `cargo test --no-fail-fast` for the new broker-messages tests passes — `broker::messages` 102 passed / 0 failed
- [x] Gate 2 — Regression: full suite green diffed against the merge-base — `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast` = **2465 passed / 0 failed across 88 suites**; branch is 2 commits ahead of `main` and 0 behind (merge-base = `e7b37ea`, not stale). **Supervisor re-verified in a clean serial env at the integrated tip (`feat/v0.13.0-specs`): 2469 passed / 0 failed / 89 suites, exit 0 — supersedes the in-session `GIT_PAW_ALLOW_LIVE_SESSION=1` run, which forced past the live-session guard the other agent correctly deferred.**
- [x] Gate 3 — Spec audit: every `Lenient list-field deserialization` scenario maps to a test; no other broker-messages requirement is contradicted (esp. the frozen serialization scenarios) — mapping below; scenario 6 needed a NEW byte-equivalence test (`artifact_payload_populated_lists_round_trip_byte_equivalent`) because the pre-existing `serde_roundtrip_artifact` only asserted values, not wire bytes
- [x] Gate 4 — Doc audit: crate doc note present, `--help`/README/mdBook consistent, MSRV surfaced, `mdbook build docs/` succeeds — build exits 0 (the two `<name>` unclosed-tag WARNs in `specifications/index.md` are pre-existing and outside this diff)
- [x] Gate 5 — Security: no secrets; relax-only parsing change introduces no unsafe shell/path handling; least-privilege unchanged — the diff is three serde attributes, doc comments, and one Cargo metadata key; no shell invocation, no path construction, no allowlist grant, no new dependency
- [x] `just check` green; `cargo fmt` before commit — `just lint` (= `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings`) and `cargo build` all run **bare** and judged by their real exit code (0), per AGENTS.md Change Checklist 5; clippy is clean including the newly enabled `missing_docs`
- [x] `openspec validate wire-api-freeze-prep --strict` passes (confirm by real exit code) — exit 0, "Change 'wire-api-freeze-prep' is valid"

### Gate 3 — scenario → test map

| Spec scenario | Test |
|---|---|
| Absent `modified_files` in a status payload defaults to empty | `status_payload_absent_modified_files_defaults_to_empty` |
| Minimal status message parses via `from_json` | `from_json_minimal_status_message_parses` |
| Absent `exports` and `modified_files` in an artifact payload default to empty | `artifact_payload_absent_lists_default_to_empty` |
| Serialization still emits the empty arrays | `artifact_payload_empty_lists_still_serialise_as_empty_arrays` |
| Non-empty-contract fields stay required and are not defaulted | `from_json_feedback_absent_errors_is_missing_field_error`, `from_json_intent_absent_files_is_missing_field_error` |
| Existing populated messages round-trip unchanged | `artifact_payload_populated_lists_round_trip_byte_equivalent` (new), plus `status_payload_v050_shape_round_trips_byte_equivalent` and `v050_string_only_intent_round_trips_byte_equivalent` still green |
