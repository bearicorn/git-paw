## 1. Picker state (pure, testable)

- [ ] 1.1 Add a `PickerState` struct in `src/interactive.rs` holding the immutable `labels: Vec<String>`, a `query: String`, and `selected: HashSet<usize>` keyed by original label index
- [ ] 1.2 Implement `visible_indices(&self) -> Vec<usize>` returning original indices whose label matches the query (empty query → all, original order; otherwise case-insensitive substring match)
- [ ] 1.3 Implement `set_query`, `push_char`, `pop_char` (or equivalent) to mutate the query, and `toggle(visible_row: usize)` mapping a visible row back to its original index in `selected`
- [ ] 1.4 Implement `confirm(&self) -> Vec<usize>` returning the selected original indices sorted ascending

## 2. ratatui fuzzy multi-select helper

- [ ] 2.1 Add `fuzzy_multi_select(prompt: &str, labels: &[String]) -> Result<Option<Vec<usize>>, PawError>` that owns the crossterm raw-mode setup/teardown (mirroring `src/dashboard.rs`) and a ratatui render loop over a `PickerState`
- [ ] 2.2 Wire key handling: printable chars edit the query, Backspace deletes, Up/Down move the cursor over `visible_indices()`, Space toggles the cursor row, Enter confirms (returns `Some(confirm())`), Ctrl+C and Esc return `None`
- [ ] 2.3 Ensure the terminal is always restored (disable raw mode, leave alternate screen) on every exit path including error/panic, following the dashboard's guard pattern

## 3. Rewire the two multi-select prompts

- [ ] 3.1 Reimplement `TerminalPrompter::select_branches` on `fuzzy_multi_select`: pass branch names as labels, map `None` → `UserCancelled`, empty selection → `UserCancelled`, else indices → branch names
- [ ] 3.2 Reimplement `TerminalPrompter::select_specs` on `fuzzy_multi_select`: pass the grouped row labels from `group_specs_by_unit`/`build_unit_label` unchanged, feed the returned `Option<Vec<usize>>` to the existing `finalize_spec_selection`
- [ ] 3.3 Remove the now-unused `dialoguer::MultiSelect` import while keeping `dialoguer::Select` for `select_mode`/`select_cli`/`select_cli_for_branch`; update module-level doc comment to mention the ratatui-based pickers

## 4. Tests

- [ ] 4.1 Unit-test `PickerState`: typing a query filters branch candidates (substring); empty query shows full list in original order
- [ ] 4.2 Unit-test `PickerState`: selection persists across query changes (toggle under one query, change query, toggle another, confirm returns both)
- [ ] 4.3 Unit-test `PickerState`: clearing the query restores the full list with prior selection still marked
- [ ] 4.4 Unit-test the spec path: filtering by `003` keeps the `003-user-list` row, and selecting it still expands to all 3 underlying `SpecEntry` values via `finalize_spec_selection`
- [ ] 4.5 Confirm existing `Prompter` test-double tests for cancellation (`select_branches`/`select_specs` Ctrl+C and zero-selection → `UserCancelled`) still pass unchanged; add filtered-Ctrl+C coverage at the `PickerState`/helper boundary where feasible

## 5. Docs and quality gates

- [ ] 5.1 Update the picker `with_prompt` help text to mention type-to-filter (e.g. "type to filter, space to toggle, enter to confirm")
- [ ] 5.2 Update mdBook user guide and any CLI/help references that describe the branch/spec pickers; run `mdbook build docs/`
- [ ] 5.3 Run `just check` (fmt + clippy + tests) and `just deny`; ensure no `unwrap()`/`expect()` in non-test code and all new public items have doc comments
