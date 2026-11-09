## 1. Source audits (F1)

- [ ] 1.1 Move `production_code` from `tests/command_library_audit.rs` into a `tests/helpers/` module, so a `#[cfg(test)]` inside a comment line no longer starts a blank-out; use it from both audits and delete `production_part` in `tests/source_audit.rs`
- [ ] 1.2 Add the `let … SpecBackendKind::X` form to the spec-format pattern, with a caught-example line
- [ ] 1.3 Tests for "Production code after an early test item is audited", "Test items and comments are not audited" and "main.rs is a positive control"
- [ ] 1.4 Fix or allowlist (with a pointer) any spec-format offender the wider filter now finds
- [ ] 1.5 Add the CLI-name audit with the D3 patterns, caught-example lines, the `src/doctor.rs` allowlist entry citing roadmap F10, and a stale-entry check; tests for the four cli-provider scenarios

## 2. Menu shapes in the profile (F2)

- [ ] 2.1 Add `menu_cursor_glyphs` and `menu_footer_markers` to `CliPromptProfile` and its override, with defaults, per-field fallback and the empty-marker check; update every struct-literal site
- [ ] 2.2 Move `menu_showing`, `strip_decoration_keep_cursor`, `composer_text_is_dim`, `styled_cells` and `apply_sgr` with their unit tests to `src/supervisor/screen.rs`; read the glyphs and footers from the profile
- [ ] 2.3 Tests for the four "A selection menu's shape comes from the profile" scenarios
- [ ] 2.4 `docs/src/user-guide/cli-prompt-profiles.md` and the configuration reference: the two new fields and their defaults

## 3. Warnings as values (F3)

- [ ] 3.1 `resolve_for_launch` returns the warning with the profile; delete `first_warning_for` and its static
- [ ] 3.2 `start.rs` (3 sites), `launch.rs`, `recover.rs` and `supervisor.rs` emit each warning once per launch through `Output::warn`
- [ ] 3.3 `AgentProfiles::record` returns its warning; the drive-loop shell prints it
- [ ] 3.4 `locate` and `ensure_unambiguous_in` take the working directory instead of reading it
- [ ] 3.5 Tests for "warns once per launch through the sink", "does not write the warning to stderr", "Two launches in one process each warn" and "A profile that fails to compile is reported to the caller"

## 4. One resolution and one dispatcher (F4, F7)

- [ ] 4.1 `send::deliver` and `add.rs` read the prompt profile and settle delay from one `CliProfile`
- [ ] 4.2 `KeyDispatcher` methods take `&self`; `SubmitPort` and drive-deps `dispatcher` become `&dyn KeyDispatcher`; delete `BackendDispatcher` and `SharedFakeDispatcher`; `RecordingDispatcher` uses a `RefCell`
- [ ] 4.3 Tests for "A send uses the backend as capturer and dispatcher" and "Drive-loop keystrokes are unchanged"

## 5. Styled capture pinned (F6)

- [ ] 5.1 `FakeBackend` records `CaptureStyled`; update operation-sequence assertions that expected `CaptureTail` from `capture_styled`
- [ ] 5.2 Golden argv test for `TmuxBackend::capture_styled`; tests for the two styled-read scenarios

## 6. Rebase-skip predicate (F5, conditional)

- [ ] 6.1 `src/git.rs` `worktree_skip_reason` calls `crate::agents::is_managed_path`: keep the skip decision pure and supply the managed-path predicate from outside the VCS adapter if that is small; otherwise add a roadmap note

## 6a. git-operations spec drift (F16)

- [ ] 6a.1 Check that tests cover the reworded "Create worktrees as siblings of the repository" rebase scenarios (rebase in the branch's own worktree, its abort in that worktree, removal of a new worktree on failure); add any missing assertion
- [ ] 6a.2 Test for "add keeps its notice text" asserting the exact notice string
- [ ] 6a.3 `docs/src/user-guide/mcp.md`: `start_session` returns skipped-rebase notices in `warnings`

## 6b. worker-usage-budget follow-ups (F17)

- [ ] 6b.1 Tests for "Headless orchestrator spend counts toward the breach", "An unevaluable cost cap alone leaves a finished wave completed" and "A CLI session git-paw did not launch is excluded"; reference the existing tests for "A cost cap is breached by reported cost alone" and "An unknown budget policy is rejected"
- [ ] 6b.2 Comments: `src/commands/status.rs` → "Same formula as `budget_max_tokens` (`UsageTotals::tokens`), over current workers only"; `src/doctor.rs` → the doctor checks the configured CLIs, the loop checks the session's workers
- [ ] 6b.3 `docs/src/user-guide/supervisor.md`: add the budget exit (`budget_on_exhausted = "abandon"` → `budget-exhausted`, non-zero) to the exit list, linking the budget section
- [ ] 6b.4 `src/cli.rs` `start` long_about and `--unattended` help, and `docs/src/cli-reference.md`: one identical sentence on `budget_max_tokens` / `budget_max_cost` and `budget_on_exhausted = "abandon"`

## 6c. worker-friction-fixes hygiene (F18)

- [ ] 6c.1 Test for the per-branch merge-loop scenario: the orchestrator skill's merge section has `git merge-base --is-ancestor` and no `git checkout main` / `git merge --ff-only feat/<branch>`
- [ ] 6c.2 `docs/src/user-guide/supervisor.md` "Per-branch merge": match `assets/agent-skills/orchestrator.md` Step 2 (resolved branch names, the ancestor check before archive and `advanced-main`, no pipes on must-succeed steps, re-running the test command after an archive)
- [ ] 6c.3 `docs/src/user-guide/coordination.md` "Activity phrases" and "Declaring regions": lead with `broker.sh status --activity …` / `broker.sh intent … --region …`, with the curl body only as a wire-shape aside
- [ ] 6c.4 Move `WORKER_ROLE_LINE` in `src/agents.rs` to an asset under `assets/` loaded with `include_str!`, so the lang-agnostic audit covers it; output byte-identical
- [ ] 6c.5 `pointer_is_scoped_to_the_root_session` normalises whitespace before matching
- [ ] 6c.6 `docs/src/user-guide/doctor.md`: one sentence that workflow locations resolve by CLI family
- [ ] 6c.7 `assets/scripts/broker.sh` usage heredoc: align the `[--region …]` continuation line

## 7. Architecture docs (F9)

- [ ] 7.1 `docs/src/architecture-direction.md` "Where the code is today": add rows for the per-CLI Strategy and the spec-format Strategy; note in the Drive loop row that `submit_and_verify` is shared with `__send`
- [ ] 7.2 `docs/src/architecture.md`: keep its module descriptions consistent (`supervisor::screen`, the audits, warnings as values); add the usage facet to the Agent CLIs row and a `supervisor/drive/budget.rs` row (F17)

## 8. Quality gates

- [ ] 8.1 `cargo fmt`, then `just lint`
- [ ] 8.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 8.3 `just deny`
- [ ] 8.4 `mdbook build docs/`
- [ ] 8.5 `openspec validate architecture-conformance-fixes --strict`
