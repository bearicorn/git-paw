## Context

`core-ci-hygiene` has required HOME/XDG isolation for `git paw` subcommand tests since
v0.5.0. The requirement was satisfied by a one-time audit across `tests/`, and its own
scenario records the outcome: every builder either applies the pattern or is documented as
needing the real HOME, "no such case exists today and none is expected".

It has regressed. `tests/default_branch_fallback_integration.rs` and
`tests/rebase_integration.rs` contain no `env("HOME", …)` at all. Both arrived with the
v0.16.0 launch-correctness work, after the audit. Their temp project names explain the
litter directly: `default-branch-fallback-{tag}-{pid}-{n}` produces
`paw-default-branch-fallback-existing-13321-0.json`, and the rebase file's names produce
`paw-rebase-default-*` and `paw-rebase-no-rebase-*`. Roughly a hundred such files sit in
the real data directory, oldest 2026-09-04.

They share that directory with the developer's live `paw-git-paw.json`. These tests call
`start`, `stop` and `purge`; a purge that enumerates the directory is operating on real
session state.

## Goals / Non-Goals

**Goals:**

- Fix the two files.
- Make the rule enforced by the suite instead of by a reviewer noticing.
- Leave the accumulated litter safely removable, without touching the live session's file.

**Non-Goals:**

- Changing where session state lives. XDG is correct;
  `bundled-helper-discovery` makes that location authoritative.
- Auto-deleting the stale files as part of the test run or the build. Tests that clean up
  a shared user directory are how this class of damage happens.
- Auditing non-`git paw` subprocess tests. The requirement is scoped to builders invoking
  the compiled binary.

## Decisions

**Replace the one-time audit with a standing test.** This is the whole point. The
requirement did not fail because anyone disagreed with it — it failed because conformance
depended on every future contributor remembering a rule that is invisible at the call
site. A convention-enforcement test is the instrument this capability already uses for
tmux sockets and for `sweep.sh`; extending the pattern here is consistent rather than
novel.

**Allow a documented exception rather than an absolute ban.** An absolute rule invites a
workaround when someone genuinely needs the real HOME. Requiring a comment makes the
exception visible, greppable, and reviewable — and today there are none, so the test
starts clean.

**Scan the test sources rather than observing the filesystem.** The alternative — asserting
after a run that nothing appeared in the real directory — would be a more direct test of
the actual harm, but it is racy with other tests, depends on the developer's environment,
and cannot name the offending file. A source scan fails fast, names the file, and runs
anywhere.

**Do not delete the stale files automatically.** They are in the user's data directory
alongside live state. Document the cleanup and let a human run it. A test or build step
that removes files from a shared user directory is precisely the failure mode this change
exists to prevent.

## Risks / Trade-offs

- **The source scan is heuristic.** Matching `assert_cmd` builders textually will have
  false negatives (an unusual construction) and false positives (a helper that applies the
  override indirectly). Prefer false positives — a spurious failure is cheap to annotate,
  a missed one writes into the user's real directory. The existing `cmd()` helper pattern
  in these files makes the common case easy to detect.
- **Helper-indirection.** If a file centralises the override inside its own `cmd()`
  helper, a naive scan flags every call site. The check should recognise the helper
  pattern, which is how the conforming files are already written.
- **Cleanup guidance can be misapplied.** Telling someone to delete
  `~/Library/Application Support/git-paw/sessions/*` would destroy their live session.
  The instruction must be explicit about preserving the active session's file, and should
  prefer a targeted glob over a wildcard.
- **This does not recover what was already overwritten.** If an earlier test run purged or
  mutated a live session file, that damage is done and undetectable now. Worth stating
  plainly rather than implying the fix is retroactive.
