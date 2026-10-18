## MODIFIED Requirements

### Requirement: Supervisor mode boot block prepending

In supervised auto-start mode, the system SHALL prepend the boot instruction block to each coding agent's task prompt before injecting it into the agent's tmux pane (panes 1..N). The dashboard pane (pane 0) is a TUI process and SHALL NOT receive a `send-keys` boot block. No other pane exists in the session.

#### Scenario: Boot block prepended to agent prompts

- **GIVEN** agent task prompt "Implement error handling"
- **WHEN** `cmd_supervisor()` constructs the full prompt for the coding agent pane
- **THEN** the injected text SHALL be:
  ```
  <boot_block>\n\nImplement error handling
  ```

#### Scenario: Boot block comes before task content

- **GIVEN** any coding-agent pane receiving its initial prompt
- **WHEN** the prompt is injected
- **THEN** the boot block SHALL appear first
- **AND** the actual task content SHALL appear after two newlines

#### Scenario: Dashboard pane receives no boot block

- **WHEN** `cmd_supervisor()` injects boot prompts
- **THEN** no boot block SHALL be sent to pane 0

### Requirement: Supervisor boot block timing

The system SHALL inject boot blocks during the supervised launch sequence, after tmux session creation and each coding-agent pane's launch-readiness gate, and before `cmd_supervisor()` returns (or, under `--unattended`, before the drive loop starts).

#### Scenario: Boot blocks injected before cmd_supervisor returns

- **GIVEN** `cmd_supervisor()` is executing without `--unattended`
- **WHEN** coding-agent panes are created and pass the readiness gate
- **THEN** boot blocks SHALL be injected for every coding-agent pane
- **AND** all `send-keys` calls SHALL complete before `cmd_supervisor()` returns

### Requirement: All agents receive boot blocks

In supervised mode, the system SHALL ensure every coding agent pane receives the boot instruction block, regardless of whether the agent has a spec file or uses a default prompt. The dashboard pane is excluded (it runs a TUI process, not a chat-style agent), and no orchestrator pane exists in the session to receive one.

#### Scenario: Coding agents with specs receive boot blocks

- **GIVEN** a coding agent pane with spec file content
- **WHEN** the prompt is constructed
- **THEN** the boot block SHALL be prepended to the spec content

#### Scenario: Coding agents without specs receive boot blocks

- **GIVEN** a coding agent pane with no spec file (default prompt)
- **WHEN** the prompt is constructed
- **THEN** the boot block SHALL be prepended to the default prompt

#### Scenario: No boot block is built for a supervisor identity

- **WHEN** a supervised launch builds its boot blocks
- **THEN** no boot block SHALL be built with `BRANCH_ID = supervisor`

### Requirement: Boot prompt includes governance documents section

When the orchestrator context — the repo-root sidecar `.git-paw/AGENTS.local.md` written by a supervised launch — is rendered AND `config.governance` has at least one path field set to `Some(_)`, the system SHALL append a "Governance documents" section to that context. The section SHALL list one bullet per configured path with the doc's canonical name and the configured path. Path fields whose value is `None` SHALL NOT appear in the bullet list.

When ALL `config.governance` path fields are `None`, the system SHALL omit the entire "Governance documents" section (no header, no empty bullet list, no placeholder text).

The section SHALL be a plain-text block separated from preceding content by a blank line. The section heading SHALL be the literal string `## Governance documents`.

The section SHALL NOT contain a "gates" sub-line, gate-flag summaries, or any per-doc enforcement metadata.

#### Scenario: Section omitted when no paths configured

- **GIVEN** `config.governance` with all five path fields `None`
- **WHEN** the orchestrator context is rendered
- **THEN** it SHALL NOT contain the substring `Governance documents`

#### Scenario: Section present with one path

- **GIVEN** `config.governance.dod = Some("docs/dod.md")` and the other path fields `None`
- **WHEN** the orchestrator context is rendered
- **THEN** it SHALL contain the heading `## Governance documents`
- **AND** the section SHALL contain a bullet referencing `dod` and `docs/dod.md`
- **AND** the section SHALL NOT contain bullets for `adr`, `test_strategy`, `security`, or `constitution`

#### Scenario: Section lists all configured paths in canonical order

- **GIVEN** `config.governance` with all five paths populated
- **WHEN** the orchestrator context is rendered
- **THEN** the section SHALL list five bullets in canonical order: `adr`, `test_strategy`, `security`, `dod`, `constitution`

