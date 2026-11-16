## 1. Hooks (commit-event truth)

- [ ] 1.1 `assets/hooks/post-commit.sh`: return early when `$(git rev-parse --git-path rebase-merge)` or `rebase-apply` exists, or when `GIT_PAW_ACTOR=orchestrator`
- [ ] 1.2 `post-commit.sh`: `modified_files` from `git diff-tree --no-commit-id --name-only -r --root HEAD`; add `sha` from `git rev-parse HEAD`
- [ ] 1.3 New `assets/hooks/post-rewrite.sh` (marker-gated, actor-gated): on `$1 = rebase`, publish one `agent.artifact { status: "rewritten", sha, modified_files }` built from the new SHAs read on stdin; nothing on `amend`
- [ ] 1.4 `src/agents.rs`: install `post-rewrite` with the managed-block chaining used for `post-commit`; `purge` removes its block
- [ ] 1.5 `assets/hooks/pre-commit.sh`: refuse when `$PAW_GD/paw-frozen` exists, naming the SHA and the lifting verdicts, independent of `PAW_STRICT_BRANCH_GUARD`
- [ ] 1.6 Export `GIT_PAW_ACTOR=orchestrator` in the orchestrator CLI's environment (foreground launch and root pane); assert that no worker pane gets it

## 2. Watcher

- [ ] 2.1 `src/broker/watcher.rs`: run `git --no-optional-locks status --porcelain` and `git --no-optional-locks rev-parse HEAD`
- [ ] 2.2 Skip the `agent.status` publish (but keep the HEAD observation) while `rebase-merge`, `rebase-apply`, `MERGE_HEAD` or `CHERRY_PICK_HEAD` exists in the worktree's gitdir
- [ ] 2.3 A per-target cancel token so `DELETE /watch` can stop a watcher before answering

## 3. Wire (after `wire-format-dtos` / `supervisor-vocabulary-split` if landed)

- [ ] 3.1 Optional `sha` on `ArtifactPayload` and `VerifiedPayload`; optional `merged_sha` on `AdvancedMainPayload`; all `skip_serializing_if = "Option::is_none"`
- [ ] 3.2 Verify nudge gains optional `trigger` and `sha`
- [ ] 3.3 `orchestrator.freeze { agent_id, sha }` variant: validation, display, `status_label`, routed to the agent's inbox, verdict credential at `/publish`, excluded from roster upsert and from progress
- [ ] 3.4 Coordination chapter documents the fields and the variant

## 4. Delivery and broker state

- [ ] 4.1 `delivery.rs`: nudge on `committed` (trigger `commit`) and `done` (trigger `complete`), never on `rewritten`
- [ ] 4.2 Record the last verified `sha` per agent; record and clear the frozen state on freeze / verified / orchestrator feedback; expose both in `/status`
- [ ] 4.3 `[merge-check]` question on an advanced-main whose `merged_sha` differs from the recorded verified tip (or none recorded)
- [ ] 4.4 Read cursor per agent from `GET /messages/<id>?since=N`; `/status` per-agent `read_through` and unread orchestrator messages (count, oldest seq, oldest age)
- [ ] 4.5 `[delivery]` question when a `committed` artifact arrives over unread orchestrator feedback or freeze, once per unread set
- [ ] 4.6 `POST /publish` answers `202 {"seq": N}` (`publish_message` returns the seq)

## 5. Conflict detector and withdrawal

- [ ] 5.1 `ConflictTracker::acknowledge` marks the record warned; `sweep_in_flight_pairs` keeps acknowledged records
- [ ] 5.2 `ConflictTracker::withdraw(agent)` drops current files, intent, triples, decisions and warned pairs involving the agent
- [ ] 5.3 `DELETE /watch` route: stop the watcher, forget the target, withdraw from the tracker, answer `204` (also `204` for an unknown path)
- [ ] 5.4 `BrokerClient::unwatch`; `commands/remove.rs` calls it after the pane kill and before `detach_worktree` (also with `--keep-worktree`), warning on failure

## 6. Wave activity

- [ ] 6.1 Broker host samples each watch target's surface every 30 s through the terminal backend, off the async workers; mid-response (prompt-profile check) plus a changed capture hash marks activity
- [ ] 6.2 `wave` gains `last_activity_unix_secs` and `busy_agents`; `stalled` also requires no activity within the threshold; `idle_seconds` unchanged

## 7. Helpers and skills

- [ ] 7.1 `sweep.sh verified` sends `sha` (worktree HEAD, or `--sha`); `advanced-main` sends `merged_sha`
- [ ] 7.2 `sweep.sh freeze <agent>`: write `paw-frozen`, publish, remove the marker on a failed publish; `verified` and `feedback-gate` remove the marker first
- [ ] 7.3 Root-side verbs print `seq <N>` from the publish response
- [ ] 7.4 `sweep.sh delivery <agent> [<seq>]` with exit `0` read / `1` unread
- [ ] 7.5 `broker.sh artifact` sends `sha`
- [ ] 7.6 Coordination skill: `done` after the final commit, freeze rules, poll before commit, no unrouted work while standing by
- [ ] 7.7 Orchestrator skill: full gates on `complete`, freeze before the first gate, `delivery` check before acting on silence, `[merge-check]` and `[delivery]` questions explained

## 8. Drive loop

- [ ] 8.1 Record `merged_sha` per branch from observed `agent.advanced-main`; `merge_state` counts merged when the tip or the recorded merged tip is an ancestor of default; report commits after the merge

## 9. Tests (one or more per scenario)

- [ ] 9.1 Hook e2e (real git, stub broker): a 27-commit rebase publishes no `committed` and one `rewritten`; an amend publishes; uncommitted files are excluded; root commit; `GIT_PAW_ACTOR` suppresses; frozen guard refuses and names the SHA; an existing user `post-rewrite` is preserved
- [ ] 9.2 Watcher: index mtime unchanged after a poll; a 30-commit rebase under polling succeeds; no status during a stopped rebase; publishing resumes after it
- [ ] 9.3 Wire round-trips for the new fields and variant; old shapes decode unchanged; freeze without the credential is `403`
- [ ] 9.4 Delivery: nudge triggers (commit, complete, rewritten, disabled); `[merge-check]` cases; read cursor and unread; `[delivery]` once per unread set; the publish response seq matches `/log`
- [ ] 9.5 Conflict tracker: re-entering acknowledged overlap not re-warned; acknowledgement survives 10 minutes of absence; a material change re-escalates; `withdraw` silences the departed agent
- [ ] 9.6 `DELETE /watch` (known and unknown path); remove e2e with an overlapping peer produces no status, feedback or question for the removed agent; remove with the broker down warns and succeeds
- [ ] 9.7 Wave activity with a fake backend: a changing mid-response pane keeps `stalled` false and lists the agent; a static pane stalls
- [ ] 9.8 `sweep.sh` tests: verified and advanced-main tips, freeze lifecycle and failure, `seq` in output, `delivery` exit codes; `broker.sh artifact` sha
- [ ] 9.9 Skill content tests for the coordination and orchestrator skill rules
- [ ] 9.10 Drive summary: a commit after the merge keeps the branch merged and reports `+1`; the existing exit-summary tests unchanged

## 10. Gates

- [ ] 10.1 `cargo fmt`, then `just lint`
- [ ] 10.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 10.3 `shellcheck` on the changed hooks and helpers
- [ ] 10.4 `mdbook build docs/`
- [ ] 10.5 `openspec validate broker-event-semantics --strict`
