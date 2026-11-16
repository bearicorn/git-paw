## REMOVED Requirements

### Requirement: Configuration reference documents the specs type value

**Reason**: Its first scenario is titled "lists all three specs.type values" but the requirement names four, and OpenSpec rejects a scenario rename inside a MODIFIED block. The requirement returns under a new name with one scenario covering every value.

**Migration**: Replaced by "Configuration reference documents every specs type value" in this change, with the same test.

## ADDED Requirements

### Requirement: Configuration reference documents every specs type value

`docs/src/configuration/README.md`'s commented `[specs]` example SHALL document `type` as accepting `"openspec"`, `"markdown"`, `"speckit"` and `"superpowers"`, every value `--specs-format` accepts.

#### Scenario: Configuration reference lists every specs.type value

- **WHEN** the `[specs]` example in `docs/src/configuration/README.md` is inspected
- **THEN** it SHALL document `"openspec"`, `"markdown"`, `"speckit"` and `"superpowers"` as valid values

Test: `tests/configuration_reference_docs.rs::the_specs_example_lists_every_specs_type_value`
