## 0. Preconditions

- [ ] 0.0 Confirm `uniform-json-output` has landed and been archived (this change MODIFIES its `cli-machine-output` roster and uses its output module)
- [ ] 0.0a Confirm v0.17 `classifier-retirement` and `cli-setup-consent` are archived (the exit-code and broad-grant deltas are written on `cli-setup-consent`'s text)
- [ ] 0.1 Confirm v0.17 `purge-unmerged-branch-safety` and v0.18 `cmd-library-surface`, `terminal-backend-seam`, `supervisor-pane-removal` have landed; read how `supervisor-pane-removal` resolved GP-49 (bare-session `remove`) and reuse that surface-close path

## 1. Seams (behaviour-preserving)

- [ ] 1.1 `src/git.rs`: add `is_ancestor_with`, `update_ref_cas_with`, `check_ref_format_branch_with`, `worktree_for_branch_with` (promote the private `find_worktree_for_branch_with`), `merge_in_worktree_with`, `merge_commit_without_checkout_with` (merge-tree → commit-tree → CAS update-ref), each through `CommandRunner`, with sandbox-repo unit tests
- [ ] 1.2 Parameterise `detach_worktree`'s branch-deletion comparison ref (today: default branch) and pass the default branch from `remove`/`purge`; existing teardown tests stay green unchanged
- [ ] 1.3 `src/error.rs`: `PawError::IntegrationRefused(String)` + `exit_code::INTEGRATION_REFUSED = 3`; `UserCancelled` and v0.17.0's `NotAttempted` keep exit 2; update `test_general_errors_exit_code`, add `IntegrationRefused` exit-code test, keep the `NotAttempted` exit-code test green
- [ ] 1.4 `src/config/`: `FinishConfig { strategy: Option<FinishStrategy> }` with `#[serde(default, skip_serializing_if)]`; merge repo over global; round-trip + "config without `[finish]` loads" tests; commented `[finish]` block in `generate_default_config`

## 2. Functional core

- [ ] 2.1 Pure `decide_integration(&IntegrationFacts, FinishStrategy) -> IntegrationPlan` (`AlreadyIntegrated`, `CreateTarget`, `FastForward`, `MergeCommit`, `Refuse(NotFastForward{missing} | Conflict{paths})`); table-driven unit tests covering every plan
- [ ] 2.2 Pure precondition check over observed facts (dirty files after managed-file filtering, worktree HEAD vs branch, linked-worktree invocation, `--into` validity) with unit tests

## 3. Use case + CLI

- [ ] 3.1 `git_paw::commands::finish(repo_root, &PawConfig, FinishOptions) -> Result<FinishOutcome>`: observe facts → decide → apply → teardown (surface close via `TerminalBackend`, `SessionLock`, discovery file, `detach_worktree` with the target as comparison ref); no printing inside the use case
- [ ] 3.2 `advanced-main` publish when the default branch moved and the broker answers; warning-only on failure
- [ ] 3.3 `src/cli.rs` `Command::Finish { branch, into, merge_commit, ff_only }` with `about`/`long_about`/examples, `conflicts_with` between the two policy flags; `main.rs` dispatch + rendering; parse tests
- [ ] 3.4 `--json`: typed `FinishReport` in the shared output module; printed on exit 0 and exit 3, stdout empty on exit 1; warnings to stderr; add `finish` to the `cli-machine-output` roster guard and its golden key set
- [ ] 3.5 Doctor broad-grant advisory (from `cli-setup-consent`): add `finish` to the shape-5 list of git-paw state-changing subcommands; unit test that `Bash(git paw finish:*)` and `Bash(git-paw finish:*)` are listed ("A durable grant to finish a branch is listed")

## 4. Tests per scenario (`tests/finish_e2e.rs`, real repos via `tempfile`, `HOME`/`XDG_DATA_HOME` isolated)

- [ ] 4.1 Fast-forward from main working tree tears down (surface, worktree, session entry)
- [ ] 4.2 Run from inside a linked worktree → non-zero, nothing changed; unknown branch → non-zero; `--merge-commit --ff-only` → usage error
- [ ] 4.3 Dirty worktree refused with file list; managed-file-only worktree proceeds; detached (mid-rebase) worktree refused
- [ ] 4.4 Default target = resolved default branch (`master` repo); missing `--into` target created at tip; invalid/self target refused
- [ ] 4.5 Diverged branch → exit 3, message with commit count, nothing changed; already-integrated → teardown only
- [ ] 4.6 `[finish] strategy = "merge-commit"` → merge commit with both parents; `--ff-only` overrides → exit 3; conflicting merge → exit 3 naming paths, no leftover merge state
- [ ] 4.7 Dirty main working tree with target checked out → refused naming it; clean checkout follows the ref; non-checked-out target moves by CAS and no checkout switches
- [ ] 4.8 Created branch deleted when contained in target; pre-existing branch kept; unmanaged worktree left in place; refused integration tears nothing down
- [ ] 4.9 `finish --json`: integrated document (all keys, `schema_version` 1); refused → document + exit 3; dirty → empty stdout + exit 1; roster guard includes `finish`
- [ ] 4.10 Live broker receives one `agent.advanced-main`; no broker → one warning, exit 0

## 5. Docs

- [ ] 5.1 `--help` text; README command table row for `finish`
- [ ] 5.2 `docs/src/cli-reference.md` (`finish` section, exit codes 0/1/2/3, the `--json` document); `docs/src/user-guide/session-lifecycle.md` (manual loop `init → start → work → finish`)
- [ ] 5.3 `docs/src/configuration/README.md` `[finish]` table; `docs/src/user-guide/security-posture.md` notes `finish` is root-only and that doctor lists a durable `finish` grant; `docs/src/user-guide/doctor.md` broad-grant list gains `finish`

## 6. Gates

- [ ] 6.1 Every scenario maps to a named test; `cargo fmt`; `just check`; `just deny`; `mdbook build docs/`
