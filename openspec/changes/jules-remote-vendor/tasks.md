## 0. Terms first (blocking: nothing below may call Jules until 0.1 is done)

- [x] 0.1 Read and record the vendor terms; stop if they forbid this use. Done 2026-10-28: the terms link holds only the Jules Privacy Notice (last updated 2025-10-07), and nothing found forbids automated use; recorded in design.md's Context. The stop-if-forbidden rule still holds: read the terms again when they change, and give every later vendor this same blocking task before its adapter calls it.
- [ ] 0.2 Record the reviewed terms facts in the remote-workers chapter and the configuration reference (`remote-work-landing` docs), with the date read, including the training use (public-repository sessions may train Google's models, private ones are not used, reviewers may read samples)

## 1. Confirm with the PoC's live run (Fleet's Jules route) — blocking before release

- [ ] 1.1 Open question 1 (free-plan API access): confirm from Fleet's live-run records whether the API works on the free plan; record the result in design.md and the docs
- [ ] 1.2 Open question 2 (limit error shape): record the status and body of a real limit error; update the limit table if it shows a shape the table misses
- [ ] 1.3 Open question 3 (ordering and cumulative patches): record whether activities come back in `createTime` order and whether the newest patch is cumulative from the session's base; if not cumulative, amend `remote-work-landing`'s patch-selection requirement before release
- [ ] 1.4 Open question 4 (patch without `AUTO_CREATE_PR`): Fleet's route creates sessions with `AUTO_CREATE_PR`, so run one extra live session created without it, and record whether a patch comes back and whether it is in the activities or in `outputs`; if no patch comes back, stop and take the choice in design.md's open question 4 to the user
- [ ] 1.5 Open question 5 (duration and question shape): record how long a small task took and what a question looked like (activity kind, fields, timing); adjust the question-text rule or the `follow_hours` default if the evidence requires it
- [ ] 1.6 Confirm the published plan limits (15/3, 100/15, 300/60) against <https://jules.google/docs/usage-limits/> and record the date

## 2. Transport and client

- [ ] 2.1 Implement the decided transport (design D1): the system `curl` through the `CommandRunner` seam (`--proto =https`, no `-L`, `--max-time 20`, `--max-filesize`, status via `--write-out`), with the key in a `--config -` header line on stdin and never in argv; tests replace it with a recording fake (no network in tests); add `curl` presence to doctor's checks
- [ ] 2.2 `src/remote/jules.rs`: pinned base URL; refuse other schemes, hosts and base paths; no redirects (3xx is a bad response); 20 s timeout; 16 MB cap; JSON-object check; session-name validation; bounded paging
- [ ] 2.3 Key handling: read `JULES_API_KEY` per request; refuse a key holding a line break or any other control character before any request or process, with an error naming `JULES_API_KEY` and no part of the key (review I-5); send it only in `X-Goog-Api-Key`; never in argv (with `curl`, on stdin via `--config -`, the header value quoted); mask the key in every recorded or printed vendor text; error naming `JULES_API_KEY` when unset
- [ ] 2.4 `RemoteAgentService` for Jules: access check (`GET /sources/github/{o}/{r}`, case-insensitive listing fallback), create (`requirePlanApproval` false, no pull-request `automationMode`), read, list activities, `:sendMessage`, find by title (`GET /sessions` paged); no `:approvePlan`
- [ ] 2.5 Mapping: states table; activities (`agentMessaged`, `planGenerated`, `sessionCompleted`, `sessionFailed`, other); patches from artifacts and from `outputs`, de-duplicated by diff; order by `createTime`; identifier `id` else `name`
- [ ] 2.6 Limit classification table (429, `RESOURCE_EXHAUSTED`, quota/limit wording in `error.message` or a failure reason); 401/403 auth, 404 not found, transport failure network
- [ ] 2.7 Tests: every scenario of *The Jules adapter talks only to its pinned API host*, *The Jules key …* (including the control-character key), *Jules sessions are created …*, *Jules states and activities map …* and *Jules limit failures …* against the recording transport; a source audit finds no `:approvePlan`

## 3. Ledger, limits, cool-down, usage

- [ ] 3.1 `src/remote/ledger.rs`: `<data_dir>/remote/jules-ledger.json`, `0700` / `0600`, records name, creation time and last-known state, no credential; the follow updates last-known state
- [ ] 3.2 Daily and concurrency checks (rolling 24 h; sessions about to be created count; non-terminal last-known state counts as open), returning the limit, the count and the oldest session's exit time; consumed by the remote gate in `remote-work-landing`; applied only to creating a session, never to a re-attach, an adoption or a message (review I-1)
- [ ] 3.3 Cool-down on a limit failure (create, read, or limit-shaped failure reason) for `limit_cooldown_minutes`; refuses launches and attaches that would create a session; the follow, re-attach, adoption, answers and the verify continuation continue (review I-1)
- [ ] 3.4 Status JSON `remote.tasks` per remote worker and `usage_total.remote_tasks`; remote workers' `usage` stays `null`; budget caps ignore remote tasks
- [ ] 3.5 Strip `JULES_API_KEY` from every git process that checks, applies or commits remote work (including hooks)
- [ ] 3.6 Tests: daily limit refuses a launch with nothing created; concurrency limit refuses; old sessions leave the window; an override replaces the plan's limit; a full ledger does not stop re-attaching; a limit on creation starts a cool-down; following continues during a cool-down; answers reach an existing session during a cool-down; status counts remote tasks; git hooks do not inherit the key

## 4. Configuration and docs

- [ ] 4.1 `[remote] vendor` (`jules` only) and `[remote.jules]` (`plan`, `daily_tasks`, `concurrent_tasks`, `limit_cooldown_minutes`); refuse `api_key` without echoing its value; per-key merge; serialized only when set
- [ ] 4.2 Tests: defaults; unknown vendor refused; key in configuration refused without echo; keys merge one by one
- [ ] 4.3 Configuration reference: every key, defaults, the published plan limits with their date, `JULES_API_KEY` only; `mdbook build docs/` succeeds
- [ ] 4.4 Gate 5 (`security-and-safety-review`) against design.md J1–J7, including the `curl` invocation (no key in argv, pinned scheme and host, no redirects); `cargo deny` clean with no new crate
- [ ] 4.5 Every scenario maps to a named test; `just check` and `just deny` green; full suite green against the merge-base with `cargo test --no-fail-fast`; no `unwrap`/`expect` in non-test code; exit codes verified with `cmd; echo $?`, not through a pipe
