# agent-skills Specification

## Purpose
TBD - created by archiving change skill-templates. Update Purpose after archive.
## Requirements
### Requirement: Embedded coordination skill

The embedded `coordination.md` skill content SHALL reflect the v0.4 state in which `agent.status` publishing is automated by the filesystem watcher and `agent.artifact` publishing is automated by the post-commit git hook. The embedded content SHALL therefore:

1. NOT contain the legacy "MUST publish agent.status" instruction. Status publishing is automatic — agents do not curl `/publish` for `agent.status` themselves.
2. Include a note explaining that git-paw automatically publishes the agent's working status when the agent edits files and automatically publishes an `agent.artifact` when the agent runs `git commit`. The note SHALL state that agents only need to publish manually if they are blocked or want to announce explicit exports.
3. Retain the `agent.blocked` curl example as an opt-in operation for blocked agents.
4. Retain the `agent.artifact` curl example with `exports`, documented as the manual escape hatch when the agent wants to advertise specific exports beyond what the post-commit hook captures automatically.
5. Include a `### Cherry-pick peer commits` section that gives the exact `git cherry-pick` command an agent should run when a peer's `agent.artifact` arrives in the agent's inbox.
6. Include a `### Messages you may receive` section that documents the two supervisor-originated message variants:
   - `agent.verified` — the agent's work has been verified by the supervisor. No action required.
   - `agent.feedback` — the agent's work has issues. The `errors` field lists problems to fix; the agent SHALL address them and re-publish `agent.artifact`.
7. Continue to use `{{BRANCH_ID}}` and `{{GIT_PAW_BROKER_URL}}` placeholders, retaining the existing polling example `GET ${GIT_PAW_BROKER_URL}/messages/{{BRANCH_ID}}`.

#### Scenario: Coordination skill documents automatic status publishing

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains text indicating that `agent.status` publishing is automatic
- **AND** it does NOT contain the substring "MUST publish agent.status"

#### Scenario: Coordination skill retains blocked and artifact curl examples

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains a `curl` example for publishing `agent.blocked`
- **AND** it contains a `curl` example for publishing `agent.artifact`

#### Scenario: Coordination skill contains cherry-pick instructions

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains the substring `git cherry-pick`
- **AND** the cherry-pick guidance is reachable under a `Cherry-pick peer commits` heading or equivalent

#### Scenario: Coordination skill documents verification and feedback messages

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains the substring `agent.verified`
- **AND** it contains the substring `agent.feedback`
- **AND** it contains guidance describing how to handle feedback (fix the listed errors and re-publish `agent.artifact`)

#### Scenario: Coordination skill retains polling reference

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains `${GIT_PAW_BROKER_URL}/messages/{{BRANCH_ID}}`

### Requirement: Skill resolution order

The system SHALL provide a function `pub fn resolve(skill_name: &str) -> Result<SkillTemplate, SkillError>` that locates the skill template by name using the following order, returning the first match:

1. The user override file at `<config_dir>/git-paw/agent-skills/<skill-name>.md` where `<config_dir>` is the result of `dirs::config_dir()`
2. The embedded default for that skill name shipped in the binary

If neither the user override nor an embedded default exists for the requested name, the function SHALL return `Err(SkillError::UnknownSkill { name })`.

The returned `SkillTemplate` SHALL include a `source: Source` field indicating whether the content came from a user override or the embedded default, for diagnostic purposes.

#### Scenario: Resolve falls back to embedded when no user override exists

- **GIVEN** no user override file exists for the `coordination` skill
- **WHEN** `skills::resolve("coordination")` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::Embedded`

#### Scenario: User override is preferred over embedded default

- **GIVEN** a file at `<config_dir>/git-paw/agent-skills/coordination.md` containing the text `custom user content`
- **WHEN** `skills::resolve("coordination")` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::User`
- **AND** the template content equals `custom user content`

#### Scenario: Unknown skill name returns an error

- **WHEN** `skills::resolve("nonexistent")` is called
- **AND** no user override exists for `nonexistent`
- **AND** no embedded default exists for `nonexistent`
- **THEN** the result is `Err(SkillError::UnknownSkill { name: "nonexistent" })`

### Requirement: User override directory may be absent

The system SHALL treat a missing user override directory as a normal condition equivalent to "no override available", not as an error. Specifically:

- If `dirs::config_dir()` returns `None`, the system SHALL skip the user override lookup and proceed to the embedded default
- If the directory `<config_dir>/git-paw/agent-skills/` does not exist, the system SHALL skip the user override lookup and proceed to the embedded default
- If the specific file `<config_dir>/git-paw/agent-skills/<skill-name>.md` does not exist, the system SHALL skip the user override lookup and proceed to the embedded default

#### Scenario: Missing user config directory falls through to embedded

