## 1. Guards first (lock current behaviour)

- [ ] 1.1 `tests/terminal_output_guard.rs`: run each non-interactive subcommand (`status`, `doctor`, `list-clis`, `--help`, each subcommand's `--help`, `approvals`, `config check` once it exists) with stdout piped, and again with `NO_COLOR=1`; assert no ESC byte (isolated `HOME`/`XDG_DATA_HOME`)
- [ ] 1.2 Source audit (in `tests/source_audit.rs`): no `Color::Rgb` / `Color::Indexed` under `src/dashboard/`

## 2. Colour predicate

- [ ] 2.1 Pure `colour_enabled(no_color, stdout_is_tty)` (non-empty `NO_COLOR` disables); unit tests
- [ ] 2.2 Copy `NO_COLOR` into the tmux session environment at launch when set (same path as `GIT_PAW_BROKER_URL`), all launch paths incl. recovery

## 3. Dashboard

- [ ] 3.1 Store the colour flag in the dashboard `Model`; selection = `REVERSED`, active chip = `REVERSED|BOLD`, inactive = `DIM`; no `fg`/`bg` when disabled
- [ ] 3.2 Minimum size computed from the layout constraints; too-small notice frame; key handling unchanged
- [ ] 3.3 Chip-row abbreviation as a pure function of width
- [ ] 3.4 `TestBackend` tests: selection and active chip attributes with colours off; 30×8 notice; resize restores layout; `q` quits while too small; 60-column chip row has no cut chip

## 4. tmux styling

- [ ] 4.1 `tmux/command.rs`: attribute-only border and label styles when `NO_COLOR` is set at launch; builder tests for both modes

## 5. Docs

- [ ] 5.1 `docs/src/user-guide/dashboard.md` (small terminals, `NO_COLOR`); terminal-support note (installation or FAQ); configuration reference note under `[layout] border_affordances`

## 6. Gates

- [ ] 6.1 Manual smoke under `NO_COLOR=1` and an 80×24 terminal recorded in the PR; `cargo fmt`; `just check`; `just deny`; `mdbook build docs/`
