## 1. Port and pure core

- [ ] 1.1 Rebase over `terminal-port-surface-capabilities`, the archived `terminal-backend-seam`, `broker-host-separation`, `drive-loop-functional-core` and `orchestrator-escalation-handoff`, and `headless-backend-adapter` and `headless-drive-adaptation`
- [ ] 1.2 `src/remote/mod.rs`: `RemoteAgentService` trait; closed `RemoteState`, `RemoteEvent` (agent message, plan, progress, patch, completion, failure, other) and `RemoteError` (limit, auth, not found, bad response, network, other); no vendor names; doc comments on every item
- [ ] 1.3 Recording fake `RemoteAgentService` (scripted reads, events, failures; records creations, messages and every other call)
- [ ] 1.4 Pure follow decision (`decide_follow(&FollowState, &FollowObservation, &FollowPolicy, now) -> (FollowState, Vec<FollowEffect>)`): question and plan escalation keys, answer binding, event de-duplication and ordering by creation time, list-events-on-change, time cap from the latest of creation / re-attach / last message, failure counting, auth stop
- [ ] 1.5 Table tests for every scenario of *Questions and plan approvals escalate*, *Answers are sent … once per distinct question* and *The follow is bounded*; the "tested with data alone" scenario
- [ ] 1.6 Gate: `just check` green

## 2. Remote backend

- [ ] 2.1 `SurfaceSpec.remote: Option<RemoteTaskPlan>`; tmux and headless ignore it (argv fixtures unchanged)
- [ ] 2.2 `src/terminal/remote.rs`: `spawn_surface` (create once and record name and URL before returning, or re-attach), `list_surfaces` (owner = worktree, no position), non-interactive, activity unknown, draft / `Enter` / clear-line, `Unsupported` for every other key, prefill, relaunch, dialog and service operation, `foreground_process` → none
- [ ] 2.3 Rendering: sort by creation time, de-duplicate by identifier, remove control characters; `capture_tail` (failed read → none) and `stream_to` (append once, owner-only create, refuse a symlink, failure logged)
- [ ] 2.4 `kill` ends the follow and removes its state, and `close_session` ends the follow and keeps its state for recovery (the landing record, including a landing still verifying, must survive `stop`), both with no vendor call; `remove` refuses, unless `--force`, a remote worker whose last-known vendor state is neither completed nor failed, naming the worker, the URL and `--force`, and changes nothing (review I-1); `attach` prints label, state and URL; `detach_clients` / `layout` / `reflow_rows` no-ops
- [ ] 2.5 Run state under `<session_state_dir>/<session>.remote/<slug>/` (`0700` / `0600`, `SessionName` / `BranchSlug` paths); record a message as sent before sending, and a title before creating
- [ ] 2.6 `WorktreeEntry.remote: Option<RemoteSessionRecord>` with `serde(default, skip_serializing_if)`; byte-identical fixtures for tmux sessions
- [ ] 2.7 Tests: every scenario of *A remote surface is a vendor session …*, *Text and Enter …*, *Capture and streaming …*, *Ending a remote surface …*, *Remote follow state …* and *The session file records …* against the fake

## 3. Broker host follow

- [ ] 3.1 The `__broker` host runs the follow on its own thread with its own cadence (≤ 60 s per session) for every remote worker; picks up a worker added by `add`; stops with the host
- [ ] 3.2 Effects applied in order through the broker (publish `agent.question` on the worker's behalf with verdict labels `remote-time-cap`, `remote-unreachable`, `remote-auth`, `remote-failed`; publish `agent.status` on a vendor state change only) and the port (send message); each logged with its outcome
- [ ] 3.3 Exhaustive `match` over `BrokerMessage` for what may reach a vendor session (`Answer`; `Feedback` deferred to `remote-work-landing`); add the site to AGENTS.md §7
- [ ] 3.4 `[remote] follow_hours` (positive integer, default 4, per-key merge, serialized only when set)
- [ ] 3.5 Tests: a state change publishes one status; awaiting feedback reads as waiting; a failed session is escalated with its reason; an attended session escalates without the drive loop; a worker added later is followed; pause stops reads with no vendor call; config parse / zero refused / repository wins / round-trip

## 4. Assets, guards and docs

- [ ] 4.1 `assets/remote-task-addendum.md`, `assets/remote-verify-continuation.md`, `assets/remote-question.md`, loaded with `include_str!`; extend the no-language-leak audit to cover them (no vendor, CLI, stack, build tool, commit convention or helper path)
- [ ] 4.2 Source audits: the follow decision module performs no I/O; vendor requests only in `src/remote/<vendor>` modules (names the offending file); the `RemoteAgentService` identifiers name no vendor
- [ ] 4.3 `docs/src/architecture.md` (module list: remote backend, remote agent service, follow) and `docs/src/architecture-direction.md` (Ports table: remote agent service; Terminal row: third adapter); configuration reference (`[remote] follow_hours`); `mdbook build docs/` succeeds
- [ ] 4.4 Gate 5 review against design.md R1–R6 (`security-and-safety-review`) and `export-agnosticism` for the three assets
- [ ] 4.5 Every scenario maps to a named test; `just check` and `just deny` green; full suite green against the merge-base with `cargo test --no-fail-fast`; no `unwrap`/`expect` in non-test code; exit codes verified with `cmd; echo $?`, not through a pipe
