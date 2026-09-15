## 1. Fix the two regressed files

- [ ] 1.1 Add the HOME/XDG override to every `assert_cmd` builder in `tests/default_branch_fallback_integration.rs`, following the existing pattern (`TempDir`-rooted `HOME` outliving the subprocess, `env_remove("XDG_DATA_HOME")`)
- [ ] 1.2 Do the same for `tests/rebase_integration.rs`
- [ ] 1.3 Confirm each file's `TempDir` lifetime exceeds the subprocess it is passed to — a dropped tempdir silently restores the real HOME

## 2. Re-run the audit

- [ ] 2.1 Grep `tests/` for every `assert_cmd` builder invoking the compiled binary and cross-check each against the override pattern
- [ ] 2.2 Fix any further call site the audit surfaces, or document it with a comment explaining why it needs the real `HOME`
- [ ] 2.3 Record the audit's result so the standing test's starting state is known-clean

## 3. Standing enforcement

- [ ] 3.1 Add a convention-enforcement test, in the style of this capability's existing tmux and `sweep.sh` convention tests, that scans `tests/` and fails when a builder invoking the binary lacks the override
- [ ] 3.2 Recognise the per-file `cmd()` helper pattern so a file that centralises the override is not flagged at every call site
- [ ] 3.3 Permit a documented exception via an accompanying comment; assert that no such exception exists today
- [ ] 3.4 Make the failure message name the offending file and the construction it matched
- [ ] 3.5 Verify the test fails when the override is temporarily removed from one file, then passes once restored

## 4. Verify the harm is gone

- [ ] 4.1 Record the contents of the real platform data directory's `git-paw/sessions/`, run the two previously-offending test files, and confirm no new file appeared
- [ ] 4.2 Confirm the live session's JSON is byte-identical before and after, per the requirement's existing scenario

## 5. Cleanup guidance

- [ ] 5.1 Document how to remove the accumulated stale session files, with an explicit warning to preserve the active session's file; prefer a targeted glob over a wildcard
- [ ] 5.2 State plainly that this change prevents further accumulation and does not recover any session state an earlier run may already have mutated

## 6. Gates

- [ ] 6.1 `cargo fmt`, then `just lint`
- [ ] 6.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code rather than piped output
- [ ] 6.3 `openspec validate test-session-state-isolation --strict`
