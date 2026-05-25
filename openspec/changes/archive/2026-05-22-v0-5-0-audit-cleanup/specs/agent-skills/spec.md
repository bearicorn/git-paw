## ADDED Requirements

### Requirement: Skill-content tests SHALL cover all rendered subsections named in archived prompt-submit-fix scenarios

Three new tests in `src/skills.rs::tests` SHALL assert behaviour of the rendered supervisor skill content that the `prompt-submit-fix` archive's `agent-skills/spec.md` scenarios required but did not cover with dedicated tests:

1. **Launch-time-sweep proactive instruction** — assert the rendered skill contains prose instructing the supervisor agent to inspect every pane immediately after attaching (i.e. before any stall threshold elapses).
2. **Escalation via `agent.question` for unknown prompts** — assert the launch-sweep section instructs escalation via `agent.question` for permission prompts that do not match the safe-command or confined-to-worktree patterns.
3. **"Complements not replaces" cross-reference** — assert the launch-sweep section explicitly states the proactive sweep complements (does NOT replace) the `[supervisor.auto_approve]` background poll thread.

#### Scenario: Launch-sweep proactive instruction test exists

- **WHEN** the test module in `src/skills.rs` is inspected
- **THEN** a behavioural test (e.g. `supervisor_skill_documents_proactive_launch_sweep`) SHALL exist
- **AND** SHALL assert the rendered supervisor skill content contains prose tied to the first-few-seconds-after-attach window

#### Scenario: Unknown-prompt escalation test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist asserting the rendered skill instructs `agent.question` escalation for unknown permission prompts

#### Scenario: Complements-not-replaces cross-reference test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist asserting the rendered skill contains "complements" / "does NOT replace" language tying the launch sweep to the `[supervisor.auto_approve]` poll thread

### Requirement: Dashboard input-handling SHALL be tested for the supervisor-as-pane removed-inbox scenarios

Three new unit tests in `src/dashboard.rs::tests` SHALL cover dashboard input-handling scenarios that the `supervisor-as-pane-followups` archive's `dashboard/spec.md` required:

1. **Tab key is ignored** — pressing `KeyCode::Tab` SHALL NOT alter any state.
2. **Printable characters do not enter a buffer** — `KeyCode::Char('a')` and `KeyCode::Char(' ')` SHALL leave no buffer state behind.
3. **Layout collapses to non-inbox shape when `show_message_log = false`** — the layout-builder helper's Vec<Constraint> SHALL be exactly `[title, table, status]` (3 chunks, no prompts/input chunks).

#### Scenario: Tab-key-ignored test exists

- **WHEN** the test module in `src/dashboard.rs` is inspected
- **THEN** a behavioural test SHALL exist asserting `KeyCode::Tab` does not alter any dashboard state

#### Scenario: Printable-char-ignored test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist asserting `KeyCode::Char('a')` and space leave no buffer state

#### Scenario: Layout-collapse test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist asserting the layout chunks are exactly `[title, table, status]` when `show_message_log = false`

### Requirement: Source-audit tests SHALL cover cmd_supervisor non-self-publish and dashboard no-phantom-row

Two new tests in `tests/source_audit.rs` SHALL close the archived `supervisor-as-pane-followups` scenarios:

