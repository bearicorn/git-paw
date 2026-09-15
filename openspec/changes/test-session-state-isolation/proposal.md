## Why

Integration tests are writing session state into the developer's real data directory, and
have been for weeks. `~/Library/Application Support/git-paw/sessions/` accumulated roughly
one hundred stale session JSONs — `paw-default-branch-fallback-existing-*`,
`paw-rebase-default-*`, `paw-rebase-no-rebase-*` — dated across every dev session since
2026-09-04, alongside the live `paw-git-paw.json`.

`core-ci-hygiene` already forbids this. *Tests invoking git paw subcommands SHALL override
HOME and XDG_DATA_HOME* requires every `assert_cmd::Command` that runs a `git paw`
subcommand to point `HOME` at a `TempDir` and remove `XDG_DATA_HOME`, and its audit
scenario asserts that every such builder in `tests/` either applies the pattern or is
explicitly documented as needing the real HOME — adding "no such case exists today and
none is expected".

Two test files violate it outright: `tests/default_branch_fallback_integration.rs` and
`tests/rebase_integration.rs` contain **zero** `env("HOME", …)` calls between them. Both
postdate the audit — they came in with the v0.16.0 launch-correctness work — so a
requirement that was satisfied when written has silently regressed, and nothing caught it.

The consequence is not cosmetic. These tests write, and can purge, session state in the
same directory that holds the developer's live session. The requirement's own scenario
demands the live `paw-git-paw.json` be byte-identical before and after a test run; with
these files that guarantee no longer holds.

## What Changes

- The two offending test files SHALL apply the HOME/XDG override pattern to every
  `git paw` subcommand invocation.
- The audit SHALL be re-run across `tests/` to catch any other call site added since the
  original pass.
- The requirement SHALL gain a **standing** guard rather than a one-time audit: a test
  that fails when any `assert_cmd` builder invoking the binary lacks the override, so the
  next file added cannot regress it silently. A one-time audit has now failed twice; the
  defect is that conformance was never mechanically enforced.
- The accumulated stale session files SHALL be documented as safe to delete, with the
  caveat that the live session's file must be preserved.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `core-ci-hygiene`: *Tests invoking git paw subcommands SHALL override HOME and
  XDG_DATA_HOME* gains a standing enforcement requirement in place of a one-time audit.

## Impact

- `tests/default_branch_fallback_integration.rs` — add the override to every `cmd()` call
  site; the file currently has none.
- `tests/rebase_integration.rs` — same.
- `tests/` — a new convention-enforcement test, in the style of the existing tmux and
  `sweep.sh` convention tests this capability already specifies, so the rule is checked by
  the suite rather than by a reviewer's memory.
- **No production code changes.** This is test hygiene; the binary's behaviour is correct.
- **Delta coordination**: `core-ci-hygiene` is not touched by any other in-flight v0.17.0
  change.
