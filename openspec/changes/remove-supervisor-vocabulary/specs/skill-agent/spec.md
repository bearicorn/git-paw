## MODIFIED Requirements

### Requirement: Skill resolution order

The system SHALL provide a function `pub fn resolve(skill_name: &str, repo_root: &Path) -> Result<SkillTemplate, SkillError>` that locates the skill template by name using the following order, returning the first match:

1. The repository override `<main>/.agents/skills/<skill-name>/SKILL.md`, where `<main>` is the main working tree of the repository containing `repo_root`
2. The user override `<config_dir>/git-paw/agent-skills/<skill-name>/SKILL.md`, where `<config_dir>` is the result of `crate::dirs::config_dir()`
3. The embedded default for that skill name shipped in the binary

No other location SHALL be consulted: not the process's current directory or its ancestors, not any directory above the repository, not the `.agents/skills/` of any linked worktree (every agent worktree is one), and not the installed `.git-paw/skills/` package copies. When the repository has no main working tree, or it cannot be determined, level 1 SHALL be skipped. A user override SHALL be read only in the directory form above; a flat `<config_dir>/git-paw/agent-skills/<skill-name>.md` file SHALL NOT be treated as an override. An override replaces the skill's text only, per *Skill overrides replace skill text, never helper scripts*.

An override exists at a level when `<level>/<skill-name>/SKILL.md` exists; a directory without `SKILL.md` SHALL NOT count as an override. An override that exists but cannot be loaded SHALL fail resolution at that level and SHALL NOT fall through to a lower level: an unreadable repository override SHALL return `Err(SkillError::DirectoryReadError { .. })`, and an override at either level whose frontmatter is invalid SHALL return `Err(SkillError::ValidationError { .. })`.

The orchestrator skill SHALL resolve only by the name `orchestrator`, and an override applies to it only from a directory named `orchestrator`. The name `supervisor` SHALL NOT resolve (per *The retired supervisor skill name points at its replacement*), and an override directory named `supervisor` at either level SHALL NOT be read.

The bundled `cli-setup` skill SHALL NOT be overridable, because it carries the consent rules for writing an agent CLI's permission settings: `resolve("cli-setup", …)` SHALL skip levels 1 and 2 and return the embedded default. An override directory named `cli-setup` at either level SHALL be ignored — neither read nor validated, so a broken one cannot fail resolution — and is reported by `git paw doctor` (*Doctor reports where each bundled skill resolves from*).

If no level has the requested name, the function SHALL return `Err(SkillError::UnknownSkill { name })`.

The returned `SkillTemplate` SHALL include a `source: Source` field — `Source::AgentsStandard` for level 1, `Source::User` for level 2, `Source::Embedded` for level 3 — and an `origin_path: Option<PathBuf>` naming the `SKILL.md` that supplied the content (`None` for the embedded default), for diagnostic purposes.

#### Scenario: Resolve falls back to embedded when no override exists

- **GIVEN** neither a repository override nor a user override exists for the `coordination` skill
- **WHEN** `skills::resolve("coordination", repo_root)` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::Embedded`
- **AND** `origin_path` is `None`

#### Scenario: Standardized override is preferred over embedded default

- **GIVEN** a repository root containing `.agents/skills/coordination/SKILL.md` with valid frontmatter and the body `custom user content`
- **WHEN** `skills::resolve("coordination", repo_root)` is called with that repository root
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::AgentsStandard`
- **AND** the template content equals `custom user content`

#### Scenario: User override is preferred over embedded default

