## Context

**Jules API facts carried over from Fleet's design** (`2026-10-24-fleet-remote-workers-design.md`
§5, from the public reference at <https://jules.google/docs/api/reference/> and
<https://developers.google.com/jules/api/reference/rest>):

- Base URL `https://jules.googleapis.com/v1alpha` (an alpha API). Auth is an `X-Goog-Api-Key`
  header with a key created in the Jules web app's settings, at most 3 keys per account.
- Calls: `GET /sources`; `POST /sessions` (`prompt`, `title`, `sourceContext.source =
  sources/github/{owner}/{repo}`, `sourceContext.githubRepoContext.startingBranch`,
  `requirePlanApproval`, `automationMode`); `GET /sessions/{id}`; `GET /sessions/{id}/activities`;
  `POST /sessions/{id}:sendMessage`; `POST /sessions/{id}:approvePlan`.
- States: `QUEUED`, `PLANNING`, `IN_PROGRESS`, `AWAITING_PLAN_APPROVAL`, `AWAITING_USER_FEEDBACK`,
  `PAUSED`, `FAILED` (with a `sessionFailed.reason` activity), `COMPLETED` (with a
  `sessionCompleted` activity), `STATE_UNSPECIFIED`.
- Output: `outputs[].pullRequest.url` when a pull request was opened; activity artifacts with
  `changeSet.gitPatch = {baseCommitId, unidiffPatch, suggestedCommitMessage}`.
- The Jules GitHub app must be installed on the repository before it is a source. Jules starts from
  a branch on GitHub.
- Limits: 15 / 100 / 300 tasks a day and 3 / 15 / 60 at once on free / Pro / Ultra, over a rolling
  24 hours ("you will not be able to trigger new tasks until the limit resets").
- Known terms facts (MILESTONE, 2026-10-28): the API is documented for automating tasks; users must
  be 18 or older; paid plans are for personal @gmail.com accounts only.

**Terms reading (task 0.1; read 2026-10-28, confirmed by the user).**

- The terms link, <https://jules.google.com/legal>, holds only the Jules Privacy Notice (last
  updated 2025-10-07). There is no separate terms of service.
- From the privacy notice: sessions on public repositories may be used to train Google's models;
  sessions on private repositories are not used for training; trained reviewers may read samples.
- The FAQ (<https://jules.google/docs/faq/>) says Jules "works autonomously — meaning you can
  submit a task, go do something else, and return when it's done", and "You are responsible for
  the code you run".
- Nothing found forbids automated use. Fleet has enabled Jules's free plan on this basis, for
  public and hobby repositories only.

The stop-if-forbidden rule stays: a change to these terms is read again before the next release
that calls Jules, and every later vendor's adapter waits for its own reading.

**The proof of concept** is Fleet's Jules route (llm-toolbox `fleet/`, branch
`worktree-fleet-catalog`, code in `fleet/fleetlib/jules.py`, `remote.py` and `runner.py`; this
change does not depend on its code). The route is built; its live run is pending, waiting on the
user installing the Jules GitHub app and exporting a key, now that Jules's terms are read. Fleet
creates its sessions **with** `AUTO_CREATE_PR`; git-paw creates them without. Its live runs answer
the open questions below.

## Goals / Non-Goals

**Goals:**
- A Jules adapter behind `RemoteAgentService` that can only talk to Jules, can never leak the key,
  never opens a pull request, never approves a plan, and never asserts an unconfirmed Jules
  behaviour as settled.
- git-paw's own Jules use stays within the account's limits, and a limit hit stops new launches.

