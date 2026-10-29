## 1. Golden fixtures (before any code change)

- [ ] 1.1 Capture byte fixtures under `tests/fixtures/wire/`: one JSON per `BrokerMessage` variant (including a `FileIntent` mix, `AdvancedMain` with and without `summary`, `StatusPayload` with `message: null`), one global session file per era (v0.2.0 minimal, broker fields, `dashboard_pane`, `cli_session_id`), one discovery file (with and without orchestrator), and one manual-approvals JSONL (`broker.log` is not a contract edge and gets no fixture)
- [ ] 1.2 Add fixture tests asserting today's encoders reproduce them; they must stay green through every wave

## 2. Manual approvals edge

- [ ] 2.1 Seam: route `append_line` / `aggregate` through `manual_approvals::wire::{encode, decode}`; then separate `ApprovalLogLine` from `ManualApproval` and drop the model's serde derives
- [ ] 2.2 Add `schema_version` (written as 1; absent means 1); `aggregate` skips newer lines and reports their count once on stderr

## 3. Session edge

- [ ] 3.1 Seam: route `save_session_in`, `load_all_sessions_in`, `find_session_for_repo_in` and `load_session_from` through `session::wire`; move the `created_at` serializer verbatim; separate `SessionFile` / `WorktreeEntryFile` from `Session` / `WorktreeEntry`, keeping every `default` / `skip_serializing_if`
- [ ] 3.2 `schema_version` serialized first; the lookup decodes to `serde_json::Value` first, and a newer version for a matching `repo_path` errors (naming the file and both versions, with the upgrade hint) while other repositories' newer files are skipped
- [ ] 3.3 `save_session_in` refuses to replace a newer-schema file
- [ ] 3.4 `RepoSessionFile` gains `schema_version` (written as 1)
- [ ] 3.5 If v0.18 `broker-verdict-authority` has landed: carry `broker_authority` and the `0600` write mode in the DTO, and keep it out of the discovery file

## 4. Broker message edge

- [ ] 4.1 Seam: `broker::wire::{encode, decode}` used by `BrokerMessage::from_json` / `to_json`, the `/publish` handler, `publish.rs` builders, MCP publish and `dashboard/broker_log.rs::pretty_json`
- [ ] 4.2 Separate `WireMessage` and the payload wire structs (verbatim attributes: `tag = "type"`, the renames, `flatten`, `untagged`, `Region` `tag = "kind"`, every default and skip); total mappers; drop serde from `BrokerMessage` and the payload model structs; `validate()` stays on the model
- [ ] 4.3 Add `src/broker/wire.rs` to AGENTS.md Change Checklist §7 as a compile-forcing `BrokerMessage` ripple site

## 5. Broker HTTP edge

- [ ] 5.1 Make `StatusResponse`, `AgentStatus`, `LogResponse`, `LogEntry`, `PollResponse` and `ErrorBody` public in `broker::wire`; the server encodes them (the `/status` `json!` and the private structs go away) and adds `schema_version: 1`
- [ ] 5.2 Replace `coordination::inventory::StatusAgent` + `StatusBody`, `supervisor::drive::AgentStatusRow` + `StatusResponse` (in `drive.rs` since v0.17.0 deleted `poll.rs`), and `publish::BrokerLogEntry` decoding with the shared types; keep skip-on-undecodable for log entries and defaults for optional agent fields
- [ ] 5.3 `probe_existing_broker` decodes `StatusResponse` instead of substring matching
- [ ] 5.4 If v0.17 `broker-repo-identity` has landed: `repo_root` lives in `StatusResponse`; if v0.18 `broker-host-separation` has landed: `BrokerClient` decodes through these types

## 6. broker.log (non-contract)

- [ ] 6.1 Leave `delivery::flush_entries` and the `broker.log` format untouched; no header, no separate formatter

## 7. Tests (one or more per scenario)

- [ ] 7.1 Session: a pre-change file loads unchanged; a saved file's first key is `schema_version` and the rest equals the fixture; full round-trip with every optional field
- [ ] 7.2 Newer schema: `git paw status` with a v2 file for this repo errors (not "no session"); `git paw start` fails and leaves the file byte-identical; another repo's v2 file is skipped (HOME overridden per `core-ci-hygiene`)
- [ ] 7.3 Discovery file contains `schema_version`; `sweep.sh` discovers the session name and agents from a versioned file (extend `tests/broker_sh_session_discovery.rs` or the sweep discovery test)
- [ ] 7.4 Envelope: the three round-trip scenarios; unknown type rejected; the emitted `type` set equals the eleven values; every variant equals its golden fixture both ways
- [ ] 7.5 Docs check: the broker protocol reference states `broker.log` is diagnostic only and points to `GET /log` (extend the existing docs-content test for that chapter)
- [ ] 7.6 HTTP: `schema_version` on `/status`, `/log` and `/messages` (and `git_paw` / `version` kept); drive-loop, inventory and MCP readers decode a row with `phase`; an undecodable log entry is skipped; the probe accepts a real status and rejects a substring decoy
- [ ] 7.7 `tests/wire_script_parity.rs`: every listed `broker.sh` / `sweep.sh` publishing verb, against a recording listener, decodes and validates
- [ ] 7.8 Approvals: new lines are versioned; pre-change logs aggregate equal to the fixture result; two v2 lines are skipped with one stderr notice
- [ ] 7.9 Standing guard `tests/wire_model_separation.rs`: `Session`, `WorktreeEntry`, `PawConfig` (once `config-schema-versioning` has landed), `BrokerMessage` and the manual-approval model derive no serde

## 8. Docs

- [ ] 8.1 Broker protocol reference (`docs/src/`, the broker/API chapter): `schema_version` on `/status`, `/log` and `/messages`, distinct from `version`; the eleven message types; `broker.log` is a diagnostic log, not a stable format, and machine consumers use `GET /log`
- [ ] 8.2 `docs/src/architecture.md` module list (`session/wire`, `broker/wire`); the `architecture-direction.md` "Where the code is today" serialized-formats row
- [ ] 8.3 User guide or configuration notes on the session file mention `schema_version` and the newer-version upgrade error

## 9. Gates

- [ ] 9.1 `cargo fmt`, then `just lint` (`clippy::await_holding_lock` stays clean; no lock, `.await` or spawn reordering in `broker/`)
- [ ] 9.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 9.3 `just deny`
- [ ] 9.4 `mdbook build docs/`
- [ ] 9.5 `openspec validate wire-format-dtos --strict`
