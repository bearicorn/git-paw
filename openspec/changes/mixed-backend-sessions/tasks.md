## 1. Worker backends and persisted state (single-backend byte-identical)

- [ ] 1.1 Rebase over `terminal-port-surface-capabilities`, `remote-surface-adapter`, `headless-backend-adapter`, `headless-drive-adaptation` and `headless-backend-selection`, and the archived changes named in the proposal
- [ ] 1.2 Core `WorkerBackend { Tmux, Headless, Remote }`; `WorktreeEntry.backend: Option<WorkerBackend>` written only when it differs from the session's backend
- [ ] 1.3 Discovery wire type: `agents` holds tmux workers only; `headless_agents` whenever a headless worker exists; new `remote_agents` (`branch_id`, `worktree_path`, `vendor`, `url`)
- [ ] 1.4 Tests: session file and discovery file of a tmux-only session byte-identical to pre-change fixtures; a mixed session records only the odd worker; the headless, tmux-only and mixed discovery scenarios

## 2. Routing backend and lifecycle

- [ ] 2.1 The backend factory becomes a routing backend built from the session record: surface operations to the creating adapter, session operations to the session backend, union listing, close in every adapter, `attach` prints remote URLs then attaches; the only `match` on worker backend kinds
- [ ] 2.2 `start` (launch, recovery, restart from pause, revival), `add`, `remove`, `pause`, `stop`, `purge` act per worker through the router; every worker registered with the broker host via `POST /watch`; no `tmux` required for a headless session with no tmux worker
- [ ] 2.3 Tests: a surface operation reaches only its own adapter; listing returns every worker (only the tmux surface positioned); tmux-only argv identical to pre-change fixtures; stop ends every kind of worker; a remote worker is registered with the broker host; recovery keeps each worker's backend

## 3. Selection and gates

- [ ] 3.1 `--worker-backend BRANCH=BACKEND` (repeatable) on `start` and `--worker-backend BACKEND` on `add`, with help text and a long-about example; unknown backend exits 2; unknown or repeated branch exits 1 before anything
- [ ] 3.2 One gate pass before any side effect: combination rule (no tmux worker in a headless session), the headless eligibility gate per headless worker whatever the session backend, the remote gate per remote worker (until `remote-work-landing` is applied, refuse remote workers as not yet available); all refusals printed together; exit 1
- [ ] 3.3 MCP `start_session.worker_backends`: schema lists the three values; same gate; MCP-level error listing refusals; omitted parameter unchanged
- [ ] 3.4 Tests: flag parsing, `zellij` exits 2, unknown branch exits 1 with nothing created, help lists the flag; tmux worker in a headless session refused with nothing created; headless worker in a tmux session gated; refusals of different backends reported together; MCP mixed start, refusal creates nothing, schema

## 4. Drive loop, wake, status, liveness

- [ ] 4.1 Drive-loop observation carries the worker backend: screen predicates and prompt escalation for tmux workers only, turn decisions for headless workers only, no text or key to remote workers; remote gate failures not counted as correction cycles
- [ ] 4.2 Wake decision per worker backend (headless only, any session)
- [ ] 4.3 Status: human lines for non-tmux workers (backend and state; remote state, follow and URL); JSON top-level `backend`, per-entry `backend`, `activity`, `remote`; tmux-only human output byte-identical
- [ ] 4.4 Liveness probe: a live headless worker or a followed remote worker keeps a tmux session alive; agents-exited only when no worker of any backend is alive
- [ ] 4.5 Tests: no drive-loop text reaches a remote worker (feedback still published); a headless worker in a tmux session gets turn decisions; a tmux worker in a mixed session still escalates prompts; the eight wake scenarios; human and JSON status of headless, tmux-only and mixed sessions; the nine liveness scenarios

## 5. Docs and contract

- [ ] 5.1 `docs/src/user-guide/` mixed-session section (in the headless chapter or a new remote-workers chapter shared with `remote-work-landing`); CLI reference (`--worker-backend` on `start` and `add`); MCP reference (`worker_backends`); README feature row; `sweep.sh` sees tmux workers only (documented)
- [ ] 5.2 Additive contract-manifest entries (`cli-contract-freeze`): `--worker-backend` and values, `start_session.worker_backends`, `WorktreeEntry.backend`, discovery `remote_agents`, status JSON entry `backend` / `remote`
- [ ] 5.3 Gate 4 (`doc-completeness`) and Gate 5 (`security-and-safety-review`); `mdbook build docs/` succeeds
- [ ] 5.4 Every scenario maps to a named test; `just check` and `just deny` green; full suite green against the merge-base with `cargo test --no-fail-fast`; no `unwrap`/`expect` in non-test code; exit codes verified with `cmd; echo $?`, not through a pipe
