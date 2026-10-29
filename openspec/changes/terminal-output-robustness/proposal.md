## Why

The pre-freeze DX item asks git-paw to honour `NO_COLOR`, degrade on limited-palette terminals, and
render cleanly at narrow widths and in non-TTY / piped output (CI logs). Verified state:

- **CLI commands are already clean**: `status`, `doctor`, `list-clis` etc. print plain text (emoji /
  ✓⚠✗ glyphs, no ANSI); the only escape (`\x1b[1m` in the root `after_help`) goes through clap's
  `anstream`, which strips it for non-terminals and under `NO_COLOR`. Nothing guards this, so it can
  regress silently.
- **The dashboard ignores `NO_COLOR` in a way that breaks it**: crossterm suppresses colours under
  `NO_COLOR`, and the broker-log selection highlight (`bg(Blue)`) and active filter chip (`bg(Cyan)`)
  are colour-only, so the selected row and active filter become invisible (inferred from the crossterm
  0.28 / ratatui 0.29 code paths; to verify on a live terminal).
- **The dashboard has no minimum-size handling**: the chip line (~120 columns) and title are clipped,
  the agent table needs ~55 columns, and only the table height has a floor (`MIN_AGENT_TABLE_HEIGHT`).
- **tmux border affordances** hard-code 256-colour indices (`colour238`, `colour45`, `colour39`) with no
  `NO_COLOR` path.
- The dashboard already uses only the 16 named colours — so it is non-truecolor-safe today — but
  nothing pins that either.

## What Changes

- Policy (new `core-terminal-output`): human-readable output of every subcommand contains no terminal
  escape sequences when stdout is not a terminal or `NO_COLOR` is set; an explicit colour request
  (`replay --color`) is the only exception. Guarded by a test that runs each subcommand piped.
- Every colour git-paw renders itself (dashboard, tmux border styling) honours `NO_COLOR`, and uses only
  the 16 named ANSI colours — never RGB or 256-colour indices in the dashboard — so limited-palette
  terminals degrade by palette mapping, not by breaking.
- Dashboard: selection and active chip stay distinguishable without colour (reverse/bold/underline);
  the chip line abbreviates to fit the width; below the minimum size the dashboard renders a single
  "terminal too small" notice instead of a clipped layout, and recovers when resized.
- tmux borders: with `NO_COLOR` set at launch, border affordances use attributes only (no colours).

## Capabilities

### New Capabilities

- `core-terminal-output`: the cross-cutting terminal-output policy — no escapes in non-terminal or
  `NO_COLOR` output, `NO_COLOR` honoured by every git-paw-rendered colour, the named-palette rule.

### Modified Capabilities

- `broker-dashboard`: ADDED colourless distinguishability and minimum-size rendering; ADDED chip-line
  abbreviation.
- `supervisor-launch`: **Active pane visually distinct** gains the `NO_COLOR` attribute-only form.

## Impact

- **Code:** `src/dashboard/broker_log.rs` (chip and selection styles; chip abbreviation),
  `src/dashboard/view.rs` (minimum-size check and notice), `src/dashboard.rs` (read `NO_COLOR` once into
  the `Model`), `src/tmux/command.rs` (border style strings), a small `terminal_env` helper (pure:
  `colour_enabled(no_color_var, is_tty)`), `tests/` standing guard over piped subcommand output and a
  source audit for `Color::Rgb` / `Color::Indexed` under `src/dashboard/`.
- **Enum-variant ripple:** none.
- **Docs:** `docs/src/user-guide/dashboard.md` (small terminals, `NO_COLOR`), a short "Terminal
  support" note in `docs/src/installation.md` or the FAQ, configuration reference note on
  `border_affordances` + `NO_COLOR`.
- **Cross-version dependencies:** v0.18 `broker-host-separation` makes the dashboard a thin client
  (render code unchanged, still `src/dashboard/`). The v0.18 dashboard-survival question is settled:
  the dashboard stays as an opt-in usage-and-events view (`dashboard-usage-view`, this release). This
  release's `uniform-json-output` already states "no ANSI" for `--json`; this change covers human
  output. `dashboard-usage-view` archives after this change: it rewrites the minimum-size requirement
  on this change's text for the usage panel, and its view keeps the `NO_COLOR` and named-palette
  rules.
