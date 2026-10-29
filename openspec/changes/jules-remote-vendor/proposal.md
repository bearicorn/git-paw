## Why

`remote-surface-adapter` gives git-paw a vendor-neutral remote surface and follow, but no vendor.
Jules is the first: its REST API is documented for automating tasks, it counts whole tasks rather
than requests (15, 100 or 300 a day on the free, Pro and Ultra plans), and it runs on Google's
machines from a branch on GitHub. Fleet's design and reviewed plan worked out how to drive it
safely; Fleet's own Jules route (llm-toolbox `fleet/`) is the proof of concept. The route is built
and its live run is pending; that run will answer what the Jules docs leave open. This change is git-paw's adapter,
carrying over Fleet's API facts, key handling, limit handling and review fixes, and leaving out
what is Fleet's (catalog, route choice, pull-request marks).

**Terms first.** Nothing calls Jules before its terms (<https://jules.google.com/legal>, behind a
sign-in) are read and recorded. That is this change's first, blocking task; the adapter ships only
after it. It was done on 2026-10-28: the link holds only the Jules Privacy Notice, and nothing found
forbids automated use (design.md). The rule stays for later terms changes and later vendors.

## What Changes

- **A hardened client** behind `RemoteAgentService`:
  - HTTPS only, to `jules.googleapis.com/v1alpha`, pinned in code;
  - redirects refused;
  - 20 s timeouts, a 16 MB response cap;
  - session names validated before use;
  - paging bounded.
- **The key** comes from `JULES_API_KEY` only and travels only in the `X-Goog-Api-Key` header. It is
  never in arguments, logs, errors, escalations, broker messages, session files, follow state, the
  ledger or captures (masked as `***` wherever vendor text is recorded). A key in the configuration
  is refused, and so is a key holding a line break or other control character, before any request.
  git commands that check, apply or commit remote work run without it.
- **Sessions** are created from the pushed base branch with `requirePlanApproval` false and without
  `AUTO_CREATE_PR`. Plan approval is never called. The access check (`GET /sources/...`) starts no
  session.
- **Mapping:**
  - Jules states map onto the remote domain; anything unrecognised is unknown;
  - activities are ordered by `createTime`, never by list position;
  - patches are read from both the activities and the session's `outputs`, because where a session
    without `AUTO_CREATE_PR` returns its patch is still open.
- **Limit failures** are classified in one table (429, `RESOURCE_EXHAUSTED`, quota/limit wording).
  A limit failure starts a cool-down (`limit_cooldown_minutes`, default 60) that blocks new remote
  launches. Neither the limits nor the cool-down ever block re-attaching to, adopting or messaging
  an existing session.
- **Budgets:**
  - an owner-only ledger under the user data directory counts the Jules sessions git-paw created
    in the rolling 24 hours, from any repository;
  - a launch that would exceed the plan's daily or concurrent limit is refused before anything is
    created;
  - the plan is `[remote.jules] plan` (free / pro / ultra, default free), with overrides.
- **Usage**: each Jules session counts as one remote task (`remote.tasks`,
  `usage_total.remote_tasks`). Token figures stay unknown, and budget caps ignore remote tasks.
- **Configuration**: `[remote] vendor = "jules"` and `[remote.jules]`.

## Capabilities

### New Capabilities

- `remote-worker-jules`: the Jules adapter — pinned host and transport bounds, key handling,
  session creation without pull requests or plan approval, state and activity mapping (patches from
  activities or outputs), limit classification, the daily and concurrency ledger, the cool-down,
  and remote-task usage.

### Modified Capabilities

- `core-configuration`: `[remote] vendor` and `[remote.jules]` (added).

## Impact

- **MODIFIED requirement headers:** none.
- **Builds on:**
  - this release: `remote-surface-adapter` (the `RemoteAgentService` port, the follow, the run
    state, `[remote] follow_hours`); `mixed-backend-sessions` (status JSON `remote` object).
  - v0.19.0 `worker-usage-budget`: `usage` / `usage_total` in status JSON, the budget caps (which do
    not count remote tasks).
  - v0.21.0 `config-schema-versioning` (optional keys do not bump the version);
    `supervisor-vocabulary-split` (the configuration layout this table joins).
- **Successor:** `remote-work-landing`.
- **Later work on the same seam (nothing specced here):** Codex cloud (`codex cloud exec`),
  Copilot's coding agent (`gh agent-task create`) and Claude Code cloud sessions (`claude --cloud`),
  each a further `RemoteAgentService` adapter once its vendor's terms are read.
- **Fleet carry-over:** the Jules API facts and calls, host pinning, the no-redirect rule, key
  masking, session-name validation, ordering by `createTime`, limit classification, the cool-down,
  counting sessions being created against the daily limit (review M10), and the case-insensitive
  source fallback (M18); from the final review of the proof of concept, reuse exempt from limits
  and cool-down (I-1) and the control-character key refusal (I-5). Not carried over: the catalog, per-plan access levels, the free-route
  choice, route placement, and pull-request marks.
- **Code:** new `src/remote/jules.rs` (client, mapping, classification) and `src/remote/ledger.rs`;
  `src/config/` (`[remote] vendor`, `[remote.jules]`); status JSON `usage_total.remote_tasks`.
- **Dependencies:** none. HTTPS goes through the system `curl` via the `CommandRunner` seam, with
  the key passed on stdin (decided 2026-10-28, design D1); `ureq` + `rustls` was rejected.
- **Enum-variant ripple:** none.
- **Security / safety:** credential handling, an external network endpoint, untrusted responses.
  Threats in design.md.
- **Version notes:** "v1.6.0" is a working number for this release and appears only in prose.
