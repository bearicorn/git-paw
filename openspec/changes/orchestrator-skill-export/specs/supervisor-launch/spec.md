## MODIFIED Requirements

### Requirement: Supervisor AGENTS.md from supervisor skill template

The system SHALL write the repo-root sidecar `.git-paw/AGENTS.local.md` (gitignored and git-excluded) as the orchestrator's **session context** before session state is saved: the session name, the broker URL, the bundled helper paths, a pointer to the per-repo discovery file, the governance section, the drive-loop coordination directive when `--unattended`, and a pointer to the standardized orchestrator skill at `.git-paw/skills/orchestrator/SKILL.md` for the procedure. `start` SHALL re-render `.git-paw/skills/orchestrator/SKILL.md` from the resolved orchestrator skill with the same session-independent inputs `git paw init` uses (the current gate commands and the configured spec format; no broker URL or session name) so it never lags the config, SHALL rewrite `.git-paw/skills/cli-setup/SKILL.md` from the resolved `cli-setup` skill so it never lags the binary, and SHALL announce the sidecar and orchestrator-skill paths through the launch command's injected output sink (`code-architecture`, *Command entry points are a callable library surface*), which the CLI writes to stdout and an in-process caller receives as data. The sidecar SHALL NOT duplicate the skill body; the session name and broker URL SHALL appear only in the sidecar, never in the exported skill.

#### Scenario: Session context is rendered to the repo-root sidecar

- **WHEN** `cmd_supervisor()` completes its launch sequence
- **THEN** `<repo_root>/.git-paw/AGENTS.local.md` SHALL exist and contain the session name, the broker URL and the path `.git-paw/skills/orchestrator/SKILL.md`
- **AND** the tracked `<repo_root>/AGENTS.md` SHALL be byte-identical to its pre-launch content

#### Scenario: Skill file is refreshed at start

- **GIVEN** `[supervisor].test_command` changed since `git paw init`
- **WHEN** `git paw start --supervisor` runs
- **THEN** `.git-paw/skills/orchestrator/SKILL.md` SHALL render `{{TEST_COMMAND}}` as the new value

#### Scenario: The cli-setup skill export is refreshed at start

- **GIVEN** a `.git-paw/skills/cli-setup/SKILL.md` whose content differs from the resolved `cli-setup` skill
- **WHEN** `git paw start --supervisor` runs
- **THEN** the file SHALL equal the resolved `cli-setup` skill

#### Scenario: Unattended context carries the drive-loop directive

- **GIVEN** a supervised session launched with `--unattended`
- **WHEN** the repo-root sidecar is inspected
- **THEN** it SHALL contain the drive-loop coordination directive

#### Scenario: Launch output names both paths

- **WHEN** `git paw start --supervisor` completes
- **THEN** stdout SHALL contain `.git-paw/AGENTS.local.md` and `.git-paw/skills/orchestrator/SKILL.md`

#### Scenario: An in-process launch receives the announcement through the sink

- **WHEN** the supervised launch use case is called in-process with a capturing output sink
- **THEN** the sink SHALL receive the lines naming `.git-paw/AGENTS.local.md` and `.git-paw/skills/orchestrator/SKILL.md`
- **AND** nothing SHALL be written to the process's stdout

### Requirement: Quick Start Supervisor chapter is internally consistent

`docs/src/quick-start-orchestrator.md` SHALL replace `docs/src/quick-start-supervisor.md` (with an mdBook redirect from the old path) and SHALL describe a single, consistent layout throughout: dashboard at pane 0, agent panes at indices 1 onwards, no supervisor pane, and the orchestrator running in the operator's own agent session outside tmux. The chapter SHALL NOT contain any sentence placing a supervisor inside the session.

#### Scenario: Quick start chapter is internally consistent

- **WHEN** `docs/src/quick-start-orchestrator.md` is inspected
- **THEN** every reference to the dashboard pane resolves to pane index 0
- **AND** every reference to an agent pane resolves to an index of 1 or greater
- **AND** the chapter states that the orchestrator runs outside the tmux session

#### Scenario: Old path redirects

- **WHEN** `mdbook build docs/` runs
- **THEN** the built site SHALL serve a redirect from `quick-start-supervisor.html` to `quick-start-orchestrator.html`

### Requirement: Quick Start Supervisor chapter does not reference nonexistent broker messages

`docs/src/quick-start-orchestrator.md` SHALL NOT reference broker message types that do not exist in `src/broker/messages.rs`. Specifically the chapter SHALL NOT contain the substrings `agent.register` or `agent.done` as broker message variants.

#### Scenario: Quick start chapter does not mention agent.register

- **WHEN** `docs/src/quick-start-orchestrator.md` is inspected
- **THEN** it does NOT contain the substring `agent.register`

#### Scenario: Quick start chapter does not mention agent.done

- **WHEN** `docs/src/quick-start-orchestrator.md` is inspected
- **THEN** it does NOT contain the substring `agent.done`

### Requirement: Quick Start Supervisor chapter reflects shipped features

