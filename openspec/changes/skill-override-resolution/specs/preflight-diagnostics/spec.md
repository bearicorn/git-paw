## ADDED Requirements

### Requirement: Doctor reports where each bundled skill resolves from

`git paw doctor` SHALL report a read-only `skill sources` check in the Bundled-scripts group that resolves every registered package through the production resolver and names each one's source: `embedded`, `repo <SKILL.md path>` or `user <SKILL.md path>`. It SHALL be ✓ when all resolve, ⚠ naming an ignored `cli-setup` override directory (remedy: remove it), and ✗ naming the package and override path when one fails to resolve (remedy: fix or remove it).

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
