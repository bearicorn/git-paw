## Purpose

Interactive selection prompts for choosing branches and AI CLIs. Supports uniform (same CLI for all branches) and per-branch assignment modes, with CLI flags that skip prompts. Logic is separated from UI via the `Prompter` trait for testability.
## Requirements
### Requirement: CLI flags skip all prompts when both provided

When both `--cli` and `--branches` flags are provided, the system SHALL skip all interactive prompts and map the CLI to all specified branches.

#### Scenario: Both flags skip all prompts
- **GIVEN** `--cli alpha` and `--branches feature/auth,fix/api` flags
- **WHEN** `run_selection()` is called
- **THEN** it SHALL return mappings without invoking any prompts

Test: `interactive::tests::both_flags_skips_all_prompts_and_maps_cli_to_all_branches`

### Requirement: CLI flag skips CLI prompt but prompts for branches

When only `--cli` is provided, the system SHALL prompt for branch selection but skip CLI selection.

#### Scenario: CLI flag provided, branches prompted
- **GIVEN** `--cli alpha` flag and no branches flag
- **WHEN** `run_selection()` is called
- **THEN** branch selection SHALL be prompted and the flag CLI SHALL be used

Test: `interactive::tests::cli_flag_skips_cli_prompt_but_prompts_for_branches`

### Requirement: Branches flag skips branch prompt but prompts for CLI

When only `--branches` is provided, the system SHALL skip branch selection but prompt for CLI assignment.

#### Scenario: Branches flag provided, CLI prompted in uniform mode
- **GIVEN** `--branches` flag and no CLI flag
- **WHEN** user selects uniform mode
- **THEN** the selected CLI SHALL be mapped to all flagged branches

Test: `interactive::tests::branches_flag_skips_branch_prompt_but_prompts_for_cli_uniform`

### Requirement: Uniform mode maps same CLI to all branches

In uniform mode, the system SHALL assign the selected CLI to every selected branch.

#### Scenario: Uniform mode selection
- **GIVEN** user selects uniform mode, picks 2 branches and 1 CLI
- **WHEN** `run_selection()` completes
- **THEN** both branches SHALL be mapped to the same CLI

Test: `interactive::tests::uniform_mode_maps_same_cli_to_all_selected_branches`

### Requirement: Per-branch mode maps different CLIs to each branch

In per-branch mode, the system SHALL prompt for a CLI for each selected branch individually.

#### Scenario: Per-branch mode selection
- **GIVEN** user selects per-branch mode with 2 branches
- **WHEN** different CLIs are chosen for each branch
- **THEN** each branch SHALL be mapped to its respective CLI

Test: `interactive::tests::per_branch_mode_maps_different_cli_to_each_branch`

#### Scenario: Per-branch mode with branches flag
- **GIVEN** branches provided via flag and per-branch mode selected
- **WHEN** different CLIs are chosen
- **THEN** each flagged branch SHALL be mapped to its selected CLI

Test: `interactive::tests::per_branch_mode_with_branches_flag`

### Requirement: Error when no CLIs available

The system SHALL return `PawError::NoCLIsFound` when the CLI list is empty.

#### Scenario: Empty CLI list
- **GIVEN** no CLIs available
- **WHEN** `run_selection()` is called
- **THEN** it SHALL return `Err(PawError::NoCLIsFound)`

Test: `interactive::tests::no_clis_available_returns_error`

### Requirement: Error when no branches available

The system SHALL return `PawError::BranchError` when the branch list is empty.

#### Scenario: Empty branch list
- **GIVEN** no branches available
- **WHEN** `run_selection()` is called
- **THEN** it SHALL return `Err(PawError::BranchError)`

Test: `interactive::tests::no_branches_available_returns_error`

### Requirement: User cancellation propagates as PawError::UserCancelled

The system SHALL propagate cancellation (Ctrl+C or empty selection) as `PawError::UserCancelled`.

#### Scenario: User cancels branch selection
- **GIVEN** user presses Ctrl+C during branch selection
- **WHEN** `run_selection()` is called
- **THEN** it SHALL return `Err(PawError::UserCancelled)`

Test: `interactive::tests::user_cancels_branch_selection_returns_cancelled`

#### Scenario: User selects no branches
- **GIVEN** user confirms with zero branches selected
- **WHEN** `run_selection()` is called
- **THEN** it SHALL return `Err(PawError::UserCancelled)`

Test: `interactive::tests::user_selects_no_branches_returns_cancelled`

#### Scenario: User cancels CLI selection
- **GIVEN** user presses Ctrl+C during CLI selection
- **WHEN** `run_selection()` is called
- **THEN** it SHALL return `Err(PawError::UserCancelled)`

Test: `interactive::tests::user_cancels_cli_selection_returns_cancelled`

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

### Requirement: CliMode display format

The `CliMode` enum SHALL display as human-readable descriptions.

#### Scenario: CliMode display strings
- **GIVEN** `CliMode::Uniform` and `CliMode::PerBranch`
- **WHEN** formatted with `Display`
- **THEN** they SHALL render as `"Same CLI for all branches"` and `"Different CLI per branch"`

Test: `interactive::tests::cli_mode_display`

### Requirement: CliInfo display format

`CliInfo` SHALL display as the binary name when it matches the display name, or as `"DisplayName (binary)"` when they differ.

#### Scenario: Same display and binary name
- **GIVEN** a `CliInfo` where `display_name` equals `binary_name`
- **WHEN** formatted with `Display`
- **THEN** it SHALL render as just the binary name

Test: `interactive::tests::cli_info_display_same_names`

#### Scenario: Different display and binary name
- **GIVEN** a `CliInfo` where `display_name` differs from `binary_name`
- **WHEN** formatted with `Display`
- **THEN** it SHALL render as `"DisplayName (binary_name)"`

Test: `interactive::tests::cli_info_display_different_names`

### Requirement: CLI picker with optional pre-selection

The `select_cli` method on the `Prompter` trait SHALL accept an optional default CLI name for pre-selection in the interactive picker.

#### Scenario: Picker with default pre-selected
- **WHEN** `select_cli()` is called with `default = Some("claude")` and `"claude"` is in the CLI list
- **THEN** the picker SHALL display with `"claude"` highlighted as the default selection

#### Scenario: Picker without default
- **WHEN** `select_cli()` is called with `default = None`
- **THEN** the picker SHALL display with the first item selected (no pre-selection)

#### Scenario: Default CLI not in available list
- **WHEN** `select_cli()` is called with `default = Some("nonexistent")` and that CLI is not available
- **THEN** the picker SHALL display with no pre-selection (graceful fallback)

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

