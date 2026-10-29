## MODIFIED Requirements

### Requirement: `docs/src/user-guide/supervisor.md` SHALL consolidate the supervisor surfaces

The user-guide supervisor chapter SHALL include the following subsections (or equivalent headings):

1. **Spec audit governance sub-step** — references `docs/src/user-guide/governance.md` and the five doc-checklist examples (DoD, ADR, security, test-strategy, constitution).
2. **Agent permissions** — states that git-paw does not approve permission prompts; that whether an agent's command runs without a prompt is decided by the agent CLI's own permission system (the `[supervisor] agent_approval` / `approval` level mapped to CLI flags, the CLI's own allow rules, or `full-auto` inside a sandbox); that live prompts are escalated for a human decision and answered explicitly with `sweep.sh approve <pane>`; and that `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` are deprecated and have no effect. It cross-links to `docs/src/configuration/README.md` and `docs/src/user-guide/security-posture.md`.
3. **Repo-configurable gate commands** — names the six `[supervisor]` gate-command keys (`test_command`, `lint_command`, `build_command`, `doc_build_command`, `spec_validate_command`, `fmt_check_command`, `security_audit_command`) and the `(not configured)` graceful-skip behaviour; cross-links to `docs/src/configuration/README.md`.
4. **Broker-side conflict detector** — names the three failure shapes (forward, in-flight, ownership) and the `[conflict-detector]` token; cross-links to `docs/src/user-guide/conflict-detection.md`.
5. **Learnings aggregator** — at minimum a one-line cross-link to `docs/src/user-guide/learnings.md`.
6. **When the user types in your session** — mirrors the bundled-skill section of the same name, covering status questions, directives, and judgement-call asks.
7. **Merge orchestration** — mirrors the bundled-skill section, covering the topological order from `agent.blocked` events, integrating the ordered batch with `git paw finish --rebase` (mechanical rebase of disjoint branches, re-test, batched rebase directives, exit codes `0`/`3`/`1`), cycle handling, and the unattended rule (merge once every gate is verified without asking first; escalate only on a conflict, an exhausted correction cycle or a regression).
8. **Driving the session from your own agent** — the workflow: where the skill and session context live (`.git-paw/skills/orchestrator/SKILL.md`, `.git-paw/AGENTS.local.md`, the AGENTS.md pointer section), the helper and MCP surfaces the orchestrator uses, how verdicts are published, and how `--unattended` pairs with an external orchestrator.

#### Scenario: Supervisor user-guide names governance sub-step + cross-link

- **WHEN** `docs/src/user-guide/supervisor.md` is inspected
- **THEN** the content SHALL contain a heading or paragraph naming the governance sub-step inside spec audit
- **AND** SHALL link to `docs/src/user-guide/governance.md`

#### Scenario: Supervisor user-guide explains agent permissions

- **WHEN** the file is inspected
- **THEN** it SHALL contain a section with a heading approximately "Agent permissions" or equivalent
- **AND** SHALL state that git-paw does not approve permission prompts and that the agent CLI's own permission system decides
- **AND** SHALL name `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` as deprecated with no effect

#### Scenario: Supervisor user-guide names the gate-command templating

- **WHEN** the file is inspected
- **THEN** it SHALL contain prose stating that orchestrator skill gate commands are repo-configurable
- **AND** SHALL name at least three of the six `[supervisor]` gate-command keys
- **AND** SHALL mention the `(not configured)` graceful-skip behaviour

#### Scenario: Supervisor user-guide names the broker-side conflict detector

- **WHEN** the file is inspected
- **THEN** it SHALL contain a section describing the broker-side conflict detector
- **AND** SHALL name the three failure shapes (forward, in-flight, ownership)
- **AND** SHALL link to `docs/src/user-guide/conflict-detection.md`

#### Scenario: Supervisor user-guide cross-links the learnings aggregator chapter

- **WHEN** the file is inspected
- **THEN** the content SHALL include a link to `docs/src/user-guide/learnings.md`

#### Scenario: Supervisor user-guide mirrors "When the user types in your session"

- **WHEN** the file is inspected
- **THEN** the content SHALL include a section approximately named "When the user types in your session"
- **AND** SHALL describe at least the three categories of user input (status question, directive, judgment-call ask)
- **AND** SHALL NOT contain the heading "When the user types in your pane"

#### Scenario: Supervisor user-guide mirrors "Merge orchestration"

- **WHEN** the file is inspected
- **THEN** the content SHALL include a section describing orchestrator-driven merge orchestration
- **AND** SHALL mention the topological order derived from `agent.blocked` events
- **AND** SHALL name `git paw finish --rebase` as the integration command and SHALL NOT present `git merge --ff-only` as the per-branch merge command
- **AND** SHALL state that an unattended run merges without asking first and escalates only on a conflict, an exhausted correction cycle or a regression

#### Scenario: Supervisor user-guide documents driving from an external agent

- **WHEN** the file is inspected
- **THEN** it SHALL contain a section describing how to drive the session from the operator's own agent
- **AND** SHALL name `.git-paw/skills/orchestrator/SKILL.md` and `.git-paw/AGENTS.local.md`
