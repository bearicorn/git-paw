## 1. Characterise the defect

- [ ] 1.1 Add a fixture for a 3-option prompt whose option 2 reads "Yes, and don't ask again for similar commands in `<path>`" over a safe-classified command; assert `__classify --resolve-option` returns `1` — this SHALL fail before the fix
- [ ] 1.2 Add the narrow-grant counterpart fixture ("don't ask again for: `<exact command>`") asserting it still returns `2`
- [ ] 1.3 Add the `timeout 1 bash /tmp/script.sh` fixture from the dogfood, asserting `1` — it is not a `-c` code string, so the existing arbitrary-code rule does not cover it
- [ ] 1.4 Add a compound-command fixture (`cargo test … ; echo … ; cargo test …`) whose only permanent option is scope-wide, asserting `1`

## 2. Classify grant scope

- [ ] 2.1 Add a grant-scope determination in `src/supervisor/auto_approve.rs` that reads a permanent-grant option's text and resolves it to specific-command / scope-wide / undeterminable
- [ ] 2.2 Treat undeterminable as scope-wide (fail closed); document the reasoning in a doc comment
- [ ] 2.3 Keep the determination out of `cli-prompt-profiles` — it is a security decision and that capability forbids profile-sourcing them. Add a comment at the rule site recording why, so a later refactor does not "tidy" it into the profile

## 3. Gate the broad grant on scope

- [ ] 3.1 Add the scope condition to the broad-grant rule as a condition independent of classification and of the arbitrary-code carve-out
- [ ] 3.2 Make the fallback the one-time option, not an escalation — assert no escalation is published for a safe command whose only permanent option is scope-wide
- [ ] 3.3 Verify the existing broad-grant scenarios (allowlisted verb, safe stack command, `python -c`, `bash -c`, unclassified) still behave identically

## 4. Helper agreement

- [ ] 4.1 Confirm `sweep.sh approve` inherits the fix through `__classify --resolve-option` and needs no independent rule
- [ ] 4.2 Inspect `assets/scripts/sweep.sh` for any local assumption that option 2 is the broad grant; fix it and refresh the repo-root `.git-paw/scripts/sweep.sh` copy (gitignored, written by `git paw init`) if so, running `bash -n` on both
- [ ] 4.3 Add a parity assertion that the helper and the in-tool approver resolve the same option for a scope-wide-grant capture

## 4b. Managed-script trust (exact location)

- [ ] 4b.1 Failing tests first, one per scenario in `specs/approval-command-safety/spec.md`: `bash .git-paw/../../tmp/sweep.sh`, `bash .git-paw/other/sweep.sh` and `bash .git-paw/tmp/sweep.sh` do NOT classify safe (all classify `safe` with the `managed-script` label today); `bash .git-paw/scripts/sweep.sh status`, `.git-paw/scripts/broker.sh --agent feat-x status booting` and `.git-paw/scripts/sweep.sh status-publish` still do; the chained-danger scenario still escalates
- [ ] 4b.2 In `src/supervisor/auto_approve.rs` `is_managed_script_path`: refuse any token with a `..` component; normalise `.` components; with a worktree root, resolve relative tokens against it (symlinks resolved when the file exists) and require the result to equal `<worktree>/.git-paw/scripts/<name>`; with no worktree root keep requiring the literal relative `.git-paw/scripts/<name>`. Stop delegating the decision to `agents::is_managed_path` (leave that function unchanged — `remove` depends on its subtree-wide answer) and update the doc comments of `is_managed_script_path` / `is_managed_script_invocation`
- [ ] 4b.3 (SHOULD) Compare the invoked file's bytes with the embedded asset (`src/init.rs` `BROKER_SCRIPT` / `SWEEP_SCRIPT` / `DOCS_FETCH_SCRIPT`); on mismatch the managed-script rule does not match. Add the "A modified helper is not trusted" test. If deferred, record the deferral and the reason in this change's design.md before archive
- [ ] 4b.4 Add the 4b.1 captures to `tests/classify_subcommand.rs` so `git paw __classify` (and therefore `sweep.sh classify`) agrees with the in-tool classifier

## 5. Spec traceability and security review

- [ ] 5.1 Verify every scenario in `specs/approval-auto/spec.md` and `specs/approval-command-safety/spec.md` maps to at least one test (Gate 3)
- [ ] 5.2 Run the `security-and-safety-review` dev skill against this change — it alters what is auto-approved and is the blast-radius control for agent execution
- [ ] 5.3 Confirm by inspection that no code path can select a permanent grant the previous implementation would have refused (this change must only narrow)

## 6. Docs and gates

- [ ] 6.1 Update the auto-approve / unattended documentation to state that permanent grants are taken only for a specific command, never for a directory or command class
- [ ] 6.2 Note in the docs that commands already granted directory-wide by earlier versions are not revoked by this change
- [ ] 6.3 `cargo fmt`, then `just lint`
- [ ] 6.4 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code rather than piped output
- [ ] 6.5 `openspec validate broad-grant-scope-safety --strict`