#### Scenario: Section contains no gates summary

- **GIVEN** any `config.governance` configuration with at least one path set
- **WHEN** the orchestrator context is rendered
- **THEN** the "Governance documents" section SHALL NOT contain a "Gated docs" line, a "Governance gates" sub-section, or any text referencing per-doc gate flags

#### Scenario: The launch path renders the section

- **GIVEN** a configured governance path and `git paw start --supervisor`
- **WHEN** the launch completes
- **THEN** the written sidecar file SHALL contain the section (the renderer SHALL have a production caller on the launch path, not only in tests)

### Requirement: Governance section follows the supervisor skill content

The "Governance documents" section SHALL appear in the rendered orchestrator context (the repo-root sidecar `.git-paw/AGENTS.local.md`) *after* the orchestrator skill content rendered from the supervisor skill template and before any other trailing content, so the orchestrator reads governance configuration in the same context where it reads its own skill instructions. The orchestrator context SHALL carry no per-agent task content (no spec section and no file-ownership section), so no task content can precede or follow the governance section.

The section SHALL be produced by the same assembly the supervisor launch uses to write the orchestrator context (the repo-root sidecar `.git-paw/AGENTS.local.md`), so a configured `[governance]` table reaches every launched supervisor session — not only a context assembled outside the launch path.

#### Scenario: Section position follows the skill content

- **GIVEN** a configured `config.governance` and a supervised session being launched
- **WHEN** the repo-root sidecar is rendered
- **THEN** the position of `## Governance documents` SHALL come after the orchestrator skill heading (`## Supervisor Skills`, or whatever the skill heading is)
- **AND** the sidecar SHALL contain no `### Spec` and no `### File Ownership` heading

#### Scenario: A launched supervisor session carries the governance section

- **GIVEN** a repository whose config sets `[governance] dod = "docs/dod.md"` and `[supervisor] enabled = true`
- **WHEN** `git paw start --supervisor` launches the session
- **THEN** the orchestrator context written to the repo-root sidecar `.git-paw/AGENTS.local.md` SHALL contain the start-of-line heading `## Governance documents` and the bullet `- dod: docs/dod.md`
- **AND** the heading SHALL come after `## Supervisor Skills`

#### Scenario: A launched supervisor session without governance paths is unchanged

- **GIVEN** a repository whose config has no `[governance]` paths set and `[supervisor] enabled = true`
- **WHEN** `git paw start --supervisor` launches the session
- **THEN** the orchestrator context written to the repo-root sidecar `.git-paw/AGENTS.local.md` SHALL NOT contain a start-of-line `## Governance documents` heading

### Requirement: Drive-loop coordination in the supervisor boot context

When a session runs `--unattended` (an in-process drive loop that escalates every live permission prompt and approves none), git-paw SHALL include in the orchestrator context (the repo-root sidecar `.git-paw/AGENTS.local.md`) a directive stating that:

- a drive loop is running; it sweeps every coding-agent surface and records each live permission prompt, judgment call and merge decision on the broker, where the orchestrator reads them;
- no git-paw component approves permission prompts. The orchestrator SHALL relay each escalated permission prompt to the human and SHALL run `sweep.sh approve <pane>` only when the human explicitly tells it to;
- the orchestrator handles the reasoning-level work the loop cannot: relaying prompts, verification, merge orchestration, and conflict handling.

When the session is NOT unattended (no drive loop), the orchestrator context SHALL NOT contain this directive. The orchestrator then finds stuck-on-prompt agents through its own sweep and relays their prompts to the human in the same way.

#### Scenario: Unattended orchestrator context announces the drive loop

- **GIVEN** a supervised session started with `--unattended`
- **WHEN** the repo-root sidecar is rendered
- **THEN** it SHALL contain the directive that a drive loop records escalations on the broker and that no component approves prompts
- **AND** it SHALL tell the orchestrator to relay each permission prompt to the human

#### Scenario: Attended orchestrator context omits the drive-loop directive

- **GIVEN** a supervised session started WITHOUT `--unattended`
- **WHEN** the repo-root sidecar is rendered
- **THEN** it SHALL NOT contain the drive-loop coordination directive

#### Scenario: No orchestrator context tells the orchestrator to approve by pattern

- **WHEN** the repo-root sidecar is rendered for an attended or an `--unattended` session
- **THEN** it SHALL NOT instruct the orchestrator to approve prompts it judges safe
- **AND** it SHALL NOT instruct the orchestrator to select a "don't ask again" option
