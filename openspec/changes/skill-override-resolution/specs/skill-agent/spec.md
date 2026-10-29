## MODIFIED Requirements

### Requirement: Skill resolution order

The system SHALL provide a function `pub fn resolve(skill_name: &str, repo_root: &Path) -> Result<SkillTemplate, SkillError>` that locates the skill template by name using the following order, returning the first match:

1. The repository override `<main>/.agents/skills/<skill-name>/SKILL.md`, where `<main>` is the main working tree of the repository containing `repo_root`
2. The user override `<config_dir>/git-paw/agent-skills/<skill-name>/SKILL.md`, where `<config_dir>` is the result of `crate::dirs::config_dir()`
3. The embedded default for that skill name shipped in the binary

No other location SHALL be consulted: not the process's current directory or its ancestors, not any directory above the repository, not the `.agents/skills/` of any linked worktree (every agent worktree is one), and not the installed `.git-paw/skills/` package copies. When the repository has no main working tree, or it cannot be determined, level 1 SHALL be skipped. A user override SHALL be read only in the directory form above; a flat `<config_dir>/git-paw/agent-skills/<skill-name>.md` file SHALL NOT be treated as an override. An override replaces the skill's text only, per *Skill overrides replace skill text, never helper scripts*.

An override exists at a level when `<level>/<skill-name>/SKILL.md` exists; a directory without `SKILL.md` SHALL NOT count as an override. An override that exists but cannot be loaded SHALL fail resolution at that level and SHALL NOT fall through to a lower level: an unreadable repository override SHALL return `Err(SkillError::DirectoryReadError { .. })`, and an override at either level whose frontmatter is invalid SHALL return `Err(SkillError::ValidationError { .. })`.

The name `supervisor` SHALL be an accepted alias of `orchestrator`: `resolve("supervisor", …)` SHALL return the orchestrator template, and an override directory named either `orchestrator` or `supervisor` (at each level above) SHALL apply to both names, an `orchestrator` directory winning over a `supervisor` directory when both exist at the same level; a higher level SHALL win over a lower one whichever of the two names it uses.

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

#### Scenario: supervisor is an alias of orchestrator

- **GIVEN** no override for either name at any level
- **WHEN** `skills::resolve("supervisor", repo_root)` and `skills::resolve("orchestrator", repo_root)` are called
- **THEN** both SHALL return `Ok` with identical embedded content

#### Scenario: A legacy supervisor override applies to the orchestrator name

- **GIVEN** a repository root containing `.agents/skills/supervisor/SKILL.md` and no `.agents/skills/orchestrator/`
- **WHEN** `skills::resolve("orchestrator", repo_root)` is called with that repository root
- **THEN** the result SHALL have `source = Source::AgentsStandard` and the override's content

#### Scenario: A legacy supervisor user override applies to the orchestrator name

- **GIVEN** a user override at `<config_dir>/git-paw/agent-skills/supervisor/SKILL.md` and none at `orchestrator/`
- **AND** no repository override for either name
- **WHEN** `skills::resolve("orchestrator", repo_root)` is called
- **THEN** the result SHALL have `source = Source::User` and the override's content

#### Scenario: An orchestrator override wins over a supervisor override

- **GIVEN** a repository root containing both `.agents/skills/orchestrator/SKILL.md` and `.agents/skills/supervisor/SKILL.md`
- **WHEN** `skills::resolve("supervisor", repo_root)` is called with that repository root
- **THEN** the result SHALL carry the `orchestrator` override's content

#### Scenario: Level precedence beats name precedence

- **GIVEN** a repository override at `<main>/.agents/skills/supervisor/SKILL.md` and a user override at `<config_dir>/git-paw/agent-skills/orchestrator/SKILL.md`
- **WHEN** `skills::resolve("orchestrator", repo_root)` is called
- **THEN** the result SHALL have `source = Source::AgentsStandard` and the repository override's content

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

### Requirement: User override directory may be absent

The system SHALL treat a missing user override location as a normal condition equivalent to "no user override", not as an error, and continue with the embedded default. Specifically:

