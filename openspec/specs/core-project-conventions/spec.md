# core-project-conventions Specification

## Purpose
Repo-wide contributor conventions and the doc accuracy they demand: the `AGENTS.md` approved-dependencies table stays in sync with `[dependencies]`/`[dev-dependencies]` in `Cargo.toml` (including `dirs` recorded as an intentional exclusion), the commit-scope enumeration and its compound-scope form, the mdBook architecture chapter's module list and its changelog include, and the README Features section's enumeration of the current user-facing surface — so contributors can trust the project docs to match the shipped code.
## Requirements
### Requirement: README features section enumerates the current user-facing surface

`README.md`'s Features section SHALL include entries describing
every user-facing capability so users can
discover the feature set from a single page. The Features list
SHALL include (at minimum) entries for: `--specs-format`,
`--no-supervisor`, `start --force`, the Spec Kit backend
(`[specs] type = "speckit"`), the `[governance]` config table,
the `[supervisor.conflict]` config table, the
`[supervisor.learnings_config]` config table, `agent.intent`
forward coordination, automatic conflict detection
(forward / in-flight / ownership), and learnings mode (the
`.git-paw/session-learnings.md` output file). The Features list
SHALL NOT present automatic approval of permission prompts as a
feature.

#### Scenario: README Features section mentions Spec Kit

- **WHEN** the README's Features section is inspected
- **THEN** it contains the substring `Spec Kit` (case-insensitive)
- **AND** it contains the substring `speckit` (the TOML value)

#### Scenario: README Features section mentions forward coordination and conflict detection

- **WHEN** the README's Features section is inspected
- **THEN** it contains the substring `agent.intent`
- **AND** it contains the substring `conflict detection` (case-insensitive)

#### Scenario: README Features section mentions learnings mode

- **WHEN** the README's Features section is inspected
- **THEN** it contains the substring `learnings` (case-insensitive)
- **AND** it contains the substring `.git-paw/session-learnings.md`

#### Scenario: README Features section mentions every CLI flag

- **WHEN** the README's Features section is inspected
- **THEN** it contains the substring `--specs-format`
- **AND** it contains the substring `--no-supervisor`
- **AND** it contains the substring `--force` (in a `start --force` context)

#### Scenario: README Features section does not advertise auto-approval

- **WHEN** the README's Features section is inspected
- **THEN** it SHALL NOT contain `[supervisor.auto_approve]`
- **AND** it SHALL NOT describe git-paw as auto-approving permission prompts

### Requirement: AGENTS.md dependency table matches `Cargo.toml`

`AGENTS.md`'s approved-dependencies table SHALL list every
production dependency declared in `[dependencies]` of `Cargo.toml`
at archive time and every dev dependency declared in
`[dev-dependencies]`. The production additions SHALL be
present: `schemars`, `chrono`, and `regex` (with
appropriate version suffixes matching the manifest). A crate removed
from `Cargo.toml` SHALL be removed from the table in the same change.

#### Scenario: AGENTS.md table lists prod dependencies

- **WHEN** the AGENTS.md Dependencies table is inspected
- **THEN** it contains the substring `schemars`
- **AND** it contains the substring `chrono`
- **AND** it contains the substring `regex`

#### Scenario: A removed dependency leaves the table

- **GIVEN** `serde_yaml` is not declared in `Cargo.toml`
- **WHEN** the AGENTS.md approved-dependencies table is inspected
- **THEN** it SHALL NOT contain a row for `serde_yaml`

### Requirement: AGENTS.md documents the `dirs` crate removal

`AGENTS.md` SHALL NOT list the upstream `dirs` crate as an
approved production dependency. Instead, AGENTS.md SHALL include a
paragraph under the Dependencies section explaining that the
`dirs` crate was removed because its transitive license
chain fails `just deny`, and that the project now uses an
in-tree `src/dirs.rs` module for platform XDG paths. The
paragraph SHALL instruct future contributors NOT to re-add the
`dirs` crate.

#### Scenario: AGENTS.md does not list dirs as an approved dep

- **WHEN** the AGENTS.md Dependencies table is inspected
- **THEN** the table does NOT contain `dirs` as an approved
  production dependency row

#### Scenario: AGENTS.md explains the dirs swap

