## Context

The contract edges and where each one stands today:

| Edge | Type today | Readers outside the process | Version |
|---|---|---|---|
| Global session file `<data_dir>/git-paw/sessions/<name>.json` | `session::Session` / `WorktreeEntry` (serde) | MCP (`RepoContext`), `selftest`, other git-paw processes | none |
| Per-repo discovery file `.git-paw/sessions/<name>.json` | `RepoSessionFile` (already a DTO) | `sweep.sh` | none |
| Broker messages (`POST /publish`, the `/log` and `/messages` payloads) | `broker::messages::BrokerMessage` (serde, `tag = "type"`) | `broker.sh`, `sweep.sh`, skills' curl examples, MCP publish | none |
| Broker HTTP responses | server-private `PollResponse` / `LogResponse` / `LogEntry`, `/status` built with `json!`, `AgentStatusEntry` | clients that re-declare them: `BrokerLogEntry`, `StatusAgent`, `AgentStatusRow`, `probe_broker` substring | the `/status` `version` is the package version, not a schema |
| `broker.log` (**not a contract**, see D5) | `delivery::flush_entries`: `format!("[{seq}] {ts} [{agent}] {msg}")` using `Display`; one machine-wide file, deleted by any purge | nothing parses it | none (stays none) |
| Manual approvals `.git-paw/sessions/<s>.manual-approvals.jsonl` | `ManualApproval` (serde) | `git paw approvals` (`aggregate`) | none |
| `config.toml` | `PawConfig` | scripts (`tomllib`) | covered by `config-schema-versioning` |

`code-architecture` freezes these serde forms:
- `FileIntent` `untagged`;
- `AdvancedMain` `flatten`;
- `StatusPayload.message` with no `skip_serializing_if`;
- `Session.created_at`'s custom serializer;
- the `BrokerMessage` variant set.

These are bytes to preserve, not code to leave untouched. This change specifies moving them
verbatim into the wire types. It is not done as a refactor wave.

## Goals / Non-Goals

**Goals:** the model types can change without changing a byte on disk or on the wire. Each
contract says which version it is. Newer files are never misread. The HTTP shapes have one
definition.

**Non-Goals:**
- DTOs for structs that never leave the process.
- MCP response types, which already have DTOs.
- Changing any message's bytes or any script.
- The broker structural tidy deferred past the freeze. No lock, `.await` or spawn ordering
  changes.
- Fixing the helper/broker divergences found while doing this (see Risks).
- Versioning, reformatting, relocating or retiring `broker.log` (D5).
- Versioning v0.22.0's `events.jsonl` line, which that change owns.

## Decisions

### D1 — Model / wire split, mechanical and seam-first
Each edge gets a `wire` module whose types carry today's serde attributes verbatim, and total
`From` mappers in both directions. Model types lose `Serialize` / `Deserialize`.

The work is staged:
1. Wire types are type aliases of the model types, and every encode or decode call site routes
   through `wire::encode` / `wire::decode`.
2. The wire types become separate structs.
3. The derives are removed from the model.

Golden fixtures captured before step 1 must stay byte-identical through step 3. For
`BrokerMessage`, the model keeps its name, variants and payload names, so the exhaustive matches
in `messages.rs`, `dashboard/broker_log.rs` and `broker/learnings.rs` compile unchanged.

### D2 — Version placement per edge
- **Session file and discovery file:** a top-level `schema_version`. It is written as `1` and
  absent means `1`. It is serialized first, so it sits at the top of the pretty JSON.
  `sweep.sh` ignores unknown keys, as the existing *Adding a field is backwards-compatible*
  scenario requires.
- **Manual-approvals line:** `schema_version` on each line written. Lines without it are
  version 1.
- **Broker:** the protocol carries the version, not each message. `/status`, `/log` and
  `/messages/:agent_id` response objects gain `schema_version: 1`; the existing `version` key
  stays the package version. The message envelope is unchanged, so helpers and skills need no
  edit.
  *Alternative:* a `schema_version` key in every message. It changes every frozen fixture,
  every helper and every skill example, and gains nothing a version on the HTTP responses does
  not already give. Rejected (open question 1).

### D3 — Newer-schema handling
- **Session lookup:** `find_session_for_repo_in` decodes each file as a generic JSON value
  first. If the file's `repo_path` matches the repository and its `schema_version` is above
  the supported version, the lookup returns `PawError::SessionError`, naming the file and both
  versions and saying to upgrade git-paw. It does not skip the file. A newer file that belongs
  to another repository is skipped, as today. A file that is not valid JSON at all is still
  skipped, as today.
- **Session save:** `save_session_in` refuses to replace a file that declares a newer version.
- **Approvals:** `aggregate` skips newer lines and reports their count once on stderr.
- **Broker clients:** they ignore `schema_version` until a v2 exists. The value is there so a
  future client can refuse a newer broker.