- If `crate::dirs::config_dir()` returns `None`, the system SHALL skip the user override lookup
- If the directory `<config_dir>/git-paw/agent-skills/` does not exist, the system SHALL skip the user override lookup
- If `<config_dir>/git-paw/agent-skills/<skill-name>/` does not exist, or exists without a `SKILL.md`, the system SHALL skip the user override lookup

#### Scenario: Missing user config directory falls through to embedded

- **GIVEN** no user configuration directory can be determined
- **AND** no repository override for `coordination`
- **WHEN** the `coordination` skill is resolved
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::Embedded`

#### Scenario: Missing agent-skills subdirectory falls through to embedded

- **GIVEN** `<config_dir>/git-paw/` exists but `<config_dir>/git-paw/agent-skills/` does not
- **WHEN** `skills::resolve("coordination", repo_root)` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::Embedded`

#### Scenario: Missing skill file falls through to embedded

- **GIVEN** `<config_dir>/git-paw/agent-skills/coordination/` exists but contains no `SKILL.md`
- **WHEN** `skills::resolve("coordination", repo_root)` is called
- **THEN** the result is `Ok(SkillTemplate)` with `source = Source::Embedded`

### Requirement: Unreadable user override is a hard error

When a user override `SKILL.md` exists but cannot be read (permission denied, I/O error, invalid UTF-8), the system SHALL return `Err(SkillError::UserOverrideRead { path, source })` rather than silently falling back to the embedded default. This makes misconfigured overrides visible to the user instead of hidden behind a working default.

#### Scenario: Permission denied on user override returns an error

- **GIVEN** a file at `<config_dir>/git-paw/agent-skills/coordination/SKILL.md` with read permissions removed
- **WHEN** `skills::resolve("coordination", repo_root)` is called
- **THEN** the result is `Err(SkillError::UserOverrideRead { .. })`
- **AND** the error message identifies the path of the unreadable file

#### Scenario: Invalid UTF-8 in user override returns an error

- **GIVEN** a file at `<config_dir>/git-paw/agent-skills/coordination/SKILL.md` containing non-UTF-8 bytes
- **WHEN** `skills::resolve("coordination", repo_root)` is called
- **THEN** the result is `Err(SkillError::UserOverrideRead { .. })`

## ADDED Requirements

### Requirement: Production callers resolve skills through the real search roots

The system SHALL resolve every skill it renders, installs, injects or serves through `skills::resolve(skill_name, repo_root)` with the session's repository root, so that the user level is always consulted: on every launch path (bare and spec-driven `start`, supervisor `start`, `add`, re-attach), during package installation and worktree provisioning, in MCP `get_skill`, and in `git paw doctor`. The explicit-roots form `skills::resolve_from(skill_name, &SkillRoots)`, which reads only the roots it is given, SHALL be used only by `src/skills.rs` and by tests. The system SHALL provide no resolution entry point that omits the user level, and a source audit SHALL fail the suite when any production source file other than `src/skills.rs` calls `resolve_from` or constructs a `SkillRoots`.

#### Scenario: A user override reaches the agent launched by start

- **GIVEN** `HOME` points at a temporary directory whose platform config directory holds `git-paw/agent-skills/coordination/SKILL.md` with the body `USER-OVERRIDE-MARKER`, and `XDG_CONFIG_HOME` is unset
- **AND** a repository with the broker enabled and no repository override
- **WHEN** `git paw start` launches an agent
- **THEN** that agent's managed `AGENTS.md` block SHALL contain `USER-OVERRIDE-MARKER`

#### Scenario: A user override reaches the agent added by add

- **GIVEN** the same user override and a running session
- **WHEN** `git paw add` adds an agent
- **THEN** the added agent's managed `AGENTS.md` block SHALL contain `USER-OVERRIDE-MARKER`

#### Scenario: A bypass of the production entry point fails the suite

- **GIVEN** a production source file other than `src/skills.rs` that calls `resolve_from` or constructs a `SkillRoots` outside test code
- **WHEN** the source audit runs
- **THEN** it SHALL fail naming that file
