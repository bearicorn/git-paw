## ADDED Requirements

### Requirement: Vendor naming is confined to the matching CLI fragment

The no-language-leak audit SHALL render every template that uses a CLI placeholder once per registered CLI family and once for `generic`, and SHALL treat a vendor product name or vendor-specific path as allowed only within text that came from that family's own fragment. Base templates and every `generic` fragment SHALL contain no vendor product name or path outside an explicitly-allowed span. A fragment that names a vendor other than its own family's SHALL fail the audit.

#### Scenario: Generic rendering is vendor-free

- **WHEN** the coordination skill and both boot-block forms are rendered with the `generic` fragment
- **THEN** the audit SHALL find no vendor product name or vendor path outside an allowed span

#### Scenario: A fragment naming another vendor fails

- **GIVEN** a copy of the `claude` fragment edited to name a different vendor's CLI
- **WHEN** the audit runs
- **THEN** it SHALL fail naming the fragment and the offending token

#### Scenario: A vendor name in a base template fails

- **GIVEN** a copy of the coordination `SKILL.md` with a vendor product name added outside any fragment and any allowed span
- **WHEN** the audit runs
- **THEN** it SHALL fail naming the offending token
