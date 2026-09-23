## ADDED Requirements

### Requirement: Non-test code does not call unwrap or expect

Non-test code in the `git-paw` library and binary crates SHALL NOT call `unwrap()`, `expect()`, `unwrap_err()`, or `expect_err()` on an `Option` or `Result`, except inside the single static-regex helper defined by *Static regular expressions compile through one audited helper*. A recoverable failure SHALL propagate with `?` (or `ok_or_else` / `map_err`) into a `PawError` variant; a state that cannot occur SHALL be made unrepresentable by construction rather than asserted.

The rule SHALL be lint-enforced: the library crate root (`src/lib.rs`) and the binary crate root (`src/main.rs`) SHALL enable `clippy::unwrap_used` and `clippy::expect_used`, and the repository `clippy.toml` SHALL set `allow-unwrap-in-tests = true` and `allow-expect-in-tests = true`, so `just lint` (`cargo clippy --all-targets -- -D warnings`) fails on a violation in library or binary code while `#[test]` functions and `#[cfg(test)]` modules remain exempt. Integration-test crates under `tests/` are outside the rule.

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
- **THEN** the process SHALL exit non-zero with a usage error on stderr
- **AND** stderr SHALL NOT contain `panicked`

Test: `tests/production_panic_free.rs::add_without_branch_or_spec_errors_without_panic`

#### Scenario: `git paw replay` without a branch or `--list` errors instead of panicking

- **WHEN** `git paw replay` is invoked inside a repository with neither a branch argument nor `--list`
- **THEN** the process SHALL exit non-zero with a usage error on stderr
- **AND** stderr SHALL NOT contain `panicked`

Test: `tests/production_panic_free.rs::replay_without_branch_or_list_errors_without_panic`

### Requirement: Static regular expressions compile through one audited helper

Every regular expression compiled from a pattern fixed at build time — a string literal, or an embedded default such as the built-in Claude Code prompt profile's patterns — SHALL be compiled through a single crate-private helper whose pattern parameter is `&'static str`. That helper SHALL be the only non-test code that calls `expect()`, under a lint suppression scoped to that one function and a comment stating why the failure is unreachable at runtime. Every pattern passed to the helper SHALL be compiled by at least one unit test, so an invalid static pattern fails `cargo test` instead of panicking in a shipped binary. A pattern supplied at runtime (configuration, user, or agent input) SHALL NOT be passed to the helper; it SHALL be compiled with its error propagated.

#### Scenario: Every static pattern compiles under test

- **GIVEN** a module that compiles a static pattern through the helper
- **WHEN** `cargo test` runs
- **THEN** a unit test in that module SHALL force each of its static regexes to compile and SHALL pass

Test: `agents::tests::static_regexes_compile`, `broker::server::tests::static_regexes_compile`, `opsx::role_guard::tests::static_regexes_compile`, `specs::speckit::tests::static_regexes_compile`, `specs::superpowers::tests::static_regexes_compile`, `supervisor::auto_approve::tests::static_regexes_compile`, `supervisor::drive::tests::static_regexes_compile`

#### Scenario: The helper is the only expect in non-test code

- **WHEN** non-test code in `src/` is searched for `.expect(` / `.unwrap(` calls on `Option` or `Result`
- **THEN** the only match SHALL be inside the static-regex helper, and it SHALL carry a function-scoped `#[allow(clippy::expect_used)]`

Test: the `just lint` gate (any other site fails `clippy::expect_used` / `clippy::unwrap_used`); `static_regex::tests::compiles_a_static_pattern`