1. **`cmd_supervisor` does not self-publish** — grep `src/main.rs::cmd_supervisor`'s body for `publish_to_broker_http` AND `build_status_message("supervisor"`; assert zero matches inside the function.
2. **Dashboard renders no supervisor row pre-bootstrap** — fixture-driven: render `Snapshot { agents: vec![] }` (i.e. the broker has no entries because the supervisor pane hasn't published yet); assert the output contains no `supervisor` substring and no divider row.

#### Scenario: cmd_supervisor source-audit test exists

- **WHEN** `tests/source_audit.rs` is inspected
- **THEN** a behavioural test SHALL exist greping `cmd_supervisor` for `publish_to_broker_http` and `build_status_message("supervisor"` substrings
- **AND** asserting both grep counts are zero

#### Scenario: No-phantom-row test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist rendering an empty-agents snapshot and asserting the output contains no `supervisor` substring

### Requirement: SpecEntry backend-tag tests SHALL cover scan() returns

Six new tests SHALL assert that the `SpecBackend::scan()` implementations populate `SpecEntry.backend` correctly per the archived `openspec-apply-boot-prompt` change:

- **OpenSpec backend (3 tests in `src/specs/openspec.rs::tests`):**
  - Single-entry scan returns `backend == SpecBackendKind::OpenSpec`.
  - Multi-entry scan: every returned entry has `backend == SpecBackendKind::OpenSpec`.
  - Backend tag is independent of `paw_cli` or `owned_files` frontmatter.
- **Markdown backend (3 tests in `src/specs/markdown.rs::tests`):**
  - Single-entry scan returns `backend == SpecBackendKind::Markdown`.
  - Multi-entry scan: every returned entry has `backend == SpecBackendKind::Markdown`.
  - Backend tag is applied AFTER filtering out non-pending entries.

#### Scenario: OpenSpec scan tags every entry with the OpenSpec backend

- **WHEN** the test module in `src/specs/openspec.rs` is inspected
- **THEN** a behavioural test SHALL exist that calls `scan()` on a fixture with at least 2 changes and asserts every returned `SpecEntry` has `backend == SpecBackendKind::OpenSpec`

#### Scenario: Markdown scan tags every entry with the Markdown backend

- **WHEN** the test module in `src/specs/markdown.rs` is inspected
- **THEN** a behavioural test SHALL exist that calls `scan()` on a fixture with at least 2 pending entries and asserts every returned `SpecEntry` has `backend == SpecBackendKind::Markdown`

### Requirement: BrokerMessage envelope tests SHALL cover the seven-variant enumeration and question-no-from-field absence

Two new tests in `src/broker/messages.rs::tests` SHALL close the archived `spec-corrections-v0-5-0` scenarios:

1. **Envelope enumerates all seven wire-format type values** — iterate the seven `BrokerMessage` variants, serialize each, assert the JSON's `"type"` field equals the spec'd discriminator string (`agent.status`, `agent.artifact`, `agent.blocked`, `agent.verified`, `agent.feedback`, `agent.question`, `agent.intent`).
2. **`QuestionPayload` omits the `from` field** — serialize a `QuestionPayload` instance, assert the resulting JSON does NOT contain a `"from"` key.

#### Scenario: Envelope-enumerates-seven test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist that serializes each of the seven `BrokerMessage` variants and asserts the `"type"` field matches the corresponding spec'd discriminator string

#### Scenario: Question-no-from-field test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist that serializes a `QuestionPayload` and asserts the resulting JSON does NOT contain a `"from"` key

### Requirement: Skill-content tests SHALL cover paste-buffer cross-ref and git-paw-status-ordering warning

Two new tests in `src/skills.rs::tests` SHALL close the archived `coordination-skill-followups` and `coordination-skill-followups-2` scenarios:

1. **Paste-buffer cross-ref in send-keys-alongside-feedback section** — assert the rendered supervisor skill's tmux-send-keys-alongside-`agent.feedback` section cross-references paste-buffer recovery for long answers.
2. **`git paw status` ordering warning in pane-resolution section** — assert the rendered supervisor skill's `pane_current_path` section contains the substring `git paw status` AND prose forbidding using its alphabetical order as a pane→agent mapping source.

#### Scenario: Paste-buffer cross-ref test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist asserting the rendered supervisor skill's send-keys-alongside-feedback section mentions paste-buffer recovery

#### Scenario: git-paw-status warning test exists

- **WHEN** the test module is inspected
- **THEN** a behavioural test SHALL exist asserting the rendered supervisor skill's pane-resolution section contains `git paw status` substring AND prose forbidding using its order as a mapping source

### Requirement: `config-test-isolation`'s "None preserves" scenario SHALL be annotated as a documented exception

`tests/config_integration.rs` (or `src/config.rs::tests`, whichever owns `load_config` test fixtures) SHALL contain a doc-comment block explaining that the "None preserves platform-default user-config resolution" scenario from `config-test-isolation`'s archived spec has no dedicated test BY DESIGN. The block SHALL state the rationale: a dedicated test would either pollute the dev machine's real config directory or require brittle process-global env-var manipulation. The block SHALL note that the scenario is exercised behaviourally by every existing production call site (which all pass `None`).

The doc comment SHALL be discoverable via grep for the scenario name (`None preserves platform-default`) so future audits find it.

#### Scenario: Documented-exception comment exists

- **WHEN** the test file(s) for `load_config` are inspected
- **THEN** a doc-comment block SHALL exist mentioning the literal substring `None preserves platform-default`
- **AND** the comment SHALL explain why a dedicated test is intentionally omitted
