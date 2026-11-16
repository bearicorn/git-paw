## ADDED Requirements

### Requirement: The auto level hands command judgement to the CLI's own auto mode

At the `auto` level, git-paw SHALL pass only the CLI's own automatic-mode flags from *Permission flag mapping*, and the CLI SHALL decide each action. git-paw SHALL NOT pair `auto` with any rule, allow list, settings file or tool grant of its own. It SHALL NOT write or extend any CLI settings file because a worker runs at `auto`. It SHALL treat a prompt that the CLI's auto mode leaves in place exactly like any other live permission prompt: report it for a human decision and send no keystroke. Documentation SHALL describe `auto` as the agent CLI's own judgement, not git-paw's. It SHALL state that `auto` is wider than `accept-edits` and is not a sandbox, and that `accept-edits` is the default.

#### Scenario: An auto worker carries only the CLI's mode flag

- **GIVEN** a version-2 config with `[workers] approval = "auto"`, `default_cli = "claude"`, and no `approval_args`
- **WHEN** a worker's pane command is built
- **THEN** it SHALL contain `--permission-mode auto`
- **AND** it SHALL NOT contain `--allowedTools`, `--allowed-tools` or `--settings`

#### Scenario: Running at auto writes no settings

- **GIVEN** a repository with no `.claude/` directory and a version-2 config with `[workers] approval = "auto"`
- **WHEN** `git paw start --branches feat/a` launches a `claude` worker
- **THEN** no `.claude/` directory and no setup record SHALL exist afterwards

#### Scenario: A prompt left by auto mode is escalated

- **GIVEN** an `--unattended` session whose `claude` worker runs at `auto` and shows a live permission prompt
- **WHEN** the drive loop sweeps that pane
- **THEN** the prompt SHALL be escalated for a human decision
- **AND** no keystroke SHALL be sent to that pane

#### Scenario: Docs separate auto from accept-edits

- **WHEN** the configuration reference and the security-posture page are inspected
- **THEN** they SHALL state that `auto` launches the agent CLI's own auto mode and that the CLI, not git-paw, decides each action
- **AND** they SHALL state that `accept-edits` is the default and launches the mode `auto` launched before 0.21.0
