## 1. Source audits (F1)

- [x] 1.1 Move `production_code` into `tests/helpers/source_scan.rs`, so a `#[cfg(test)]` inside a comment line no longer starts a blank-out; use it from every splitter in `tests/source_audit.rs` and from the command-library, terminal-port and surface-role audits
- [x] 1.2 Add the `let … SpecBackendKind::X` form to the spec-format pattern, with a caught-example line
- [x] 1.3 Tests for "Production code after an early test item is audited", "Test items and comments are not audited" and "main.rs is a positive control"
- [x] 1.4 Fix or allowlist (with a pointer) any spec-format offender the wider filter now finds
- [x] 1.5 Add the CLI-name audit with the D3 patterns and caught-example lines; tests for the four cli-provider scenarios

## 2. Menu shapes in the profile (F2)

- [x] 2.1 Add `menu_cursor_glyphs` and `menu_footer_markers` to `CliPromptProfile` and its override, with defaults, per-field fallback and the empty-marker check; update every struct-literal site
- [x] 2.2 Move `menu_showing`, `strip_decoration_keep_cursor`, `composer_text_is_dim`, `styled_cells` and `apply_sgr` with their unit tests to `src/supervisor/screen.rs`; read the glyphs and footers from the profile
- [x] 2.3 Tests for the four "A selection menu's shape comes from the profile" scenarios
- [x] 2.4 `docs/src/user-guide/cli-prompt-profiles.md` and the configuration reference: the two new fields and their defaults

## 3. Warnings as values (F3)

- [x] 3.1 `resolve_for_launch` stops printing; delete `first_warning_for` and its static (the warning stays the pure `unsandboxed_full_auto_warning`)
- [x] 3.2 `start.rs`, `launch.rs`, `recover.rs` and `supervisor.rs` emit each warning once per launch through `Output::warn` (`LaunchWarnings`)
- [x] 3.3 `AgentProfiles::record` returns its warning; the drive-loop shell prints it
- [x] 3.4 `locate`, `executable_in` and `ensure_unambiguous_in` take the working directory; only the process-environment wrappers read it
- [x] 3.5 Tests for "warns once per launch through the sink", "does not write the warning to stderr", "Two launches in one process each warn" and "A profile that fails to compile is reported to the caller"

## 4. One resolution and one dispatcher (F4, F7)

- [x] 4.1 `send::deliver` and `add.rs` read the prompt profile and settle delay from one `CliProfile`
- [x] 4.2 `KeyDispatcher` methods take `&self`; `SubmitPort` and drive-deps `dispatcher` become `&dyn KeyDispatcher`; delete `BackendDispatcher` and `SharedFakeDispatcher`; `RecordingDispatcher` uses a `RefCell`; `SharedFakeDispatcher` callers pass the fake itself
- [x] 4.3 Tests for "A send uses the backend as capturer and dispatcher" and "Drive-loop keystrokes are unchanged"

## 5. Styled capture pinned (F6)

- [x] 5.1 `FakeBackend` records `CaptureStyled`; update operation-sequence assertions that expected `CaptureTail` from `capture_styled`
- [x] 5.2 Golden argv test for `TmuxBackend::capture_styled`; tests for the two styled-read scenarios

## 6. Rebase-skip predicate (F5, conditional)

- [x] 6.1 `src/git.rs` `worktree_skip_reason` calls `crate::agents::is_managed_path`. Injecting the predicate would change `create_worktree` and its 31 call sites, so it is not small: the dependency is recorded in the architecture-direction status table (7.1) for the roadmap

## 6a. git-operations spec drift (F16)

