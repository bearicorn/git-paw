## ADDED Requirements

### Requirement: The exit summary attributes usage per profile

When any worker of the run was launched with a profile, or any delegate run started during the drive loop's run, the exit summary's usage section SHALL add one line per profile with its known usage and its count of workers or runs with unknown usage, labelled worker profile or delegate profile. The existing lines, totals and budget comparison SHALL be unchanged; profiled workers SHALL still count in their CLI's line.

#### Scenario: Profiles appear in the usage section

- **GIVEN** a drive loop whose run had two workers on profile `routine` (CLI `beta`) and one worker on plain CLI `alpha`, and during which one `spec-audit` delegate run completed
- **WHEN** the summary is printed
- **THEN** its usage section SHALL show the `beta` and `alpha` CLI totals as before, a `routine` worker-profile total, and a `spec-audit` delegate-profile total

#### Scenario: No profile leaves the usage section unchanged

- **GIVEN** a drive loop whose workers all used plain CLIs and during which no delegate ran
- **WHEN** the summary is printed
- **THEN** the usage section SHALL be byte-identical to the section printed before this requirement for the same figures
