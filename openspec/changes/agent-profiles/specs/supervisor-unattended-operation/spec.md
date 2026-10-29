## ADDED Requirements

### Requirement: The exit summary attributes usage per profile

When any worker of the run was launched with a profile, or any delegate run started during the drive loop's run, the exit summary's usage section SHALL additionally report known usage per profile — worker profiles and delegate profiles labelled by kind — and the number of each profile's workers or runs whose usage is unknown. Workers launched without a profile SHALL stay grouped by CLI id as before. When no profile was used, the usage section SHALL be identical to the section printed before this requirement.

#### Scenario: Profiles appear in the usage section

- **GIVEN** a drive loop whose run had two workers on profile `routine` and one worker on plain CLI `alpha`, and during which one `spec-audit` delegate run completed
- **WHEN** the summary is printed
- **THEN** its usage section SHALL show a `routine` worker-profile total, an `alpha` CLI total, and a `spec-audit` delegate-profile total

#### Scenario: No profile leaves the usage section unchanged

- **GIVEN** a drive loop whose workers all used plain CLIs and during which no delegate ran
- **WHEN** the summary is printed
- **THEN** the usage section SHALL be byte-identical to the section printed before this requirement for the same figures
