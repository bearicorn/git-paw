## MODIFIED Requirements

### Requirement: Active pane visually distinct

The session builder SHALL set
`pane-active-border-style fg=colour45,bold` and
`pane-border-style fg=colour238` so the focused pane's
border colour visually stands out from the others. When `NO_COLOR`
is set to a non-empty value in git-paw's environment at launch, the
session builder SHALL instead set `pane-active-border-style bold`
and `pane-border-style default`, and the per-pane label strip SHALL
use attributes only (bold, reverse) with no colour, so the focused
pane stays distinguishable without colour.

#### Scenario: Active border style is applied

- **WHEN** the session builder completes without `NO_COLOR` set
- **THEN** the session's `pane-active-border-style` SHALL
  contain `colour45,bold`, and `pane-border-style` SHALL contain
  `colour238`

#### Scenario: NO_COLOR uses attributes only

- **GIVEN** `NO_COLOR=1` at launch
- **WHEN** the session builder completes
- **THEN** `pane-active-border-style` SHALL be `bold`, `pane-border-style` SHALL be `default`
- **AND** no border or label style git-paw sets SHALL contain a `colour` or `fg=`/`bg=` term
