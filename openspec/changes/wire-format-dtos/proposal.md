## Why

Three of the formats v1.0.0 freezes use the in-memory model type as their serialized form, so
freezing the format would freeze the internals too:
- the global session file (`Session` / `WorktreeEntry`);
- the broker wire (`BrokerMessage`, whose JSON the bundled scripts build by hand);
- the manual-approvals JSONL (`ManualApproval`).

`config.toml` is a fourth, and `config-schema-versioning` covers it. MILESTONE also lists
`broker.log`, which is written through `BrokerMessage`'s `Display`. It is a poor format to
freeze: one machine-wide file (`<data_dir>/git-paw/sessions/broker.log`), written in a lossy
human form, deleted by any repository's purge, and parsed back by nothing. v0.22.0 adds a
per-run `events.jsonl` that persists the `GET /log` entry shape, and that change recommends
v1.0.0 decide whether to retire `broker.log`. This change therefore declares `broker.log` a
diagnostic log and not a contract, and does not version it.

None of these formats carries a version today. A session file written by a newer git-paw is
skipped as unreadable, so it looks like "no session" and can then be overwritten. The broker's
HTTP response shapes are declared once on the server and re-declared by every client:
`BrokerLogEntry` (`broker/publish.rs`, decoded by walking `serde_json::Value`), `StatusAgent`
(`coordination/inventory.rs`), `AgentStatusRow` (`supervisor/drive.rs`, moved there from the
deleted `supervisor/poll.rs` by v0.17.0), and `probe_broker`'s
raw substring match. These copies drift (none of the client copies reads `phase`), and no test
checks the scripts' hand-built JSON against the Rust decoder. The `broker-protocol` envelope
requirement still enumerates seven message types; the binary has eleven.

## What Changes

- **A DTO plus mapper at each contract edge, and nowhere else.** `session::wire`, `broker::wire`
  and `supervisor::manual_approvals::wire` hold the serde types. The in-memory `Session`,
  `BrokerMessage` (and payloads) and `ManualApproval` lose their serde derives and map to and
  from the DTOs. The serialized bytes stay identical, including the frozen surfaces:
  - `FileIntent` `untagged`;
  - `AdvancedMain` `flatten`;
  - `StatusPayload.message` without `skip_serializing_if`;
  - `Session.created_at`'s serializer.
- **Version fields, additive only:**
  - the global session file and the per-repo discovery file gain `schema_version` (written as
    1; absent means 1);
  - each manual-approvals line gains `schema_version`;
  - broker `GET /status`, `GET /log` and `GET /messages/:agent_id` responses gain a protocol
    `schema_version`.

  `BrokerMessage` envelope bytes do not change. Scripts keep publishing exactly what they
  publish today.
- **Newer files are never misread.** A session file for this repository that declares a newer
  `schema_version` makes the lookup fail with an upgrade error instead of reading as absent, and
  git-paw refuses to overwrite it. A manual-approvals line from a newer schema is skipped with
  one stderr note.
- **One shared `broker::wire` module** for the HTTP shapes: the status response and agent row,
  the log response and entry, the poll response, and the error body. The server serializes these
  types and every client decodes them. `BrokerLogEntry`, `StatusAgent` and `AgentStatusRow`
  become uses of the shared types, and `probe_broker` parses the status response. Client
  tolerance is kept: missing optional fields default, and an undecodable log entry is skipped.
  The log entry type is the one v0.22.0 persists as each `events.jsonl` line.
- **`broker.log` is declared non-contract.** The architecture contract lists it explicitly as a
  diagnostic log whose format may change, and the broker reference points machine consumers to
  `GET /log`. Its bytes are unchanged and it gets no version header.
- **Script↔Rust wire parity.** A test runs every publishing verb of `broker.sh` and `sweep.sh`
  against a recording stub and decodes each captured body through `broker::wire`.
- The `broker-protocol` envelope requirement enumerates all eleven `type` values.

No breaking change. The only new bytes are the version fields.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `session-state`: ADDED the versioned session wire document, newer-schema refusal, and the
  versioned per-repo discovery file.
- `broker-protocol`: MODIFIED *Broker message envelope* (eleven types, wire type separate from
  the model). ADDED shared HTTP wire types with a protocol `schema_version`, and helper-payload
  parity.