- **WHEN** the AGENTS.md Dependencies section is inspected
- **THEN** it contains text describing the `dirs` crate's removal
  and its replacement by `src/dirs.rs`
- **AND** it instructs contributors not to re-add the `dirs` crate

### Requirement: AGENTS.md scopes list covers the scopes and compound forms

`AGENTS.md`'s Commit Conventions section's scope enumeration SHALL include the scopes used in shipped commits. At minimum the scope list SHALL include: `user-guide`, `worktree`, `governance`, `learnings`, and `pause`. The section SHALL explicitly document compound scopes of the form `<a>,<b>,<c>` (comma-separated scopes inside the parentheses) as permitted.

#### Scenario: AGENTS.md scopes list mentions the scopes

- **WHEN** the AGENTS.md Commit Conventions scope list is inspected
- **THEN** it contains the substring `user-guide`
- **AND** it contains the substring `worktree`
- **AND** it contains the substring `governance`
- **AND** it contains the substring `learnings`

#### Scenario: AGENTS.md documents compound scopes

- **WHEN** the AGENTS.md Commit Conventions section is inspected
- **THEN** it contains an example or explicit statement permitting
  compound scope form `(<a>,<b>,...)`
- **AND** the example uses commas (no whitespace between scopes is
  required, but the comma separator is explicit)

### Requirement: mdBook architecture chapter has an accurate module list

`docs/src/architecture.md` SHALL describe every Rust module that
ships in `src/` at archive time. The chapter SHALL NOT reference
modules that do not exist in `src/`. Specifically, the chapter
SHALL NOT contain the substrings `src/broker/state.rs` or
`src/broker/flush.rs` (modules that were referenced in v0.4 docs
but do not exist in the current source tree). The chapter SHALL
include references to the module additions
(`src/supervisor/`, `src/broker/conflict.rs`,
`src/broker/learnings.rs`, `src/broker/watcher.rs`,
`src/broker/delivery.rs`, `src/broker/publish.rs`,
`src/specs/resolve.rs`, `src/specs/speckit.rs`).

#### Scenario: Architecture chapter does NOT reference nonexistent broker modules

- **WHEN** `docs/src/architecture.md` is inspected
- **THEN** it does NOT contain the substring `src/broker/state.rs`
- **AND** it does NOT contain the substring `src/broker/flush.rs`

#### Scenario: Architecture chapter references the module additions

- **WHEN** `docs/src/architecture.md` is inspected
- **THEN** it contains the substring `src/broker/conflict.rs`
- **AND** it contains the substring `src/broker/learnings.rs`
- **AND** it contains the substring `src/specs/speckit.rs`
- **AND** it contains a reference to the `src/supervisor/` subtree

### Requirement: mdBook changelog chapter includes the project changelog

`docs/src/changelog.md` SHALL render the contents of the
project's root `CHANGELOG.md` rather than maintaining a separate
copy. The chapter file SHALL contain the mdBook
`{{#include ../../CHANGELOG.md}}` directive (or equivalent
preprocessor directive) so that future `git cliff` regenerations
of `CHANGELOG.md` automatically flow through to the rendered
mdBook output.

#### Scenario: Changelog chapter is an include of the root CHANGELOG.md

- **WHEN** `docs/src/changelog.md` is inspected
- **THEN** it contains the substring `{{#include ../../CHANGELOG.md}}`

#### Scenario: Changelog chapter does NOT hand-maintain `[Unreleased]` content

- **WHEN** `docs/src/changelog.md` is inspected
- **THEN** it does NOT contain a hand-maintained `[Unreleased]` section header (the rendered output is sourced entirely from the included file)

### Requirement: AGENTS.md dependency table SHALL list the dependencies and note `dirs` as intentionally absent

The top-level `AGENTS.md` "Dependencies" table SHALL include rows for `schemars`, `chrono`, and `regex`. Each row's "Purpose" cell SHALL be one sentence (e.g. `schemars` → "JSON Schema derivation for governance config validation").

The `dirs` row SHALL be moved out of the approved-dependencies table and into a "Notable exclusions" sub-section beneath the table. The exclusion entry SHALL state: "Replaced by a homegrown `src/dirs.rs` because the upstream crate's license is not FOSS-compatible. Do not re-add."

