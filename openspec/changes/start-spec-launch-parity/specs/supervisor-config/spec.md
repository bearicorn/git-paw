## ADDED Requirements

### Requirement: Resolved approval flags apply on every worker-launch path

Every launch path that spawns a worker CLI SHALL build that pane's command from the resolved approval flags, so `agent_approval` and any `[clis.<name>].approval_args` override take effect regardless of which command started the session. The resolution chain itself is unchanged (per "Permission flag mapping"); what this requires is that no launch path bypass it.

A path that spawns a worker with the bare CLI name SHALL be treated as a defect: the worker then inherits whatever permission mode its CLI's own configuration happens to carry, which silently contradicts the configured `agent_approval` and leaves `approval_args` inert. This applies to the supervisor launch, the bare launch, the spec-driven launch, and `git paw add` alike.

#### Scenario: Spec-driven launch without supervisor applies approval flags

- **GIVEN** a config with `agent_approval = "auto"` and a CLI whose resolved `Auto` flags are non-empty
- **WHEN** a session is launched with `--specs` and no supervisor mode
- **THEN** each worker pane's command SHALL include the resolved flags

#### Scenario: Bare launch applies approval flags

- **GIVEN** a config with `agent_approval = "auto"` and a CLI whose resolved `Auto` flags are non-empty
- **WHEN** a session is launched with no spec selection and no supervisor mode
- **THEN** each worker pane's command SHALL include the resolved flags

#### Scenario: A per-CLI override reaches a non-supervisor launch

- **GIVEN** a config defining `[clis.<name>].approval_args` for the configured approval level
- **WHEN** a session is launched without supervisor mode
- **THEN** the override's value SHALL appear verbatim in the worker pane's command

#### Scenario: Empty resolved flags launch the bare command

- **GIVEN** a CLI and approval level whose resolution yields an empty flag string
- **WHEN** any launch path spawns that worker
- **THEN** the pane command SHALL be the CLI alone, with no trailing separator
