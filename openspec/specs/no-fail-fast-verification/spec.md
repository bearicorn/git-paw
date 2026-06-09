# no-fail-fast-verification Specification

## Purpose
TBD - created by archiving change verification-no-fail-fast-v0-6-x. Update Purpose after archive.
## Requirements
### Requirement: Testing gate states the full-suite discipline generically

The bundled supervisor skill's testing gate SHALL direct the supervisor to
run the configured test command (`{{TEST_COMMAND}}`) in a whole-suite /
no-fail-fast mode, and SHALL state that a run which aborted early is
incomplete — not a PASS. The wording SHALL remain stack-agnostic (no
runner- or repo-specific literals), so it passes the no-language-leak audit
across all supported spec backends.

#### Scenario: Skill mandates running the whole suite

- **WHEN** the supervisor.md testing-gate section is inspected
- **THEN** it SHALL direct the gate to run `{{TEST_COMMAND}}` without
  fail-fast (run every test group) and name the environment **guard test**
  as the failure that must not be allowed to truncate the run

#### Scenario: Early-aborted run is not a PASS

- **WHEN** the testing-gate section is read
- **THEN** it SHALL state that "the only failure is a known environment
  guard" is NOT a pass unless the full suite ran to completion

#### Scenario: Wording is stack-agnostic

- **WHEN** the no-language-leak audit runs against the updated supervisor.md
- **THEN** it SHALL pass across all supported spec backends (no
  runner/repo-specific tokens in the testing-gate prose)

### Requirement: git-paw provides a trustworthy verification recipe

The repository SHALL provide a `just verify` recipe that runs the WHOLE
test suite the correct way for git-paw — `cargo test --no-fail-fast` with
the no-tmux-server guard neutralised via `GIT_PAW_ALLOW_LIVE_SESSION=1`
(the suite is socket-isolated) — alongside the fmt, clippy, deny, and audit
gates, exiting non-zero on any real (non-guard) failure.

#### Scenario: just verify runs the full guard-neutralised suite

- **WHEN** `just verify` is invoked
- **THEN** it SHALL run `cargo test --no-fail-fast` with
  `GIT_PAW_ALLOW_LIVE_SESSION=1` plus fmt/clippy/deny/audit, so a single
  environmental guard failure can neither abort the run nor be mistaken for
  a code failure

### Requirement: git-paw routes verification through the recipe

git-paw's repo config SHALL set `[supervisor].test_command` to the
verification recipe (`just verify`) so the rendered supervisor skill's
`{{TEST_COMMAND}}` resolves to the trustworthy, no-fail-fast invocation
rather than a fail-fast-prone default.

#### Scenario: Configured test command is the verify recipe

- **GIVEN** git-paw's `.git-paw/config.toml`
- **WHEN** the supervisor skill is rendered
- **THEN** `{{TEST_COMMAND}}` SHALL resolve to `just verify` (the
  no-fail-fast, guard-neutralised recipe)