The "Notable exclusions" sub-section SHALL also carry a `serde_yaml` entry stating that it was replaced by a dependency-free skill-frontmatter parser because the crate is deprecated upstream and depends on the unmaintained `unsafe-libyaml` (RUSTSEC-2024-0449), and that it SHALL NOT be re-added.

#### Scenario: AGENTS.md lists schemars in dependency table

- **WHEN** `AGENTS.md` is read
- **THEN** the dependencies table SHALL contain a row with `schemars` in the Crate column
- **AND** the Purpose column for that row SHALL mention JSON Schema or governance config

#### Scenario: AGENTS.md lists chrono and regex

- **WHEN** the dependencies table is inspected
- **THEN** rows for `chrono` and `regex` SHALL exist
- **AND** each row's Purpose cell SHALL be non-empty

#### Scenario: AGENTS.md notes `dirs` as intentionally absent

- **WHEN** `AGENTS.md` is read
- **THEN** the `dirs` row SHALL NOT appear in the approved-dependencies table
- **AND** a separate "Notable exclusions" section SHALL describe `dirs` as replaced by `src/dirs.rs` due to a non-FOSS license

#### Scenario: AGENTS.md notes `serde_yaml` as intentionally absent

- **WHEN** `AGENTS.md` is read
- **THEN** no `serde_yaml` row SHALL appear in the approved-dependencies table
- **AND** the "Notable exclusions" section SHALL describe `serde_yaml` as replaced by a dependency-free frontmatter parser, citing its deprecation and the unmaintained `unsafe-libyaml` advisory

### Requirement: AGENTS.md commit-conventions scopes SHALL include the scope names and document the compound-scope form

The "Commit Conventions" section of `AGENTS.md` SHALL list the following scope names in addition to the v0.4 set: `user-guide`, `worktree`, `governance`, `learnings`, `pause`. The section SHALL also document the compound-scope syntax `(scope1,scope2,...)` with at least one inline example (e.g. `feat(cli,config): ...`).

#### Scenario: Scopes line includes the scope names

- **WHEN** the AGENTS.md Scopes line is inspected
- **THEN** it SHALL contain each of `user-guide`, `worktree`, `governance`, `learnings`, `pause` (as inline-code-style backtick names)

#### Scenario: Compound-scope syntax is documented

- **WHEN** the Commit Conventions section is inspected
- **THEN** there SHALL be at least one example using compound-scope syntax `(scope1,scope2)`
- **AND** the section SHALL state that compound scopes are permitted when a commit legitimately touches more than one scope

### Requirement: Non-test code does not call unwrap or expect

Non-test code in the `git-paw` library and binary crates SHALL NOT call `unwrap()`, `expect()`, `unwrap_err()`, or `expect_err()` on an `Option` or `Result`, except inside the single static-regex helper defined by *Static regular expressions compile through one audited helper*. A recoverable failure SHALL propagate with `?` (or `ok_or_else` / `map_err`) into a `PawError` variant; a state that cannot occur SHALL be made unrepresentable by construction rather than asserted.

The rule SHALL be lint-enforced: the library crate root (`src/lib.rs`) and the binary crate root (`src/main.rs`) SHALL enable `clippy::unwrap_used` and `clippy::expect_used`, and the repository `clippy.toml` SHALL set `allow-unwrap-in-tests = true` and `allow-expect-in-tests = true`, so `just lint` (`cargo clippy --all-targets -- -D warnings`) fails on a violation in library or binary code while `#[test]` functions and `#[cfg(test)]` modules remain exempt. Integration-test crates under `tests/` are outside the rule.

The gate has two known limits, and neither is a licence to violate the rule. Platform-gated code (`#[cfg(target_os = ..)]`) is linted only on a host where that `cfg` is active, and CI lints on Linux only, so a violation inside a macOS-only block is not caught by the gate. A new `#[allow(clippy::unwrap_used)]` / `#[allow(clippy::expect_used)]` anywhere in `src/` silences the lint by construction; the suppression set is therefore pinned by a test rather than by the gate.

#### Scenario: The lint configuration enforces the rule on both crate roots

- **WHEN** `src/lib.rs`, `src/main.rs`, and `clippy.toml` are inspected
- **THEN** both crate roots SHALL carry a crate-level attribute enabling `clippy::unwrap_used` and `clippy::expect_used` at `warn` or stricter
- **AND** `clippy.toml` SHALL set `allow-unwrap-in-tests = true` and `allow-expect-in-tests = true`

