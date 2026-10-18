## Context

The broker is a loopback HTTP server. `POST /publish` validates the envelope (`agent_id` regex,
placeholder guard) and routes by variant; it has no caller identity. `agent.verified` carries
`payload.verified_by`, `agent.feedback` carries `payload.from`; delivery treats both as data.
Client-side identity exists (the MCP publish tools derive `agent_id` from the worktree branch and omit
the two verdict verbs; `broker.sh` derives its id the same way), but any worker with `curl` can post a
verdict for its own branch.

Facts that shape the rule (verified at the tip):

- The worktree post-commit hook written by `agents::install_git_hooks` publishes
  `agent.feedback` with `from: "branch-guard"` from inside a worktree. A rule that gated every
  `agent.feedback` would 403 it.
- The drive loop's correction loop counts only feedback whose `from` is `supervisor`
  (`drive::gate_failure`); the branch-refresh notification (`GitBranchRefresher::notify`) is
  `agent.feedback` from `supervisor`, published by the root-side drive loop.
- Three `sweep.sh` verbs publish verdict messages from the root: `verified` (`agent.verified`),
  `feedback-gate` and `conflict-ack` (both `agent.feedback` from `supervisor`, `sweep.sh:826` for the
  latter, which exits 1 when the broker rejects it).
- A fresh launch saves the session receipt only after the panes exist (`start.rs:377/:790`,
  `supervisor.rs:848`); released *A failed start preserves persisted session state* requires that
  order, and `broker-host-separation` starts the host before any pane.
- The conflict detector and opsx role guard publish feedback in-process (`publish_message`), never
  over HTTP.
- `BrokerMessage` is a frozen wire surface (`code-architecture`); the user-data `Session` lives under
  the platform data directory — outside the repository, so outside every worktree, and inside what the
  sandbox's deny-home tier hides from workers.

## Goals / Non-Goals

**Goals:**
- Enforce in Rust at `/publish` that only the root session publishes verdicts.
- Keep the envelope, every non-verdict message, the branch-guard hook, and every worker helper unchanged.
- Give every root-side publisher the credential without secrets on argv or in the environment.
- State the threat model honestly.

**Non-Goals:**
- Gating `agent.answer`, `agent.advanced-main`, `agent.learning` (not merge-authorising).
- Authenticating workers to each other, or encrypting loopback.
- The MCP verdict tools (`mcp-orchestrator-write-tools` consumes this credential).

## Decisions

### D1 — A per-session credential, because HTTP has no caller identity

Loopback HTTP cannot tell a root curl from a worktree curl; the broker must be given something only
the root holds. A random credential per session is the smallest such thing. *Rejected:* trusting
`from == "supervisor"` (the status quo a mistaken worker already exploits); a second Unix-socket
listener (correct, heavier: UDS serving, `curl --unix-socket`, two URLs); peer-PID inspection (not
portable for TCP loopback).

### D2 — Verdict = `agent.verified`, or `agent.feedback` claiming the root identity

The gate covers exactly the messages that carry the root's authority: every `agent.verified`, and
`agent.feedback` whose `payload.from` is `supervisor`. Other feedback (branch-guard, a person,
peer-to-peer notes) stays ungated and stays inert for the correction loop, as today. This keeps the
worktree hook working and matches the consumer's own notion of authority. *Rejected:* gate all
feedback — breaks the branch-guard hook and makes ordinary notes need a secret.

### D3 — Header, not envelope

`X-Paw-Authority: <hex>` keeps `BrokerMessage` byte-identical, keeps the log free of the secret, and lets
the check run before routing. Missing and wrong values both yield `403 {"error":"authority required"}`;
the comparison is constant-time.

### D4 — Credential lives in the user-data `Session`, `0600`, and nowhere else

`broker_authority: Option<String>` is additive (`serde(default, skip_serializing_if)`); `save_session`
writes owner-only. The discovery JSON, `/status`, `/log`, `status --json`, MCP `get_session_status` and
the broker log never carry it.

### D5 — Mint in the launcher, hand it over on stdin, persist with the receipt