- **GIVEN** `dirs::config_dir()` is unable to determine a config directory
- **WHEN** `skills::resolve("coordination")` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::Embedded`

#### Scenario: Missing agent-skills subdirectory falls through to embedded

- **GIVEN** `<config_dir>/git-paw/` exists but `<config_dir>/git-paw/agent-skills/` does not
- **WHEN** `skills::resolve("coordination")` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::Embedded`

#### Scenario: Missing skill file falls through to embedded

- **GIVEN** `<config_dir>/git-paw/agent-skills/` exists but contains no `coordination.md`
- **WHEN** `skills::resolve("coordination")` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::Embedded`

### Requirement: Unreadable user override is a hard error

When a user override file exists but cannot be read (permission denied, I/O error, invalid UTF-8), the system SHALL return `Err(SkillError::UserOverrideRead { path, source })` rather than silently falling back to the embedded default. This makes misconfigured overrides visible to the user instead of hidden behind a working default.

#### Scenario: Permission denied on user override returns an error

- **GIVEN** a file at `<config_dir>/git-paw/agent-skills/coordination.md` with read permissions removed
- **WHEN** `skills::resolve("coordination")` is called
- **THEN** the result is `Err(SkillError::UserOverrideRead { .. })`
- **AND** the error message identifies the path of the unreadable file

#### Scenario: Invalid UTF-8 in user override returns an error

- **GIVEN** a file at `<config_dir>/git-paw/agent-skills/coordination.md` containing non-UTF-8 bytes
- **WHEN** `skills::resolve("coordination")` is called
- **THEN** the result is `Err(SkillError::UserOverrideRead { .. })`

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

### Requirement: Unknown placeholder warning

The system SHALL detect when the rendered output contains any `{{...}}` substring that was not consumed by substitution. When such a substring is found, the system SHALL emit a warning to the standard error stream identifying the unsubstituted placeholder. The presence of such a placeholder SHALL NOT cause `render` itself to fail; the rendering completes and the warning is informational.

This protects users who add typos like `{{GIT_PAW_BROKER_URL}}` (incorrect double-curly form) to their override files.

#### Scenario: Unknown placeholder triggers a warning

- **GIVEN** a `SkillTemplate` whose content contains the literal text `url={{UNKNOWN_PLACEHOLDER}}`
- **WHEN** `render(template, "feat/x", "http://127.0.0.1:9119", "proj")` is called
- **THEN** the function returns a string still containing `{{UNKNOWN_PLACEHOLDER}}`
- **AND** a warning has been written to standard error mentioning `UNKNOWN_PLACEHOLDER`

#### Scenario: No warning when only known placeholders are present

- **GIVEN** a `SkillTemplate` whose content contains only `{{BRANCH_ID}}`, `{{PROJECT_NAME}}`, and `{{GIT_PAW_BROKER_URL}}`
- **WHEN** `render(template, "feat/x", "http://127.0.0.1:9119", "proj")` is called
- **THEN** no warning is written to standard error

### Requirement: SkillTemplate value type

The system SHALL define a public type `SkillTemplate` with at least these fields:

- `name: String` — the skill name (e.g. `"coordination"`)
- `content: String` — the unrendered template content
- `source: Source` — an enum with at least the variants `Embedded` and `User`

`SkillTemplate` SHALL derive `Debug` and `Clone`. The `Source` enum SHALL derive `Debug`, `Clone`, `Copy`, `PartialEq`, and `Eq`.

#### Scenario: SkillTemplate from embedded source has correct fields

- **WHEN** `skills::resolve("coordination")` is called with no user override
- **THEN** the returned `SkillTemplate` has `name == "coordination"`
- **AND** `source == Source::Embedded`
- **AND** `content` is non-empty

#### Scenario: SkillTemplate is cloneable

- **GIVEN** a `SkillTemplate` returned by `skills::resolve`
- **WHEN** `template.clone()` is called
- **THEN** the clone has identical `name`, `content`, and `source` fields

### Requirement: Embedded supervisor skill

The embedded supervisor skill SHALL include a "Spec Audit Procedure" section that instructs the supervisor to verify implementation matches spec before publishing `agent.verified`. The procedure SHALL include:

- How to locate spec files for a given change
- How to extract WHEN/THEN scenarios from spec files
- How to search the codebase for matching tests
- How to verify struct fields, function signatures, and types match SHALL/MUST requirements
- How to compile gaps into an `agent.feedback` error list
- When to publish `agent.verified` (no gaps) vs `agent.feedback` (gaps found)

The spec audit SHALL run after the test command passes and before `agent.verified` is published.

#### Scenario: Supervisor skill contains spec audit procedure

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains the substring `Spec Audit`
- **AND** it contains instructions to read `openspec/changes/` spec files
- **AND** it contains instructions to grep for matching tests
- **AND** it contains instructions to verify field names match spec

#### Scenario: Spec audit runs after tests, before verified

- **WHEN** the embedded supervisor skill workflow is inspected
- **THEN** the spec audit step appears after the test command step
- **AND** the spec audit step appears before the `agent.verified` publish step

