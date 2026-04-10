# agent-skills Specification

## Purpose
TBD - created by archiving change skill-templates. Update Purpose after archive.
## Requirements
### Requirement: Embedded coordination skill

The system SHALL embed the v0.3.0 coordination skill at compile time via `include_str!` from the file `assets/agent-skills/coordination.md`. The embedded content SHALL always be available regardless of how git-paw was installed and SHALL never depend on a runtime file lookup for fallback.

The embedded coordination skill content SHALL contain `curl` commands matching the broker wire format from the `broker-messages` capability:

- A `POST /publish` example for `agent.status`
- A `GET /messages/{{BRANCH_ID}}` example for polling
- A `POST /publish` example for `agent.artifact`
- A `POST /publish` example for `agent.blocked`

The embedded content SHALL use `{{BRANCH_ID}}` as the agent identity placeholder and `${GIT_PAW_BROKER_URL}` as the broker URL placeholder.

#### Scenario: Embedded coordination skill is reachable without any user files

- **GIVEN** no `~/.config/git-paw/agent-skills/` directory exists
- **WHEN** `skills::resolve("coordination")` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::Embedded`
- **AND** the template content is non-empty

#### Scenario: Embedded coordination skill contains all four operations

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains the substring `agent.status`
- **AND** it contains the substring `agent.artifact`
- **AND** it contains the substring `agent.blocked`
- **AND** it contains the substring `${GIT_PAW_BROKER_URL}/messages/{{BRANCH_ID}}`

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

The system SHALL provide a function `pub fn render(template: &SkillTemplate, branch: &str, broker_url: &str) -> String` that produces the final text to embed into a worktree's `AGENTS.md`. The function SHALL apply the following substitutions to `template.content`:

1. Every literal occurrence of `{{BRANCH_ID}}` SHALL be replaced with `slugify_branch(branch)` from the `broker-messages` capability
2. Every literal occurrence of `${GIT_PAW_BROKER_URL}` SHALL be preserved unchanged so it is expanded by the agent's shell at command-execution time

The function SHALL be deterministic: the same `(template, branch, broker_url)` input SHALL always produce the same output. The function MUST NOT perform any I/O.

The `broker_url` parameter is accepted by the function signature for future use (e.g., embedding the URL at render time as an alternative substitution mode) but MUST NOT be substituted into the output in v0.3.0; the literal `${GIT_PAW_BROKER_URL}` string SHALL be preserved.

#### Scenario: Branch ID placeholder is substituted

- **GIVEN** a `SkillTemplate` whose content contains the literal text `agent_id":"{{BRANCH_ID}}"`
- **WHEN** `render(template, "feat/http-broker", "http://127.0.0.1:9119")` is called
- **THEN** the resulting string contains `agent_id":"feat-http-broker"`
- **AND** the resulting string contains no occurrence of the literal `{{BRANCH_ID}}`

#### Scenario: Broker URL placeholder is preserved verbatim

- **GIVEN** a `SkillTemplate` whose content contains the literal text `curl ${GIT_PAW_BROKER_URL}/status`
- **WHEN** `render(template, "feat/x", "http://127.0.0.1:9119")` is called
- **THEN** the resulting string contains the literal text `curl ${GIT_PAW_BROKER_URL}/status`

#### Scenario: Branch slugification matches broker-messages

- **GIVEN** a `SkillTemplate` whose content is `id={{BRANCH_ID}}`
- **WHEN** `render(template, "Feature/HTTP_Broker", "http://127.0.0.1:9119")` is called
- **THEN** the resulting string is `id=feature-http_broker`
- **AND** the substitution matches what `slugify_branch("Feature/HTTP_Broker")` from the `broker-messages` capability would produce

#### Scenario: Render is deterministic

- **WHEN** `render(template, "feat/x", "http://127.0.0.1:9119")` is called twice with the same arguments
- **THEN** both calls produce identical output strings

#### Scenario: Render performs no I/O

- **GIVEN** a `SkillTemplate` whose `source = Source::User`
- **WHEN** `render(template, "feat/x", "http://127.0.0.1:9119")` is called
- **AND** the original user override file is deleted between resolution and rendering
- **THEN** the call still succeeds and returns content based on the in-memory template

### Requirement: Unknown placeholder warning

The system SHALL detect when the rendered output contains any `{{...}}` substring that was not consumed by substitution. When such a substring is found, the system SHALL emit a warning to the standard error stream identifying the unsubstituted placeholder. The presence of such a placeholder SHALL NOT cause `render` itself to fail; the rendering completes and the warning is informational.

This protects users who add typos like `{{GIT_PAW_BROKER_URL}}` (incorrect double-curly form) to their override files.

#### Scenario: Unknown placeholder triggers a warning

- **GIVEN** a `SkillTemplate` whose content contains the literal text `url={{UNKNOWN_PLACEHOLDER}}`
- **WHEN** `render(template, "feat/x", "http://127.0.0.1:9119")` is called
- **THEN** the function returns a string still containing `{{UNKNOWN_PLACEHOLDER}}`
- **AND** a warning has been written to standard error mentioning `UNKNOWN_PLACEHOLDER`

#### Scenario: No warning when only known placeholders are present

- **GIVEN** a `SkillTemplate` whose content contains only `{{BRANCH_ID}}` and `${GIT_PAW_BROKER_URL}`
- **WHEN** `render(template, "feat/x", "http://127.0.0.1:9119")` is called
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

