## ADDED Requirements

### Requirement: Binary tests also remove XDG_CONFIG_HOME

Every `assert_cmd` builder in `tests/` that invokes the compiled binary SHALL also remove `XDG_CONFIG_HOME` with `env_remove("XDG_CONFIG_HOME")`, alongside the `HOME` and `XDG_DATA_HOME` overrides, so a user skill override under the developer's real config directory can never reach a test. The standing convention-enforcement test SHALL check it, with the same documented-exception rule.

#### Scenario: A test file that keeps XDG_CONFIG_HOME fails the suite

- **GIVEN** a test file in `tests/` whose `assert_cmd` builder sets `HOME` to a tempdir and removes `XDG_DATA_HOME` but not `XDG_CONFIG_HOME`
- **AND** no comment documenting that it requires the real configuration directory
- **WHEN** the suite runs
- **THEN** the convention-enforcement test SHALL fail
- **AND** its failure message SHALL name the offending file

#### Scenario: A real user override does not change a binary test's output

- **GIVEN** `XDG_CONFIG_HOME` pointing at a directory holding `git-paw/agent-skills/coordination/SKILL.md`
- **WHEN** a binary test that asserts the coordination skill text runs
- **THEN** it SHALL see the embedded text, not the override
