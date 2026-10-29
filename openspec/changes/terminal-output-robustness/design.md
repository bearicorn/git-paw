## Context

No colour library is used in `src/` (`console` is only dialoguer's dependency). Human output is
`println!`. clap 4 with the `color` feature routes help through `anstream::AutoStream`, which honours
`NO_COLOR`, `CLICOLOR(_FORCE)` and terminal detection. The dashboard (ratatui 0.29 / crossterm 0.28)
styles: chips `fg(Black).bg(Cyan)`+BOLD (active) / `fg(DarkGray)`; selection
`bg(Blue).fg(White)`+BOLD; overlay `bg(Black)`; roster uses BOLD/DIM only. crossterm drops colours under
a non-empty `NO_COLOR` and still emits an SGR reset in `SetColors`, which can cancel a preceding BOLD
(to verify). Layout constraints: title `Length(1)`, table `Min(MIN_AGENT_TABLE_HEIGHT=6)`, status
`Length(1)`, log `Length(height_lines)`; table widths `Min(15), Length(10), Length(15), Length(12)`.
tmux styles (`tmux/command.rs`): `pane-border-style fg=colour238`, `pane-active-border-style
fg=colour45,bold`, label `#[fg=colour39,bold,reverse]`.

## Goals / Non-Goals

**Goals:** lock today's clean CLI output with a guard; make the dashboard usable under `NO_COLOR` and in
small terminals; give tmux styling a colourless form.

**Non-Goals:** ASCII fallbacks for emoji/glyphs (UTF-8 is assumed; not asked); theming; changing
`replay --color` (explicit request wins); the dashboard's content and layout (`dashboard-usage-view`).

## Decisions

- **D1 — One pure predicate.** `colour_enabled(no_color: Option<&OsStr>, stdout_is_tty: bool)`:
  disabled when `NO_COLOR` is present and non-empty (the no-color.org rule) or stdout is not a
  terminal. Read once at startup; the dashboard keeps it in its `Model`, so `view` stays pure and
  testable with `TestBackend`.
- **D2 — Distinguish by attribute, colour as decoration.** Selection = `REVERSED` (+ colour when
  enabled); active chip = `REVERSED`+`BOLD`; inactive chip = `DIM`. Under `NO_COLOR` no `fg`/`bg` is set
  at all, so crossterm emits no colour SGR that could reset the attributes.
- **D3 — Minimum size derived, not invented.** `min_width` = the sum of the agent table's minimum
  column widths plus borders/spacing; `min_height` = title + `MIN_AGENT_TABLE_HEIGHT` + status (+ the
  log panel's minimum when visible). Below it, render one centred line `terminal too small: W×H (need
  ≥ w×h)`; key handling continues, and the next frame after a resize renders normally.
- **D4 — Chip abbreviation.** Full labels when they fit; otherwise `digit:short` (e.g. `1:st`);
  otherwise digits only. Pure function of width, unit-tested.
- **D5 — Standing guards.** (a) An integration test runs every non-interactive subcommand with stdout
  piped (and once with `NO_COLOR=1`) and asserts no `\x1b` byte; (b) a source audit asserts no
  `Color::Rgb` / `Color::Indexed` under `src/dashboard/`.
- **D6 — tmux under NO_COLOR.** When `NO_COLOR` is set in git-paw's environment at launch, border
  styles become `pane-border-style default` and `pane-active-border-style bold`, label `#[bold,reverse]`.

## Risks / Trade-offs

- [`NO_COLOR` in the operator's shell but not inherited by the dashboard pane] → the dashboard process
  is launched by tmux with the session environment; git-paw copies `NO_COLOR` into the session
  environment when set at launch (same mechanism as `GIT_PAW_BROKER_URL`).
- [Crossterm reset behaviour is inferred] → the `TestBackend` tests assert the attributes git-paw sets;
  a manual smoke under `NO_COLOR=1` is recorded in the task list.
