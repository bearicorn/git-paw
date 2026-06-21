## ADDED Requirements

### Requirement: Documentation path fields in GovernanceConfig

`GovernanceConfig` SHALL gain two additional optional path fields alongside the existing governance-document fields, each defaulting to `None` when absent from `.git-paw/config.toml`:

- `readme: Option<PathBuf>` — path to the repository README (e.g. `README.md`).
- `docs: Option<PathBuf>` — path to the documentation root directory (e.g. `docs/src`).

Both are bring-your-own path pointers (no hardcoded locations) resolved against the repository root, consistent with the existing governance fields. They are surfaced by the MCP documentation tools ([[mcp-read-tools]]). A config file that omits them SHALL load with both set to `None`, leaving the documentation tools to degrade to empty/null results — so pre-existing `[governance]` sections load unchanged.

#### Scenario: GovernanceConfig parses readme and docs fields

- **GIVEN** a config file with `[governance]` setting `readme = "README.md"` and `docs = "docs/src"`
- **WHEN** the config is loaded
- **THEN** `config.governance.readme` SHALL be `Some("README.md")` and `config.governance.docs` SHALL be `Some("docs/src")`

#### Scenario: readme and docs default to None when omitted

- **GIVEN** a config file with a `[governance]` section that sets only `dod`
- **WHEN** the config is loaded
- **THEN** `config.governance.readme` and `config.governance.docs` SHALL both be `None`
- **AND** loading SHALL NOT error

#### Scenario: GovernanceConfig with readme/docs survives round-trip serialization

- **GIVEN** a `GovernanceConfig` whose `readme` and `docs` are both set
- **WHEN** it is serialized to TOML and re-parsed
- **THEN** the re-parsed `readme` and `docs` fields SHALL equal the originals
