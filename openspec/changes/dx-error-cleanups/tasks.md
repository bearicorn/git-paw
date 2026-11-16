## 1. Uncommitted-spec guard (D1, D2)

- [ ] 1.1 Confirm whether `commands/supervisor.rs` runs `check_uncommitted_specs` today; add `force` to `SupervisorOptions` and pass it from `main.rs` dispatch
- [ ] 1.2 `check_uncommitted_specs_with` takes the backend's `spec_locations` map and runs `git status --porcelain -- <location.path>` (path relative to the repo root); a spec with no location is skipped
- [ ] 1.3 `start_with_specs` and the coordinated launch return `PawError::NotAttempted` naming each spec and `--force` before creating any worktree, session file or tmux session; with `--force` print the one "ignored because of --force" line and launch
- [ ] 1.4 Tests: untracked `openspec/changes/<id>/` refused with exit 2 and nothing created (bare and coordinated); Markdown spec refused; clean specs launch; `--force` launches and prints the line once; Spec Kit feature directory checked; OpenSpec change found without any `specs/<id>`
- [ ] 1.5 `cli.rs` help for `--force`: "Launch even when selected specs have uncommitted changes"

## 2. Budget error (D3)

- [ ] 2.1 Add `PawError::BudgetExhausted(String)` (`#[error("Budget: {0}")]`, exit 1); update every exhaustive `PawError` match
- [ ] 2.2 `main.rs::run_drive_loop` returns it on `DriveOutcome::BudgetExhausted`
- [ ] 2.3 `BudgetCost::new` returns `Result<Self, InvalidBudgetCost>`; `Display` byte-identical to today's text; `Deserialize` unchanged in behaviour
- [ ] 2.4 Tests: abandon run stderr has `error: Budget:` and no `Session:`, exit 1; `exit_code()` of the variant is 1; `BudgetCost::new(0.0)` error holds `0`; config load of `budget_max_cost = -1` error text unchanged (golden)

## 3. Budget escalation text (D4)

- [ ] 3.1 Add `assets/escalation/budget-exhausted.md` and `assets/escalation/budget-cost-unenforceable.md` (question and hand-off sections; `{{CAP}}`, `{{USAGE}}`, `{{DETAIL}}`)
- [ ] 3.2 A pure renderer in `drive/budget.rs`; `Escalation::question()` and `handoff_text()` gain arms for both budget verdicts
- [ ] 3.3 Tests: rendered question contains the detail and "budget", not `stalled` / `review the pane`; fixed prose equals the asset; hand-off text likewise

## 4. Stopped rebase at another path (D5)

- [ ] 4.1 `find_rebasing_worktree_with(runner, repo_root, branch)`: walk `git worktree list --porcelain`, read `rebasing_branch_ref` for each `detached` block
- [ ] 4.2 `create_worktree_with` consults it when `find_worktree_for_branch_with` returns `None`; reuse at the found path, judged by `worktree_skip_reason`
- [ ] 4.3 Callers record the returned path (not the computed one) in the session

## 5. Tests for D5

- [ ] 5.1 Real-repo test: sibling worktree with a stopped `edit` rebase, config switched to child; `create_worktree` returns the sibling path with the rebase-in-progress reason, no child worktree, rebase still in progress (also pins git's refusal when the old code path runs `worktree add`)
- [ ] 5.2 Fake-runner test: a detached worktree rebasing `feat/y` is not returned for `feat/x`

## 6. Config warnings as values (D6)

- [ ] 6.1 `load_config_file` / `load_config` return the config with `warnings: Vec<ConfigWarning>` (`#[must_use]`); delete `warn_once_for`
- [ ] 6.2 Every `load_config` caller emits through its sink once per command; repeated loads discard explicitly
- [ ] 6.3 Tests: `start` prints the deprecated-table warning exactly once; two loads in one process each return it; in-process capture holds it and stderr does not
- [ ] 6.4 Coordinate with `supervisor-vocabulary-split`: its config deprecation entries ride in the same list

## 7. Struct-literal rule (D7)

- [ ] 7.1 Count external literal sites per struct (`rg -c '<Struct> \{' src tests`) and record the covered set in the audit's table
- [ ] 7.2 Add `Default` or a `test_support` fixture builder for each covered struct; persisted / wire structs get a builder only
- [ ] 7.3 Convert the literal sites; add the source-audit test (fails naming file, line and struct)
- [ ] 7.4 Test: a deliberately exhaustive literal in a fixture file makes the audit fail

## 8. Docs

- [ ] 8.1 CLI reference and user guide: `start --force` and the refusal (exit 2); the budget abandon error
- [ ] 8.2 `docs/src/architecture.md`: config warnings as values; the struct-literal rule
- [ ] 8.3 CHANGELOG: the `--from-specs` refusal is a behaviour change

## 9. Gates

- [ ] 9.1 `cargo fmt`, then `just lint`
- [ ] 9.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 9.3 `mdbook build docs/`
- [ ] 9.4 `openspec validate dx-error-cleanups --strict`
