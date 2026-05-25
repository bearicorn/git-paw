## ADDED Requirements

### Requirement: Spec multi-select picker

The `Prompter` trait SHALL include a `select_specs(&self, specs: &[SpecEntry]) -> Result<Vec<SpecEntry>, PawError>` method that presents a multi-select picker for spec entries and returns the user's chosen subset.

The default `TerminalPrompter` implementation SHALL display one row per logical spec unit (a *feature* in Spec Kit terms; a *change* in OpenSpec; a *file* in plain Markdown), grouping multiple `SpecEntry` values that decompose from the same Spec Kit feature into a single row. Selecting a row SHALL cause every `SpecEntry` belonging to that row's logical unit to be returned.

Each row's display label SHALL include the unit identifier and, for Spec Kit features that decompose into multiple worktrees, a worktree-count hint summarising the breakdown (e.g. `"003-user-list — 3 worktrees: 2 [P] + 1 phase/"`). For OpenSpec changes and Markdown specs, the label SHALL be the unit identifier alone (one entry → one worktree, no hint needed).

The picker SHALL behave the same way as `select_branches` for cancellation:
- User pressing Ctrl+C → `PawError::UserCancelled`.
- User confirming with zero rows selected → `PawError::UserCancelled`.

#### Scenario: select_specs returns the chosen subset

- **GIVEN** 3 OpenSpec entries `add-auth`, `fix-session`, `add-logging`
- **WHEN** the user toggles `add-auth` and `add-logging` and presses enter
- **THEN** `select_specs` returns a `Vec` containing those two entries

#### Scenario: select_specs groups Spec Kit entries by feature

- **GIVEN** 4 SpecEntry values from a Spec Kit project: two `[P]` entries (`003-user-list-T009`, `003-user-list-T010`), one consolidated entry (`003-user-list-phase-2`), and one entry from a different feature (`004-error-handling-phase-1`)
- **WHEN** the picker renders
- **THEN** it displays exactly 2 rows — one per logical feature
- **AND** the row for feature `003-user-list` shows a worktree-count hint summarising the 3 underlying entries (`2 [P] + 1 phase/`)

#### Scenario: Selecting a Spec Kit feature row pulls in all its entries

- **GIVEN** a picker rendering one row for feature `003-user-list` (3 underlying SpecEntry values)
- **WHEN** the user selects only that row and confirms
- **THEN** `select_specs` returns all 3 underlying SpecEntry values

#### Scenario: User cancels spec picker via Ctrl+C

- **GIVEN** the spec picker is open
- **WHEN** the user presses Ctrl+C
- **THEN** `select_specs` returns `Err(PawError::UserCancelled)`

#### Scenario: User confirms with zero rows selected

- **GIVEN** the spec picker is open with N rows displayed
- **WHEN** the user confirms without toggling any row
- **THEN** `select_specs` returns `Err(PawError::UserCancelled)`

### Requirement: Spec picker requires an interactive terminal

When the start command would invoke `select_specs` (i.e. the user passed `--specs` with no values), the system SHALL detect whether stdin is connected to a terminal before invoking the picker. If stdin is NOT a terminal (CI, scripted invocation, redirected input), the system SHALL exit with an actionable error pointing at the explicit forms (`--specs NAME[,NAME...]` to narrow, `--from-all-specs` to launch every discovered spec).

The system SHALL NOT block waiting for picker input on a non-interactive stdin.

#### Scenario: Bare --specs in non-TTY environment exits with guidance

- **GIVEN** the user runs `git paw start --specs` with stdin redirected (or no controlling terminal)
- **WHEN** the start command attempts to open the picker
- **THEN** the command SHALL exit with a non-zero status before any picker UI is drawn
- **AND** the error message SHALL point the user at `--specs NAME[,NAME...]` and `--from-all-specs`

#### Scenario: Bare --specs on TTY proceeds to picker

- **GIVEN** the user runs `git paw start --specs` from an interactive terminal
- **WHEN** the start command runs
- **THEN** the picker SHALL open
- **AND** no TTY-required error SHALL be emitted

### Requirement: Spec name resolution for narrow mode

When `--specs` is passed with one or more values (narrow mode), the system SHALL resolve each value against the discovered `SpecEntry` set returned by `scan_specs()`. Resolution SHALL apply the following matching strategies in order, taking the first that succeeds:

1. **Exact match** on `SpecEntry.id` (case-sensitive). For Spec Kit, this matches a specific decomposed entry like `003-user-list-T009`. For OpenSpec / Markdown, it matches the change name or filename stem.
2. **Spec Kit feature match** on the feature directory prefix of the `SpecEntry.id` (e.g. `003-user-list` matches all entries belonging to that feature). When the value matches a Spec Kit feature unambiguously, ALL entries belonging to that feature SHALL be selected.
3. **Spec Kit numeric prefix match** (e.g. `003`) matching a Spec Kit feature directory name's leading numeric portion. The match SHALL succeed only when exactly one feature directory begins with the given prefix followed by a non-digit boundary; ambiguous prefixes SHALL be rejected (see below).

Resolution SHALL fail (and the start command SHALL exit before any worktrees are created) when:
- A value matches no `SpecEntry` and no feature.
- A Spec Kit numeric prefix matches more than one feature directory (ambiguous).

The resulting error SHALL list the unresolved or ambiguous names AND the discovered candidate names so the user can correct quickly.

#### Scenario: Exact match resolves to a single SpecEntry

- **GIVEN** a discovered set including OpenSpec change `add-auth`
- **WHEN** the user passes `--specs add-auth`
- **THEN** the resolved set SHALL contain exactly that one `SpecEntry`

#### Scenario: Spec Kit feature name resolves to all decomposed entries

- **GIVEN** a Spec Kit feature `003-user-list` decomposing into 3 SpecEntry values (2 `[P]` + 1 consolidated)
- **WHEN** the user passes `--specs 003-user-list`
- **THEN** the resolved set SHALL contain all 3 entries belonging to that feature

#### Scenario: Spec Kit numeric prefix resolves unambiguously

- **GIVEN** a Spec Kit project with a single feature directory beginning with `003-` (e.g. `003-user-list`)
- **WHEN** the user passes `--specs 003`
- **THEN** the resolved set SHALL contain all entries belonging to that feature

#### Scenario: Ambiguous numeric prefix is rejected

- **GIVEN** a Spec Kit project containing both `003-user-list` and `003a-experiment`
- **WHEN** the user passes `--specs 003`
- **THEN** the start command SHALL exit with an error
- **AND** the error message SHALL list both candidate feature names

#### Scenario: Unknown spec name is rejected with candidate list

- **GIVEN** a discovered set containing `add-auth`, `fix-session`
- **WHEN** the user passes `--specs no-such-spec`
- **THEN** the start command SHALL exit with an error
- **AND** the error message SHALL include `no-such-spec`
- **AND** the error message SHALL list `add-auth` and `fix-session` as candidates

#### Scenario: Multiple values are resolved independently

- **GIVEN** a discovered set including `add-auth`, `fix-session`, `add-logging`
- **WHEN** the user passes `--specs add-auth,add-logging`
- **THEN** the resolved set SHALL contain entries for `add-auth` and `add-logging`
- **AND** the resolved set SHALL NOT contain the entry for `fix-session`

#### Scenario: Partial-failure batches do not partially start

- **GIVEN** a user passes `--specs add-auth,no-such-spec`
- **WHEN** resolution runs
- **THEN** the start command SHALL exit with the unknown-name error
- **AND** no worktrees SHALL be created
- **AND** the error message SHALL include `no-such-spec` (the unresolved name)