### D4 — `broker::wire` HTTP types
- `StatusResponse {git_paw, version, schema_version, uptime_seconds, repo_root?, agents}`
- `AgentStatus {agent_id, cli, status, last_seen_seconds, phase?, modified_files}`
- `LogResponse {schema_version, entries, last_seq}`
- `LogEntry {seq, timestamp_unix_secs, message}`
- `PollResponse {schema_version, messages, last_seq}`
- `ErrorBody {error, value?, field?, detail?}`

Server-side serialization is unchanged apart from `schema_version`. On decode, the fields older
clients made optional (`status`, `last_seen_seconds`, `cli`) take serde defaults, which is the
most tolerant of the three existing copies. Against a real broker this changes nothing
observable, because the server always emits those fields. `fetch_log_entries_over_http` decodes
`entries` as raw values and maps each one, keeping today's skip-on-undecodable behavior.
`probe_existing_broker` decodes `StatusResponse` and checks `git_paw == true`; anything that
does not decode is a foreign server, as today.

### D5 — `broker.log` is declared non-contract, not versioned
MILESTONE lists `broker.log` among the five edges. On inspection it is not worth freezing:
- it is one machine-wide file for every repository;
- it is written in `Display`'s lossy one-line form;
- any repository's purge deletes it;
- nothing reads it back.

v0.22.0 (`session-record-capture`) adds a per-run `events.jsonl` whose lines are
`broker::wire::LogEntry`, the `GET /log` entry shape. That file is the machine-readable broker
history, and that change recommends v1.0.0 decide whether to retire `broker.log`.

So this change names `broker.log` in the architecture contract as a diagnostic log whose format
may change, and the broker reference sends machine consumers to `GET /log`. The broker-protocol
requirement *Periodic log flush to disk* (Display-formatted lines) is not modified. It stays a
description of today's diagnostic output, with no version header and no separate formatter.
Versioning the persisted event line belongs to v0.22.0. Because the line type is the shared
`LogEntry`, it is defined in one place either way.

*Alternative:* add a `# git-paw broker log v1` header and a formatter decoupled from `Display`.
That would freeze a format the next release is about to supersede. Rejected (open question 2).

### D6 — Parity test runs the real scripts
`tests/wire_script_parity.rs` reuses the stub-recorder pattern of `tests/sweep_sh_status_publish.rs`:
1. Point `GIT_PAW_BROKER_URL`, or a config `[broker] port`, at a local recording listener.
2. Run each publishing verb: `broker.sh` `status`, `artifact`, `blocked`, `question`, and
   `intent` with a TTL; `sweep.sh` `feedback-gate`, `verified`, `status-publish` and `learn`.
3. Decode each body with `broker::wire::decode`, then `validate()`.

This also closes the gap where no test feeds script JSON through the Rust decoder.

## Risks / Trade-offs

- [A mapper silently drops a field] → golden byte fixtures for every variant and every edge,
  captured before wave 1.
- [The `flatten` and `untagged` surfaces behave differently on a new struct] → the attributes
  are copied verbatim, and the existing `advanced_main_*` / `file_intent_*` round-trip tests
  move onto the wire types unchanged.
- [`schema_version` in pretty-printed session JSON reorders keys] → it is declared first and
  the other fields keep their order; the golden test covers this.
- **Found while scoping, not fixed here, and reported:**
  - `broker.sh intent` without a TTL omits `valid_for_seconds`, which `IntentPayload` requires,
    so the broker rejects it with a 400. The parity test therefore covers `intent` with a TTL.
  - `sweep.sh learn` computes its id with `sha1(category|title|body)[:16]`, which is not the
    aggregator's id scheme.
  - `sweep.sh`'s phantom filter still assumes `feat[-/]` branch names.
  - `sweep.sh resolve_agent_for_path` reads `worktrees[].branch` from the discovery file, but
    that file has `agents[].branch_id`.

## Migration Plan

Waves 1-3 of D1 run per edge, in this order: approvals (smallest), session, broker messages,
broker HTTP types. Each wave is gated on golden fixtures plus `just check`. Version
fields land last, in one commit per edge. Rollback is a revert. Files written with
`schema_version` still load on an older binary, because the unknown key is ignored.

## Open Questions

1. The broker version lives on the HTTP responses (recommended), not on each message.
2. Declare `broker.log` non-contract and leave it unversioned (recommended, following
   v0.22.0's `events.jsonl` plan), rather than adding a version header as MILESTONE's original
   list implies. Whether v1.0.0 retires `broker.log` is that release's decision.
3. Key name `schema_version` (recommended; matches `uniform-json-output`) or `version` (matches
   the config key).
