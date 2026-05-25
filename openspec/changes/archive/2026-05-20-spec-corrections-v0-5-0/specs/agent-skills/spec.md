## MODIFIED Requirements

### Requirement: Skill template rendering

The `render()` function SHALL accept a `project: &str` parameter and a
`test_command: Option<&str>` parameter and substitute `{{PROJECT_NAME}}` with
the project name and `{{TEST_COMMAND}}` with the supervisor's configured
test command, alongside the existing `{{BRANCH_ID}}` and `{{GIT_PAW_BROKER_URL}}`
substitutions.

The function signature SHALL be:

```rust
pub fn render(
    template: &SkillTemplate,
    branch: &str,
    broker_url: &str,
    project: &str,
    test_command: Option<&str>,
) -> String
```

When `test_command` is `Some(cmd)`, every occurrence of `{{TEST_COMMAND}}` in
the template SHALL be replaced with the string `cmd`. When `test_command` is
`None`, every occurrence of `{{TEST_COMMAND}}` SHALL be replaced with the
literal string `"(not configured)"` so the rendered prose remains readable
and the unknown-placeholder warning path is not triggered.

#### Scenario: PROJECT_NAME placeholder is substituted

- **GIVEN** a `SkillTemplate` whose content contains `paw-{{PROJECT_NAME}}`
- **WHEN** `render(template, "feat/x", "http://127.0.0.1:9119", "my-app", None)` is called
- **THEN** the resulting string contains `paw-my-app`
- **AND** the resulting string contains no `{{PROJECT_NAME}}`

#### Scenario: Both BRANCH_ID and PROJECT_NAME substituted

- **GIVEN** a template containing both `{{BRANCH_ID}}` and `{{PROJECT_NAME}}`
- **WHEN** `render(template, "feat/http-broker", "url", "git-paw", None)` is called
- **THEN** the output contains `feat-http-broker` and `git-paw`
- **AND** no `{{BRANCH_ID}}` or `{{PROJECT_NAME}}` placeholders remain

#### Scenario: TEST_COMMAND placeholder is substituted when test_command is Some

- **GIVEN** a `SkillTemplate` whose content contains `run {{TEST_COMMAND}} after merge`
- **WHEN** `render(template, "feat/x", "http://127.0.0.1:9119", "proj", Some("just check"))` is called
- **THEN** the resulting string contains `run just check after merge`
- **AND** the resulting string contains no `{{TEST_COMMAND}}`

#### Scenario: TEST_COMMAND placeholder substitutes a literal when test_command is None

- **GIVEN** a `SkillTemplate` whose content contains `run {{TEST_COMMAND}} after merge`
- **WHEN** `render(template, "feat/x", "http://127.0.0.1:9119", "proj", None)` is called
- **THEN** the resulting string contains `run (not configured) after merge`
- **AND** the resulting string contains no `{{TEST_COMMAND}}`
- **AND** no `{{TEST_COMMAND}}` placeholder warning is written to standard error
