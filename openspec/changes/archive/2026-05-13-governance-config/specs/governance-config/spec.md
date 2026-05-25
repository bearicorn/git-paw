## ADDED Requirements

### Requirement: GovernanceConfig struct with optional paths

The system SHALL define a `GovernanceConfig` struct exposed as `PawConfig.governance`. The struct SHALL contain the following optional path fields, each defaulting to `None` when absent from `.git-paw/config.toml`:

- `adr: Option<PathBuf>` — directory containing ADR files (project chooses the convention).
- `test_strategy: Option<PathBuf>` — single Markdown file describing the project's test strategy.
- `security: Option<PathBuf>` — single Markdown file containing the project's security checklist.
- `dod: Option<PathBuf>` — single Markdown file containing the project's Definition of Done.
- `constitution: Option<PathBuf>` — single Markdown file containing the project's constitution (Spec Kit's `constitution.md` or a project-specific equivalent).

Paths SHALL be stored as raw `PathBuf` values (not pre-resolved). Relative paths are resolved relative to the repository root at use time, not at config-load time. Absolute paths are stored as-is.

The struct SHALL derive `Debug`, `Clone`, `Default`, `Deserialize`, and `Serialize` matching local conventions for other config sections.

The struct SHALL NOT contain a nested `gates` field, a `[governance.gates]` table, per-doc boolean flags, or any other gating semantics. This change ships path storage only; runtime usage is owned by the `governance-context` capability.

#### Scenario: GovernanceConfig defaults to all None when section absent

- **GIVEN** a config file with no `[governance]` section
- **WHEN** the config is loaded
- **THEN** `config.governance` SHALL be present
- **AND** all five path fields SHALL be `None`

#### Scenario: GovernanceConfig with all paths populated

- **GIVEN** a config file with `[governance]` containing all five paths
- **WHEN** the config is loaded
- **THEN** all five fields of `config.governance` SHALL be `Some(path)` matching the configured values

#### Scenario: GovernanceConfig with partial paths

- **GIVEN** a config file with `[governance]` setting only `dod = "docs/dod.md"` and `security = "docs/security.md"`
- **WHEN** the config is loaded
- **THEN** `config.governance.dod` and `config.governance.security` SHALL be `Some(path)`
- **AND** `config.governance.adr`, `config.governance.test_strategy`, and `config.governance.constitution` SHALL be `None`

#### Scenario: Absolute path is preserved as-is

- **GIVEN** a config file with `[governance] adr = "/absolute/path/to/adr"`
- **WHEN** the config is loaded
- **THEN** `config.governance.adr` SHALL be `Some(PathBuf::from("/absolute/path/to/adr"))`

#### Scenario: GovernanceConfig round-trips through save and load

- **GIVEN** a `GovernanceConfig` with all five paths populated
- **WHEN** the config is saved to TOML and loaded back
- **THEN** the loaded values SHALL match the original

#### Scenario: GovernanceConfig with non-existent paths still loads cleanly

- **GIVEN** a config file with `[governance] dod = "docs/never-existed.md"`
- **WHEN** the config is loaded
- **THEN** loading SHALL succeed without error
- **AND** `config.governance.dod` SHALL be `Some(PathBuf::from("docs/never-existed.md"))`

#### Scenario: GovernanceConfig has no gates field

- **WHEN** `GovernanceConfig::default()` is inspected
- **THEN** the struct SHALL contain only the five path fields
- **AND** SHALL NOT contain any `gates` field, nested `GovernanceGates` struct, or per-doc boolean flags

### Requirement: Spec Kit constitution auto-wiring

When `governance.constitution` is `None` after deserialisation AND the SpecKit backend is active for the session (`config.specs.type == "speckit"`) AND `git_paw::specs::speckit::detect_constitution(specs_dir)` returns `Some(path)`, the system SHALL populate `governance.constitution` with the detected path during config-load post-processing.

If `governance.constitution` is set explicitly (`Some(_)`), the system SHALL NOT override it — explicit user values always win, even if the explicit value points at a path that doesn't exist.

If the SpecKit backend is not active (`specs.type != "speckit"` or `[specs]` section absent), no auto-wiring SHALL occur.

#### Scenario: Constitution auto-wires when unset and SpecKit detected

- **GIVEN** a repository with `.specify/memory/constitution.md` present
- **AND** `[specs] type = "speckit"`, `[specs] dir = ".specify/specs"`
- **AND** no `governance.constitution` set in TOML
- **WHEN** the config is loaded
- **THEN** `config.governance.constitution` SHALL be `Some(PathBuf::from(".specify/memory/constitution.md"))` (or the equivalent path returned by `detect_constitution`)

#### Scenario: Explicit constitution path is preserved

- **GIVEN** a repository with `.specify/memory/constitution.md` present
- **AND** `[governance] constitution = "docs/principles.md"` set in TOML
- **WHEN** the config is loaded
- **THEN** `config.governance.constitution` SHALL be `Some(PathBuf::from("docs/principles.md"))`
- **AND** the auto-wiring SHALL NOT override it

#### Scenario: Auto-wiring skipped when SpecKit backend is inactive

- **GIVEN** a repository with `.specify/memory/constitution.md` present but `[specs] type = "openspec"`
- **AND** no `governance.constitution` set in TOML
- **WHEN** the config is loaded
- **THEN** `config.governance.constitution` SHALL remain `None`

#### Scenario: Auto-wiring skipped when constitution.md is absent

- **GIVEN** a repository with `[specs] type = "speckit"` but no `.specify/memory/constitution.md`
- **AND** no `governance.constitution` set in TOML
- **WHEN** the config is loaded
- **THEN** `config.governance.constitution` SHALL remain `None`
- **AND** loading SHALL succeed without error

### Requirement: Backward compatibility with pre-v0.5 configs

The system SHALL load v0.4 `.git-paw/config.toml` files (which contain no `[governance]` section) without error. Loaded `PawConfig.governance` SHALL be `GovernanceConfig::default()` — all path fields `None`.

This SHALL hold for every v0.4 config shape, including configs with all subsystems disabled, configs with only `[broker]`, configs with only `[supervisor]`, and configs with `[clis]` and `[specs]` populated.

#### Scenario: v0.4 config without [governance] loads with defaults

- **GIVEN** a `.git-paw/config.toml` produced by v0.4 (no `[governance]` section, may have `[broker]`, `[supervisor]`, `[specs]`, `[clis]`)
- **WHEN** the config is loaded with the v0.5.0 binary
- **THEN** loading SHALL succeed without error
- **AND** `config.governance.adr`, `test_strategy`, `security`, `dod`, `constitution` SHALL all be `None`
