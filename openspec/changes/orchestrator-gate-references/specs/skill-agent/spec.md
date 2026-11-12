## ADDED Requirements

### Requirement: The orchestrator package carries the audit gates as references

The orchestrator package SHALL carry the spec-audit, doc-audit and security-audit gate procedures as references `references/gates/spec-audit.md`, `references/gates/doc-audit.md` and `references/gates/security-audit.md`, moved out of `SKILL.md` rather than copied. The spec-audit reference SHALL carry the Spec Audit Procedure with its governance-verification sub-step. References SHALL be rendered with the same placeholder set as `SKILL.md`.

#### Scenario: The three audit references exist and are rendered

- **WHEN** the orchestrator package is rendered with a configured `spec_validate_command`
- **THEN** `references/gates/spec-audit.md`, `references/gates/doc-audit.md` and `references/gates/security-audit.md` SHALL be present
- **AND** the spec-audit reference SHALL contain the rendered spec-validate command

Test: `tests/orchestrator_skill_content.rs::the_three_audit_references_exist_and_are_rendered`

#### Scenario: Gate content is moved, not duplicated

- **WHEN** the rendered orchestrator `SKILL.md` is inspected
- **THEN** it SHALL NOT contain the Spec Audit Procedure body or the doc-audit surface list

Test: `tests/orchestrator_skill_content.rs::gate_content_is_moved_not_duplicated`
Test: `tests/orchestrator_skill_content.rs::every_pre_move_gate_sentence_survives_in_the_package`

### Requirement: Each audit-gate reference is self-contained

