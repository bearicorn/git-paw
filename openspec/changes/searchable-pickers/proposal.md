## Why

The branch and spec multi-select pickers (`select_branches` and `select_specs`)
are built on `dialoguer::MultiSelect`, which offers no search box. On repos with
many branches (or projects with many discovered specs), the user must scroll a
long flat list to find and toggle the items they want, which is slow and
error-prone. A type-to-filter picker makes both flows usable at scale.

## What Changes

- Replace the `dialoguer::MultiSelect`-based branch picker (`select_branches`)
  with a fuzzy-filter multi-select: the user types a query that narrows the
  visible candidates, toggles items in the filtered view, and confirms.
- Apply the **same** fuzzy-filter multi-select to the spec picker
  (`select_specs`) so both pickers behave identically. The existing
  logical-unit grouping (Spec Kit feature / OpenSpec change / Markdown file)
  and the worktree-count hint labels are preserved — filtering matches against
  the displayed row labels.
- An empty filter shows the full candidate list (no items hidden).
- Toggling selection while a filter is active toggles only against the visible
  (filtered) subset; previously-selected items that are filtered out stay
  selected and are returned on confirm.
- Clearing the filter restores the full list with prior selections intact.
- Cancellation semantics are unchanged: Ctrl+C and confirming with zero items
  selected both yield `PawError::UserCancelled`.

No CLI surface, config, or wire-format changes. Only the interactive picker UI
behind `--branches`/`--specs` (when invoked without explicit values) changes.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `interactive-selection`: the two multi-select picker requirements gain a
  fuzzy-filter search affordance. Specifically:
  - The branch-selection behavior described under **Subset branch selection**
    (the `select_branches` multi-select) is modified to require type-to-filter
    search.
  - The **Spec multi-select picker** requirement (`select_specs`) is modified to
    require the same type-to-filter search over the grouped rows, while keeping
    grouping, labels, and cancellation semantics.

## Impact

- **Code:** `src/interactive.rs` — `TerminalPrompter::select_branches` and
  `TerminalPrompter::select_specs` reimplemented on a shared `ratatui`-based
  fuzzy multi-select helper; the `Prompter` trait signatures are unchanged so
  the core `run_selection` flow and all test doubles are unaffected.
- **Dependencies:** built on `ratatui` + `crossterm`, both already approved and
  in use by the dashboard. `dialoguer` is retained for the single-select
  prompts (`select_mode`, `select_cli`, `select_cli_for_branch`). No new
  dependency is added; in particular `inquire` is NOT introduced (it is not in
  the approved dependency set).
- **Tests:** filter-matching, selection-under-filter, and clear-filter behavior
  are tested via the pure filtering/selection-state logic; cancellation and
  return-shape contracts continue to be covered by the existing `Prompter`
  test doubles.
