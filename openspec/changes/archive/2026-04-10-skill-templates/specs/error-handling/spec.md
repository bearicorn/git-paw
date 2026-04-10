## ADDED Requirements

### Requirement: SkillError variants with actionable messages

The system SHALL define a `SkillError` type with variants for skill loading failures. Each variant SHALL produce a user-facing message that explains the problem and suggests a remedy. `SkillError` SHALL be wrappable inside `PawError` as a variant.

The following variants SHALL exist:

- `UnknownSkill { name: String }` — no embedded or user override found for the requested skill name
- `UserOverrideRead { path: PathBuf, source: std::io::Error }` — a user override file exists but cannot be read

#### Scenario: UnknownSkill is actionable
- **GIVEN** `SkillError::UnknownSkill { name: "nonexistent" }`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL mention the skill name `"nonexistent"` and indicate no embedded default exists

#### Scenario: UserOverrideRead is actionable
- **GIVEN** `SkillError::UserOverrideRead { path: "/home/user/.config/git-paw/agent-skills/coordination.md", .. }`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL include the file path and suggest checking permissions

#### Scenario: SkillError exit code
- **GIVEN** any `SkillError` variant wrapped in `PawError`
- **WHEN** `exit_code()` is called
- **THEN** it SHALL return `1`
