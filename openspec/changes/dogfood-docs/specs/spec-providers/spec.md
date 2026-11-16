## ADDED Requirements

### Requirement: The OpenSpec guide documents the telemetry opt-out

The OpenSpec section of `docs/src/user-guide/spec-driven-launch.md` SHALL state that the `openspec` CLI sends usage telemetry by default, that git-paw itself sends none, and that setting `OPENSPEC_TELEMETRY=0` turns it off. It SHALL advise setting the variable in the shell profile, so that every pane git-paw launches inherits it, worker panes included.

#### Scenario: The guide names the opt-out variable

- **WHEN** the OpenSpec format section of the spec-driven-launch guide is inspected
- **THEN** it SHALL contain `OPENSPEC_TELEMETRY=0`
- **AND** it SHALL say that the telemetry comes from the `openspec` CLI, not from git-paw

Test: `tests/docs_dogfood_notes.rs::spec_driven_launch_documents_the_openspec_telemetry_opt_out`

#### Scenario: The advice reaches worker panes

- **WHEN** the same section is inspected
- **THEN** it SHALL advise setting the variable where worker panes inherit it, such as the shell profile, rather than only in the operator's current shell
