# core-project-conventions Specification

## Purpose
Repo-wide contributor conventions and the doc accuracy they demand: the `AGENTS.md` approved-dependencies table stays in sync with `[dependencies]`/`[dev-dependencies]` in `Cargo.toml` (including `dirs` recorded as an intentional exclusion), the commit-scope enumeration and its compound-scope form, the mdBook architecture chapter's module list and its changelog include, and the README Features section's enumeration of the current user-facing surface — so contributors can trust the project docs to match the shipped code.

## Requirements

### Requirement: README Highlights section enumerates the current user-facing surface

`README.md`'s `## Highlights` section SHALL give one line per capability family, each linking the docs page that covers it, so users can discover the feature set from a single page. Highlights SHALL cover spec-driven launch (naming Spec Kit), the broker with conflict detection, and learnings mode, and the linked pages SHALL hold the detail. The README SHALL NOT present automatic approval of permission prompts as a feature.

#### Scenario: README Features section mentions Spec Kit

- **WHEN** the README's Highlights section is inspected
- **THEN** it contains the substring `Spec Kit` (case-insensitive) and links `spec-driven-launch.md`
- **AND** `docs/src/user-guide/spec-driven-launch.md` contains the substring `speckit` (the TOML value)

Test: `tests/readme_features.rs::highlights_mention_spec_kit_and_link_its_page`

#### Scenario: README Features section mentions forward coordination and conflict detection

- **WHEN** the README's Highlights section is inspected
- **THEN** it contains the substring `conflict detection` (case-insensitive) and links `conflict-detection.md`
- **AND** `docs/src/user-guide/conflict-detection.md` contains the substring `agent.intent`

Test: `tests/readme_features.rs::highlights_mention_conflict_detection`

#### Scenario: README Features section mentions learnings mode

- **WHEN** the README's Highlights section is inspected
- **THEN** it contains the substring `learnings` (case-insensitive) and links `learnings.md`
- **AND** `docs/src/user-guide/learnings.md` contains the substring `.git-paw/session-learnings.md`

Test: `tests/readme_features.rs::highlights_mention_learnings_and_link_its_page`

#### Scenario: README Features section mentions every CLI flag

- **WHEN** the README's Commands section is inspected
- **THEN** it links `cli-reference.md`
- **AND** `docs/src/cli-reference.md` contains the substrings `--specs-format`, `--no-supervisor` and `--force` (in a `start --force` context)

Test: `tests/readme_features.rs::commands_link_the_cli_reference_which_names_every_flag`

#### Scenario: README Features section does not advertise auto-approval

- **WHEN** the README's Highlights section is inspected
- **THEN** it SHALL NOT contain `[supervisor.auto_approve]`
- **AND** it SHALL NOT describe git-paw as auto-approving permission prompts

Test: `tests/readme_features.rs::highlights_do_not_advertise_auto_approval`

#### Scenario: Every Highlights bullet links a docs page

- **WHEN** the README's Highlights section is inspected
- **THEN** every bullet SHALL contain a Markdown link (`](`)

Test: `tests/readme_features.rs::every_highlights_bullet_links_a_page`

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

### Requirement: README is a short landing page that links the docs

`README.md` SHALL be a landing page of at most 130 lines. It SHALL contain, in this order: title, tagline and badges; What it does; Demo; Highlights; Install; Quick start; Commands; Configuration; Supported CLIs; Security; Documentation; Contributing; License. Detail SHALL live in `docs/src/` and be linked rather than repeated, and the README SHALL NOT carry release-procedure or contributor-only build detail.

#### Scenario: The README fits the landing-page budget

- **WHEN** `README.md` is read
- **THEN** it SHALL have at most 130 lines
- **AND** its `## ` headings SHALL appear in the order above

#### Scenario: The quick start links the three quick-start pages

- **WHEN** the README's Quick start section is inspected
- **THEN** it SHALL link `quick-start-same-cli.md`, `quick-start-per-branch.md` and `quick-start-orchestrator.md`
- **AND** it SHALL show `git paw init`, `git paw start --supervisor` and `git paw start --unattended --from-all-specs`

#### Scenario: The command table carries no version tags and links the reference

- **WHEN** the README's Commands table is inspected
- **THEN** no cell SHALL contain a `(v0.` version tag
- **AND** the section SHALL link `cli-reference.md`

#### Scenario: Superseded content is gone

- **WHEN** `README.md` is read
- **THEN** it SHALL NOT contain `show_message_log`, a `## Releases` section, or a worktree diagram that places agent worktrees only at `../`

### Requirement: README-only facts move into the docs before removal

`docs/src/user-guide/agents-md.md` SHALL document the `CLAUDE.md` import of `AGENTS.md`, and `docs/src/cli-reference.md` and `docs/src/user-guide/spec-driven-launch.md` SHALL state that `start --force` has no effect under supervisor mode, so the README can link these facts instead of stating them.

#### Scenario: The CLAUDE.md import tip lives in the AGENTS.md guide

- **WHEN** `docs/src/user-guide/agents-md.md` is inspected
- **THEN** it SHALL contain `@AGENTS.md` as the first line of a `CLAUDE.md`, explain why the import beats a symlink, and show `ln -s AGENTS.md GEMINI.md` for a CLI without imports

Test: `tests/readme_features.rs::claude_md_import_tip_lives_in_the_agents_md_guide`

#### Scenario: start --force under supervisor mode is documented

- **WHEN** `docs/src/cli-reference.md` and `docs/src/user-guide/spec-driven-launch.md` are inspected
- **THEN** each SHALL state that `start --force` has no effect under supervisor mode

Test: `tests/readme_features.rs::start_force_no_effect_under_supervisor_is_documented`

### Requirement: README Security note states the trust model in brief

The README `## Security` section SHALL state, in at most 6 non-blank lines, that agents run arbitrary code with the user's privileges, that git-paw approves no permission prompt, that the agent CLI's own permission system decides, and that an optional sandbox confines writes. It SHALL link `security-posture.md` and `sandbox.md`.

#### Scenario: The Security note names the trust model and links both pages

- **WHEN** the README's `## Security` section is inspected
- **THEN** it SHALL contain `arbitrary code`, `sandbox` and a statement that git-paw approves no prompt
- **AND** it SHALL link `security-posture.md` and `sandbox.md`
- **AND** it SHALL have at most 6 non-blank lines

Test: `tests/readme_features.rs::security_note_names_the_trust_model_and_links_both_pages`
