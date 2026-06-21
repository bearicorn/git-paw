## Context

`src/interactive.rs` exposes a `Prompter` trait whose `TerminalPrompter`
implementation drives `git paw start`'s interactive flow with `dialoguer`. Two
of those prompts are multi-selects:

- `select_branches(&[String]) -> Result<Vec<String>, PawError>` — pick the
  branches to spin worktrees for.
- `select_specs(&[SpecEntry]) -> Result<Vec<SpecEntry>, PawError>` — pick the
  specs to launch. Rows are grouped by logical unit (Spec Kit feature /
  OpenSpec change / Markdown file) via `group_specs_by_unit`, with
  worktree-count hint labels via `build_unit_label`.

Both are implemented with `dialoguer::MultiSelect`, which renders a flat,
scroll-only list with no search box. On a repo with dozens of branches or a
project with many discovered specs, the user has to page through the whole list
to find each item. This is the friction this change removes.

The dashboard (`src/dashboard.rs`, `src/dashboard/broker_log.rs`) already drives
a `ratatui` + `crossterm` TUI, including raw-mode setup/teardown
(`enable_raw_mode`, `EnterAlternateScreen`, `CrosstermBackend`,
`disable_raw_mode`). So the building blocks for a richer picker are already
vendored and proven in this codebase.

## Goals / Non-Goals

**Goals:**

- Add a type-to-filter affordance to both multi-select pickers so the user can
  narrow a long candidate list by typing.
- Keep `select_branches` and `select_specs` behaving identically as pickers
  (same key bindings, same filter semantics, same cancellation rules).
- Preserve everything observable today: `select_specs` still groups by logical
  unit and shows the worktree-count hint; both still return the same value
  shapes; Ctrl+C and zero-selection still map to `PawError::UserCancelled`.
- Keep the `Prompter` trait signatures unchanged so `run_selection` and all test
  doubles continue to compile and pass untouched.

**Non-Goals:**

- Changing the single-select prompts (`select_mode`, `select_cli`,
  `select_cli_for_branch`). They stay on `dialoguer::Select`.
- Changing any CLI flag, config field, or wire format.
- Adding ranking/scoring UI beyond filtering (no preview pane, no sorting by
  match score is required — substring containment is sufficient; a fuzzy
  subsequence match MAY be used as an enhancement).
- Touching the spec-name resolution / TTY-detection requirements
  (`--specs NAME,...`, `--from-all-specs`); those are out of scope.

## Decisions

### Decision: Build the picker on `ratatui` + `crossterm`, not `inquire`

`ratatui` and `crossterm` are both in git-paw's approved dependency set and are
already used by the dashboard, so we get a searchable multi-select with **zero
new dependencies**. `inquire` would give a fuzzy multi-select almost for free,
but it is **not** in the approved dependency set (see the approved-deps table in
AGENTS.md / CLAUDE.md), and adding it requires explicit maintainer approval.
Reusing `ratatui` keeps the supply chain unchanged and reuses the terminal
setup/teardown patterns already validated in `src/dashboard.rs`.

`dialoguer` remains a dependency for the three single-select prompts; this
change does not remove it.

**Alternatives considered:**
- `inquire::MultiSelect` (built-in fuzzy filter) — rejected: not an approved
  dependency.
- Patching `dialoguer` upstream to add filtering — rejected: out of our control,
  slow, and `dialoguer::MultiSelect` has no filter API.

### Decision: Extract a single reusable fuzzy multi-select helper

Introduce one internal helper (e.g. `fuzzy_multi_select(prompt, &labels) ->
Result<Option<Vec<usize>>, PawError>`) that owns the `ratatui`/`crossterm`
render-and-input loop and returns the selected **indices into the original
label slice** (or `None` for Ctrl+C). Both `select_branches` and `select_specs`
call it:

- `select_branches` passes the branch names as labels and maps the returned
  indices back to branch names.
- `select_specs` passes the grouped row labels (unchanged from
  `group_specs_by_unit` / `build_unit_label`) and feeds the returned indices to
  the existing `finalize_spec_selection`, which already expands grouped rows to
  underlying `SpecEntry` values.

Returning original-list indices is what makes the two pickers identical and lets
all existing post-processing (`finalize_spec_selection`, the branch index→name
map) stay as-is.

### Decision: Separate pure filtering/selection state from the I/O loop

The render loop needs a terminal and is exempt from the coverage gate (TUI draw
loops). To keep the new behavior testable without a TTY, the filter and
selection bookkeeping live in a pure struct (e.g. `PickerState`) with no
terminal dependency:

- holds the immutable `labels`, the current `query` string, and a
  `selected: HashSet<usize>` keyed by **original** label index;
- exposes `visible_indices()` — the original indices whose label matches the
  current query (empty query → all indices, in original order);
- exposes `toggle(visible_row)` — toggles the original index that the given
  visible row maps to;
- exposes `confirm() -> Vec<usize>` — the selected original indices, sorted.

This struct is where filtering and selection-under-filter are unit-tested. The
`ratatui` loop is a thin shell over it (draw `visible_indices()`, mutate `query`
on key, call `toggle`/`confirm`), and is verified by smoke testing.

### Decision: Filter matching is case-insensitive substring, fuzzy optional

The match predicate: empty query matches everything; otherwise a label matches
when the query is a case-insensitive substring of the label (subsequence/fuzzy
matching MAY be layered on, but substring is the contract the spec asserts).
This makes "type to filter" predictable and keeps the spec's scenarios
deterministic.

### Decision: Selection persists across filter changes

`selected` is keyed by original index, so toggling under one filter and then
changing the query never silently drops a selection. Clearing the query
restores the full list with every prior toggle still marked. `confirm()` returns
the union of all selected original indices regardless of the current filter.
This matches the proposal: "previously-selected items that are filtered out stay
selected and are returned on confirm."

### Decision: Cancellation semantics unchanged

The helper returns `None` for Ctrl+C (or Esc), which both call sites translate
to `PawError::UserCancelled`. Confirming with an empty selection set also yields
`PawError::UserCancelled`, exactly as the current `select_branches` /
`finalize_spec_selection` do. `finalize_spec_selection` is reused verbatim, so
the spec picker's zero-selection and Ctrl+C paths are preserved by construction.

## Risks / Trade-offs

- [Raw-mode terminal not restored on panic in the picker loop] → Mitigation:
  follow the dashboard's teardown discipline — restore terminal state (disable
  raw mode, leave alternate screen) on every exit path, including error, mirror
  the guard pattern already in `src/dashboard.rs`.
- [`select_branches`/`select_specs` are now hard to unit-test through the
  trait because they need a TTY] → Mitigation: the testable logic lives in the
  pure `PickerState` struct and `finalize_spec_selection`; the TUI loop is
  coverage-exempt and smoke-tested. The trait-level cancellation/return-shape
  contracts remain covered by the existing `Prompter` test doubles, which do not
  open a terminal.
- [Behavior divergence between the two pickers over time] → Mitigation: both
  share the one `fuzzy_multi_select` helper and the same `PickerState`; there is
  no second code path to drift.
- [Larger interactive surface than `dialoguer`] → Trade-off accepted: the
  `ratatui` building blocks are already vendored and exercised by the dashboard,
  so the marginal maintenance cost is low and no new dependency enters the tree.
