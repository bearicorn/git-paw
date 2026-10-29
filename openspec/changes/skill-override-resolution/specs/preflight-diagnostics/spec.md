## ADDED Requirements

### Requirement: Doctor reports where each bundled skill resolves from

`git paw doctor` SHALL report one check named `skill sources`, in the group that reports the bundled packages, that resolves every package in the bundled package registry through the production skill resolver for the repository under diagnosis and names, for each package, the level that supplied it: `embedded`, `repo` followed by the winning `SKILL.md` path, or `user` followed by the winning `SKILL.md` path. The check SHALL be ✓ when every package resolves, whatever its source. When an override directory exists, at either level, for a skill that is not overridable (`cli-setup`), the check SHALL report ⚠ naming that directory and stating that it is ignored, with a remedy to remove it, and SHALL still name that package as `embedded`. When any package fails to resolve, the check SHALL be ✗, naming the package and the failing override path, with a remedy to fix or remove that override. The check SHALL only read files and SHALL appear in `--json` output like every other check.

#### Scenario: All bundled skills embedded

- **GIVEN** no repository or user override for any bundled package
- **WHEN** `git paw doctor` is run
- **THEN** the `skill sources` check SHALL be ✓
- **AND** its detail SHALL name every registered package with `embedded`

#### Scenario: A user override is named with its path

- **GIVEN** a user override for `coordination` and no repository override
- **WHEN** `git paw doctor` is run
- **THEN** the `skill sources` detail SHALL name `coordination` as `user` with the override's `SKILL.md` path
- **AND** the check SHALL be ✓

#### Scenario: An ignored cli-setup override warns

- **GIVEN** a repository override directory `<main>/.agents/skills/cli-setup/` containing a `SKILL.md`
- **WHEN** `git paw doctor` is run
- **THEN** the `skill sources` check SHALL be ⚠
- **AND** its detail SHALL name `cli-setup` as `embedded` and name the override directory as ignored
- **AND** it SHALL NOT be ✗

#### Scenario: A broken override fails the check

- **GIVEN** a repository override for `coordination` whose frontmatter is invalid
- **WHEN** `git paw doctor` is run
- **THEN** the `skill sources` check SHALL be ✗
- **AND** its detail SHALL name `coordination` and the override's path
- **AND** its remedy SHALL say to fix or remove that override

#### Scenario: The check appears in JSON output

- **WHEN** `git paw doctor --json` is run
- **THEN** the output SHALL contain a check named `skill sources` with its status and detail
