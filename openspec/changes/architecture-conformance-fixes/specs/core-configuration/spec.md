## MODIFIED Requirements

### Requirement: Configuration reference documents the specs type value

`docs/src/configuration/README.md`'s commented `[specs]` example SHALL document `type` as accepting `"openspec"`, `"markdown"`, `"speckit"` AND `"superpowers"`, every value `--specs-format` accepts.

#### Scenario: Configuration reference lists all three specs.type values

- **WHEN** the `[specs]` example in `docs/src/configuration/README.md` is inspected
- **THEN** it documents `"speckit"` as a valid value alongside `"openspec"` and `"markdown"`

Test: `tests/configuration_reference_docs.rs::the_specs_example_lists_every_specs_type_value`

#### Scenario: Configuration reference lists superpowers as a specs.type value

- **WHEN** the `[specs]` example in `docs/src/configuration/README.md` is inspected
- **THEN** it documents `"superpowers"` as a valid value

Test: `tests/configuration_reference_docs.rs::the_specs_example_lists_every_specs_type_value`
