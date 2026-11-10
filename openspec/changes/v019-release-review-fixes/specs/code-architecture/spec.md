## MODIFIED Requirements

### Requirement: Core modules report warnings as values

The agent-CLI module and launch resolution SHALL return a warning to their caller as a value, not print it to stderr, and SHALL NOT keep process-global state to deduplicate warnings. Launch resolution SHALL take the working directory as a parameter and return the warning with the profile, so a caller cannot drop it. The caller SHALL emit it through its own output sink, once per launch, as each CLI resolves.

#### Scenario: An unsandboxed full-auto launch warns once per launch through the sink

- **GIVEN** a CLI that declares no native sandbox, launched at the full-auto level for two workers in one `start`
- **WHEN** the launch runs with a capturing output sink
- **THEN** the sink SHALL hold the unsandboxed-full-auto warning exactly once

Test: `src/commands/launch.rs::an_unsandboxed_full_auto_launch_warns_once_through_the_sink`

#### Scenario: A captured launch does not write the warning to stderr

- **GIVEN** a launch that produces the unsandboxed-full-auto warning, run with a capturing output sink as the MCP server runs it
- **WHEN** the launch completes
- **THEN** the warning SHALL be in the sink
- **AND** the process SHALL NOT have written it to stderr

Test: `src/commands/launch.rs::a_captured_launch_does_not_write_the_warning_to_stderr`

#### Scenario: An earlier CLI's warning survives a later refusal

- **GIVEN** a launch whose first CLI warns at full-auto and whose second CLI is refused
- **WHEN** the launch resolves its CLIs
- **THEN** the sink SHALL hold the first CLI's warning
- **AND** the launch SHALL fail with the second CLI's refusal

Test: `src/commands/launch.rs::an_earlier_cli_warning_survives_a_later_refusal`

#### Scenario: A command path is found without a working directory

- **GIVEN** a CLI whose command contains `/` and no readable working directory
- **WHEN** `doctor` locates its executable
- **THEN** the executable SHALL be found

Test: `src/agent_cli/tests.rs::a_slash_command_is_located_without_a_working_directory`

#### Scenario: Two launches in one process each warn

- **GIVEN** two launches of the same unsandboxed CLI at full-auto in one process
- **WHEN** each runs with its own capturing output sink
- **THEN** each sink SHALL hold the warning once

Test: `src/commands/launch.rs::two_launches_in_one_process_each_warn`

#### Scenario: A profile that fails to compile is reported to the caller

- **GIVEN** an agent recorded with a CLI whose prompt-profile override does not compile
- **WHEN** the agent profile map records it
- **THEN** the record call SHALL return a warning naming the agent
- **AND** the agent SHALL resolve the `generic` profile

Test: `src/agent_cli/tests.rs::a_profile_that_fails_to_compile_is_reported_to_the_caller`

### Requirement: Keystrokes reach a surface through the backend's own dispatcher

Every terminal backend SHALL act as a keystroke dispatcher through one shared implementation, and the submit-and-verify actuator SHALL take that dispatcher by shared reference. No command or test SHALL wrap a backend in its own dispatcher adapter.

#### Scenario: A send uses the backend as capturer and dispatcher

- **GIVEN** `__send` delivering a message to a fake backend's surface
- **WHEN** the message is typed and submitted
- **THEN** the fake SHALL record the text operation and the `Enter` key operation in order, from the same backend the screen was read through

Test: `src/commands/send.rs::a_send_uses_the_backend_as_capturer_and_dispatcher`

#### Scenario: Drive-loop keystrokes are unchanged

- **GIVEN** the drive loop wired to a fake backend that is both its capturer and its dispatcher
- **WHEN** it submits a message and the input box still shows a paste placeholder after the first `Enter`
- **THEN** the fake SHALL record, in order: the message text, `Enter`, `ClearLine`, the message text again, and `Enter`

Test: `src/supervisor/drive.rs::submit_and_verify_recovers_a_paste_placeholder_and_reports_delivered`

### Requirement: Source audits read every production line

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

#### Scenario: An if-let on a spec-format variant is caught

- **GIVEN** the line `if let SpecBackendKind::OpenSpec = kind {` in production code outside `src/specs/`
- **WHEN** the spec-format dispatch audit runs
- **THEN** it SHALL report that line

Test: `tests/source_audit.rs::spec_format_dispatch_happens_only_inside_specs`

#### Scenario: A match on a wrapped spec-format variant is caught

- **GIVEN** the line `Some(SpecBackendKind::OpenSpec) => 2,` in production code outside `src/specs/`
- **WHEN** the spec-format dispatch audit runs
- **THEN** it SHALL report that line

Test: `tests/source_audit.rs::spec_format_dispatch_happens_only_inside_specs`

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
