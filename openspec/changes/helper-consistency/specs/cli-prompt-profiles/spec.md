## ADDED Requirements

### Requirement: Compiled readers take the menu cursor glyph from the profile

The permission-prompt detector and the launch-readiness dialog answerer SHALL recognise an option line behind a selection cursor using the pane's profile `menu_cursor_glyphs`, not a compiled glyph. A cursor glyph SHALL be skipped only where an option line follows it. Box-drawing borders and bullets MAY stay compiled.

#### Scenario: An overridden cursor glyph marks a live permission prompt

- **GIVEN** `[clis.mycli].prompt_profile` with `menu_cursor_glyphs = ["»"]`, and a `mycli` pane whose permission prompt renders option one as `» 1. Yes`
- **WHEN** the permission-prompt detector reads the pane
- **THEN** it SHALL find option one

#### Scenario: The readiness gate answers a dialog drawn with an overridden glyph

- **GIVEN** a CLI whose profile sets `menu_cursor_glyphs = ["»"]`, and a recognised first-run dialog whose affirmative option renders as `» 2. Yes, continue`
- **WHEN** the readiness gate classifies the pane
- **THEN** it SHALL answer with digit 2

#### Scenario: A leading redirect is not taken for a cursor

- **GIVEN** the default profile, whose glyphs include `>`
- **WHEN** the detector cleans the line `> /dev/null`
- **THEN** the `>` SHALL be kept

### Requirement: The helper script takes the menu cursor glyph from the profile

The bundled orchestrator helper script SHALL read `menu_cursor_glyphs` from the resolved profile of each pane's CLI and SHALL use those glyphs, not a literal of its own, where its option-line and footer-line patterns skip a selection cursor. A glyph SHALL be matched literally, not as a regex.

#### Scenario: sweep.sh approve honours an overridden glyph

- **GIVEN** a `mycli` pane whose profile sets `menu_cursor_glyphs = ["»"]`, showing a live permission prompt whose option one renders as `» 1. Yes`
- **WHEN** `sweep.sh approve` targets that pane
- **THEN** it SHALL find option one and send `1` and then `Enter`

#### Scenario: No cursor glyph is defined in the script

- **WHEN** the helper script's option-line and footer-line patterns are inspected
- **THEN** they SHALL contain no menu cursor glyph literal outside the profile fallback
