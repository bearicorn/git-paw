## MODIFIED Requirements

### Requirement: Tests invoking git paw subcommands SHALL override HOME and XDG_DATA_HOME

Every integration test that constructs an `assert_cmd::Command` (via the test-helper `cmd()` function or equivalent) to run a `git paw` subcommand against the compiled binary SHALL configure the `Command` builder to:

- Set `HOME` to a `tempfile::TempDir`-rooted path that is unique to the test and whose lifetime exceeds the subprocess.
- Remove `XDG_DATA_HOME` via `env_remove("XDG_DATA_HOME")`.

The pattern SHALL be the existing one demonstrated by `tests/cli_tests.rs:278-279`:

```rust
let fake_home = TempDir::new().expect("home tempdir");
let output = cmd()
    .current_dir(repo_path)
    .env("HOME", fake_home.path())
    .env_remove("XDG_DATA_HOME")
    .args(["<subcommand>", ...])
    .output()
    .expect("run <subcommand>");
```

This ensures the subprocess resolves `crate::dirs::data_dir()` (and the sessions directory below it) into the test-owned `TempDir`, not into the user's real `~/Library/Application Support/git-paw/` (macOS) or `~/.local/share/git-paw/` (Linux).

The audit SHALL cover every `cmd().args(["<subcommand>", ...])` invocation in `tests/` and SHALL specifically fix `tests/e2e_tests.rs::broker_session_full_lifecycle` (around line 874 in the v0.5.0 baseline, the `["start", "--cli", "echo", "--branches", ...]` call), the matching `stop` and `purge --force` calls in the same test, and any other call sites the audit surfaces. Calls that already follow the pattern (e.g. `tests/cli_tests.rs::test_purge_no_unmerged_runs_without_warning`) SHALL be left as-is.

Conformance SHALL be enforced by a standing test rather than by a one-time audit. The suite SHALL fail when any `assert_cmd` builder in `tests/` that invokes the compiled binary lacks the override and is not accompanied by an explicit comment documenting why it needs the real `HOME`. A review-time audit is insufficient: the rule has regressed twice by way of new test files added after an audit passed, and a regression writes into the developer's real session directory where their live session state lives.

#### Scenario: `broker_session_full_lifecycle` does not write into the real sessions directory

- **GIVEN** a developer with `~/Library/Application Support/git-paw/sessions/` populated with the live `paw-git-paw.json` (and other real session files)
- **WHEN** `cargo test --test e2e_tests broker_session_full_lifecycle` runs
- **THEN** the test SHALL apply the HOME/XDG override pattern to every `cmd()` builder in the test (start, stop, purge)
- **AND** the test SHALL NOT create, modify, or delete any file inside the user's real `~/Library/Application Support/git-paw/sessions/`
- **AND** the live `paw-git-paw.json` file SHALL be byte-identical before and after the test runs

Test: maintainer-run smoke check during the change rollout — record the directory contents before/after the test runs. Documented in `tasks.md` as the rollout verification step.

#### Scenario: Every `git paw start` test in `tests/` applies the override

- **GIVEN** the v0.5.0 source tree after this change is applied
- **WHEN** the audit step in `tasks.md` is complete
- **THEN** every `assert_cmd::Command` builder in `tests/*.rs` that invokes a `git paw` subcommand SHALL either (a) have `.env("HOME", <tempdir>)` and `.env_remove("XDG_DATA_HOME")` applied, or (b) be explicitly documented in a code comment as needing the user's real HOME (no such case exists today and none is expected)
- **AND** the audit's grep query `grep -n 'cmd()\|args(\["start"\|args(\["stop"\|args(\["purge"' tests/*.rs` SHALL be cross-checked against the test bodies to confirm

Test: covered by `cargo test --test e2e_tests` plus the spec-audit grep documented in `tasks.md` step 3.

#### Scenario: A test file without the override fails the suite

- **GIVEN** a test file in `tests/` containing an `assert_cmd` builder that invokes the compiled binary without setting `HOME` to a tempdir and removing `XDG_DATA_HOME`
- **AND** no comment documenting that it requires the real `HOME`
- **WHEN** the suite runs
- **THEN** the convention-enforcement test SHALL fail
- **AND** its failure message SHALL name the offending file

#### Scenario: Previously-regressed files conform

- **GIVEN** `tests/default_branch_fallback_integration.rs` and `tests/rebase_integration.rs`
- **WHEN** the convention-enforcement test runs
- **THEN** it SHALL pass for both files
- **AND** running their tests SHALL create no file under the user's real platform data directory

#### Scenario: A documented exception is permitted

- **GIVEN** an `assert_cmd` builder accompanied by a comment documenting that it requires the real `HOME`
- **WHEN** the convention-enforcement test runs
- **THEN** that builder SHALL NOT cause a failure
