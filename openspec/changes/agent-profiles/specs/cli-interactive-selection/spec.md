## ADDED Requirements

### Requirement: The CLI picker lists worker profiles

When worker profiles are configured, every interactive CLI picker — the uniform-mode picker, each per-branch picker, and the spec-driven picker — SHALL list each worker profile as a choice after the available CLIs, displayed as `<name> — <description> [<cli>]`, and SHALL list no delegate profile. Choosing a profile SHALL map the branch to that worker profile; choosing a plain CLI SHALL map it to the CLI as before. The selection result SHALL distinguish a profile choice from a CLI choice even when a profile and a CLI share a name. A configured default — `default_profile` when set, else `default_cli` — SHALL be the picker's starting entry. When no worker profile is configured, the picker's items and their order SHALL be identical to those shown before this requirement.

#### Scenario: Profiles appear with their descriptions

- **GIVEN** available CLIs `alpha` and `beta`, a worker profile `routine` with description `Routine implementation` on CLI `beta`, and a delegate profile `explorer`
- **WHEN** the uniform-mode CLI picker is built
- **THEN** its items SHALL be `alpha`, `beta`, then `routine — Routine implementation [beta]`
- **AND** `explorer` SHALL NOT be listed

#### Scenario: Picking a profile maps the branch to it

- **GIVEN** the picker above in per-branch mode for `feat/a` and `feat/b`
- **WHEN** the user picks `routine` for `feat/a` and `alpha` for `feat/b`
- **THEN** `feat/a` SHALL map to the worker profile `routine` and `feat/b` to the CLI `alpha`

#### Scenario: A profile sharing a CLI's name stays distinct

- **GIVEN** an available CLI `fast` and a worker profile also named `fast`
- **WHEN** the user picks the profile entry
- **THEN** the mapping SHALL be the profile `fast`, not the CLI `fast`

#### Scenario: default_profile is the starting entry

- **GIVEN** `default_profile = "routine"` and `default_cli = "alpha"`
- **WHEN** the picker is shown
- **THEN** the `routine` entry SHALL be pre-selected

#### Scenario: No profiles leaves the picker unchanged

- **GIVEN** no `[profiles]` table
- **WHEN** the picker is built
- **THEN** its items SHALL be exactly the available CLIs in their previous order and format