**Non-Goals:**
- Fleet's catalog, per-plan access levels, free-route choice, route placement and pull-request
  marks (Fleet's job).
- Opening, commenting on or closing pull requests (v1.5.0).
- Other vendors: Codex cloud (`codex cloud exec`), Copilot's coding agent (`gh agent-task create`)
  and Claude Code cloud sessions (`claude --cloud`) follow later as further `RemoteAgentService`
  adapters, each after its own terms are read. Nothing is specced for them here.
- Repositories not hosted on github.com.

## Decisions

### D1 — Transport: system `curl` through the process seam (decided 2026-10-28)

No approved crate is an HTTPS client. `tokio`/`axum` serve; `hyper` is a dev dependency only;
nothing provides TLS.

| Option | New crates | Notes |
|---|---|---|
| **`curl` via `CommandRunner`** (chosen) | none | `--proto =https`, no `-L`, `--max-time 20`, `--max-filesize`, status via `--write-out`; the key is passed in a `--config -` header line on stdin, never in argv; `curl` presence joins doctor's checks. A process per request at a 60 s cadence is cheap. |
| `ureq` + `rustls` | several (TLS stack, roots) | Pure Rust and in-process, but adds a TLS stack to the supply chain (`cargo deny` licences: ring/rustls/webpki-roots) and needs explicit approval under AGENTS.md. |
| `hyper` client + `tokio-rustls` | several | The same TLS stack plus more glue; no benefit over `ureq`. |

The requirements are transport-neutral: pinned host, no redirects, bounded time and size, and the
key in a header and never in a process's arguments. The user chose `curl` via `CommandRunner`
(2026-10-28), so this release adds no crate; `curl` joins doctor's checks.

### D2 — Pinned host, no redirects, validated names

The base URL is a constant. Every request is built from it plus a validated session name
(`^sessions/[A-Za-z0-9_-]+$`) or a percent-encoded owner and repository, so neither vendor data nor
configuration can redirect a request or smuggle a path. A 3xx is a failure, so the key can never
follow a redirect to another host. (Fleet plan, Global Constraints; review M4.)

### D3 — The key

Read from `JULES_API_KEY` in the process that makes the request: the launch (create, access check)
and the broker host (follow). It is never stored and never printed; any vendor text that echoes it
is masked. A key in the configuration is refused, so it cannot end up in a committed file. git
commands that touch remote work run with the variable removed, so a repository hook cannot read it.
A key holding a line break or any other control character is refused before any request, with an
error that names the variable and shows no part of the key. Fleet's final review (I-5) found a key
with a trailing carriage return, from a CRLF `.env` file, printed in a traceback by its check
command. With `curl --config -` such a key would also start a new configuration line.
**Residual:** tmux panes and headless turns inherit the user's environment, as v1.3.0 documents for
headless turns. A local worker can read the key if the shell that started git-paw exports it. The
docs recommend exporting it only where remote workers are launched, and the security posture
chapter names this residual.

### D4 — No `AUTO_CREATE_PR`, no plan approval

The patch comes back over the API and is landed locally (`remote-work-landing`); pull requests
belong to v1.5.0. `requirePlanApproval` is false, and `:approvePlan` is never called, even if a
session asks: git-paw approves nothing (v0.17.0). Whether a session created without `AUTO_CREATE_PR`
still returns a patch is open question 4, below.

### D5 — Mapping that holds whatever the live run shows

- **Order:** by `createTime`, never by list position (open question 3).
- **Patch location:** activities **and** `outputs`, de-duplicated by diff (open question 4).
- **Unknown values:** an unknown state is unknown and the follow keeps going until its cap; an
  unknown activity is "other", rendered and otherwise ignored.
- **Event identity:** `id`, else `name` (Fleet plan, Task 1).

### D6 — Limit classification in one table

429, `error.status == RESOURCE_EXHAUSTED`, and quota/limit wording in `error.message` or a
`sessionFailed` reason (Fleet plan Task 1 `looks_like_limit`; review M19). The shape is open
question 2, so the table is the one place the live run's findings land.

### D7 — The ledger: count our own use, per machine

Each created session is recorded with its name, creation time and last-known state in
`<data_dir>/remote/jules-ledger.json` (owner-only, no credential), shared by every repository on the
machine, because Jules's limits are per account. Sessions about to be created count (review M10).
The daily window rolls over 24 hours. Concurrency counts sessions whose last-known state is not
terminal, including ones git-paw no longer follows (they still hold a Jules slot). Sessions started
by hand in the Jules app are invisible, so the count can be low; the cool-down catches the rest.
Re-attaching to, adopting or messaging a session already in the ledger creates nothing and is
never refused or counted again. Fleet's final review (I-1) found its daily budget blocking a resume
and a verify retry from reusing their own session, which was then abandoned.

### D8 — Plan-agnostic limits

`[remote.jules] plan` selects the published numbers; `daily_tasks` / `concurrent_tasks` override
them, so a changed Jules plan needs no release. Fleet's per-plan access levels (free / promo /
subscription) do not carry over: git-paw uses Jules only when the user names it in `[remote] vendor`
and selects a remote worker.

### D9 — Cool-down