- [x] 6a.1 Check that tests cover the reworded "Create worktrees as siblings of the repository" rebase scenarios (rebase in the branch's own worktree, its abort in that worktree, removal of a new worktree on failure); add any missing assertion
- [x] 6a.2 Test for "add keeps its notice text" asserting the exact notice string
- [x] 6a.3 `docs/src/user-guide/mcp.md`: `start_session` returns skipped-rebase notices in `warnings`

## 6b. worker-usage-budget follow-ups (F17)

- [x] 6b.1 Tests for "Headless orchestrator spend counts toward the breach", "An unevaluable cost cap alone leaves a finished wave completed" and "A CLI session git-paw did not launch is excluded"; reference the existing tests for "A cost cap is breached by reported cost alone" and "An unknown budget policy is rejected"
- [x] 6b.2 Comments: `src/commands/status.rs` → "Same formula as `budget_max_tokens` (`UsageTotals::tokens`), over current workers only"; `src/doctor.rs` → the doctor checks the configured CLIs, the loop checks the session's workers
- [x] 6b.3 `docs/src/user-guide/supervisor.md`: add the budget exit (`budget_on_exhausted = "abandon"` → `budget-exhausted`, non-zero) to the exit list, linking the budget section
- [x] 6b.4 `src/cli.rs` `start` long_about and `--unattended` help, and `docs/src/cli-reference.md`: one identical sentence on `budget_max_tokens` / `budget_max_cost` and `budget_on_exhausted = "abandon"`

## 6c. worker-friction-fixes hygiene (F18)

- [x] 6c.1 Test for the per-branch merge-loop scenario: the orchestrator skill's merge section has `git merge-base --is-ancestor` and no `git checkout main` / `git merge --ff-only feat/<branch>`
- [x] 6c.2 `docs/src/user-guide/supervisor.md` "Per-branch merge": match `assets/agent-skills/orchestrator.md` Step 2 (resolved branch names, the ancestor check before archive and `advanced-main`, no pipes on must-succeed steps, re-running the test command after an archive)
- [x] 6c.3 `docs/src/user-guide/coordination.md` "Activity phrases" and "Declaring regions": lead with `broker.sh status --activity …` / `broker.sh intent … --region …`, with the curl body only as a wire-shape aside
- [x] 6c.4 Move `WORKER_ROLE_LINE` in `src/agents.rs` to an asset under `assets/` loaded with `include_str!`, so the lang-agnostic audit covers it; output byte-identical
- [x] 6c.5 `pointer_is_scoped_to_the_root_session` normalises whitespace before matching
- [x] 6c.6 `docs/src/user-guide/doctor.md`: one sentence that workflow locations resolve by CLI family
- [x] 6c.7 `assets/scripts/broker.sh` usage heredoc: align the `[--region …]` continuation line

## 6d. readme-landing-page hygiene (F19, reduced)

- [x] 6d.1 Point the Highlights conflict-detection scenario and `highlights_mention_conflict_detection` at `conflict-detection.md`; add `every_highlights_bullet_links_a_page`
- [x] 6d.2 Reword "README-only facts move into the docs before removal" as steady-state SHALLs on `agents-md.md` and the `start --force` pages
- [x] 6d.3 Security note: "at most 6 non-blank lines", asserted in `security_note_names_the_trust_model_and_links_both_pages`
- [x] 6d.4 `core-configuration` specs-type requirement adds `"superpowers"`; test `the_specs_example_lists_every_specs_type_value`

## 6e. Review round 1

- [x] 6e.1 `resolve_for_launch` takes `cwd` and returns a `#[must_use]` `Resolved`; `start` warns as each CLI resolves; `locate` finds a slash command without a cwd
- [x] 6e.2 The shared scanner skips literals and comments; the in-crate escalation and decide audits use it; the dispatch audits flag every comparing or matching form
- [x] 6e.3 Empty menu lists fail the load; footers match with Unicode lowercasing
- [x] 6e.4 `Test:` pointers under every scenario in the delta files; three stale requirement titles renamed
- [x] 6e.5 F5 reason, `test_command` in the merge guide, and the misplaced scenario comment

## 7. Architecture docs (F9)

- [x] 7.1 `docs/src/architecture-direction.md` "Where the code is today": add rows for the per-CLI Strategy and the spec-format Strategy; note in the Drive loop row that `submit_and_verify` is shared with `__send`
- [x] 7.2 `docs/src/architecture.md`: keep its module descriptions consistent (`supervisor::screen`, the audits, warnings as values); add the usage facet to the Agent CLIs row and a `supervisor/drive/budget.rs` row (F17)

## 8. Quality gates

- [x] 8.1 `cargo fmt`, then `just lint`
- [x] 8.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [x] 8.3 `just deny`
- [x] 8.4 `mdbook build docs/`
- [x] 8.5 `openspec validate architecture-conformance-fixes --strict`
