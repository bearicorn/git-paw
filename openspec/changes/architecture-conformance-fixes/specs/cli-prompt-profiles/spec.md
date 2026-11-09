## ADDED Requirements

### Requirement: A selection menu's shape comes from the profile

The profile SHALL carry `menu_cursor_glyphs` and `menu_footer_markers`, the glyphs that mark a menu's selected option and the footer phrases that mark a menu. The embedded default SHALL be `❯`, `›`, `>` and `enter to select`, footers matched case-insensitively. A `[clis.<name>].prompt_profile` override MAY replace either field; an empty list SHALL fail the load naming the field. The fields describe appearance only.

#### Scenario: The default profile detects a menu as before

- **GIVEN** an agent whose CLI has no prompt-profile override
- **WHEN** `__send` reads a screen ending in `❯ 1. Send`, `2. Wait` and `Enter to select`
- **THEN** it SHALL refuse with `selection menu showing` and send no keystroke

Test: `src/commands/send.rs::a_selection_menu_blocks_the_send`, `src/supervisor/screen.rs::the_default_glyphs_and_footer_mark_a_menu`

#### Scenario: An overridden cursor glyph is honoured

- **GIVEN** `[clis.mycli].prompt_profile` with `menu_cursor_glyphs = ["»"]`
- **WHEN** `__send` to an agent running `mycli` reads a screen whose last lines are `» 1. Send` and `2. Wait`
- **THEN** it SHALL refuse with `selection menu showing`

Test: `src/commands/send.rs::an_overridden_cursor_glyph_is_honoured`, `src/supervisor/screen.rs::an_overridden_cursor_glyph_replaces_the_defaults`

#### Scenario: An overridden footer replaces the default

- **GIVEN** `[clis.mycli].prompt_profile` with `menu_footer_markers = ["press space to choose"]`
- **WHEN** `__send` to an agent running `mycli` reads a screen whose last line is `Press space to choose`
- **THEN** it SHALL refuse with `selection menu showing`

Test: `src/commands/send.rs::an_overridden_footer_replaces_the_default`, `src/supervisor/screen.rs::an_overridden_footer_replaces_the_default`

#### Scenario: The helper command prints the menu fields

- **WHEN** `git paw __prompt-profile --cli claude --field menu_cursor_glyphs` is run with no override
- **THEN** it SHALL print the embedded default glyphs

Test: `tests/prompt_profile_threading.rs::prompt_profile_subcommand_prints_the_menu_fields`

#### Scenario: An empty menu list fails the load

- **GIVEN** `[clis.mycli].prompt_profile` with `menu_cursor_glyphs = []` or `menu_footer_markers = []`
- **WHEN** the configuration is loaded
- **THEN** loading SHALL fail with an error naming that field

Test: `tests/prompt_profile_threading.rs::an_empty_menu_list_fails_the_load_naming_the_field`