A limit failure on create or read, or a limit-shaped failure reason, records a cool-down (default
60 minutes, because the window rolls and its reset time is unknown). It blocks launches and
attaches that would create a session. It never blocks the follow of existing sessions, re-attaching
or adopting them, answers, or the verify continuation (review I-1).

### D10 — Usage counts tasks

Jules reports no tokens to git-paw. Each created session is one remote task, reported per worker and
in `usage_total.remote_tasks`. v0.19.0's token and cost caps are untouched; the daily limit is the
remote budget.

## Risks / Trade-offs

Threat analysis (`security-and-safety-review`).

- **J1 — Terms forbid this use.** → Task 0.1 blocked everything until the terms were read. Done
  2026-10-28: the terms link holds only the privacy notice, and nothing found forbids automated use
  (Context). The rule stays for later terms changes and for every later vendor.
- **J2 — Key leakage.** → Header only, env only, masked everywhere, refused in configuration,
  refused before any request when it holds a control character (I-5), stripped from git commands.
  Residual: panes and headless turns inherit the user's environment (D3).
- **J3 — Request redirection or path smuggling.** → Pinned host, no redirects, validated names
  (D2).
- **J4 — Hostile or malformed responses.** → Size cap, JSON-object check, unknown values mapped to
  unknown, control characters stripped at rendering (`remote-surface-adapter`).
- **J5 — Alpha API churn.** → Unknown values degrade to unknown; the classification and mapping are
  one table each; the release stays after the 1.0 freeze on purpose, so the alpha API is not under
  the contract.
- **J6 — Spend beyond the plan.** → The ledger refuses over-limit launches; the cool-down stops
  launches after a limit; the follow's cap and once-per-question rules bound messages.
- **J7 — Data egress.** → Task prompts, answers and (in landing) failing verify output go to Google.
  The repository is already on GitHub with the Jules app installed. Per the privacy notice, sessions
  on public repositories may train Google's models, private ones are not used for training, and
  reviewers may read samples. The docs and the security posture chapter say so; there is no gate,
  because the user chooses the repository.

## Migration Plan

1. Terms first (task group 0). Stop if they forbid this use. (Read 2026-10-28; nothing forbids it.)
2. The transport: system `curl` through `CommandRunner` (D1, decided).
3. Client, mapping, classification and ledger against a recording transport; no network in tests.
4. Configuration; usage fields.
5. Fold in the live run's answers (task group 1) before release.

Rollback: revert. The ledger is ignored by older binaries.

## Open Questions

The proof of concept's five questions, verbatim, with where each is pinned:

1. **Does the API work on the free plan?** — Pinned in *Jules sessions are created … without pull
   requests or plan approval* (the access check, which starts no session) and in doctor's remote
   check (`remote-work-landing`). Holds either way: without API access the check fails as an
   authentication failure and the launch is refused. Blocking task 1.1.
2. **What does a limit error look like (status and body)?** — Pinned in *Jules limit failures are
   recognised in one place*. The rules are broad on purpose (429, `RESOURCE_EXHAUSTED`, wording).
   Blocking task 1.2 confirms the table.
3. **Do activities come back in `createTime` order, and is the newest patch cumulative?** —
   Ordering is pinned in *Jules states and activities map onto the remote domain* (sort by
   `createTime`, which holds either way). Cumulativeness is pinned in `remote-work-landing`'s
   *Only a patch that covers the session's whole change is landed*, which lands nothing and
   escalates when it cannot tell. Blocking task 1.3.
4. **Does a session without `AUTO_CREATE_PR` still hand back a patch, in the activities or in
   `outputs`?** — Pinned in *Jules states and activities map onto the remote domain* (both places
   are read) and in `remote-work-landing`'s completed-without-patch escalation. Fleet's route
   creates its sessions **with** `AUTO_CREATE_PR`, so answering this needs one extra live session
   created without it (blocking task 1.4). If the answer is "no patch without it", this release
   cannot land work without opening pull requests, and the user must decide between pulling
   v1.5.0's pull-request work forward and fetching Jules's branch instead.
5. **How long does a small task take, and what does a question look like?** — Pinned in
   `remote-surface-adapter` (the question text is the newest agent message, with a fixed fallback;
   `[remote] follow_hours` defaults to 4). Blocking task 1.5 may change the defaults.

Also open:
- **Terms** — read 2026-10-28 (Context, task 0.1); read again whenever they change.
