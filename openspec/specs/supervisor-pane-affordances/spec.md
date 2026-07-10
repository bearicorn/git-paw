# supervisor-pane-affordances Specification

## Purpose
Applies tmux visual affordances to git-paw sessions — double-line pane borders, per-pane role labels via a stable `@paw_role` option, a reverse-video border-format header bar, and active-pane border styling — gated by the `[layout].border_affordances` config field and degrading gracefully on older tmux.

## Requirements
### Requirement: Session builder applies double-line borders

The tmux session builder SHALL set
`pane-border-lines double` on the `paw-<project>` session
immediately after the session is created. The option SHALL be
scoped to the session (`tmux set-option -t <session>`), not
to the tmux server or to other windows. Double lines (`═║`) read
as a stronger row separator than single/heavy lines; tmux has no
inter-pane margin or padding (panes tile flush), so the divider
weight and the label bar are the only levers for perceived
separation between rows.

#### Scenario: Double-line border option is set on the session

- **WHEN** the session builder constructs a new
  `paw-<project>` session
- **THEN** the resulting `tmux set-option` invocations
  SHALL include `-t paw-<project> pane-border-lines double`

#### Scenario: Option does not leak to other sessions

- **GIVEN** another tmux session unrelated to git-paw
- **WHEN** the git-paw session builder runs
- **THEN** the other session's `pane-border-lines` setting
  SHALL be unchanged (verified via
  `tmux show-options -t <other-session> -v
  pane-border-lines`)

### Requirement: Per-pane title labelling

The session builder SHALL set each pane's title via
`tmux select-pane -t <pane> -T '<title>'` after pane
creation. Pane 0 SHALL receive the title `supervisor`. Pane 1
SHALL receive the title `dashboard`. Each agent pane SHALL
receive a title equal to its branch_id (e.g.
`feat/cold-start-ci-parity`).

In addition to `select-pane -T`, the session builder SHALL set a
pane-scoped user option `@paw_role` to the same label via
`tmux set-option -p -t <pane> @paw_role '<title>'`. This option is
the authoritative, stable source of the border label: the agent CLI
running in a pane emits OSC title escape sequences that overwrite
`#{pane_title}` with its current activity (e.g. `Searching files…`),
so the `select-pane -T` value does not survive past the CLI's first
title update. The `@paw_role` pane option is git-paw's own and is
never overwritten by the CLI, so the role label remains stable for
the life of the pane. The `set-option -p @paw_role` call SHALL be a
*soft* command (a non-zero exit on older tmux warns and the build
continues, matching the border affordances).

#### Scenario: Each pane gets a stable @paw_role option

- **GIVEN** an agent attached at pane index N for branch `feat/foo`
- **WHEN** the session builder completes
- **THEN** `tmux show-options -p -t paw-<project>:0.N @paw_role`
  SHALL return `feat/foo`, and this value SHALL NOT change when the
  CLI subsequently sets `#{pane_title}` via an OSC sequence

#### Scenario: Supervisor pane title is supervisor

- **WHEN** the session builder completes
- **THEN** `tmux display-message -t paw-<project>:0.0 -p
  '#{pane_title}'` SHALL return `supervisor`

#### Scenario: Dashboard pane title is dashboard

- **WHEN** the session builder completes
- **THEN** `tmux display-message -t paw-<project>:0.1 -p
  '#{pane_title}'` SHALL return `dashboard`

#### Scenario: Agent pane title is the branch id

- **GIVEN** an agent attached at pane index N for branch
  `feat/foo`
- **WHEN** the session builder completes
- **THEN** `tmux display-message -t paw-<project>:0.N -p
  '#{pane_title}'` SHALL return `feat/foo`

#### Scenario: Add via git paw add sets the new pane's title

- **GIVEN** an active session and the user runs
  `git paw add feat/bar` per [[git-paw-add]]
- **WHEN** the new pane is created
- **THEN** the new pane's title SHALL be `feat/bar`

### Requirement: Pane border format renders the role label

