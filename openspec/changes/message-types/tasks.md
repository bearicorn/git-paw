## 1. Module scaffolding

- [ ] 1.1 Create directory `src/broker/`
- [ ] 1.2 Create `src/broker/mod.rs` with module-level doc comment and a single `pub mod messages;` declaration
- [ ] 1.3 Add `pub mod broker;` to `src/main.rs` (or `src/lib.rs` if applicable) so the new module is reachable from the crate root
- [ ] 1.4 Confirm `cargo build` succeeds with the empty module

## 2. Payload structs

- [ ] 2.1 Create `src/broker/messages.rs` with module-level doc comment
- [ ] 2.2 Define `StatusPayload` struct with fields `status: String`, `modified_files: Vec<String>`, `message: Option<String>`; derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`
- [ ] 2.3 Define `ArtifactPayload` struct with fields `status: String`, `exports: Vec<String>`, `modified_files: Vec<String>`; derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`
- [ ] 2.4 Define `BlockedPayload` struct with fields `needs: String`, `from: String`; derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`
- [ ] 2.5 Add doc comments to all three payload structs explaining their purpose and field semantics

## 3. BrokerMessage envelope

- [ ] 3.1 Define `BrokerMessage` enum with three variants `Status`, `Artifact`, `Blocked`, each with named fields `agent_id: String` and `payload: <PayloadType>`
- [ ] 3.2 Apply `#[serde(tag = "type", rename_all = "snake_case")]` and per-variant `#[serde(rename = "agent.status")]` etc. so the wire discriminator matches the spec exactly
- [ ] 3.3 Derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`
- [ ] 3.4 Add module-level doc comment summarizing the wire format with a JSON example for each variant

## 4. Validation

- [ ] 4.1 Define a `MessageError` error type using `thiserror`, with variants for `EmptyAgentId`, `InvalidAgentIdChars`, `EmptyStatusField`, `EmptyNeedsField`, `EmptyFromField`, and `Deserialize(serde_json::Error)`
- [ ] 4.2 Add `MessageError` to `src/error.rs` `PawError` as a wrapped variant, OR keep it module-local if `PawError` integration is left to `http-broker`; make a deliberate choice and document it in the doc comment
- [ ] 4.3 Implement `BrokerMessage::from_json(input: &str) -> Result<Self, MessageError>` that deserializes via `serde_json::from_str` and then calls `validate`
- [ ] 4.4 Implement a private `BrokerMessage::validate(&self) -> Result<(), MessageError>` method that checks: non-empty trimmed `agent_id`, `agent_id` matches the slug character set `[a-z0-9-_]+`, and per-variant required-field non-emptiness
- [ ] 4.5 Document on `BrokerMessage` that direct construction via enum variants bypasses validation and is intended only for tests and trusted internal callers; production code MUST use `from_json`

## 5. Display formatting

- [ ] 5.1 Implement `impl std::fmt::Display for BrokerMessage` matching the exact format strings in the spec
- [ ] 5.2 Verify no `\n` is emitted by any branch of the formatter
- [ ] 5.3 Verify no ANSI escape sequences are emitted

## 6. Branch slug function

- [ ] 6.1 Implement `pub fn slugify_branch(branch: &str) -> String` in `src/broker/messages.rs` following the five rules in the spec, in order
- [ ] 6.2 Add doc comment with the rule list and at least three input/output examples
- [ ] 6.3 Document the non-ASCII lossiness limitation in the doc comment

## 7. Unit tests

- [ ] 7.1 Add `#[cfg(test)] mod tests` block at the bottom of `src/broker/messages.rs`
- [ ] 7.2 Write round-trip tests for each variant (Status, Artifact, Blocked) — serialize a value, parse it back, assert equality, and assert the JSON contains the expected `type` discriminator
- [ ] 7.3 Write a test that parsing `{"type": "agent.unknown", ...}` fails
- [ ] 7.4 Write tests for `StatusPayload` with all fields and with empty `modified_files` + `message: None`
- [ ] 7.5 Write tests for `ArtifactPayload` with and without exports
- [ ] 7.6 Write a test for `BlockedPayload` round-trip
- [ ] 7.7 Write validation rejection tests: empty `agent_id`, whitespace-only `agent_id`, `agent_id` containing `/`, empty `status` in `agent.status`, empty `status` in `agent.artifact`, empty `needs` in `agent.blocked`, empty `from` in `agent.blocked`
- [ ] 7.8 Write a positive validation test: a well-formed JSON message of each variant produces a `BrokerMessage` whose fields match the input
- [ ] 7.9 Write `Display` tests for each of the three example outputs in the spec, asserting exact string equality
- [ ] 7.10 Write `Display` tests asserting no newlines and no ANSI escape sequences in any output
- [ ] 7.11 Write `slugify_branch` tests covering every scenario in the spec: simple feature branch, uppercase, nested path, underscores preserved, runs collapsed, leading/trailing trimmed, non-ASCII replaced, empty input, all-separator input, determinism (call twice)
- [ ] 7.12 Confirm every spec scenario in `specs/broker-messages/spec.md` has at least one corresponding test in this file

## 8. Quality gates

- [ ] 8.1 `cargo fmt` clean
- [ ] 8.2 `cargo clippy --all-targets -- -D warnings` clean (no `unwrap`/`expect` in non-test code, all public items documented)
- [ ] 8.3 `cargo test` — all new tests pass
- [ ] 8.4 `cargo doc --no-deps` builds without warnings for the new module
- [ ] 8.5 Run `just check` — full pipeline green

## 9. Handoff readiness

- [ ] 9.1 Confirm `src/broker/mod.rs` exposes `pub mod messages;` and nothing else (no premature `BrokerState` or server stubs — those belong to `http-broker`)
- [ ] 9.2 Confirm no changes to files outside `src/broker/`, `src/main.rs` (or `src/lib.rs`), and `src/error.rs` (if `MessageError` was integrated)
- [ ] 9.3 Confirm Cargo.toml is unchanged (no new dependencies added — this change uses existing serde + thiserror only)
- [ ] 9.4 Commit with message: `feat(broker): add message types and slug function`
