## MODIFIED Requirements

### Requirement: Subset branch selection

The system SHALL support selecting a subset of available branches.

The branch picker (`select_branches`) SHALL present a fuzzy-filter
multi-select. The user SHALL be able to type a query that filters the visible
branch candidates by case-insensitive substring match against the branch name.
An empty query SHALL show the full list of available branches in their original
order. Toggling selection SHALL operate on the currently visible (filtered)
rows; branches selected under one query that are then hidden by a different
query SHALL remain selected and SHALL be returned on confirm. Clearing the
query (back to empty) SHALL restore the full list with all prior selections
intact.

Cancellation semantics SHALL be unchanged: pressing Ctrl+C SHALL yield
`PawError::UserCancelled`, and confirming with zero branches selected SHALL
yield `PawError::UserCancelled`.

#### Scenario: Selecting one of two branches
- **GIVEN** 2 available branches
- **WHEN** user selects only the second
- **THEN** only that branch SHALL appear in the result

Test: `interactive::tests::selecting_subset_of_branches_works`

#### Scenario: Typing a query filters the branch candidates
- **GIVEN** branches `feature/auth`, `fix/api`, `feature/login`
- **WHEN** the user types `feature` into the branch picker filter
- **THEN** only `feature/auth` and `feature/login` SHALL be visible
- **AND** `fix/api` SHALL NOT be visible

#### Scenario: Empty filter shows the full branch list
- **GIVEN** the branch picker with 3 candidates
- **WHEN** the filter query is empty
- **THEN** all 3 branches SHALL be visible in their original order

#### Scenario: Selection under an active filter is preserved when the filter changes
- **GIVEN** branches `feature/auth`, `fix/api`, `feature/login`
- **WHEN** the user types `feature`, toggles `feature/auth`, then changes the query to `fix` and toggles `fix/api`, then confirms
- **THEN** the result SHALL contain both `feature/auth` and `fix/api`

#### Scenario: Clearing the filter restores the full list with selections intact
- **GIVEN** the branch picker where the user has typed `feature` and toggled `feature/auth`
- **WHEN** the user clears the query back to empty
- **THEN** all branches SHALL be visible again
- **AND** `feature/auth` SHALL still be marked as selected

#### Scenario: User cancels the filtered branch picker via Ctrl+C
- **GIVEN** the branch picker is open with an active filter query
- **WHEN** the user presses Ctrl+C
- **THEN** `select_branches` SHALL return `Err(PawError::UserCancelled)`

#### Scenario: Confirming with zero branches selected cancels
- **GIVEN** the branch picker is open
- **WHEN** the user confirms without toggling any branch
- **THEN** `select_branches` SHALL return `Err(PawError::UserCancelled)`

### Requirement: Spec multi-select picker

The `Prompter` trait SHALL include a `select_specs(&self, specs: &[SpecEntry]) -> Result<Vec<SpecEntry>, PawError>` method that presents a multi-select picker for spec entries and returns the user's chosen subset.

The default `TerminalPrompter` implementation SHALL display one row per logical spec unit (a *feature* in Spec Kit terms; a *change* in OpenSpec; a *file* in plain Markdown), grouping multiple `SpecEntry` values that decompose from the same Spec Kit feature into a single row. Selecting a row SHALL cause every `SpecEntry` belonging to that row's logical unit to be returned.

Each row's display label SHALL include the unit identifier and, for Spec Kit features that decompose into multiple worktrees, a worktree-count hint summarising the breakdown (e.g. `"003-user-list — 3 worktrees: 2 [P] + 1 phase/"`). For OpenSpec changes and Markdown specs, the label SHALL be the unit identifier alone (one entry → one worktree, no hint needed).

The spec picker SHALL present a fuzzy-filter multi-select that behaves
identically to the branch picker (`select_branches`). The user SHALL be able to
type a query that filters the visible rows by case-insensitive substring match
against the row's display label. An empty query SHALL show every grouped row in
its original order. Toggling selection SHALL operate on the currently visible
(filtered) rows; rows selected under one query that are then hidden by a
different query SHALL remain selected and SHALL be returned on confirm.
Clearing the query (back to empty) SHALL restore the full set of rows with all
prior selections intact. The logical-unit grouping and worktree-count hint
labels SHALL be unchanged from the non-filtered behavior — filtering matches
against the displayed labels, and selecting a visible row SHALL still expand to
every underlying `SpecEntry` for that unit.

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

#### Scenario: Typing a query filters the spec rows

- **GIVEN** OpenSpec entries `add-auth`, `fix-session`, `add-logging`
- **WHEN** the user types `add` into the spec picker filter
- **THEN** only the rows for `add-auth` and `add-logging` SHALL be visible
- **AND** the row for `fix-session` SHALL NOT be visible

#### Scenario: Empty filter shows every grouped spec row

- **GIVEN** the spec picker with 3 grouped rows
- **WHEN** the filter query is empty
- **THEN** all 3 rows SHALL be visible in their original order

#### Scenario: Selecting a filtered Spec Kit row still expands to all its entries

- **GIVEN** a Spec Kit feature `003-user-list` (3 underlying SpecEntry values) and another feature `004-error-handling`
- **WHEN** the user types `003`, selects the visible `003-user-list` row, and confirms
- **THEN** `select_specs` SHALL return all 3 underlying SpecEntry values for `003-user-list`

#### Scenario: Clearing the spec filter restores all rows with selections intact

- **GIVEN** the spec picker where the user has typed `add` and toggled the `add-auth` row
- **WHEN** the user clears the query back to empty
- **THEN** all rows SHALL be visible again
- **AND** the `add-auth` row SHALL still be marked as selected
