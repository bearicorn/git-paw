## Why

`agent.verified` authorises a branch to merge, and `agent.feedback` from the root identity
(`payload.from == "supervisor"`) is a gate verdict: the drive loop's correction loop acts only on
feedback from that identity (`supervisor::drive::gate_failure`). Today the broker accepts both from
**any** HTTP client: `verified_by` / `from` are caller-supplied strings and `POST /publish` has no
notion of who is calling. The identity primitive the roadmap points at — `agent_id` derives from the
worktree branch, and a root-driven orchestrator has no worktree branch — exists only on the
**client** side (the MCP publish tools and `broker.sh` derive their own id). With the orchestrator
relocated outside the session, verdicts arrive over the same loopback HTTP as every worker's status,
so "who may publish a verdict" must be enforced by the broker, in Rust. Authority cannot be a skill:
a worker can ignore an instruction.

Archive position 16 of 20 for v0.18 (after `broker-host-separation` and `cmd-library-surface`).

## What Changes

- **Per-session authority credential.** The launcher mints a random credential (OS randomness via
  `src/random.rs`) when it starts a broker-enabled session, hands it to the `__broker` host on the
  host's stdin (never argv or env), and persists the same credential with the session receipt, which it
  still saves only after the panes exist — so released *A failed start preserves persisted session
  state* stays untouched. It lives only in the user-data `Session` file, now written owner-only
  (`0600`) — never in the per-repo discovery JSON that worktrees read, never in `/status`, `/log`,
  `git paw status --json`, MCP `get_session_status`, or the broker log.
- **Broker enforces the rule at `/publish`: the root session may publish verdicts; worktree sessions
  may not.** `agent.verified` and any `agent.feedback` whose `payload.from` is the root identity
  `supervisor` require an `X-Paw-Authority` header equal to the session credential; otherwise
  `403 {"error":"authority required"}` and no state changes. Feedback from any other sender — the
  worktree branch-guard post-commit hook (`from: "branch-guard"`), a person — carries no authority, is
  accepted as before, and the correction loop keeps ignoring it. The envelope is untouched (header,
  not payload). In-process publishers (conflict detector, role guard) never cross HTTP and are not
  gated.
- **Root-side clients carry it.** The broker host takes it from stdin (or from the saved `Session`
  when recovery or restart-from-pause starts it); the drive loop attaches it to its own verdict-bearing
  publishes (the branch-refresh notification is `agent.feedback` from `supervisor`); `sweep.sh
  verified` / `feedback-gate` / `conflict-ack` (all three publish `agent.feedback` or `agent.verified`
  from `supervisor`) fetch it through a hidden `git paw __authority` that refuses to run from a linked
  worktree (the shipped `cli_setup::shell::RepoLayout::is_linked_worktree` check). The root-session MCP
  tools (`mcp-orchestrator-write-tools`) read the same `Session`.
- **Skill examples stop showing raw verdict curls** — a raw curl would now be refused; the two
  `skill-agent` example requirements name the helper verbs (v0.17's `supervisor.md` already uses them;
  this change pins it in the spec and tests).
- **Not gated, stated plainly.** `agent.status` stays ungated in v0.18 — including one claiming
  `supervisor`, which can wind down the drive loop's wave — and so does one agent publishing a
  non-verdict message as another (spec-decisions §D, open item). The posture chapter says so.
