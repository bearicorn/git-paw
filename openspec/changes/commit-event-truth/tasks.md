## 1. Hooks

- [ ] 1.1 `assets/hooks/post-commit.sh`: return early when `$(git rev-parse --git-path rebase-merge)` or `rebase-apply` exists, or when `GIT_PAW_ACTOR=orchestrator`
- [ ] 1.2 `post-commit.sh`: `modified_files` from `git diff-tree --no-commit-id --name-only -r --root HEAD`; add `sha` from `git rev-parse HEAD`
- [ ] 1.3 New `assets/hooks/post-rewrite.sh` (marker-gated, actor-gated): on `$1 = rebase`, publish one `agent.artifact { status: "rewritten", sha, modified_files }` built from the new SHAs read on stdin; nothing on `amend`
- [ ] 1.4 `src/agents.rs`: install `post-rewrite` with the managed-block chaining used for `post-commit`; `purge` removes its block
- [ ] 1.5 Export `GIT_PAW_ACTOR=orchestrator` in the orchestrator CLI's environment (foreground launch and root pane); assert that no worker pane gets it

## 2. Watcher

- [ ] 2.1 `src/broker/watcher.rs`: run `git --no-optional-locks status --porcelain` and `git --no-optional-locks rev-parse HEAD`
- [ ] 2.2 Skip the `agent.status` publish (but keep the HEAD observation) while `rebase-merge`, `rebase-apply`, `MERGE_HEAD` or `CHERRY_PICK_HEAD` exists in the worktree's gitdir
- [ ] 2.3 A per-target cancel token so `DELETE /watch` can stop a watcher before answering

## 3. Wire (after `wire-format-dtos` / `supervisor-vocabulary-split` if landed)

- [ ] 3.1 Optional `sha` on `ArtifactPayload` (`skip_serializing_if = "Option::is_none"`) and the `rewritten` artifact status; `delivery.rs` emits no nudge for `rewritten`
- [ ] 3.2 Coordination chapter documents the field and the status

## 4. Conflict detector and withdrawal

- [ ] 4.1 `ConflictTracker::acknowledge` marks the record warned; `sweep_in_flight_pairs` keeps acknowledged records
- [ ] 4.2 `ConflictTracker::withdraw(agent)` drops current files, intent, triples, decisions and warned pairs involving the agent
- [ ] 4.3 `DELETE /watch` route: stop the watcher, forget the target, withdraw from the tracker, answer `204` (also `204` for an unknown path)
- [ ] 4.4 `BrokerClient::unwatch`; `commands/remove.rs` calls it after the pane kill and before `detach_worktree` (also with `--keep-worktree`), warning on failure
- [ ] 4.5 User guide conflict-detection chapter: acknowledgement lifetime

## 5. Tests (one or more per scenario)

- [ ] 5.1 Hook e2e (real git, stub broker): a 27-commit rebase publishes no `committed` and one `rewritten`; an amend publishes; uncommitted files are excluded; root commit; `GIT_PAW_ACTOR` suppresses; an existing user `post-rewrite` is preserved
- [ ] 5.2 Watcher: index mtime unchanged after a poll; a 30-commit rebase under polling succeeds; no status during a stopped rebase; publishing resumes after it
- [ ] 5.3 Wire round-trips for the artifact `sha`; old shapes decode unchanged
- [ ] 5.4 Conflict tracker: re-entering acknowledged overlap not re-warned; acknowledgement survives 10 minutes of absence; a material change re-escalates; `withdraw` silences the departed agent
- [ ] 5.5 `DELETE /watch` (known and unknown path); remove e2e with an overlapping peer produces no status, feedback or question for the removed agent; remove with the broker down warns and succeeds

## 6. Gates

- [ ] 6.1 `cargo fmt`, then `just lint`
- [ ] 6.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 6.3 `shellcheck` on the changed hooks
- [ ] 6.4 `mdbook build docs/`
- [ ] 6.5 `openspec validate commit-event-truth --strict`