The session builder SHALL set `pane-border-format` to a reverse-video
label bar —
`#[fg=colour39,bold,reverse] #{pane_index}: #{?#{@paw_role},#{@paw_role},#{pane_title}} #[default]`
— and `pane-border-status top` so each pane shows its index and role
label as a colored header chip above the pane content (the reverse-video
styling makes the label read as a header bar rather than plain text on
the divider line, aiding row separation). The format SHALL prefer the
pane-scoped `@paw_role` option (set per [Per-pane title labelling]) and
fall back to `#{pane_title}` only when `@paw_role` is unset (e.g. a
user-created pane). This keeps the role label stable even after the agent
CLI overwrites `#{pane_title}` with its current activity via OSC title
escape sequences.

#### Scenario: Border format is the reverse-video bar preferring @paw_role

- **WHEN** the session builder completes
- **THEN** the session's `pane-border-format` SHALL be exactly
  `#[fg=colour39,bold,reverse] #{pane_index}: #{?#{@paw_role},#{@paw_role},#{pane_title}} #[default]`,
  and `pane-border-status` SHALL be `top`

#### Scenario: Role label survives a CLI title overwrite

- **GIVEN** a built session where pane 0's `@paw_role` is `supervisor`
- **WHEN** the CLI in pane 0 emits an OSC sequence that sets
  `#{pane_title}` to `Thinking…`
- **THEN** the rendered border label for pane 0 SHALL still read
  `0: supervisor` (the format resolves `@paw_role`, not `#{pane_title}`)

### Requirement: Active pane visually distinct

The session builder SHALL set
`pane-active-border-style fg=colour45,bold` and
`pane-border-style fg=colour238` so the focused pane's
border colour visually stands out from the others.

#### Scenario: Active border style is applied

- **WHEN** the session builder completes
- **THEN** the session's `pane-active-border-style` SHALL
  contain `colour45,bold`, and `pane-border-style` SHALL contain
  `colour238`

### Requirement: border_affordances config field

The system SHALL accept `[layout].border_affordances` as a
boolean config field defaulting to `true`. When `false`, the
session builder SHALL skip every `set-option` invocation and
every `select-pane -T` call described in this capability,
leaving the user's tmux defaults in effect.

#### Scenario: Default true applies all affordances

- **GIVEN** no `[layout]` section in config (or
  `border_affordances` unset)
- **WHEN** the session builder runs
- **THEN** all five `set-option` invocations and the
  per-pane title sets SHALL be emitted

#### Scenario: Explicit false skips all affordances

- **GIVEN** `[layout].border_affordances = false`
- **WHEN** the session builder runs
- **THEN** none of the five `set-option` invocations and
  none of the per-pane title sets SHALL be emitted; the
  session SHALL inherit the user's default tmux styling

### Requirement: Graceful degradation on older tmux

The session builder SHALL tolerate `tmux set-option` failures
for options unsupported by older tmux versions. The builder
SHALL emit a stderr warning naming the unsupported option and
SHALL continue building the session.

#### Scenario: Unsupported option produces a stderr warning

- **GIVEN** a tmux version where `pane-border-lines double`
  is not recognised (pre-3.2)
- **WHEN** the session builder runs
- **THEN** the build SHALL complete without fatal error,
  and stderr SHALL contain a warning naming the unsupported
  option

#### Scenario: Other affordances still apply when one fails

- **GIVEN** the same older-tmux scenario where
  `pane-border-lines double` fails
- **WHEN** the session builder runs
- **THEN** the other affordances (title format, status
  position, active-border style) SHALL still be set, since
  they have shipped in tmux since 2.3

### Requirement: Applies to both supervisor and non-supervisor sessions

The pane affordances SHALL apply to every git-paw-managed
tmux session regardless of supervisor mode. The
`[layout].border_affordances` config field SHALL govern both
`git paw start` (no supervisor) and `git paw start
--supervisor` paths.

#### Scenario: Non-supervisor session also receives affordances

- **GIVEN** a `git paw start` (no `--supervisor`) session
  with `border_affordances = true`
- **WHEN** the session builder completes
- **THEN** all the documented affordances SHALL be applied
  to the non-supervisor session's panes