Test: `tests/production_panic_free.rs::crate_roots_enable_unwrap_and_expect_lints`, `tests/production_panic_free.rs::clippy_toml_exempts_test_code`

#### Scenario: A new expect in non-test code fails the lint gate

- **GIVEN** a non-test function in `src/` that calls `.expect(..)` on an `Option`
- **WHEN** `just lint` runs
- **THEN** it SHALL fail with a `clippy::expect_used` error naming that call site

Test: the `just lint` gate (verified at implementation by temporarily introducing a violation and observing a non-zero exit); configuration pinned by `tests/production_panic_free.rs::crate_roots_enable_unwrap_and_expect_lints`

#### Scenario: The current tree passes the lint gate

- **WHEN** `just lint` runs on the tree after this change
- **THEN** it SHALL exit `0` with no `clippy::unwrap_used` or `clippy::expect_used` finding, while unit tests that call `unwrap()` / `expect()` still compile and run

Test: the `just lint` gate; `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`

#### Scenario: `git paw add` without a branch or spec errors instead of panicking

- **WHEN** `git paw add` is invoked inside a repository with neither a branch argument nor `--from-spec`
- **THEN** the process SHALL exit `2` with clap's missing-argument usage error on stderr
- **AND** stderr SHALL NOT contain `panicked`

Test: `tests/production_panic_free.rs::add_without_branch_or_spec_errors_without_panic`. The test pins the observable contract, which clap's `required_unless_present` already satisfies; the `ok_or_else` in `cmd_add` is unreachable defence in depth should that attribute drift, and is not separately covered (an in-process call returns the no-active-session error before reaching it).

#### Scenario: `git paw replay` without a branch or `--list` errors instead of panicking

- **WHEN** `git paw replay` is invoked inside a repository with neither a branch argument nor `--list`
- **THEN** the process SHALL exit `2` with clap's missing-argument usage error on stderr
- **AND** stderr SHALL NOT contain `panicked`

Test: `tests/production_panic_free.rs::replay_without_branch_or_list_errors_without_panic`. As above, the `ok_or_else` in `cmd_replay` is unreachable defence in depth behind clap and is not separately covered.

### Requirement: Static regular expressions compile through one audited helper

Every regular expression compiled in the library crate from a pattern fixed at build time — a string literal, or an embedded default such as the built-in Claude Code prompt profile's patterns — SHALL be compiled through a single crate-private helper whose pattern parameter is `&'static str`. The helper is private to the library crate; the binary crate (`src/main.rs`, `src/commands/`) compiles no regexes, and SHALL gain access through a re-export rather than its own `expect` if it ever needs one. That helper SHALL be the only non-test code that calls `expect()`, under a lint suppression scoped to that one function and a comment stating why the failure is unreachable at runtime. Every pattern passed to the helper SHALL be compiled by at least one unit test, so an invalid static pattern fails `cargo test` instead of panicking in a shipped binary. A pattern supplied at runtime (configuration, user, or agent input) SHALL NOT be passed to the helper; it SHALL be compiled with its error propagated.

#### Scenario: Every static pattern compiles under test

- **GIVEN** a module that compiles a static pattern through the helper
- **WHEN** `cargo test` runs
- **THEN** a unit test in that module SHALL force each of its static regexes to compile and SHALL pass

Test: `agents::tests::static_regexes_compile`, `broker::server::tests::static_regexes_compile`, `opsx::role_guard::tests::static_regexes_compile`, `specs::speckit::tests::static_regexes_compile`, `specs::superpowers::tests::static_regexes_compile`, `supervisor::permission_prompt::tests::static_regexes_compile`, `supervisor::drive::tests::static_regexes_compile`

#### Scenario: The helper is the only expect in non-test code

- **WHEN** non-test code in `src/` is searched for `.expect(` / `.unwrap(` calls on `Option` or `Result`
- **THEN** the only match SHALL be inside the static-regex helper, and it SHALL carry a function-scoped `#[allow(clippy::expect_used)]`

Test: `tests/production_panic_free.rs::static_regex_helper_is_the_only_panic_lint_suppression` (the `just lint` gate catches an unsuppressed call site, but cannot see through an `#[allow]` — the suppression set needs its own test); `static_regex::tests::compiles_a_static_pattern`