Each audit-gate reference SHALL read correctly with no other context: it SHALL state the inputs the gate needs (the re-resolved branch tip, the merge-base diff, the change id, and the gate's rendered tooling command or `(not configured)`), the rule to run that command bare and read its exit status without piping it, the procedure, and the finding format with the gate's bracketed prefix.

#### Scenario: Each reference is self-contained

- **WHEN** any audit-gate reference is read on its own
- **THEN** it SHALL state that the gate runs against the re-resolved branch tip, that change contribution is diffed against the merge-base, and the bracketed prefix its findings use

Test: `tests/orchestrator_skill_content.rs::each_reference_is_self_contained`

#### Scenario: Each reference restates the bare-command rule

- **WHEN** any audit-gate reference is read on its own
- **THEN** it SHALL instruct running the gate's tooling command bare and reading its exit status directly
- **AND** SHALL forbid piping that command into another command

Test: `tests/orchestrator_skill_content.rs::each_reference_restates_the_bare_command_rule`

### Requirement: The orchestrator Workflow points each audit gate at its reference

The orchestrator `SKILL.md` SHALL keep the testing and regression gates inline and SHALL keep one Workflow step per audit gate, in the five-gate order, that states the gate's purpose, names its reference by the repository-root-relative path where the package is installed, and instructs reading that reference before running the gate.

#### Scenario: Testing and regression stay inline

- **WHEN** the rendered orchestrator `SKILL.md` is inspected
- **THEN** it SHALL contain the testing and regression-analysis gate procedures

Test: `tests/orchestrator_skill_content.rs::testing_and_regression_stay_inline`

#### Scenario: Each audit gate step names its installed reference

- **WHEN** the rendered orchestrator `SKILL.md` is inspected
- **THEN** it SHALL contain one Workflow step per audit gate naming `.git-paw/skills/orchestrator/references/gates/<gate>.md` for that gate
- **AND** each step SHALL instruct reading the reference before running the gate

Test: `tests/orchestrator_skill_content.rs::each_audit_gate_step_names_its_installed_reference`

#### Scenario: A headless hand-off that reads SKILL.md reaches the gates

- **GIVEN** an unattended merge hand-off whose stdin names `.git-paw/skills/orchestrator/SKILL.md` and asks for the branch to be verified against every gate
- **WHEN** the one-shot follows that `SKILL.md`
- **THEN** each audit gate step SHALL lead it to an installed reference path that exists at the repository root

Test: `tests/cli_init_orchestrator_export.rs::a_headless_hand_off_that_reads_skill_md_reaches_the_gates`

### Requirement: The orchestrator skill delegates audit gates through a compressed-verdict contract

The orchestrator `SKILL.md` SHALL include a delegation section stating that, when its CLI can run isolated subagents and after testing and regression pass, the three audit gates MAY run concurrently, each in an isolated context given only its reference plus the branch, re-resolved tip and change id. Each SHALL return a compressed verdict — gate name, `pass` or `fail`, one line per finding as `[<gate prefix>] <location>: <issue>` — and no raw tool output.

#### Scenario: The delegation section defines the verdict shape

- **WHEN** the rendered orchestrator `SKILL.md` is inspected
- **THEN** its delegation section SHALL define a verdict of gate name, `pass`/`fail`, and one-line findings carrying the gate's bracketed prefix
- **AND** SHALL state that delegated gates return no raw tool output

Test: `tests/orchestrator_skill_content.rs::the_delegation_section_defines_the_verdict_shape`

#### Scenario: Delegation follows testing and regression

- **WHEN** the delegation section is inspected
- **THEN** it SHALL state that the audit gates are delegated only after the testing and regression gates have passed

Test: `tests/orchestrator_skill_content.rs::delegation_follows_testing_and_regression`

### Requirement: Only the orchestrator publishes verdicts for delegated gates

The delegation section SHALL state that a delegated gate never publishes, and that the orchestrator itself publishes `agent.verified` only when testing and regression passed and all three audit verdicts are `pass`, otherwise one `feedback-gate` call per finding.

#### Scenario: Only the orchestrator publishes

- **WHEN** the delegation section is inspected
- **THEN** it SHALL state that a delegated gate never publishes `agent.verified` or `agent.feedback`
- **AND** that `agent.verified` follows only when testing, regression and all three audit verdicts pass

Test: `tests/orchestrator_skill_content.rs::only_the_orchestrator_publishes`

### Requirement: Delegation degrades to inline gates

The delegation section SHALL instruct an orchestrator whose CLI cannot run isolated subagents to read each reference and run the gate inline. It MAY direct broad read-only exploration to an isolated read-only subagent that returns a map rather than file contents.

#### Scenario: Inline fallback without subagents

- **WHEN** the delegation section is inspected
- **THEN** it SHALL instruct an orchestrator whose CLI cannot run isolated subagents to read each reference and run the gate inline

Test: `tests/orchestrator_skill_content.rs::inline_fallback_without_subagents`

### Requirement: The delegation section names no vendor

The delegation section SHALL name no vendor product, model, or persona file format or path.

#### Scenario: The delegation section is vendor-neutral

- **WHEN** the delegation section is audited
- **THEN** it SHALL contain no vendor product name, model name, or vendor persona path

Test: `tests/orchestrator_skill_content.rs::the_delegation_section_is_vendor_neutral`

### Requirement: Orchestrator skill content requirements are satisfied package-wide

Any requirement that the orchestrator (formerly supervisor) skill include, contain or enumerate content SHALL be satisfied by the rendered orchestrator package as a whole — its `SKILL.md` followed by every reference that `SKILL.md` names — so content moved into a reference continues to satisfy the requirement that pinned it.

#### Scenario: A moved section still satisfies its requirement

- **GIVEN** the requirement that the orchestrator skill include a "Spec Audit Procedure" section
- **WHEN** its covering test renders the orchestrator package
- **THEN** the procedure SHALL be found in `references/gates/spec-audit.md`
- **AND** the requirement SHALL be satisfied

Test: `tests/orchestrator_skill_content.rs::a_moved_section_still_satisfies_its_requirement`

### Requirement: The orchestrator Baseline step installs dependencies before recording

The orchestrator `SKILL.md` Baseline step SHALL instruct the orchestrator, before recording the baseline, to make sure the checkout it runs the gate commands in (the root checkout or the verify worktree) has the project's dependencies installed the way the project's own setup or its `on_create` hook installs them, and, when the baseline fails for missing dependencies, to install them and re-run rather than record a red baseline. It SHALL name no package manager.

#### Scenario: The Baseline step prepares the checkout first

- **WHEN** the rendered orchestrator `SKILL.md` Baseline step is inspected
- **THEN** it SHALL instruct making sure the root checkout or verify worktree has the project's dependencies installed, as the project's own setup or `on_create` hook does, before recording the baseline

Test: `tests/orchestrator_skill_content.rs::the_baseline_step_prepares_the_checkout_first`

#### Scenario: A baseline that fails for missing dependencies is not recorded

- **WHEN** the rendered orchestrator `SKILL.md` Baseline step is inspected
- **THEN** it SHALL instruct installing the missing dependencies and re-running the baseline when it fails for missing dependencies
- **AND** SHALL forbid recording that failing run as the baseline

Test: `tests/orchestrator_skill_content.rs::a_baseline_that_fails_for_missing_dependencies_is_not_recorded`

#### Scenario: The Baseline instruction names no package manager

- **WHEN** the Baseline step is audited
- **THEN** it SHALL contain no package-manager or build-tool command
- **AND** the rendered orchestrator package SHALL pass the no-language-leak audit

Test: `tests/orchestrator_skill_content.rs::the_baseline_instruction_names_no_package_manager`
Test: `tests/lang_agnostic_skill_audit.rs::audit_covers_every_registered_package_text_file`

### Requirement: A self-hosted session runs the binary built from the branch under test

The orchestrator `SKILL.md` SHALL state that when the repository being orchestrated is the source of the git-paw that runs the session (a self-hosted or dogfood session), the session SHALL run the git-paw built from the branch under test, not a previously installed copy, because a stale binary silently brings back bugs the branch has already fixed. The instruction SHALL name no build tool or build command.

#### Scenario: A self-hosted session checks the binary it runs

- **WHEN** the rendered orchestrator `SKILL.md` is inspected
- **THEN** it SHALL instruct the orchestrator, in a self-hosted session, to confirm at bootstrap that the `git paw` the session runs is the build from the branch under test rather than an installed copy
- **AND** SHALL give the reason that a stale binary silently re-introduces bugs the branch has fixed

Test: `tests/orchestrator_skill_content.rs::a_self_hosted_session_checks_the_binary_it_runs`

#### Scenario: A stale binary is raised before work is driven

- **WHEN** the rendered orchestrator `SKILL.md` is inspected
- **THEN** it SHALL instruct the orchestrator, when the session runs a binary not built from the branch under test, to tell the human to rebuild with the project's own build and run the session on that build before it drives any agent

Test: `tests/orchestrator_skill_content.rs::a_stale_binary_is_raised_before_work_is_driven`

#### Scenario: The instruction names no build command

- **WHEN** the self-hosted-session instruction is audited
- **THEN** it SHALL contain no build-tool, package-manager or build-recipe command
- **AND** the rendered orchestrator package SHALL pass the no-language-leak audit

Test: `tests/orchestrator_skill_content.rs::the_self_hosted_instruction_names_no_build_command`
Test: `tests/lang_agnostic_skill_audit.rs::audit_covers_every_registered_package_text_file`