`docs/src/quick-start-orchestrator.md` SHALL describe the shipped workflow for driving a session from the operator's own agent: `git paw init` (installs the skill and the AGENTS.md pointer), `git paw start --supervisor`, loading `.git-paw/skills/orchestrator/SKILL.md` in the driving agent, sweeping and verifying via the bundled helpers or the MCP server, publishing verdicts via `sweep.sh verified`, and pairing with `--unattended`. It SHALL NOT advertise as "not yet supported" any feature that has shipped, and SHALL note that a remote-controllable driving session (e.g. via the CLI's own remote-control feature) drives git-paw without any extra topology.

#### Scenario: Quick start describes the external-orchestrator workflow

- **WHEN** the chapter is inspected
- **THEN** it SHALL mention `git paw init`, `git paw start --supervisor`, `.git-paw/skills/orchestrator/SKILL.md`, and `sweep.sh verified`

#### Scenario: Quick start does not mark shipped features as deferred

- **WHEN** the chapter is inspected
- **THEN** the substrings `conflict detection` and `learnings` do not appear inside a section that describes them as not yet supported

### Requirement: `docs/src/user-guide/supervisor.md` SHALL consolidate the supervisor surfaces

The user-guide supervisor chapter SHALL include the following subsections (or equivalent headings):

1. **Spec audit governance sub-step** — references `docs/src/user-guide/governance.md` and the five doc-checklist examples (DoD, ADR, security, test-strategy, constitution).
2. **Agent permissions** — states that git-paw does not approve permission prompts; that whether an agent's command runs without a prompt is decided by the agent CLI's own permission system (the `[supervisor] agent_approval` / `approval` level mapped to CLI flags, the CLI's own allow rules, or `full-auto` inside a sandbox); that live prompts are escalated for a human decision and answered explicitly with `sweep.sh approve <pane>`; and that `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` are deprecated and have no effect. It cross-links to `docs/src/configuration/README.md` and `docs/src/user-guide/security-posture.md`.
3. **Repo-configurable gate commands** — names the six `[supervisor]` gate-command keys (`test_command`, `lint_command`, `build_command`, `doc_build_command`, `spec_validate_command`, `fmt_check_command`, `security_audit_command`) and the `(not configured)` graceful-skip behaviour; cross-links to `docs/src/configuration/README.md`.
4. **Broker-side conflict detector** — names the three failure shapes (forward, in-flight, ownership) and the `[conflict-detector]` token; cross-links to `docs/src/user-guide/conflict-detection.md`.
5. **Learnings aggregator** — at minimum a one-line cross-link to `docs/src/user-guide/learnings.md`.
6. **When the user types in your session** — mirrors the bundled-skill section of the same name, covering status questions, directives, and judgement-call asks.
7. **Merge orchestration** — mirrors the bundled-skill section, covering the topological order from `agent.blocked` events, per-branch `git merge --ff-only` + test loop, cycle handling, and the unattended rule (merge once every gate is verified without asking first; escalate only on a conflict, an exhausted correction cycle or a regression).
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
- **AND** SHALL mention `git merge --ff-only` as the per-branch merge command
- **AND** SHALL state that an unattended run merges without asking first and escalates only on a conflict, an exhausted correction cycle or a regression

#### Scenario: Supervisor user-guide documents driving from an external agent

- **WHEN** the file is inspected
- **THEN** it SHALL contain a section describing how to drive the session from the operator's own agent
- **AND** SHALL name `.git-paw/skills/orchestrator/SKILL.md` and `.git-paw/AGENTS.local.md`

## ADDED Requirements

### Requirement: Remote-control workflow chapter

The documentation SHALL include a chapter, `docs/src/user-guide/remote-control.md`, describing how to drive a git-paw session from a phone or another device by remote-controlling the operator's own driving agent session through that agent CLI's native remote-control feature, with git-paw's MCP server attached to that session. The chapter SHALL be the target of the "Remote control (documented workflow)" row of the controls table in `docs/src/user-guide/security-posture.md`, which names the workflow without linking to any chapter today. The chapter SHALL state that git-paw provides no bridge, tunnel or network transport of its own; SHALL include a table of which agent CLIs can act as a remote-controlled driving session, marked with the date its facts were last checked; SHALL state the caveats that the driving process must keep running on the local machine and that a remote session is lost after an outage of roughly ten minutes; and SHALL link the quick start and the MCP chapter. It SHALL NOT describe a remote-controlled supervisor pane or any topology in which an orchestrator runs inside the tmux session.

#### Scenario: Chapter states the topology and the local-process caveat

- **WHEN** the remote-control chapter is inspected
- **THEN** it SHALL state that the driving session runs on the local machine with git-paw's MCP server attached
- **AND** it SHALL state that the local process must stay running and that an outage of about ten minutes ends the remote session
- **AND** it SHALL state that git-paw ships no bridge or tunnel

#### Scenario: Chapter names supported CLIs with a check date

- **WHEN** the chapter's CLI table is inspected
- **THEN** it SHALL list each CLI's support as a remote-controlled driving session
- **AND** it SHALL carry the date the facts were last verified

#### Scenario: The security-posture controls table links the chapter

- **WHEN** the "Remote control" row of the controls table in `docs/src/user-guide/security-posture.md` is inspected
- **THEN** it SHALL link to `remote-control.md`

#### Scenario: Chapter describes no in-session orchestrator

- **WHEN** the chapter is inspected
- **THEN** it SHALL NOT contain the phrase `supervisor pane`
