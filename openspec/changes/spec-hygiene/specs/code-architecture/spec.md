## REMOVED Requirements

### Requirement: Source audits read every production line

**Reason**: Two of its scenarios, "An if-let on a spec-format variant is caught" and "A match on a wrapped spec-format variant is caught", test which spec-format dispatch the audit forbids. That rule belongs to `spec-providers` "Per-format behaviour is reached only through the backend", not to how audits read source. OpenSpec rejects dropping scenarios from a MODIFIED block, so the requirement returns under a new name without them.

**Migration**: Replaced by "Source audits check every production line" in this change, which keeps the other five scenarios and their tests. The two spec-format scenarios move to `spec-providers` in this change, with the same test.

## ADDED Requirements

### Requirement: Source audits check every production line

A source audit SHALL check every production line of each file it scans. It SHALL skip only `#[cfg(test)]` items, measured by their braces, and comment lines, including lines inside a `/* */` block comment, so code after an early `#[cfg(test)]` item is still audited. Braces inside string, char and raw-string literals and comments SHALL NOT count. All source audits SHALL share one implementation of that rule.

#### Scenario: Production code after an early test item is audited

- **GIVEN** a source file whose first `#[cfg(test)]` item is followed by production code that matches the audit's forbidden pattern
- **WHEN** the audit scans the file
- **THEN** the audit SHALL report that production line

Test: `tests/source_audit.rs::production_code_after_an_early_test_item_is_audited`

#### Scenario: Test items and comments are not audited

- **GIVEN** a source file whose only matches of the forbidden pattern are inside a `#[cfg(test)]` item or on a comment line
- **WHEN** the audit scans the file
- **THEN** the audit SHALL report nothing for that file

Test: `tests/source_audit.rs::test_items_and_comments_are_not_audited`

#### Scenario: main.rs is a positive control

- **WHEN** the shared production-code filter is applied to `src/main.rs`
- **THEN** a known production line after the file's first `#[cfg(test)]` SHALL remain visible to the audit

Test: `tests/source_audit.rs::main_rs_is_a_positive_control`

#### Scenario: A brace in a literal does not end a test item

- **GIVEN** a `#[cfg(test)]` module holding `"{"`, `'}'` and `r#"{{"#`, followed by production code
- **WHEN** the shared filter is applied
- **THEN** the production code after the module SHALL remain visible
- **AND** every `#[cfg(test)]` item under `src/` SHALL close before the end of its file

Test: `tests/source_audit.rs::braces_in_literals_do_not_count`, `tests/source_audit.rs::every_test_item_in_src_closes_before_eof`

#### Scenario: Lines inside a block comment are not audited

- **GIVEN** a source file whose only match of the forbidden pattern is on a line inside a `/* */` block comment, followed by production code
- **WHEN** the shared filter is applied
- **THEN** the line inside the block comment SHALL be blank
- **AND** the production code after the comment SHALL remain visible on its original line

Test: `tests/source_audit.rs::lines_inside_a_block_comment_are_not_audited`