- `approval-pattern-surfacing`: ADDED versioned manual-decision log lines.
- `code-architecture`: ADDED "Serialized contracts are wire types separate from the domain
  model", which names the contract edges, excludes `broker.log` explicitly, and adds a guard.

## Impact

- **Code:**
  - new `src/session/wire.rs` (or `session_wire.rs`) and `src/broker/wire.rs`;
  - `src/session.rs` (`save_session_in`, `find_session_for_repo_in`, `load_all_sessions_in`,
    `RepoSessionFile`);
  - `src/broker/messages.rs` (serde moves to the wire; `from_json` / `to_json` delegate);
  - `src/broker/server.rs` (`PollResponse`, `LogResponse`, `LogEntry`, the `/status` `json!`);
  - `src/broker/mod.rs` (`AgentStatusEntry`, `probe_existing_broker`);
  - `src/broker/publish.rs` (`BrokerLogEntry`, `fetch_log_entries_over_http`);
  - `src/coordination/inventory.rs` (`StatusAgent`);
  - `src/supervisor/drive.rs` (`AgentStatusRow`, `fetch_status_over_http`);
  - `src/supervisor/manual_approvals.rs`;
  - `src/mcp/query/{session,conflicts,intents}.rs` and `src/mcp/tools/publish.rs`, which
    consume the shared types;
  - `src/dashboard/broker_log.rs::pretty_json`.
  - `src/broker/delivery.rs::flush_entries` (the `broker.log` writer) is not touched.
- **Scripts:** none change. The parity test pins them.
- **Enum-variant ripple (`BrokerMessage`):**
  - `src/broker/wire.rs` becomes a new compile-forcing site: the wire↔model mapper is two
    exhaustive matches.
  - The existing sites (`messages.rs` accessors, `dashboard/broker_log.rs`, `broker/learnings.rs`
    `observe`, and the semantic routing in `delivery.rs`) are untouched, because the variant set
    does not change.
  - AGENTS.md §7 gains `src/broker/wire.rs` in its list (a doc task).
- **Tests:**
  - golden-byte fixtures for every message variant, the session file, the discovery file and an
    approvals line, captured before the change;
  - `tests/wire_script_parity.rs`;
  - newer-schema refusal tests;
  - the existing `messages.rs` round-trip tests, which keep passing.
- **Docs:**
  - `docs/src/architecture.md` (wire modules);
  - `architecture-direction.md` ("Where the code is today" serialized-formats row);
  - the broker protocol reference (`/status`, `/log` and `/messages` gain `schema_version`;
    `broker.log` is diagnostic only);
  - the user guide note on the session file location.
- **Security / safety:** a newer-schema session file can no longer be silently replaced. No new
  input surface. Decoding stays lenient exactly where it is lenient today.
- **Cross-version dependencies:**
  - **v0.17** `classifier-retirement`: deletes `src/supervisor/poll.rs` after moving
    `AgentStatusRow` and `fetch_status_over_http` into `src/supervisor/drive.rs`, and has the drive
    loop write the manual-decision log for every escalated live prompt (same four fields). This
    change decodes `/status` there and versions those lines.
  - **v0.17** `broker-repo-identity`: adds `repo_root` to `/status` (the shared status DTO
    carries it) and renames the log to a per-session file. That does not change the log's
    non-contract status.
  - **v0.17** `bundled-helper-discovery`: modifies *Find session by repository path*. The
    newer-schema rule is ADDED, so there is no collision, but the lookup code is shared.
  - **v0.18** `broker-verdict-authority`: `Session.broker_authority` and the `0600` mode must be
    carried by the session DTO, and stay excluded from the discovery file.
  - **v0.18** `broker-host-separation`: its `broker::client::BrokerClient` decodes through
    `broker::wire`. Whichever change lands second rebases.
  - **v0.18** `terminal-backend-seam`: persisted `pane_index` / `dashboard_pane` stay as they
    are in the DTO, as that change's back-compat rule requires.
  - **v0.21** `config-schema-versioning` covers config, and `session-state-locking` takes the
    lock around the session DTO write this change introduces.
  - **v0.22** `session-record-capture` persists `broker::wire::LogEntry` as each `events.jsonl`
    line. That change owns versioning the line or the record; its `manifest.json` already carries
    `version: 1`. Its design defers `broker.log`'s format to this change, and the answer is
    "non-contract". Whether to retire `broker.log` is left for v1.0.0.
