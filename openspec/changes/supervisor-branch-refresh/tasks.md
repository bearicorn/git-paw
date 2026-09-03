## 1. Resolve the open design questions

- [ ] 1.1 Decide refresh breadth: all live worker branches on each merge, vs only branches whose declared intent overlaps the merged branch's files. Record the decision and its rationale in `design.md` before writing code.
- [ ] 1.2 Confirm the decision in 1.1 against the verified-branch exclusion so the two rules cannot select the same branch.

## 2. Git layer (`src/git.rs`)

- [ ] 2.1 Expose a mid-session rebase entry point built on the existing `rebase_branch_onto_default_with` (`:276`), which already runs inside the checked-out worktree and already aborts + restores on failure.
- [ ] 2.2 Confirm the already-current case exits zero with no rewrite and is treated as success.
- [ ] 2.3 Verify `create_worktree`'s creation-time `rebase_onto_main` contract is untouched — same ordering relative to the existence check, same idempotent resume behaviour.
- [ ] 2.4 Unit tests via the `CommandRunner` seam: rebase invoked in the right worktree; non-zero exit triggers `--abort` and returns an error; already-current is a successful no-op.

## 3. Conflict prediction

- [ ] 3.1 Implement the pre-rebase conflict prediction (D2) so a predicted-conflicting branch is skipped without the worktree ever entering a rebase state.
- [ ] 3.2 Tests: predicted-conflict branch is skipped and the rebase entry point is never invoked.

## 4. Precondition gates (`src/supervisor/`)

- [ ] 4.1 Cleanliness gate — read the watcher's tracked per-agent modified-file state (D1). Do NOT add a second working-tree probe.
- [ ] 4.2 Idleness gate — reuse the existing mid-response detection (`MID_RESPONSE_MARKERS`, `src/supervisor/drive.rs:1786`).
- [ ] 4.3 Claim gate — reuse `PaneClaim` (`src/supervisor/claim.rs:62`).
- [ ] 4.4 Behind-the-base gate.
- [ ] 4.5 Verified-branch exclusion — a branch that has passed verification and awaits merge is never refreshed.
- [ ] 4.6 Wire the gates as a strict conjunction: any failure is a no-op for that branch, with no partial or degraded attempt.
- [ ] 4.7 Record skips; do not retry within the same merge event and do not escalate repeated skips to the supervisor inbox.

## 5. Post-merge trigger

- [ ] 5.1 Evaluate refresh only after a successful merge into the default branch — not on a timer or interval.
- [ ] 5.2 Test: no merge → no refresh, even as time passes and agents keep working.

## 6. Worker notification

- [ ] 6.1 Notify the refreshed worker that its HEAD was rewritten, via an existing broker message type carrying a distinguishing bracket tag (the `[conflict-detector]` precedent).
- [ ] 6.2 Confirm no new `BrokerMessage` variant is introduced — if one becomes unavoidable, stop and scope the enum ripple per the AGENTS.md checklist (`messages.rs`, `dashboard/broker_log.rs`, `learnings.rs`, `delivery.rs`) before proceeding.
- [ ] 6.3 Test: the notification is not an `agent.advanced-main` message.

## 7. Configuration

- [ ] 7.1 Add the opt-in field with `#[serde(default)]` and `skip_serializing_if`, defaulting to disabled.
- [ ] 7.2 Test: a config written before this capability loads unchanged with refresh disabled.
- [ ] 7.3 Test: with refresh disabled, post-merge behaviour is identical to before the capability existed.

## 8. E2E

- [ ] 8.1 Cross-module E2E: merge → gate evaluation → rebase → worker notification, exercising the full flow.
- [ ] 8.2 E2E: a dirty worker is skipped and its uncommitted changes are provably untouched afterwards.

## 9. Docs

- [ ] 9.1 Supervisor user-guide section covering branch refresh, every gate, and why the feature is opt-in.
- [ ] 9.2 Configuration reference entry for the new field.
- [ ] 9.3 `--help` text if a flag is added.
- [ ] 9.4 Confirm the worker-side *When main advances* guidance in the coordination skill is NOT contradicted — it governs the worker and remains correct.
- [ ] 9.5 `mdbook build docs/` succeeds.

## 10. Quality gates

- [ ] 10.1 `cargo fmt` before committing.
- [ ] 10.2 `just check` — verify by real exit code, not piped output.
- [ ] 10.3 Security review: this rewrites history under a live agent. Re-read the gates against `.agents/skills/security-and-safety-review/SKILL.md` and confirm no path can rebase a dirty worktree.
