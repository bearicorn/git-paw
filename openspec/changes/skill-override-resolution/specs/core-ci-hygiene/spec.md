## ADDED Requirements

### Requirement: Binary tests also remove XDG_CONFIG_HOME

Every function in `tests/` that overrides `HOME` for the compiled binary SHALL also remove or redirect `XDG_CONFIG_HOME`, so a user skill override under the developer's real config directory can never reach a test. Tests that run `git paw init` SHALL override `HOME`, `XDG_DATA_HOME` and `XDG_CONFIG_HOME`. The convention-enforcement test SHALL check both.

#### Scenario: A test file that keeps XDG_CONFIG_HOME fails the suite

- **GIVEN** a test file in `tests/` whose `assert_cmd` builder sets `HOME` to a tempdir and removes `XDG_DATA_HOME` but not `XDG_CONFIG_HOME`
- **AND** no comment documenting that it requires the real configuration directory
- **WHEN** the suite runs
- **THEN** the convention-enforcement test SHALL fail
- **AND** its failure message SHALL name the offending file

#### Scenario: Redirecting XDG_CONFIG_HOME into the tempdir satisfies the convention

- **GIVEN** a test file in `tests/` whose builder sets `HOME` to a tempdir, removes `XDG_DATA_HOME`, and sets `XDG_CONFIG_HOME` to a path inside that tempdir
- **WHEN** the suite runs
- **THEN** the convention-enforcement test SHALL accept it, as it accepts `env_remove("XDG_CONFIG_HOME")`

#### Scenario: A documented exception is exempt

- **GIVEN** a test file whose override omits `XDG_CONFIG_HOME`, or a test that runs `git paw init` with the real `HOME`
- **AND** a comment documenting that it requires the real directory
- **WHEN** the suite runs
- **THEN** the convention-enforcement test SHALL not flag it, under the same documented-exception rule as the `XDG_DATA_HOME` check

#### Scenario: A real user override does not change a binary test's output

- **GIVEN** `XDG_CONFIG_HOME` pointing at a directory holding `git-paw/agent-skills/coordination/SKILL.md`
- **WHEN** a binary test that asserts the coordination skill text runs
- **THEN** it SHALL see the embedded text, not the override

#### Scenario: A test running git paw init with the real HOME fails the suite

- **GIVEN** a test in `tests/` whose builder rooted at the compiled binary passes `init` (as `.args(["init", …])` or `.arg("init")`) without overriding `HOME`
- **AND** no comment documenting that it requires the real home directory
- **WHEN** the suite runs
- **THEN** the convention-enforcement test SHALL fail naming the file and the test
