## Why

This release adds workers whose surface is a vendor's cloud session instead of a local pane. The
session runs on the vendor's machines from the branch's pushed base; git-paw follows it, relays its
questions to the orchestrator, sends the orchestrator's answers back, and later lands its patch on
the agent's local branch. `terminal-port-surface-capabilities` made the terminal port describe each
surface by what it can do. This change adds the adapter that answers the port for a remote surface,
plus a vendor-neutral port to the vendor and the follow that runs in the broker host. It ships no
vendor (the Jules adapter is `jules-remote-vendor`) and selects nothing, so it changes nothing a
user can observe.

## What Changes

- **`RemoteBackend`**, a third terminal adapter, behind the same port as tmux and headless:
  - `spawn_surface` with a remote task plan creates exactly one vendor session from the plan
    (repository, base branch, title, prompt), or re-attaches to a recorded one and creates none;
  - `send_text` drafts, `Enter` sends the draft as one message (the vendor's send-message call),
    the clear-line key discards it; every other key, prefill, relaunch, dialog answer and service
    operation is a typed `Unsupported`;
  - `capture_tail` and `stream_to` render the session's events as plain text, ordered by creation
    time, never by list position, with control characters removed;
  - `list_surfaces` reports the local worktree the patch lands in as the owner, and no layout
    position; every remote surface is non-interactive with activity unknown;
  - `kill` / `close_session` end git-paw's follow and never cancel (the vendor has no cancel);
    `remove`, `stop` and `purge` print the session URL, and `remove` needs `--force` while the
    session may still be running; `attach` prints label, state and URL.
- **`RemoteAgentService`**, a port in git-paw's terms: confirm a repository is visible (spends no
  work), create a session, read its state, list its events, send a message, find sessions by
  title. Closed state and event sets; typed failures (limit, auth, not found, bad response,
  network, other). A recording fake serves every test.
- **The follow**, on a thread of the broker host, in attended and unattended sessions:
  - a question (awaiting feedback) or a plan (awaiting plan approval) is escalated once as an
    `agent.question` on the worker's behalf, with the session URL; git-paw approves nothing;
  - an `agent.answer` for the worker is sent to the session once per distinct question;
  - the worker's `agent.status` is published on its behalf when the vendor state changes, so the
    roster and dashboard show remote workers;
  - reads at most once a minute, events only when the session changed;
  - the follow stops, with an escalation, at a time cap (`[remote] follow_hours`, default 4), on the
    third consecutive read failure, on an authentication failure, or when the session fails.
- **State**: the draft and follow state live owner-only under the user data directory, and each
  vendor-changing action is recorded before it is made, so a restart never sends a message twice.
  The session file's worktree entry records the vendor session (name, URL, title, base commit).
- **Port addition**: an optional remote task plan on `SurfaceSpec`; tmux and headless ignore it.
- **Bundled texts**: the remote task addendum, the verify-continuation framing and the question
  framing are agnostic assets.

## Capabilities

### New Capabilities

- `session-remote-backend`: the remote terminal backend — surfaces as followed vendor sessions,
  message delivery, event rendering, ending a follow without cancelling, the remote agent service
  port, question and plan escalation, answer relay, the follow's bounds, its persisted state and
  the session-file record.

### Modified Capabilities

- `code-architecture`: surface specs may carry a remote task plan; remote follow decisions are
  pure; vendor network access is confined to the vendor adapters (all added).
- `core-lang-agnostic`: the remote agent-facing texts are bundled agnostic assets (added).
- `core-configuration`: `[remote] follow_hours` (added).

## Impact

- **MODIFIED requirement headers:** none.
- **Builds on:**
  - `terminal-port-surface-capabilities` (this release): per-surface interactivity, optional
    positions, interactive-only operations.
  - v0.18.0 (locked): `terminal-backend-seam` (the port, `SurfaceSpec`, the fake);
    `broker-host-separation` (the `__broker` host, which gains the follow thread);
    `drive-loop-functional-core` (observe → decide → apply and the effect log, mirrored by the
    follow); `orchestrator-escalation-handoff` (escalations reach the orchestrator; a worker's
    `agent.question` becomes a judgment call).
  - v1.3.0: `headless-backend-adapter` (`activity`, `PawError::Unsupported`, the owner-only run
    state precedent); `headless-drive-adaptation` (the broker host's own-thread precedent).
  - v0.21.0: `wire-format-dtos` (the session file's new optional field is a wire DTO field);
    `config-schema-versioning` (an optional key with a default does not bump the version).
  - v0.22.0 `session-record-capture`: `stream_to` targets the record's capture file.
- **Successors:** `mixed-backend-sessions`, `jules-remote-vendor`, `remote-work-landing`.
- **Code:** new `src/remote/` (port, follow decision, fake), new `src/terminal/remote.rs`; the
  `__broker` host gains the follow thread; `SurfaceSpec` gains `remote: Option<RemoteTaskPlan>`;
  `WorktreeEntry` gains `remote: Option<RemoteSessionRecord>`; `src/config/` gains `[remote]`; three
  assets under `assets/`.
- **Enum-variant ripple:** `BrokerMessage` is not changed. The follow adds one exhaustive `match`
  over it (which messages may reach a vendor session: only `Answer`, and `Feedback` under the
  landing rule); add the site to AGENTS.md §7's `BrokerMessage` list. `SpecBackendKind`
  untouched.
- **Persisted state:** additive only (`WorktreeEntry.remote`, run state under the user data
  directory).
- **Security / safety:** the follow publishes on a worker's behalf and sends orchestrator text to
  a vendor; nothing is approved, cancelled or pushed. Vendor text is untrusted input and is
  sanitised and framed as quoted. Threats in design.md.
- **Dependencies:** none in this change (the vendor transport is decided in `jules-remote-vendor`).
- **Version notes:** "v1.6.0" is a working number for this release and appears only in prose.
