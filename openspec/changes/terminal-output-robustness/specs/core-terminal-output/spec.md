## ADDED Requirements

### Requirement: Human-readable output carries no escape sequences when not on a terminal

Every git-paw subcommand's human-readable output SHALL contain no terminal escape sequences (no ESC byte) when its standard output is not a terminal, and SHALL contain none when the `NO_COLOR` environment variable is set to a non-empty value. The only exception SHALL be output the user explicitly requested in colour (`git paw replay --color`). A standing test SHALL run each non-interactive subcommand with stdout piped, and again with `NO_COLOR=1`, and fail on any ESC byte.

#### Scenario: Piped output is plain

- **WHEN** `git paw status`, `git paw doctor`, `git paw list-clis` and `git paw --help` run with stdout piped to a file
- **THEN** none of the outputs SHALL contain an ESC byte

#### Scenario: NO_COLOR suppresses escapes

- **GIVEN** `NO_COLOR=1` in the environment
- **WHEN** `git paw --help` runs
- **THEN** its output SHALL contain no ESC byte

#### Scenario: Explicit colour request is honoured

- **WHEN** the user runs `git paw replay --color <branch>`
- **THEN** colour escape sequences from the recording MAY be passed through

### Requirement: Colours git-paw renders honour NO_COLOR and the named palette

Every colour git-paw applies itself — dashboard styles and the tmux border styling of git-paw sessions — SHALL be omitted when `NO_COLOR` is set to a non-empty value in git-paw's environment at launch, and the information it conveyed SHALL remain visible through text attributes (bold, reverse, underline, dim). The dashboard SHALL use only the 16 named ANSI colours, never RGB or 256-colour indexed colours, so terminals with a limited palette render it through their own palette mapping. When `NO_COLOR` is set at launch, git-paw SHALL copy it into the session environment so processes it starts inside the session see it too.

#### Scenario: Dashboard source uses only named colours

- **WHEN** the dashboard source under `src/dashboard/` is audited
- **THEN** it SHALL contain no `Color::Rgb` and no `Color::Indexed`

#### Scenario: NO_COLOR reaches the dashboard

- **GIVEN** `NO_COLOR=1` when the session is launched
- **WHEN** the dashboard process starts inside the session
- **THEN** it SHALL see `NO_COLOR` set and render without colours
