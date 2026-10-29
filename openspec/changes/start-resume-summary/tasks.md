## 1. Seams (behaviour-preserving)

- [ ] 1.1 Extract `resume_mode(entry, cli_profile) -> Resume` from `recover.rs::cli_command_for_recovery` and make the launch command use it; recovery tests unchanged and green
- [ ] 1.2 Extract the duplicated resume-decision block (start / start-with-specs / supervisor) into one helper returning which path was taken (no output change)
- [ ] 1.3 `src/git.rs`: `merge_base_with`, `diff_shortstat_with` (parse `N files changed, X insertions(+), Y deletions(-)`, all parts optional) through `CommandRunner`; sandbox-repo unit tests

## 2. Summary

- [ ] 2.1 `ResumeFacts` probe (worktree existence, `uncommitted_files` minus managed paths, `unmerged_commit_count`, shortstat) tolerant of per-field failure
- [ ] 2.2 Pure `render_resume_summary(&[ResumeFacts]) -> String`; unit tests for each resume mode, missing worktree, unknown fields
- [ ] 2.3 Print it on the paused / stopped / active paths before `attach_or_print_hint` / reattach; expose it in the start outcome type for the library surface

## 3. Tests per scenario

- [ ] 3.1 E2E with real repos + tmux (`tests/`, isolated `HOME`/`XDG_DATA_HOME`): recovered session lines (session id vs fresh, counts); paused restart prints summary; missing worktree flagged; non-TTY ordering before "Attach with"; fresh launch prints none
- [ ] 3.2 Reattach path test (TTY-gated helper tested through the extracted decision helper with a fake terminal)

## 4. Docs

- [ ] 4.1 `docs/src/user-guide/session-lifecycle.md` (resuming: what the summary shows), `docs/src/cli-reference.md` (`start` on an existing session)

## 5. Gates

- [ ] 5.1 `cargo fmt`; `just check`; `just deny`; `mdbook build docs/`