`cmd_start` / `launch_spec_session` / `cmd_supervisor` mint 32 bytes from `/dev/urandom` (a byte
reader added to `src/random.rs` beside `generate_uuid_v4`) as 64 hex chars and write them to the
`__broker` host's stdin when they spawn it (design call 3, shared with `broker-host-separation`, which
hands the session name and log path on argv). The launcher keeps the value and persists it in the
`Session` it saves after the panes exist — the receipt's existing position, so a failed start still
writes nothing. *Rejected:* persisting before the host starts — it contradicts the failed-start rule,
and a host back-fill racing the launcher's later save would leave the host enforcing C2 while the
receipt (read by `sweep.sh` via `__authority`, the MCP tools and the drive loop) holds C1, so every
verdict would 403.

Recovery and restart-from-pause start the host for a saved `Session`: they hand it the saved
credential. A `Session` saved before this change has none; then (and only then) the host mints one and
persists it before serving, and `restart_from_pause` — which saves the `Session` again after the host
is up — re-reads the credential rather than writing its pre-host copy back. Recovery saves its receipt
before the host starts (`recover.rs:164`), so its back-fill is not overwritten.

### D6 — Root-side clients: in-process read, or `git paw __authority`

- **Drive loop** — reads the `Session`; attaches the header to the branch-refresh feedback.
- **`sweep.sh verified` / `feedback-gate` / `conflict-ack`** — call hidden `git paw __authority`,
  which resolves the repository's session and prints the credential; path resolution stays in Rust (no
  mirrored `dirs` logic in shell). It refuses from a linked worktree — defence in depth, not the
  boundary — using the shipped `cli_setup::shell::resolve_layout` + `RepoLayout::is_linked_worktree`
  (git dir ≠ common git dir, which also handles `--separate-git-dir`), not a "`.git` is a file" test.
- **Root MCP tools** — `mcp-orchestrator-write-tools` reads the same `Session`.

### D7 — Threat model, stated plainly

A structural guard against the likely failures: a worker following a stale skill example, an LLM that
decides its own work is verified, a sandboxed worker probing. Not a defence against an unsandboxed
rogue worker that reads the operator's home directory — the sandbox's responsibility. Not a gate on
`agent.status` either: in v0.18 a status claiming `supervisor`, or one agent publishing as another
(seen in the peer dogfood: `broker.sh --agent <worker> status done`), is still accepted
(spec-decisions §D, open item). The posture chapter says so.

## Risks / Trade-offs

- [Stale skill/doc examples show raw verdict curls and now 403] → the two `skill-agent` example
  requirements move to the helper verbs in this change; the coordination chapter documents the 403.
- [Session file permissions on shared machines] → `0600` on save; back-filled files are re-moded.
- [A test broker without a `Session`] → `BrokerState` takes the credential as `Option`; `None` fails
  closed (no verdict accepted). Tests inject a known credential.
- [A worker publishes `agent.status` as `supervisor` with a terminal status] → `detect_completion`
  would wind the drive loop down early; not merge-authorising (the exit summary is ref-derived).
  Accepted and left ungated in v0.18 (spec-decisions §D); documented in the posture chapter.
- [Credential split between host and receipt] → one value: minted by the launcher, handed over on
  stdin, persisted with the receipt; the host mints only for an older session (D5).

## Migration Plan

1. `Session.broker_authority` + `0600` save + round-trip tests; random byte reader.
2. Launcher minting, stdin hand-off, persist with the receipt; host stdin / load / back-fill;
   `restart_from_pause` keeps a back-filled value.
3. `BrokerState` credential, `/publish` gate, 403, constant-time compare.
4. Drive-loop header on its verdict publishes; client optional header.
5. `__authority` (reusing `RepoLayout::is_linked_worktree`); `sweep.sh` `verified`, `feedback-gate`,
   `conflict-ack`.
6. Skill examples; docs; posture chapter.

Rollback: revert; older binaries ignore the additive field.

## Open Questions

- Should `agent.status` published as `supervisor` (which can end the drive loop's wave via its
  verdict-status completion rule), or one agent publishing as another, require the credential? Not in
  v0.18: it stays ungated and the posture chapter says so (spec-decisions §D, still open); revisit with
  the drive-loop functional core, where completion becomes a pure decision over observed inputs.
- Should `agent.answer` be gated? Recommendation: not now.