- **GIVEN** a file at `<config_dir>/git-paw/agent-skills/coordination/SKILL.md` with valid frontmatter and the body `custom user content`
- **AND** no repository override for `coordination`
- **WHEN** `skills::resolve("coordination", repo_root)` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::User`
- **AND** the template content equals `custom user content`
- **AND** `origin_path` names that `SKILL.md`

#### Scenario: Repository override is preferred over user override

- **GIVEN** a repository override at `<main>/.agents/skills/coordination/SKILL.md` and a user override for `coordination`
- **WHEN** `skills::resolve("coordination", repo_root)` is called
- **THEN** the result has `source = Source::AgentsStandard` and the repository override's content

#### Scenario: A linked worktree's override is ignored

- **GIVEN** a repository whose main working tree has no `.agents/skills/coordination/`
- **AND** a linked worktree of that repository whose checkout contains `.agents/skills/coordination/SKILL.md`
- **AND** no user override
- **WHEN** `skills::resolve("coordination", <linked worktree root>)` is called
- **THEN** the result has `source = Source::Embedded`

#### Scenario: Resolution does not depend on the current directory

- **GIVEN** a directory above the repository root containing `.agents/skills/coordination/SKILL.md`
- **AND** no repository or user override
- **WHEN** `skills::resolve("coordination", repo_root)` is called from a process whose current directory is that ancestor, and again from one whose current directory is inside the repository
- **THEN** both results have `source = Source::Embedded`

#### Scenario: Installed package copies are not overrides

- **GIVEN** the main working tree's `.git-paw/skills/coordination/SKILL.md` has been edited
- **AND** no repository or user override
- **WHEN** `skills::resolve("coordination", repo_root)` is called
- **THEN** the result has `source = Source::Embedded` and the embedded content

#### Scenario: A legacy flat user file is not an override

- **GIVEN** `<config_dir>/git-paw/agent-skills/coordination.md` exists and `<config_dir>/git-paw/agent-skills/coordination/` does not
- **WHEN** `skills::resolve("coordination", repo_root)` is called
- **THEN** the result has `source = Source::Embedded`

#### Scenario: An invalid repository override is a hard error

- **GIVEN** a repository override whose `SKILL.md` frontmatter lacks `description`
- **AND** a valid user override for the same skill
- **WHEN** `skills::resolve` is called for that skill
- **THEN** the result is `Err(SkillError::ValidationError { .. })`
- **AND** the user override is not returned

#### Scenario: The orchestrator skill resolves by its own name

- **GIVEN** no override for `orchestrator` at any level
- **WHEN** `skills::resolve("orchestrator", repo_root)` is called
- **THEN** the result SHALL be `Ok` with the embedded orchestrator skill

#### Scenario: A supervisor override directory is not read

- **GIVEN** a user override at `<config_dir>/git-paw/agent-skills/supervisor/SKILL.md` and none at `orchestrator/`
- **WHEN** `skills::resolve("orchestrator", repo_root)` is called
- **THEN** the result SHALL have `source = Source::Embedded`

#### Scenario: A supervisor repository override directory is not read

- **GIVEN** a repository root containing `.agents/skills/supervisor/SKILL.md` and no `.agents/skills/orchestrator/`
- **AND** no user override for either name
- **WHEN** `skills::resolve("orchestrator", repo_root)` is called with that repository root
- **THEN** the result SHALL have `source = Source::Embedded`

#### Scenario: The cli-setup skill ignores overrides

- **GIVEN** a repository override at `<main>/.agents/skills/cli-setup/SKILL.md` and a user override at `<config_dir>/git-paw/agent-skills/cli-setup/SKILL.md`
- **WHEN** `skills::resolve("cli-setup", repo_root)` is called
- **THEN** the result SHALL have `source = Source::Embedded` and the embedded content
- **AND** `origin_path` SHALL be `None`

#### Scenario: A broken cli-setup override does not fail resolution

- **GIVEN** a repository override at `<main>/.agents/skills/cli-setup/SKILL.md` whose frontmatter is invalid
- **WHEN** `skills::resolve("cli-setup", repo_root)` is called
- **THEN** the result SHALL be `Ok` with `source = Source::Embedded`

#### Scenario: Unknown skill name returns an error

- **WHEN** `skills::resolve("nonexistent", repo_root)` is called
- **AND** no override exists for `nonexistent` at either level
- **AND** no embedded default exists for `nonexistent`
- **THEN** the result is `Err(SkillError::UnknownSkill { name: "nonexistent" })`

## ADDED Requirements

### Requirement: The retired supervisor skill name points at its replacement

Resolving the skill name `supervisor` SHALL return the unknown-skill result, and every surface that reports it — the MCP `get_skill` response's message and any CLI error — SHALL name `orchestrator` as the replacement. An override directory named `supervisor` (at `<main>/.agents/skills/supervisor/` or `<config_dir>/git-paw/agent-skills/supervisor/`) SHALL NOT be read; when one exists and no `orchestrator` override exists at the same level, resolving the orchestrator skill SHALL print one stderr line naming the ignored directory and telling the user to rename it to `orchestrator`.

#### Scenario: get_skill for the retired name names the replacement

- **WHEN** an MCP client calls `get_skill` with the name `supervisor`
- **THEN** the response SHALL be a successful JSON-RPC response carrying no skill and a message naming `orchestrator`

#### Scenario: A leftover override is reported, not read

- **GIVEN** a user override at `<config_dir>/git-paw/agent-skills/supervisor/SKILL.md` and none at `orchestrator/`
- **WHEN** the orchestrator skill is resolved
- **THEN** the embedded orchestrator skill SHALL be returned with `source = Source::Embedded`
- **AND** stderr SHALL carry one line naming the `supervisor` directory and `orchestrator`

## REMOVED Requirements

### Requirement: The supervisor skill name is a deprecated alias

**Reason**: The alias was the v0.21.0 compatibility window for the v0.18.0 rename of the supervisor skill to `orchestrator`, scheduled for removal in v1.0.0. Freezing it would carry two names for one skill through 1.x.

**Migration**: Request the skill as `orchestrator` (MCP `get_skill`, scripts) and rename a `supervisor` override directory to `orchestrator`.
