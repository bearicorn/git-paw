## ADDED Requirements

### Requirement: SupervisorConfig SHALL carry six gate-command template fields

`SupervisorConfig` in `src/config.rs` SHALL expose the following six optional fields beyond the existing `test_command`:

- `lint_command: Option<String>` — pre-stage lint invocation.
- `build_command: Option<String>` — compile step when distinct from test.
- `doc_build_command: Option<String>` — documentation build (gate 4 input).
- `spec_validate_command: Option<String>` — spec validator (gate 3 input). MAY contain a `{{CHANGE_ID}}` placeholder that the supervisor agent expands at verification time with the change name.
- `fmt_check_command: Option<String>` — formatter check.
- `security_audit_command: Option<String>` — security audit tooling (gate 5 input).

Each field SHALL be annotated with `#[serde(default, skip_serializing_if = "Option::is_none")]` so older TOMLs without the fields deserialise as `None` and configs that set the fields to `None` omit them on round-trip.

#### Scenario: Fields default to None when absent from TOML

- **GIVEN** a `.git-paw/config.toml` containing `[supervisor]\nenabled = true\ncli = "claude"\n` (no gate-command keys)
- **WHEN** `load_config` reads the file
- **THEN** the resulting `SupervisorConfig` has `lint_command = None`, `build_command = None`, `doc_build_command = None`, `spec_validate_command = None`, `fmt_check_command = None`, `security_audit_command = None`
- **AND** the existing `test_command` field is also `None`

#### Scenario: Fields round-trip through serialize + deserialize

- **GIVEN** a `SupervisorConfig` with `test_command = Some("just check")`, `lint_command = Some("cargo clippy -- -D warnings")`, `build_command = None`, `doc_build_command = Some("mdbook build docs/")`, `spec_validate_command = Some("openspec validate {{CHANGE_ID}} --strict")`, `fmt_check_command = Some("cargo fmt --check")`, `security_audit_command = Some("cargo audit")`
- **WHEN** the value is serialised to TOML and deserialised back
- **THEN** the resulting struct SHALL equal the original

#### Scenario: None-valued fields omit from serialised TOML

- **GIVEN** a `SupervisorConfig` with all six gate-command fields set to `None`
- **WHEN** the value is serialised to TOML
- **THEN** the resulting TOML SHALL NOT contain the keys `lint_command`, `build_command`, `doc_build_command`, `spec_validate_command`, `fmt_check_command`, `security_audit_command`

### Requirement: `git paw init` SHALL write a commented-out [supervisor] block enumerating gate keys

`src/init.rs::run_init` SHALL append (or include in the initial `config.toml` content it writes) a commented-out `[supervisor]` block listing the six new gate-command keys with example values illustrating common stacks. The block SHALL be entirely commented (every line prefixed with `#`) so TOML parsing ignores it.

The block SHALL include at minimum the six keys (`lint_command`, `build_command`, `doc_build_command`, `spec_validate_command`, `fmt_check_command`, `security_audit_command`) plus the existing `test_command`, `enabled`, and `cli` keys, each with one example value.

#### Scenario: `git paw init` writes the commented block

- **GIVEN** a fresh repository with no `.git-paw/config.toml`
- **WHEN** `git paw init` is invoked
- **THEN** `.git-paw/config.toml` SHALL exist
- **AND** the file content SHALL contain `# [supervisor]` (commented section header) on its own line
- **AND** the file content SHALL contain a commented line for each of the seven `[supervisor]` keys (`enabled`, `cli`, `test_command`, `lint_command`, `build_command`, `doc_build_command`, `spec_validate_command`, `fmt_check_command`, `security_audit_command`)

#### Scenario: The written commented block is valid TOML when uncommented

- **GIVEN** the `git paw init`-written commented block
- **WHEN** every line prefixed with `# ` has its prefix stripped (turning the block into uncommented TOML)
- **THEN** the resulting TOML SHALL parse without error
- **AND** the parsed `SupervisorConfig` SHALL have all seven listed keys populated with the example values