- **Back-compat.** A `Session` saved earlier has no credential; when recovery or restart-from-pause
  starts the host for it with no credential on stdin, the host mints and persists one before serving
  (and the flow's later save keeps it), so a recovered session is enforced from its first request. This
  back-fill is the only case in which the host mints. Worker-side
  helpers, MCP worker tools and every non-authority message are byte-for-byte unchanged.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `broker-protocol`: `/publish` requires the root credential for verdict messages.
- `broker-runtime`: the launcher mints the credential, hands it to the host on stdin and persists it
  with the receipt; the broker host loads (or, for an older session, back-fills) and enforces it; the
  credential is confined to root-side surfaces.
- `session-state`: the user-data `Session` records the credential and is written owner-only.
- `broker-agent-helper`: hidden `git paw __authority`; `sweep.sh verified` / `feedback-gate` /
  `conflict-ack` attach the header; `broker.sh` never carries a credential.
- `skill-agent`: the `agent.verified` / `agent.feedback` examples use the helper verbs.
- `core-security-posture`: the security-posture chapter documents verdict authority and its limits,
  including what stays ungated (`agent.status`).

## Impact

- **Predecessors (archive order):** position 16, after `broker-host-separation` (13: the `__broker`
  host, which takes the session name and log path on argv — this change adds the credential on stdin)
  and `cmd-library-surface` (15).
  v0.17: `worker-session-id-capture` (archived; supplies `src/random.rs`, extended here with a byte
  reader); `bundled-helper-discovery` (`sweep.sh` session discovery the helper verbs rely on);
  `mcp-superpowers-support` touches other `mcp-server` requirements only.
- **Successors:** `mcp-orchestrator-write-tools` (authority publish tools), `orchestrator-skill-export`
  (skill verdict section), `orchestrator-escalation-handoff` (one-shot publishes via the helpers).
- **Code:** `src/broker/server.rs` (`publish`: header check, 403), `src/broker/mod.rs` (`BrokerState`
  credential), `src/session.rs` (`Session.broker_authority`, `0600` save), `src/random.rs` (hex
  token), `src/commands/{start,supervisor}.rs` (mint, stdin hand-off, persist with the receipt;
  `restart_from_pause` keeps a back-filled credential), `src/commands/recover.rs` (hands the saved
  credential to the host), `src/commands/broker_host.rs` (stdin / load / back-fill),
  `src/supervisor/drive.rs` (`GitBranchRefresher::notify` attaches the header),
  `src/broker/client.rs` (optional header), `src/cli.rs` + hidden `__authority` (root detection reuses
  `cli_setup::shell::{resolve_layout, RepoLayout::is_linked_worktree}`),
  `assets/scripts/sweep.sh` (`verified`, `feedback-gate`, `conflict-ack`; the `.git-paw/scripts/` copy is
  gitignored and written by `git paw init`, so there is no tracked copy),
  `assets/agent-skills/supervisor.md` (already uses the helper verbs since v0.17; skill-content tests
  only).
- **Wire:** `BrokerMessage` unchanged; one request header; one 403 shape.
- **Persisted state:** additive `Session` field; discovery JSON unchanged.
- **Tests:** `broker_integration.rs`, `broker_agent_id_validation.rs`, `broker_runtime_hardening.rs`
  (403 paths), `sweep_sh_conventions.rs` + `sweep.sh verified` / `conflict-ack` header tests, a
  branch-guard hook test (still 202), session round-trip and `0600`, the launch tests (stdin hand-off,
  same credential in the receipt, failed start saves nothing, restart keeps a back-fill), skill-content
  tests for the two examples.
- **Docs:** `docs/src/user-guide/security-posture.md`, `user-guide/coordination.md` (403 shape),
  `user-guide/supervisor.md` (verdict verbs via the helper), `architecture.md` (broker section).
- **Enum-variant ripple:** none — the check is a match on two existing variants in `server.rs`.
- **Security / safety:** a structural guard against a mistaken or skill-misled worker and, under the
  sandbox's deny-home tier, a sandboxed rogue worker. Not a defence against an unsandboxed worker that
  reads the operator's home directory — the sandbox's job, stated in the posture chapter. Not a guard on
  `agent.status`: a status claiming `supervisor`, or one agent publishing as another, stays accepted in
  v0.18 (spec-decisions §D). Constant-time compare; never logged; never on argv or in the environment
  (stdin only).
- **Dependencies:** none.

## Rebase note (2026-10-25): audit against released v0.17.0

- Launch ordering (design call 3): "persist the credential before the host starts" contradicted released
  *A failed start preserves persisted session state* (the receipt is saved only after the panes exist,
  `start.rs:377/:790`, `supervisor.rs:848`), and a host back-fill (C2) racing the launcher's later save
  (C1) would have refused every verdict. Now: credential to the host on stdin, the same credential
  persisted with the receipt, host back-fill only for an older session being recovered or restarted
  (and the restart's later save keeps it). Requirement, D5 and task 1.3 rewritten.
- `sweep.sh conflict-ack` publishes `agent.feedback` from `supervisor` (`assets/scripts/sweep.sh:826`) and
  exits 1 on rejection; it now fetches and sends the credential beside `verified` / `feedback-gate`.
- `__authority` reuses the shipped `cli_setup::shell::RepoLayout::is_linked_worktree` (git dir vs
  common dir, handles `--separate-git-dir`) instead of "`.git` is a file".
- Task 4.1 is mostly done in v0.17 (`supervisor.md:162/182` use the helper verbs); the delta and tests
  remain.
- Impact adds `src/commands/recover.rs`; `.git-paw/scripts/sweep.sh` is gitignored, not tracked.
- The posture text now says plainly that `agent.status` (including one from `supervisor`) and one agent
  publishing as another stay ungated in v0.18 (spec-decisions §D).
