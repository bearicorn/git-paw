## 0. Preconditions

- [ ] 0.1 `finish-command` is merged (this change extends its capability and reuses its core, policy, teardown and exit code 3)

## 1. Seams

- [ ] 1.1 `src/git.rs`: `changed_paths_since_with(repo, base, side)` (`git diff --name-only -z <base>..<side>`), `merge_base_with`, and `rebase_branch_onto_with(target)` generalising `rebase_branch_onto_default_with` (default-branch behaviour unchanged), plus `restore_branch_tip_with(worktree, sha)`; sandbox-repo unit tests
- [ ] 1.2 Expose the `supervisor-branch-refresh` idleness check and `supervisor::claim` acquire/release as a reusable precondition probe (no behaviour change to branch refresh; its tests stay green)

## 2. Functional core

- [ ] 2.1 Pure `decide_rebase(&RebaseFacts) -> RebasePlan` (`Rebase` | `Skip(Overlap{paths} | Conflict{paths} | Dirty | AgentBusy | NoTestCommand)`); table tests per variant
- [ ] 2.2 Pure queue summary + exit-status function over per-branch outcomes (0 / 3 / 1); unit tests

## 3. Use case + CLI

- [ ] 3.1 `src/cli.rs`: `Finish { branches: Vec<String>, verified, rebase, into, merge_commit, ff_only }`; `--verified` conflicts with branch names; help text with queue examples; parse tests (single-branch behaviour unchanged)
- [ ] 3.2 `commands::finish` queue loop: per branch run the `finish-command` pipeline; on `Refuse(NotFastForward)` with `--rebase` evaluate `decide_rebase`, claim, rebase, run `[supervisor] test_command` in the worktree, integrate or restore; collect outcomes; stop on error
- [ ] 3.3 After the pass publish one `agent.feedback` per skipped live agent (root identity/credential), or print them when no broker
- [ ] 3.4 Extend `FinishReport` additively (`integrated_after_rebase`, `directives`, `error`); print on exit 1 only when at least one branch was processed; update the `finish` golden key set
- [ ] 3.5 `--verified` selection from broker status (`WorkerPhase::Verified`) in session order

## 4. Tests per scenario (`tests/finish_queue_e2e.rs`, real repos, isolated `HOME`/`XDG_DATA_HOME`)

- [ ] 4.1 Ordered queue integrates both; `--verified` selects only verified agents; `--verified` without broker exits non-zero
- [ ] 4.2 Disjoint branch rebased, re-tested (stub test command exiting 0), integrated, reported "after rebase"
- [ ] 4.3 Overlapping-but-merge-tree-clean (`CHANGELOG.md`) skipped naming the path; busy agent skipped (fake terminal backend reporting mid-response); no test command → skipped with reason
- [ ] 4.4 Failing re-test (stub exiting 1) restores pre-rebase tip, clean worktree, no rebase in progress, target unmoved; failed rebase restores
- [ ] 4.5 Exactly one `agent.feedback` per skipped live agent, all naming the final target tip, none for integrated agents
- [ ] 4.6 Exit 3 on partial success; exit 1 stops the queue and keeps earlier integrations
- [ ] 4.7 Queue `--json`: three-branch document with outcomes, one directive, `error` null, exit 3; mid-queue error prints the document with `error` and exits 1
- [ ] 4.8 Drive-loop test: all workers verified → no ref moves, no rebase (unattended loop with fakes)

## 5. Skill + docs

- [ ] 5.1 `assets/agent-skills/orchestrator.md` (renamed from `supervisor.md` by v0.18 `orchestrator-skill-export`; a package directory after v0.20 `skill-package-layout`) "Merge orchestration": replace the raw `git checkout`/`git merge --ff-only`/`git reset --hard` loop with the `git paw finish --rebase` procedure and exit-code handling; update `src/skills.rs` content tests (the `git merge --ff-only` assertion at the merge-orchestration test) and `tests/lang_agnostic_skill_audit.rs` stays green
- [ ] 5.2 `docs/src/user-guide/supervisor.md` merge-orchestration mirror (or its v0.18 replacement chapter; the v0.17 *Agent permissions* section stays as is), `docs/src/cli-reference.md` queue flags, `docs/src/user-guide/session-lifecycle.md` (merging a wave), README `finish` row mentions batches

## 6. Gates

- [ ] 6.1 Every scenario maps to a named test; `cargo fmt`; `just check`; `just deny`; `mdbook build docs/`
