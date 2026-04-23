## ADDED Requirements

### Requirement: Embedded supervisor skill

The system SHALL embed a supervisor skill at compile time via `include_str!` from the file `assets/agent-skills/supervisor.md`. The skill SHALL be resolvable via `skills::resolve("supervisor")`.

The embedded supervisor skill SHALL contain:
- A role definition stating the supervisor monitors and verifies but does NOT write code
- `curl` commands for polling `${GIT_PAW_BROKER_URL}/status` and `${GIT_PAW_BROKER_URL}/messages/supervisor`
- `curl` commands for publishing `agent.verified` and `agent.feedback` messages
- `tmux capture-pane` and `tmux send-keys` commands using `paw-{{PROJECT_NAME}}` session name
- A `{{TEST_COMMAND}}` placeholder for the test command (substituted at launch, not by render)
- Guidance on detecting file conflicts from `modified_files` overlap
- Guidance on merge ordering (merge agents with no dependents first)
- Guidance on when to escalate to the human

#### Scenario: Supervisor skill is resolvable

- **GIVEN** no user override for the supervisor skill
- **WHEN** `skills::resolve("supervisor")` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::Embedded`
- **AND** the template content is non-empty

#### Scenario: Supervisor skill contains role definition

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains the substring `do NOT write code` (or equivalent)

#### Scenario: Supervisor skill contains broker commands

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains `${GIT_PAW_BROKER_URL}/status`
- **AND** it contains `agent.verified`
- **AND** it contains `agent.feedback`

#### Scenario: Supervisor skill contains tmux commands

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains `tmux capture-pane`
- **AND** it contains `tmux send-keys`
- **AND** it contains `paw-{{PROJECT_NAME}}`

#### Scenario: Supervisor skill user override

- **GIVEN** a file at `<config_dir>/git-paw/agent-skills/supervisor.md` containing custom content
- **WHEN** `skills::resolve("supervisor")` is called
- **THEN** the result has `source = Source::User` and the custom content

## MODIFIED Requirements

### Requirement: Skill template rendering

The `render()` function SHALL accept an additional `project: &str` parameter and substitute `{{PROJECT_NAME}}` with the project name alongside the existing `{{BRANCH_ID}}` substitution.

The function signature SHALL be: `pub fn render(template: &SkillTemplate, branch: &str, broker_url: &str, project: &str) -> String`

#### Scenario: PROJECT_NAME placeholder is substituted

- **GIVEN** a `SkillTemplate` whose content contains `paw-{{PROJECT_NAME}}`
- **WHEN** `render(template, "feat/x", "http://127.0.0.1:9119", "my-app")` is called
- **THEN** the resulting string contains `paw-my-app`
- **AND** the resulting string contains no `{{PROJECT_NAME}}`

#### Scenario: Both BRANCH_ID and PROJECT_NAME substituted

- **GIVEN** a template containing both `{{BRANCH_ID}}` and `{{PROJECT_NAME}}`
- **WHEN** `render(template, "feat/http-broker", "url", "git-paw")` is called
- **THEN** the output contains `feat-http-broker` and `git-paw`
- **AND** no `{{...}}` placeholders remain (except `{{TEST_COMMAND}}` which is handled externally)
