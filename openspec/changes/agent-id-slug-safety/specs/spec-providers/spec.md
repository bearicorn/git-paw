## MODIFIED Requirements

### Requirement: Branch-name shape for SpecKit entries

The system SHALL produce branch names using the spec providers' branch-segment slug — lowercase ASCII, replace every character not in `[a-z0-9_]` with `-`, collapse runs of `-`, trim leading and trailing `-`, and fall back to `agent` when the result is empty — applied to the appropriate input string:

- For `[P]` entries: input is `<task-id>-<description>` (e.g. `T009-add-login-form`); branch becomes `task/<slugified-input>` → `task/t009-add-login-form`.
- For consolidated entries: input is `<feature-dir>-<phase-name>` (e.g. `003-user-list-Foundational`); branch becomes `phase/<slugified-input>` → `phase/003-user-list-foundational`.

Branch names SHALL contain only characters from the slug character set `[a-z0-9/_-]`; they MAY contain `_`. The broker `agent_id` of such a branch is derived separately by `slugify_branch` (`broker-protocol`, "Branch slug function"), which maps `_` to `-`, so a branch keeping `_` never yields an id the broker rejects.

#### Scenario: [P] entry produces task/ branch

- **WHEN** a `[P]` `SpecEntry` is produced for task `T009` with description `"Add login form component"` in feature `003-user-list`
- **THEN** the branch is `task/t009-add-login-form-component`

#### Scenario: Consolidated entry produces phase/ branch

- **WHEN** a consolidated `SpecEntry` is produced for phase 2 (`Foundational`) of feature `003-user-list`
- **THEN** the branch is `phase/003-user-list-foundational`

#### Scenario: Branch slug contains only safe characters

- **WHEN** any SpecKit `SpecEntry` branch is produced
- **THEN** the branch contains only characters from the slug set `[a-z0-9/_-]`

#### Scenario: A task naming an underscored file keeps the underscore in its branch

- **WHEN** a `[P]` `SpecEntry` is produced for task `T001` with description `"Create ROOM_SETUP.md"`
- **THEN** the branch is `task/t001-create-room_setup-md`
- **AND** `slugify_branch` of that branch is `task-t001-create-room-setup-md`, which the broker accepts as an `agent_id`

### Requirement: Branch-name shape for Superpowers entries

The system SHALL derive each entry's branch by applying the spec providers' branch-segment slug (the rules in "Branch-name shape for SpecKit entries") to the plan file stem, prefixed with `plan/`: `plan/<slugified-stem>`. Branch names SHALL contain only characters from the slug set `[a-z0-9/_-]`.

#### Scenario: Plan file produces a plan/ branch

- **WHEN** an entry is produced for `2026-07-20-Add-Auth.md`
- **THEN** the branch is `plan/2026-07-20-add-auth`

#### Scenario: Branch slug contains only safe characters

- **WHEN** any Superpowers `SpecEntry` branch is produced
- **THEN** it contains only characters from the slug set `[a-z0-9/_-]`

#### Scenario: An underscored plan stem keeps the underscore in its branch

- **WHEN** an entry is produced for `add_auth.md`
- **THEN** the branch is `plan/add_auth`
