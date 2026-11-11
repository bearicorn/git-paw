## ADDED Requirements

### Requirement: The no-leak audit covers every registered package file

The no-language-leak audit SHALL derive its audited set from the bundled package registry, covering every text file of every registered package — `SKILL.md` and every file under `references/` and `assets/`, rendered where templated — so a newly registered package, reference or asset is audited without editing the audit. Shell helpers under `scripts/` SHALL stay covered by their existing convention and stack-neutrality tests instead of the prose audit.

#### Scenario: A newly registered reference is audited automatically

- **GIVEN** a reference file added to a registered package that contains a forbidden stack token outside an allowed span
- **WHEN** the no-leak audit runs with no change to the audit itself
- **THEN** the audit SHALL fail naming that file and token

#### Scenario: Every registered prose file is in the audited set

- **WHEN** the audit's input set is compared with the registry
- **THEN** every registered `SKILL.md`, reference and asset text file SHALL be in the audited set
