## 0. Order

- [ ] 0.1 Rebase onto `commit-event-truth` (artifact `sha`, `rewritten` status)

## 1. Hook

- [ ] 1.1 `assets/hooks/pre-commit.sh`: refuse when `$PAW_GD/paw-frozen` exists, naming the SHA and the lifting verdicts, independent of `PAW_STRICT_BRANCH_GUARD`

## 2. Wire (after `wire-format-dtos` / `supervisor-vocabulary-split` if landed)

- [ ] 2.1 Optional `sha` on `VerifiedPayload`; optional `merged_sha` on `AdvancedMainPayload`; all `skip_serializing_if = "Option::is_none"`
- [ ] 2.2 Verify nudge gains optional `trigger` and `sha`
- [ ] 2.3 `orchestrator.freeze { agent_id, sha }` variant: validation, display, `status_label`, routed to the agent's inbox, verdict credential at `/publish`, excluded from roster upsert and from progress
- [ ] 2.4 Coordination chapter documents the fields and the variant

## 3. Delivery and broker state

- [ ] 3.1 `delivery.rs`: nudge on `committed` (trigger `commit`) and `done` (trigger `complete`), never on `rewritten`
- [ ] 3.2 Record the last verified `sha` per agent; record and clear the frozen state on freeze / verified / orchestrator feedback; expose both in `/status`
- [ ] 3.3 `[merge-check]` question on an advanced-main whose `merged_sha` differs from the recorded verified tip (or none recorded)

## 4. Helpers and skills

- [ ] 4.1 `sweep.sh verified` sends `sha` (worktree HEAD, or `--sha`); `advanced-main` sends `merged_sha`
- [ ] 4.2 `sweep.sh freeze <agent>`: write `paw-frozen`, publish, remove the marker on a failed publish; `verified` and `feedback-gate` remove the marker first
- [ ] 4.3 `broker.sh artifact` sends `sha`
- [ ] 4.4 Coordination skill: `done` after the final commit, freeze rules, no unrouted work while standing by
- [ ] 4.5 Orchestrator skill: full gates on `complete`, freeze before the first gate, `[merge-check]` questions explained

## 5. Drive loop

- [ ] 5.1 Record `merged_sha` per branch from observed `agent.advanced-main`; `merge_state` counts merged when the tip or the recorded merged tip is an ancestor of default; report commits after the merge

## 6. Tests (one or more per scenario)

- [ ] 6.1 Hook e2e: the frozen guard refuses and names the SHA; without the marker commits proceed
- [ ] 6.2 Wire round-trips for the new fields and variant; old shapes decode unchanged; freeze without the credential is `403`
- [ ] 6.3 Delivery: nudge triggers (commit, complete, rewritten, disabled); `[merge-check]` cases; a verdict clears the freeze
- [ ] 6.4 `sweep.sh` tests: verified and advanced-main tips, freeze lifecycle and failure; `broker.sh artifact` sha
- [ ] 6.5 Skill content tests for the coordination and orchestrator skill rules
- [ ] 6.6 Drive summary: a commit after the merge keeps the branch merged and reports `+1`; the existing exit-summary tests unchanged

## 7. Gates

- [ ] 7.1 `cargo fmt`, then `just lint`
- [ ] 7.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 7.3 `shellcheck` on the changed hooks and helpers
- [ ] 7.4 `mdbook build docs/`
- [ ] 7.5 `openspec validate verification-lifecycle --strict`
